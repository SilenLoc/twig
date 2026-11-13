//! The New issue page and its form POST. Signed-in viewers with access may open an
//! issue. The title is required; a missing title shows the form again with the reason.

use actix_web::{HttpRequest, HttpResponse, Result as AwResult, get, http::StatusCode, post, web};
use maud::{Markup, html};
use serde::Deserialize;

use crate::{
    auth::TwigContext,
    config,
    git::bare::RepoHandle,
    http::repository::pages::{
        Params,
        context::{RepoContext, open_repo},
        tabs::render_tab_response,
    },
    issues::{self, Actor, CreateInput, IssueError, RepoRef},
};

use super::{error_response, html_response, see_other, signed_in};

#[derive(Debug, Deserialize)]
pub(super) struct IssueForm {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
}

/// `GET /{namespace}/{repo}/issues/new`
#[get("/{namespace}/{repo}/issues/new")]
pub async fn new_issue_page_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, &params.namespace, &params.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };
    let viewer = signed_in(&req, &server, &auth_state, &params.namespace).await;
    let body = render_form(&ctx, viewer.is_some(), "", "", None);
    Ok(render_tab_response(&req, &ctx.frame(), "issues", &body))
}

/// `POST /{namespace}/{repo}/issues/new`
#[post("/{namespace}/{repo}/issues/new")]
pub async fn create_issue_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
    form: web::Form<IssueForm>,
) -> HttpResponse {
    let Some((user, permissions)) = signed_in(&req, &server, &auth_state, &params.namespace).await
    else {
        return error_response(StatusCode::UNAUTHORIZED, "Sign in to open an issue.");
    };
    let Ok(handle) = RepoHandle::open(server.project_root(), &params.namespace, &params.repo)
    else {
        return error_response(StatusCode::NOT_FOUND, "Not found.");
    };
    let repo_id = match handle.repo_id() {
        Ok(id) => id,
        Err(error) => {
            log::error!("Failed to read repository identity: {error}");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "The issue could not be saved.",
            );
        }
    };
    let scope = RepoRef {
        repo_id: &repo_id,
        namespace: &params.namespace,
    };
    let actor = Actor { user_id: &user.id };
    let input = CreateInput {
        title: &form.title,
        body: &form.body,
        assignees: &[],
    };
    match issues::create(auth_state.db(), &scope, &actor, &permissions, input).await {
        Ok(issue) => see_other(&format!(
            "/{}/{}/issues/{}",
            params.namespace, params.repo, issue.number
        )),
        Err(IssueError::Invalid(message)) => {
            let ctx = match open_repo(&req, &server, &auth_state, &params.namespace, &params.repo)
                .await
            {
                Ok(ctx) => ctx,
                Err(rendered) => {
                    return html_response(StatusCode::NOT_FOUND, rendered);
                }
            };
            let body = render_form(&ctx, true, &form.title, &form.body, Some(&message));
            html_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                render_tab_response(&req, &ctx.frame(), "issues", &body),
            )
        }
        Err(IssueError::Forbidden) => {
            error_response(StatusCode::FORBIDDEN, "You may not open issues here.")
        }
        Err(error) => {
            log::error!(
                "Failed to open an issue in {}/{}: {error:?}",
                params.namespace,
                params.repo
            );
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "The issue could not be saved.",
            )
        }
    }
}

/// The form for a new issue, or a sign-in notice when the viewer may not open one.
fn render_form(
    ctx: &RepoContext,
    can_open: bool,
    title: &str,
    body: &str,
    notice: Option<&str>,
) -> Markup {
    let namespace = &ctx.namespace;
    let repo = &ctx.repo;
    if !can_open {
        return html! {
            div class="twig-empty" {
                p class="twig-eyebrow" { "SIGN IN" }
                p class="twig-empty-body" { "Sign in to open an issue in this repository." }
            }
        };
    }
    html! {
        div class="twig-stack" {
            @if let Some(message) = notice {
                div class="twig-notice twig-notice--danger" role="alert" {
                    p class="twig-eyebrow" { "ERROR" }
                    p class="twig-notice-body" { (message) }
                }
            }
            form class="twig-stack" method="post" action=(format!("/{namespace}/{repo}/issues/new")) {
                label for="issue-title" { "Title" }
                input id="issue-title" name="title" type="text" value=(title) required;
                label for="issue-body" { "Description" }
                textarea id="issue-body" name="body" rows="12" placeholder="Describe the problem or request. Markdown is supported." {
                    (body)
                }
                div class="twig-row-actions" {
                    button type="submit" class="twig-btn twig-btn--primary" { "Submit new issue" }
                }
            }
        }
    }
}
