//! POST handlers that change a request: comments, reactions, state, and edits.
//! Each one checks the session and the rules in `crate::pulls`, then redirects
//! back to the conversation, so a refresh never repeats the change.

use actix_web::{HttpRequest, HttpResponse, Result as AwResult, get, post, web};
use maud::Markup;
use serde::Deserialize;

use crate::{
    auth::TwigContext,
    config,
    db::pull_requests::{
        PullRequest, PullRequestEdit, PullRequestState, reactions::ReactionContent,
    },
    git::bare::RepoHandle,
    http::repository::pages::{context::open_repo, tabs::render_tab_response},
    pulls::{self, Actor, PullError, RepoRef},
};

use super::signed_in;

#[derive(Debug, Deserialize)]
pub(super) struct PullPath {
    namespace: String,
    repo: String,
    number: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct CommentPath {
    namespace: String,
    repo: String,
    number: i64,
    id: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct BodyForm {
    #[serde(default)]
    body: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct StateForm {
    action: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct EditForm {
    title: String,
    #[serde(default)]
    body: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReviewForm {
    event: String,
    #[serde(default)]
    body: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReviewerForm {
    action: String,
    reviewer: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReactionForm {
    target: String,
    content: String,
    on: String,
}

fn see_other(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header(("Location", location.to_string()))
        .finish()
}

fn login_redirect(namespace: &str, repo: &str) -> HttpResponse {
    see_other(&format!("/auth/login?return_to=/{namespace}/{repo}/pulls"))
}

fn error_page(status: actix_web::http::StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status)
        .content_type("text/html; charset=utf-8")
        .body(crate::http::view::render_error(message).into_string())
}

fn error_for(error: &PullError) -> HttpResponse {
    match error {
        PullError::NotFound => error_page(actix_web::http::StatusCode::NOT_FOUND, "Not found."),
        PullError::Forbidden => error_page(
            actix_web::http::StatusCode::FORBIDDEN,
            "You do not have permission to change this pull request.",
        ),
        PullError::Invalid(message) => {
            error_page(actix_web::http::StatusCode::UNPROCESSABLE_ENTITY, message)
        }
        PullError::Conflict(message) => error_page(actix_web::http::StatusCode::CONFLICT, message),
        PullError::Internal(detail) => {
            log::error!("Pull request action failed: {detail}");
            error_page(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "The change could not be saved.",
            )
        }
    }
}

/// The session user, the open repository, and the request, or the response that
/// explains why not.
struct Scope {
    user: crate::auth::User,
    permissions: crate::auth::PermissionSet,
    pull: PullRequest,
}

async fn scope(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
    repo: &str,
    number: i64,
) -> Result<Scope, HttpResponse> {
    let Some((user, permissions)) = signed_in(req, server, auth_state, namespace).await else {
        return Err(login_redirect(namespace, repo));
    };
    let handle = RepoHandle::open(server.project_root(), namespace, repo)
        .map_err(|_| error_page(actix_web::http::StatusCode::NOT_FOUND, "Not found."))?;
    let repo_id = handle.repo_id().map_err(|error| {
        log::error!("Failed to read repository identity: {error}");
        error_page(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "The change could not be saved.",
        )
    })?;
    let pull = pulls::get(auth_state.db(), &RepoRef { repo_id: &repo_id }, number)
        .await
        .map_err(|error| error_for(&error))?;
    Ok(Scope {
        user,
        permissions,
        pull,
    })
}

fn to_location(namespace: &str, repo: &str, number: i64) -> String {
    format!("/{namespace}/{repo}/pulls/{number}")
}

/// `POST /{namespace}/{repo}/pulls/{number}/comments`
#[post("/{namespace}/{repo}/pulls/{number:\\d+}/comments")]
pub async fn comment_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    form: web::Form<BodyForm>,
) -> HttpResponse {
    let scope = match scope(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &scope.user.id,
    };
    match pulls::add_comment(
        auth_state.db(),
        &scope.pull,
        &actor,
        &scope.permissions,
        &form.body,
    )
    .await
    {
        Ok(_) => see_other(&to_location(&path.namespace, &path.repo, path.number)),
        Err(error) => error_for(&error),
    }
}

/// `POST /{namespace}/{repo}/pulls/{number}/comments/{id}/edit`
#[post("/{namespace}/{repo}/pulls/{number:\\d+}/comments/{id:\\d+}/edit")]
pub async fn edit_comment_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CommentPath>,
    form: web::Form<BodyForm>,
) -> HttpResponse {
    let scope = match scope(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &scope.user.id,
    };
    match pulls::edit_comment(
        auth_state.db(),
        &scope.pull,
        path.id,
        &actor,
        &scope.permissions,
        &form.body,
    )
    .await
    {
        Ok(_) => see_other(&format!(
            "{}#comment-{}",
            to_location(&path.namespace, &path.repo, path.number),
            path.id
        )),
        Err(error) => error_for(&error),
    }
}

/// `POST /{namespace}/{repo}/pulls/{number}/comments/{id}/delete`
#[post("/{namespace}/{repo}/pulls/{number:\\d+}/comments/{id:\\d+}/delete")]
pub async fn delete_comment_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CommentPath>,
) -> HttpResponse {
    let scope = match scope(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &scope.user.id,
    };
    match pulls::delete_comment(
        auth_state.db(),
        &scope.pull,
        path.id,
        &actor,
        &scope.permissions,
    )
    .await
    {
        Ok(()) => see_other(&to_location(&path.namespace, &path.repo, path.number)),
        Err(error) => error_for(&error),
    }
}

/// `POST /{namespace}/{repo}/pulls/{number}/state` with `action=close|reopen`
#[post("/{namespace}/{repo}/pulls/{number:\\d+}/state")]
pub async fn state_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    form: web::Form<StateForm>,
) -> HttpResponse {
    let scope = match scope(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let state = match form.action.as_str() {
        "close" => PullRequestState::Closed,
        "reopen" => PullRequestState::Open,
        _ => {
            return error_page(
                actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                "Unknown action.",
            );
        }
    };
    let actor = Actor {
        user_id: &scope.user.id,
    };
    let change = PullRequestEdit {
        state: Some(state),
        ..Default::default()
    };
    match pulls::edit(
        auth_state.db(),
        &scope.pull,
        &actor,
        &scope.permissions,
        change,
    )
    .await
    {
        Ok(_) => see_other(&to_location(&path.namespace, &path.repo, path.number)),
        Err(error) => error_for(&error),
    }
}

/// `GET /{namespace}/{repo}/pulls/{number}/edit`: title and description form.
#[get("/{namespace}/{repo}/pulls/{number:\\d+}/edit")]
pub async fn edit_form_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, &path.namespace, &path.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };
    let scope = match scope(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(scope) => scope,
        Err(_) => {
            return Ok(crate::http::view::render_error(
                "Sign in to edit this pull request.",
            ));
        }
    };
    if !super::timeline::may_change_request(&scope.pull, Some((&scope.user, &scope.permissions))) {
        return Ok(crate::http::view::render_error(
            "You do not have permission to edit this pull request.",
        ));
    }
    let body = super::conversation::edit_form(&ctx, &scope.pull);
    Ok(render_tab_response(&req, &ctx.frame(), "pulls", &body))
}

/// `POST /{namespace}/{repo}/pulls/{number}/edit`
#[post("/{namespace}/{repo}/pulls/{number:\\d+}/edit")]
pub async fn edit_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    form: web::Form<EditForm>,
) -> HttpResponse {
    let scope = match scope(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &scope.user.id,
    };
    let change = PullRequestEdit {
        title: Some(form.title.clone()),
        body: Some(form.body.clone()),
        state: None,
    };
    match pulls::edit(
        auth_state.db(),
        &scope.pull,
        &actor,
        &scope.permissions,
        change,
    )
    .await
    {
        Ok(_) => see_other(&to_location(&path.namespace, &path.repo, path.number)),
        Err(error) => error_for(&error),
    }
}

