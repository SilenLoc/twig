//! The global Issues page at `/issues`: issues from every repository the viewer may
//! read, newest first. It takes the same state, search, and account filters as a
//! repository's Issues tab. Visibility and every rule come from `crate::issues`.

use std::collections::HashMap;

use actix_web::{HttpRequest, Result as AwResult, get, web};
use maud::{Markup, html};

use crate::{
    auth::TwigContext,
    config,
    db::{
        Database,
        issues::{Issue, IssueFilter, IssueState},
    },
    http::view::{percent_encode, render_error, render_layout},
    issues::{self, AccountFilter, Viewer, VisibleRepo},
};

/// Issues per page.
const PAGE_SIZE: u32 = 50;

#[derive(serde::Deserialize)]
pub struct GlobalIssuesQuery {
    state: Option<String>,
    q: Option<String>,
    author: Option<String>,
    assignee: Option<String>,
    page: Option<u32>,
}

/// The view as the visitor asked for it. Kept for the form, the tab links, and the pager.
struct View {
    /// `open`, `closed`, or `all`.
    state: &'static str,
    q: String,
    author: String,
    assignee: String,
    /// One-based.
    page: u32,
}

impl View {
    fn from_query(query: &GlobalIssuesQuery) -> Self {
        let trimmed =
            |value: &Option<String>| value.as_deref().unwrap_or_default().trim().to_string();
        let state = match query.state.as_deref() {
            Some("closed") => "closed",
            Some("all") => "all",
            _ => "open",
        };
        Self {
            state,
            q: trimmed(&query.q),
            author: trimmed(&query.author),
            assignee: trimmed(&query.assignee),
            page: query.page.unwrap_or(1).max(1),
        }
    }

    /// A link to `/issues` that keeps the search and account filters in place.
    fn href(&self, state: &str, page: u32) -> String {
        let mut pairs = vec![("state", state.to_string())];
        if !self.q.is_empty() {
            pairs.push(("q", self.q.clone()));
        }
        if !self.author.is_empty() {
            pairs.push(("author", self.author.clone()));
        }
        if !self.assignee.is_empty() {
            pairs.push(("assignee", self.assignee.clone()));
        }
        if page > 1 {
            pairs.push(("page", page.to_string()));
        }
        let query = pairs
            .iter()
            .map(|(key, value)| format!("{key}={}", percent_encode(value)))
            .collect::<Vec<_>>()
            .join("&");
        format!("/issues?{query}")
    }
}

/// One listed issue with the names its row shows.
struct Row {
    issue: Issue,
    namespace: String,
    repo: String,
    author: String,
}

/// What the page shows for one view.
struct Listing {
    rows: Vec<Row>,
    has_more: bool,
    open: u64,
    closed: u64,
}

/// `GET /issues`: the global Issues list.
#[get("/issues")]
pub async fn issues_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<GlobalIssuesQuery>,
) -> AwResult<Markup> {
    let db = auth_state.db();
    let user = match auth_state.user_id_from_request(&req).await {
        Some(user_id) => db.get_user_by_id(&user_id).await.ok().flatten(),
        None => None,
    };
    let username = user.as_ref().map(|user| user.username.clone());
    let viewer_id = user.as_ref().map(|user| user.id.clone());
    let is_site_admin = user
        .as_ref()
        .is_some_and(|user| server.is_configured_admin(&user.username));
    let viewer = viewer_id.as_deref().map(|user_id| Viewer {
        user_id,
        is_site_admin,
    });
    let view = View::from_query(&query);

    let content = match load(db, server.project_root(), viewer.as_ref(), &view).await {
        Ok(listing) => render(&view, &listing),
        Err(error) => {
            log::error!("Failed to load the global issue list: {error}");
            render_error("Issues could not be loaded.")
        }
    };
    Ok(render_layout(&content, username.as_deref(), Some("Issues")))
}

/// Loads the counts and one page of issues across every repository the viewer may read.
async fn load(
    db: &Database,
    root: &str,
    viewer: Option<&Viewer<'_>>,
    view: &View,
) -> Result<Listing, String> {
    let repos = issues::visible_repos(db, root, viewer)
        .await
        .map_err(|error| format!("{error:?}"))?;
    let repo_ids: Vec<String> = repos.iter().map(|repo| repo.repo_id.clone()).collect();
    let (open, closed) = db.count_issues_by_state_in_repos(&repo_ids).await?;

    let author = issues::account_filter(db, Some(&view.author))
        .await
        .map_err(|error| format!("{error:?}"))?;
    let assignee = issues::account_filter(db, Some(&view.assignee))
        .await
        .map_err(|error| format!("{error:?}"))?;

    // A name that matches no account lists nothing, rather than everything.
    let names_no_one =
        matches!(author, AccountFilter::NoOne) || matches!(assignee, AccountFilter::NoOne);
    let (rows, has_more) = if names_no_one {
        (Vec::new(), false)
    } else {
        let filter = IssueFilter {
            state: match view.state {
                "closed" => Some(IssueState::Closed),
                "all" => None,
                _ => Some(IssueState::Open),
            },
            query: (!view.q.is_empty()).then(|| view.q.clone()),
            author_user_id: only_id(author),
            assignee_user_id: only_id(assignee),
            limit: PAGE_SIZE,
            offset: (view.page - 1).saturating_mul(PAGE_SIZE),
        };
        db.list_issues_in_repos(&repo_ids, &filter).await?
    };
    let rows = attach_names(db, &repos, rows).await;
    Ok(Listing {
        rows,
        has_more,
        open,
        closed,
    })
}

