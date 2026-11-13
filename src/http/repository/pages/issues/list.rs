//! The Issues list: state filters with counts, a search and account filters, one row per
//! issue, and the New issue button for signed-in viewers.

use actix_web::{HttpRequest, Result as AwResult, get, web};
use maud::{Markup, html};

use crate::{
    auth::TwigContext,
    config,
    db::issues::{Issue, IssueFilter, IssueState},
    http::repository::pages::{
        Params,
        context::{RepoContext, open_repo},
        tabs::render_tab_response,
    },
    http::view::render_error,
    issues::{self, AccountFilter, RepoRef},
};

use super::{author_name, signed_in};

#[derive(serde::Deserialize)]
pub(super) struct ListQuery {
    state: Option<String>,
    q: Option<String>,
    author: Option<String>,
    assignee: Option<String>,
}

/// The search and account filters as typed, kept for the form and the tab links.
#[derive(Default)]
struct Filters {
    q: String,
    author: String,
    assignee: String,
}

/// `GET /{namespace}/{repo}/issues`: the Issues tab.
#[get("/{namespace}/{repo}/issues")]
pub async fn issues_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
    query: web::Query<ListQuery>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, &params.namespace, &params.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };
    let viewer = signed_in(&req, &server, &auth_state, &params.namespace).await;
    let state = query.state.as_deref().unwrap_or("open");
    let filters = Filters {
        q: query.q.clone().unwrap_or_default().trim().to_string(),
        author: query.author.clone().unwrap_or_default().trim().to_string(),
        assignee: query
            .assignee
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string(),
    };
    let body = match load(&ctx, &auth_state, state, &filters, viewer.is_some()).await {
        Ok(body) => body,
        Err(error) => {
            log::error!(
                "Failed to load issues for {}/{}: {error}",
                params.namespace,
                params.repo
            );
            return Ok(render_error("Issues could not be loaded."));
        }
    };
    Ok(render_tab_response(&req, &ctx.frame(), "issues", &body))
}

/// Loads the requested view of the list and renders the whole tab body.
async fn load(
    ctx: &RepoContext,
    auth_state: &web::Data<TwigContext>,
    state: &str,
    filters: &Filters,
    can_open: bool,
) -> Result<Markup, String> {
    let repo_id = ctx.handle.repo_id().map_err(|error| error.to_string())?;
    let db = auth_state.db();
    let scope = RepoRef {
        repo_id: &repo_id,
        namespace: &ctx.namespace,
    };
    let selected = match state {
        "closed" => Some(IssueState::Closed),
        "all" => None,
        _ => Some(IssueState::Open),
    };
    let author = account(db, &filters.author).await?;
    let assignee = account(db, &filters.assignee).await?;
    let (open, closed) = db.count_issues_by_state(&repo_id).await?;

    // A name that matches no account lists nothing, rather than everything.
    let items = match (author, assignee) {
        (Some(Account::NoOne), _) | (_, Some(Account::NoOne)) => Vec::new(),
        (author, assignee) => {
            let filter = IssueFilter {
                state: selected,
                query: (!filters.q.is_empty()).then(|| filters.q.clone()),
                author_user_id: author.and_then(Account::id),
                assignee_user_id: assignee.and_then(Account::id),
                limit: 50,
                ..Default::default()
            };
            let page = issues::list(db, &scope, &filter)
                .await
                .map_err(|error| format!("{error:?}"))?;
            page.items
        }
    };

    let mut rows = Vec::with_capacity(items.len());
    for issue in items {
        let author = author_name(auth_state, &issue.author_user_id).await;
        rows.push((issue, author));
    }
    Ok(render(ctx, state, filters, &rows, open, closed, can_open))
}

/// One resolved account filter, for the UI's list query.
enum Account {
    Only(String),
    NoOne,
}

impl Account {
    fn id(self) -> Option<String> {
        match self {
            Self::Only(id) => Some(id),
            Self::NoOne => None,
        }
    }
}

/// Resolves a typed username. An empty name filters nothing.
async fn account(db: &crate::db::Database, name: &str) -> Result<Option<Account>, String> {
    match issues::account_filter(db, Some(name)).await {
        Ok(AccountFilter::Anyone) => Ok(None),
        Ok(AccountFilter::Only(id)) => Ok(Some(Account::Only(id))),
        Ok(AccountFilter::NoOne) => Ok(Some(Account::NoOne)),
        Err(error) => Err(format!("{error:?}")),
    }
}

/// A tab link that keeps the search and account filters in place.
fn list_href(namespace: &str, repo: &str, state: &str, filters: &Filters) -> String {
    let mut pairs = vec![("state", state)];
    if !filters.q.is_empty() {
        pairs.push(("q", filters.q.as_str()));
    }
    if !filters.author.is_empty() {
        pairs.push(("author", filters.author.as_str()));
    }
    if !filters.assignee.is_empty() {
        pairs.push(("assignee", filters.assignee.as_str()));
    }
    let query = pairs
        .iter()
        .map(|(key, value)| format!("{key}={}", percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("/{namespace}/{repo}/issues?{query}")
}

/// Percent-encodes a query value. Everything but unreserved characters is escaped.
fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn render(
    ctx: &RepoContext,
    state: &str,
    filters: &Filters,
    rows: &[(Issue, String)],
    open: u64,
    closed: u64,
    can_open: bool,
) -> Markup {
    let namespace = &ctx.namespace;
    let repo = &ctx.repo;
    let tab = |value: &str, label: String| {
        html! {
            a class="twig-pr-filter" href=(list_href(namespace, repo, value, filters))
                aria-current=[(state == value).then_some("page")] {
                (label)
            }
        }
    };
    html! {
        div class="twig-stack" {
            div class="twig-pr-toolbar" {
                nav class="twig-pr-filters" aria-label="Issue state" {
                    (tab("open", format!("{open} Open")))
                    (tab("closed", format!("{closed} Closed")))
                    (tab("all", "All".to_string()))
                }
                @if can_open {
                    a class="twig-btn twig-btn--primary" href=(format!("/{namespace}/{repo}/issues/new")) {
                        "New issue"
                    }
                }
            }
            form class="twig-pr-toolbar" method="get" action=(format!("/{namespace}/{repo}/issues")) {
                input type="hidden" name="state" value=(state);
                input type="search" name="q" value=(filters.q) placeholder="Search issues";
                input type="text" name="author" value=(filters.author) placeholder="Author";
                input type="text" name="assignee" value=(filters.assignee) placeholder="Assignee";
                button type="submit" class="twig-btn" { "Filter" }
            }
            @if rows.is_empty() {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO ISSUES" }
                    p class="twig-empty-body" {
                        "There are no issues in this view."
                    }
                }
            } @else {
                section class="twig-panel twig-panel--flush" {
                    div class="twig-panel-body" {
                        @for (issue, author) in rows {
                            (row(namespace, repo, issue, author))
                        }
                    }
                }
            }
        }
    }
}

fn row(namespace: &str, repo: &str, issue: &Issue, author: &str) -> Markup {
    let (label, key) = match issue.state {
        IssueState::Open => ("Open", "open"),
        IssueState::Closed => ("Closed", "closed"),
    };
    let opened = issue.created_at.split('T').next().unwrap_or_default();
    let number = issue.number;
    html! {
        div class="twig-row twig-pr-row" {
            div class="twig-pr-row-title" {
                span class="twig-pr-state" data-state=(key) { (label) }
                a href=(format!("/{namespace}/{repo}/issues/{number}")) { (issue.title) }
            }
            div class="twig-row-meta" {
                "#" (number) " opened " (opened) " by " (author)
            }
        }
    }
}
