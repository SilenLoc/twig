//! The repository's Pull requests tab and its pages: list, compare (New pull
//! request), and conversation. Handlers read the session and call `crate::pulls`
//! for every rule; rendering lives in the sibling modules.

use actix_web::{HttpRequest, Result as AwResult, get, web};
use maud::Markup;

use crate::{
    auth::{PermissionSet, TwigContext, User, permissions},
    config,
    db::pull_requests::PullRequest,
};

use super::Params;
use super::context::{RepoContext, open_repo};
use super::tabs::render_tab_response;
use crate::http::view::render_error;

mod actions;
mod commits;
mod compare;
mod conversation;
mod files;
mod list;
mod timeline;

pub use actions::{
    comment_handler, delete_comment_handler, edit_comment_handler, edit_form_handler, edit_handler,
    merge_handler, reaction_handler, review_handler, reviewer_handler, state_handler,
};
pub use commits::commits_handler;
pub use compare::{create_pull_handler, new_pull_page_handler};
pub use conversation::pull_page_handler;
pub use files::{files_handler, inline_comment_handler};

/// The signed-in viewer's account and their permissions in one namespace.
/// Anonymous and unauthorized viewers get `None`.
pub(super) async fn signed_in(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
) -> Option<(User, PermissionSet)> {
    let user_id = auth_state.user_id_from_request(req).await?;
    let db = auth_state.db();
    let user = db.get_user_by_id(&user_id).await.ok()??;
    let permissions = permissions::for_user(
        db,
        &user.id,
        namespace,
        server.is_configured_admin(&user.username),
    )
    .await
    .ok()??;
    Some((user, permissions))
}

/// The author's username for display, or a placeholder when the account is gone.
pub(super) async fn author_name(auth_state: &web::Data<TwigContext>, pull: &PullRequest) -> String {
    match auth_state.db().get_user_by_id(&pull.author_user_id).await {
        Ok(Some(user)) => user.username,
        _ => "unknown".to_string(),
    }
}

#[derive(serde::Deserialize)]
pub(super) struct ListQuery {
    state: Option<String>,
}

/// `GET /{namespace}/{repo}/pulls`: the Pull requests tab.
#[get("/{namespace}/{repo}/pulls")]
pub async fn pulls_tab_handler(
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
    let state = query.state.as_deref().unwrap_or("open");
    let body = match list::load(&ctx, &auth_state, state).await {
        Ok(body) => body,
        Err(error) => {
            log::error!(
                "Failed to load pull requests for {}/{}: {error}",
                params.namespace,
                params.repo
            );
            return Ok(render_error("Pull requests could not be loaded."));
        }
    };
    Ok(render_tab_response(&req, &ctx.frame(), "pulls", &body))
}

/// The request's three views, as the tab bar above a request's page.
pub(super) fn request_nav(namespace: &str, repo: &str, number: i64, active: &str) -> maud::Markup {
    let base = format!("/{namespace}/{repo}/pulls/{number}");
    maud::html! {
        nav class="twig-pr-views" aria-label="Pull request views" {
            a class="twig-pr-view" href=(base) aria-current=[(active == "conversation").then_some("page")] { "Conversation" }
            a class="twig-pr-view" href=(format!("{base}/commits")) aria-current=[(active == "commits").then_some("page")] { "Commits" }
            a class="twig-pr-view" href=(format!("{base}/files")) aria-current=[(active == "files").then_some("page")] { "Files changed" }
        }
    }
}

/// The request for a page, or the notice to show instead. Shared by the commit and
/// file views, which need the same repository identity and request lookup.
pub(super) async fn request_for_page(
    ctx: &RepoContext,
    auth_state: &web::Data<TwigContext>,
    number: i64,
) -> Result<PullRequest, maud::Markup> {
    let repo_id = ctx.handle.repo_id().map_err(|error| {
        log::error!("Failed to read repository identity: {error}");
        crate::http::view::render_error("Pull request could not be loaded.")
    })?;
    crate::pulls::get(
        auth_state.db(),
        &crate::pulls::RepoRef { repo_id: &repo_id },
        number,
    )
    .await
    .map_err(|_| crate::http::view::render_error("Pull request was not found."))
}