/// `POST /{namespace}/{repo}/pulls/{number}/reactions`: toggles one reaction on the
/// request (`target=pull`) or one comment (`target=comment:<id>`).
#[post("/{namespace}/{repo}/pulls/{number:\\d+}/reactions")]
pub async fn reaction_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    form: web::Form<ReactionForm>,
) -> HttpResponse {
    let scope = match scope(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let Some(content) = ReactionContent::parse(&form.content) else {
        return error_page(
            actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            "Unknown reaction.",
        );
    };
    let comment_id = match form.target.strip_prefix("comment:") {
        Some(id) => match id.parse::<i64>() {
            Ok(id) => Some(id),
            Err(_) => {
                return error_page(
                    actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                    "Unknown target.",
                );
            }
        },
        None if form.target == "pull" => None,
        None => {
            return error_page(
                actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                "Unknown target.",
            );
        }
    };
    let actor = Actor {
        user_id: &scope.user.id,
    };
    let on = form.on == "true";
    match pulls::react(
        auth_state.db(),
        &scope.pull,
        comment_id,
        &actor,
        &scope.permissions,
        content,
        on,
    )
    .await
    {
        Ok(()) => see_other(&to_location(&path.namespace, &path.repo, path.number)),
        Err(error) => error_for(&error),
    }
}

/// `POST /{namespace}/{repo}/pulls/{number}/reviews`: a review with the composer's
/// text. The verdict comes from the button pressed.
#[post("/{namespace}/{repo}/pulls/{number:\\d+}/reviews")]
pub async fn review_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    form: web::Form<ReviewForm>,
) -> HttpResponse {
    let scope = match scope(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let Some(state) = crate::db::pull_requests::reviews::ReviewState::parse(&form.event) else {
        return error_page(
            actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            "Unknown review.",
        );
    };
    let head = match RepoHandle::open(server.project_root(), &path.namespace, &path.repo)
        .ok()
        .and_then(|handle| handle.branch_tip(&scope.pull.head_branch).ok().flatten())
    {
        Some(oid) => oid.to_string(),
        None => {
            return error_page(
                actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                "The head branch is missing.",
            );
        }
    };
    let actor = Actor {
        user_id: &scope.user.id,
    };
    match pulls::submit_review(
        auth_state.db(),
        &scope.pull,
        &actor,
        &scope.permissions,
        state,
        &form.body,
        &head,
        &[],
    )
    .await
    {
        Ok(_) => see_other(&to_location(&path.namespace, &path.repo, path.number)),
        Err(error) => error_for(&error),
    }
}

/// `POST /{namespace}/{repo}/pulls/{number}/reviewers` with `action=add|remove`.
#[post("/{namespace}/{repo}/pulls/{number:\\d+}/reviewers")]
pub async fn reviewer_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    form: web::Form<ReviewerForm>,
) -> HttpResponse {
    let scope = match scope(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let requested = match form.action.as_str() {
        "add" => true,
        "remove" => false,
        _ => {
            return error_page(
                actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                "Unknown action.",
            );
        }
    };
    let actor = Actor {
        user_id: &scope.user.id,
    };
    match pulls::set_requested_reviewer(
        auth_state.db(),
        &path.namespace,
        &scope.pull,
        &actor,
        &scope.permissions,
        &form.reviewer,
        requested,
    )
    .await
    {
        Ok(()) => see_other(&to_location(&path.namespace, &path.repo, path.number)),
        Err(error) => error_for(&error),
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct MergeForm {
    method: String,
}

/// `POST /{namespace}/{repo}/pulls/{number}/merge` with `method=merge|squash|rebase`.
#[post("/{namespace}/{repo}/pulls/{number:\\d+}/merge")]
pub async fn merge_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    form: web::Form<MergeForm>,
) -> HttpResponse {
    let scope = match scope(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let strategy = match form.method.as_str() {
        "merge" => crate::git::bare::MergeStrategy::Merge,
        "squash" => crate::git::bare::MergeStrategy::Squash,
        "rebase" => crate::git::bare::MergeStrategy::Rebase,
        _ => {
            return error_page(
                actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                "Unknown merge style.",
            );
        }
    };
    let handle = match RepoHandle::open(server.project_root(), &path.namespace, &path.repo) {
        Ok(handle) => handle,
        Err(_) => return error_page(actix_web::http::StatusCode::NOT_FOUND, "Not found."),
    };
    let email = scope
        .user
        .email
        .clone()
        .unwrap_or_else(|| format!("{}@localhost", scope.user.username));
    let merger = pulls::Merger {
        name: &scope.user.username,
        email: &email,
    };
    let actor = Actor {
        user_id: &scope.user.id,
    };
    match pulls::merge(
        auth_state.db(),
        &handle,
        &scope.pull,
        &actor,
        &scope.permissions,
        strategy,
        None,
        None,
        &merger,
    )
    .await
    {
        Ok(_) => see_other(&to_location(&path.namespace, &path.repo, path.number)),
        Err(error) => error_for(&error),
    }
}
