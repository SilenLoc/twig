//! New pull request: choose base and head branches, review the commits they
//! differ by, and write the title and description.

use actix_web::{HttpRequest, HttpResponse, Result as AwResult, get, post, web};
use maud::{Markup, html};
use serde::Deserialize;

use crate::{
    auth::TwigContext,
    config,
    git::bare::{BranchInfo, CommitSummary},
    http::repository::pages::{
        Params,
        context::{RepoContext, open_repo},
        tabs::render_tab_response,
    },
    pulls::{self, Actor, CreateInput, PullError, RepoRef},
};

use super::signed_in;

#[derive(Debug, Deserialize)]
pub(super) struct CompareQuery {
    base: Option<String>,
    head: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateForm {
    base: String,
    head: String,
    title: String,
    #[serde(default)]
    body: String,
}

/// What the compare page shows for the chosen branches.
struct Compare {
    branches: Vec<String>,
    base: String,
    head: String,
    commits: Vec<CommitSummary>,
    title: String,
    body: String,
    /// A request that already covers these branches, linked instead of offering a new one.
    existing: Option<i64>,
}

fn choose_branches(branches: &[BranchInfo], query: &CompareQuery) -> (String, String) {
    let default = branches
        .iter()
        .find(|branch| branch.is_default)
        .or_else(|| branches.first())
        .map(|branch| branch.name.clone())
        .unwrap_or_default();
    let base = query
        .base
        .clone()
        .filter(|name| branches.iter().any(|branch| &branch.name == name))
        .unwrap_or_else(|| default.clone());
    let head = query
        .head
        .clone()
        .filter(|name| branches.iter().any(|branch| &branch.name == name))
        .or_else(|| {
            branches
                .iter()
                .map(|branch| branch.name.clone())
                .find(|name| *name != base)
        })
        .unwrap_or_default();
    (base, head)
}

fn load(ctx: &RepoContext, query: &CompareQuery) -> Result<Compare, git2::Error> {
    let mut branches = ctx.handle.list_branches()?;
    branches.sort_by(|left, right| left.name.cmp(&right.name));
    let (base, head) = choose_branches(&branches, query);
    let base_tip = branches.iter().find(|b| b.name == base).and_then(|b| b.tip);
    let head_tip = branches.iter().find(|b| b.name == head).and_then(|b| b.tip);

    let mut commits = Vec::new();
    if let (Some(base_tip), Some(head_tip)) = (base_tip, head_tip)
        && base_tip != head_tip
    {
        for oid in ctx.handle.commit_ids_in_range(base_tip, head_tip)? {
            commits.push(ctx.handle.commit_summary(oid)?);
        }
    }
    // A single commit suggests its own title and body; several leave the title
    // to the author and suggest the branch name.
    let (title, body) = match commits.as_slice() {
        [only] => (only.summary.clone(), only.body.clone()),
        _ => (String::new(), String::new()),
    };
    commits.reverse();
    Ok(Compare {
        branches: branches.into_iter().map(|branch| branch.name).collect(),
        base,
        head,
        commits,
        title,
        body,
        existing: None,
    })
}

/// `GET /{namespace}/{repo}/pulls/new`
#[get("/{namespace}/{repo}/pulls/new")]
pub async fn new_pull_page_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
    query: web::Query<CompareQuery>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, &params.namespace, &params.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };
    let viewer = signed_in(&req, &server, &auth_state, &params.namespace).await;
    let may_create = viewer
        .as_ref()
        .is_some_and(|(_, perms)| perms.contains(crate::auth::NamespacePermission::PushChanges));
    let mut compare = match load(&ctx, &query) {
        Ok(compare) => compare,
        Err(error) => return Ok(render_error_page(&req, &ctx, &error.to_string())),
    };
    if let Ok(repo_id) = ctx.handle.repo_id() {
        compare.existing = auth_state
            .db()
            .find_open_pull_request(&repo_id, &compare.head, &compare.base)
            .await
            .ok()
            .flatten()
            .map(|pull| pull.number);
    }
    let body = render(&ctx, &compare, may_create, None);
    Ok(render_tab_response(&req, &ctx.frame(), "pulls", &body))
}

