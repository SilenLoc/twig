//! The repository's Issues tab and its pages: the list, the new-issue form, and one
//! issue. Handlers read the session and call `crate::issues` for every rule; rendering
//! lives in the sibling modules.

use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use maud::Markup;

use crate::{
    auth::{PermissionSet, TwigContext, User},
    config,
    git::bare::RepoHandle,
    http::view::render_error,
    issues::IssueError,
};

mod actions;
mod list;
mod new;
mod view;

pub use actions::{
    assignees_handler, comment_handler, delete_comment_handler, edit_comment_handler, edit_handler,
    state_handler,
};
pub use list::issues_tab_handler;
pub use new::{create_issue_handler, new_issue_page_handler};
pub use view::issue_page_handler;

pub(super) use super::pulls::signed_in;

/// A signed-in viewer and the repository's ID, for POST handlers that change an issue.
pub(super) struct Writer {
    pub(super) user: User,
    pub(super) permissions: PermissionSet,
    pub(super) repo_id: String,
}

/// The signed-in viewer for a change. Signed-out visitors get a sign-in notice.
pub(super) async fn writer_for(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
    repo: &str,
) -> Result<Writer, HttpResponse> {
    let Some((user, permissions)) = signed_in(req, server, auth_state, namespace).await else {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "Sign in to change this issue.",
        ));
    };
    let handle = RepoHandle::open(server.project_root(), namespace, repo)
        .map_err(|_| error_response(StatusCode::NOT_FOUND, "Not found."))?;
    let repo_id = handle.repo_id().map_err(|error| {
        log::error!("Failed to read repository identity: {error}");
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The change could not be saved.",
        )
    })?;
    Ok(Writer {
        user,
        permissions,
        repo_id,
    })
}

/// The response for a failed issue change: its status and a page that says why.
pub(super) fn issue_error_response(error: IssueError) -> HttpResponse {
    match error {
        IssueError::NotFound => error_response(StatusCode::NOT_FOUND, "Issue was not found."),
        IssueError::Forbidden => error_response(StatusCode::FORBIDDEN, "You may not change this."),
        IssueError::Invalid(message) => error_response(StatusCode::UNPROCESSABLE_ENTITY, &message),
        IssueError::Internal(detail) => {
            log::error!("Issue change failed: {detail}");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "The change could not be saved.",
            )
        }
    }
}

/// The account's username for display, or a placeholder when the account is gone.
pub(super) async fn author_name(auth_state: &web::Data<TwigContext>, user_id: &str) -> String {
    match auth_state.db().get_user_by_id(user_id).await {
        Ok(Some(user)) => user.username,
        _ => "unknown".to_string(),
    }
}

/// An HTML response with a status, for POST handlers that show a page instead of redirecting.
pub(super) fn html_response(status: StatusCode, markup: Markup) -> HttpResponse {
    HttpResponse::build(status)
        .content_type("text/html; charset=utf-8")
        .body(markup.into_string())
}

/// The error page for a failed POST, with a status.
pub(super) fn error_response(status: StatusCode, message: &str) -> HttpResponse {
    html_response(status, render_error(message))
}

/// Redirects after a successful POST, so a refresh never repeats the change.
pub(super) fn see_other(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header(("Location", location.to_string()))
        .finish()
}