/// The account ID an account filter names, if it names one.
fn only_id(filter: AccountFilter) -> Option<String> {
    match filter {
        AccountFilter::Only(id) => Some(id),
        AccountFilter::Anyone | AccountFilter::NoOne => None,
    }
}

/// Pairs each issue with its repository's names and its author's username. A repository
/// missing from `repos` cannot be linked, so its issue is left out.
async fn attach_names(db: &Database, repos: &[VisibleRepo], found: Vec<Issue>) -> Vec<Row> {
    let by_id: HashMap<&str, &VisibleRepo> = repos
        .iter()
        .map(|repo| (repo.repo_id.as_str(), repo))
        .collect();
    let mut authors: HashMap<String, String> = HashMap::new();
    let mut rows = Vec::with_capacity(found.len());
    for issue in found {
        let Some(repo) = by_id.get(issue.repo_id.as_str()) else {
            continue;
        };
        if !authors.contains_key(&issue.author_user_id) {
            let name = match db.get_user_by_id(&issue.author_user_id).await {
                Ok(Some(user)) => user.username,
                _ => "unknown".to_string(),
            };
            authors.insert(issue.author_user_id.clone(), name);
        }
        let author = authors[&issue.author_user_id].clone();
        rows.push(Row {
            namespace: repo.namespace.clone(),
            repo: repo.repo.clone(),
            author,
            issue,
        });
    }
    rows
}

fn render(view: &View, listing: &Listing) -> Markup {
    let tab = |value: &str, label: String| {
        html! {
            a class="twig-pr-filter" href=(view.href(value, 1))
                aria-current=[(view.state == value).then_some("page")] {
                (label)
            }
        }
    };
    html! {
        section class="twig-pagehead" {
            h1 class="twig-section" { "Issues" }
        }
        div class="twig-stack" {
            div class="twig-pr-toolbar" {
                nav class="twig-pr-filters" aria-label="Issue state" {
                    (tab("open", format!("{} Open", listing.open)))
                    (tab("closed", format!("{} Closed", listing.closed)))
                    (tab("all", "All".to_string()))
                }
            }
            form class="twig-search twig-issue-filters" role="search" method="get" action="/issues" {
                input type="hidden" name="state" value=(view.state);
                label class="twig-sr" for="issue-search" { "Search issues" }
                input id="issue-search" class="twig-input" type="search" name="q" value=(view.q) placeholder="Search issues";
                label class="twig-sr" for="issue-author" { "Author" }
                input id="issue-author" class="twig-input twig-input--mono twig-input--person" type="text" name="author" value=(view.author) placeholder="Author";
                label class="twig-sr" for="issue-assignee-filter" { "Assignee" }
                input id="issue-assignee-filter" class="twig-input twig-input--mono twig-input--person" type="text" name="assignee" value=(view.assignee) placeholder="Assignee";
                button type="submit" class="twig-btn" { "Filter" }
            }
            @if listing.rows.is_empty() {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO ISSUES" }
                    p class="twig-empty-body" {
                        "There are no issues in this view."
                    }
                }
            } @else {
                section class="twig-panel twig-panel--flush" {
                    div class="twig-panel-body" {
                        @for row in &listing.rows {
                            (issue_row(row))
                        }
                    }
                }
            }
            @if view.page > 1 || listing.has_more {
                nav class="twig-cluster" aria-label="Issue pages" {
                    @if view.page > 1 {
                        a class="twig-btn twig-btn--quiet" href=(view.href(view.state, view.page - 1)) { "Newer" }
                    }
                    @if listing.has_more {
                        a class="twig-btn twig-btn--quiet" href=(view.href(view.state, view.page + 1)) { "Older" }
                    }
                }
            }
        }
    }
}

fn issue_row(row: &Row) -> Markup {
    let issue = &row.issue;
    let (label, key) = match issue.state {
        IssueState::Open => ("Open", "open"),
        IssueState::Closed => ("Closed", "closed"),
    };
    let (namespace, repo, number) = (&row.namespace, &row.repo, issue.number);
    let opened = issue.created_at.split('T').next().unwrap_or_default();
    html! {
        div class="twig-row twig-pr-row twig-issue-row" {
            div class="twig-pr-row-title" {
                span class="twig-pr-state" data-state=(key) { (label) }
                a href=(format!("/{namespace}/{repo}/issues/{number}")) { (issue.title) }
            }
            div class="twig-row-meta" {
                a href=(format!("/{namespace}/{repo}/issues")) { (namespace) "/" (repo) }
                " #" (number) " opened " (opened) " by " (row.author)
            }
        }
    }
}