/// `POST /{namespace}/{repo}/pulls/new`
#[post("/{namespace}/{repo}/pulls/new")]
pub async fn create_pull_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
    form: web::Form<CreateForm>,
) -> HttpResponse {
    let ctx = match open_repo(&req, &server, &auth_state, &params.namespace, &params.repo).await {
        Ok(ctx) => ctx,
        Err(_) => return HttpResponse::NotFound().finish(),
    };
    let Some((user, permissions)) = signed_in(&req, &server, &auth_state, &params.namespace).await
    else {
        return HttpResponse::SeeOther()
            .insert_header((
                "Location",
                format!(
                    "/auth/login?return_to=/{}/{}/pulls/new",
                    params.namespace, params.repo
                ),
            ))
            .finish();
    };
    let repo_id = match ctx.handle.repo_id() {
        Ok(id) => id,
        Err(error) => return server_error(&error.to_string()),
    };
    let scope = RepoRef { repo_id: &repo_id };
    let actor = Actor { user_id: &user.id };
    let input = CreateInput {
        base: &form.base,
        head: &form.head,
        title: &form.title,
        body: &form.body,
    };
    match pulls::create(
        auth_state.db(),
        &ctx.handle,
        &scope,
        &actor,
        &permissions,
        input,
    )
    .await
    {
        Ok(pull) => HttpResponse::SeeOther()
            .insert_header((
                "Location",
                format!(
                    "/{}/{}/pulls/{}",
                    params.namespace, params.repo, pull.number
                ),
            ))
            .finish(),
        Err(error) => {
            let message = match &error {
                PullError::Invalid(message) | PullError::Conflict(message) => message.clone(),
                PullError::Forbidden => {
                    "You do not have permission to open pull requests here.".to_string()
                }
                PullError::NotFound => "The branch could not be found.".to_string(),
                PullError::Internal(detail) => {
                    log::error!("Failed to create pull request: {detail}");
                    "The pull request could not be created.".to_string()
                }
            };
            let compare = Compare {
                branches: ctx.handle.list_branches().map_or_else(
                    |_| Vec::new(),
                    |list| list.into_iter().map(|b| b.name).collect(),
                ),
                base: form.base.clone(),
                head: form.head.clone(),
                commits: Vec::new(),
                title: form.title.clone(),
                body: form.body.clone(),
                existing: None,
            };
            let body = render(&ctx, &compare, true, Some(&message));
            let markup = render_tab_response(&req, &ctx.frame(), "pulls", &body);
            HttpResponse::UnprocessableEntity()
                .content_type("text/html; charset=utf-8")
                .body(markup.into_string())
        }
    }
}

fn server_error(detail: &str) -> HttpResponse {
    log::error!("Pull request form failed: {detail}");
    HttpResponse::InternalServerError().body("The pull request could not be created.")
}

fn render_error_page(req: &HttpRequest, ctx: &RepoContext, detail: &str) -> Markup {
    log::error!("Compare page failed: {detail}");
    let body = html! {
        div class="twig-notice twig-notice--danger" role="alert" {
            "The branches could not be compared."
        }
    };
    render_tab_response(req, &ctx.frame(), "pulls", &body)
}

fn render(ctx: &RepoContext, compare: &Compare, may_create: bool, error: Option<&str>) -> Markup {
    let namespace = &ctx.namespace;
    let repo = &ctx.repo;
    let action = format!("/{namespace}/{repo}/pulls/new");
    let ready = !compare.commits.is_empty() && compare.existing.is_none();
    html! {
        div class="twig-stack twig-pr-compare" {
            form class="twig-pr-branches" method="GET" action=(action) {
                label class="twig-sr" for="pr-base" { "Base branch" }
                select id="pr-base" name="base" class="twig-input twig-input--mono" {
                    @for name in &compare.branches {
                        option value=(name) selected[name == &compare.base] { (name) }
                    }
                }
                span aria-hidden="true" { "←" }
                label class="twig-sr" for="pr-head" { "Head branch" }
                select id="pr-head" name="head" class="twig-input twig-input--mono" {
                    @for name in &compare.branches {
                        option value=(name) selected[name == &compare.head] { (name) }
                    }
                }
                button type="submit" class="twig-btn" { "Compare" }
            }
            @if let Some(message) = error {
                div class="twig-notice twig-notice--danger" role="alert" { (message) }
            }
            @if let Some(number) = compare.existing {
                div class="twig-notice twig-notice--info" role="status" {
                    "A pull request for these branches already exists: "
                    a href=(format!("/{namespace}/{repo}/pulls/{number}")) { "#" (number) }
                }
            } @else if compare.head.is_empty() || compare.base.is_empty() {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO BRANCHES" }
                    p class="twig-empty-body" { "Push a branch to this repository before opening a pull request." }
                }
            } @else if compare.commits.is_empty() {
                div class="twig-notice twig-notice--info" {
                    "These branches are equal. There is no need to create a pull request."
                }
            } @else {
                div class="twig-notice twig-notice--success" {
                    "Able to create a pull request from "
                    span class="twig-mono" { (compare.head) } " into "
                    span class="twig-mono" { (compare.base) } "."
                }
            }
            @if ready {
                div class="twig-pr-compare-grid" {
                    section class="twig-pr-compare-form" {
                        @if may_create {
                            form method="POST" action=(action) class="twig-form" {
                                input type="hidden" name="base" value=(compare.base);
                                input type="hidden" name="head" value=(compare.head);
                                label class="twig-label" for="pr-title" { "Title" }
                                input id="pr-title" class="twig-input" type="text" name="title"
                                    required value=(compare.title);
                                label class="twig-label" for="pr-body" { "Description" }
                                textarea id="pr-body" class="twig-input twig-input--mono" name="body" rows="12" {
                                    (compare.body)
                                }
                                div class="twig-form-actions" {
                                    button type="submit" class="twig-btn twig-btn--primary" {
                                        "Create pull request"
                                    }
                                }
                            }
                        } @else {
                            p class="twig-hint" { "Sign in with permission to push changes to open a pull request." }
                        }
                    }
                    aside class="twig-pr-compare-commits" {
                        h2 class="twig-section" { (compare.commits.len()) " commits" }
                        ol class="twig-commits" {
                            @for commit in &compare.commits {
                                li class="twig-commit" {
                                    span class="twig-commit-hash twig-mono" { (commit.short_id) }
                                    span class="twig-commit-msg" { (commit.summary) }
                                    span class="twig-commit-author" { (commit.author_name) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
