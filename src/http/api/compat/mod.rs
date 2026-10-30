//! Forgejo/Gitea-compatible REST routes under `/api/v1/user`, `/api/v1/version`,
//! and `/api/v1/repos/{owner}/{repo}/...`.
//!
//! Clients such as T3 Code (through `fj` or `tea`) call these paths with
//! `Authorization: token <personal access token>`. Responses and errors follow
//! Forgejo's shapes, which differ from Twig's own `/api/v1/namespaces` routes.
//! `owner` is the Twig namespace.

use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedPrincipal, NamespacePermission, PermissionSet, TwigContext},
    config,
    git::bare::RepoHandle,
};

use crate::pulls::PullError;

use super::v1::{self, ApiError};

pub mod code;
pub mod comments;
pub mod issues;
pub mod json;
pub mod pulls;
pub mod repos;
pub mod reviews;
pub mod user;

/// Registers every compatibility route. Called before the repository catch-all.
pub fn configure(cfg: &mut web::ServiceConfig) {
    // Suffixed paths (`{index}.diff`, `{sha}.diff`) must be registered before the
    // plain `{index}` and `{sha}` routes, which would otherwise match them first.
    cfg.service(code::pull_diff)
        .service(code::pull_patch)
        .service(code::get_commit_diff)
        .service(code::get_commit)
        .service(code::list_pull_commits)
        .service(code::get_contents)
        .service(user::current_user)
        .service(user::server_version)
        .service(user::create_user_repo)
        .service(repos::get_repository)
        .service(repos::get_branch)
        .service(repos::list_statuses)
        .service(repos::combined_status)
        .service(repos::list_labels)
        .service(repos::list_assignees)
        .service(pulls::list_pulls)
        .service(issues::list_issues)
        .service(code::get_raw_file)
        .service(code::compare)
        .service(code::list_pull_files)
        .service(pulls::create_pull)
        .service(pulls::get_pull)
        .service(pulls::update_pull)
        .service(pulls::merge_pull)
        .service(pulls::merge_status)
        .service(pulls::update_branch_handler)
        .service(comments::list_comments)
        .service(comments::create_comment)
        .service(comments::edit_comment)
        .service(comments::delete_comment)
        .service(comments::list_pull_reactions)
        .service(comments::add_pull_reaction)
        .service(comments::remove_pull_reaction)
        .service(comments::list_comment_reactions)
        .service(comments::add_comment_reaction)
        .service(comments::remove_comment_reaction)
        .service(reviews::list_reviews)
        .service(reviews::get_review)
        .service(reviews::list_review_comments)
        .service(reviews::submit_review)
        .service(reviews::list_requested_reviewers)
        .service(reviews::request_reviewers)
        .service(reviews::withdraw_reviewers)
        // Any other path under the API answers with Forgejo's JSON 404, not an empty body,
        // because clients decode error bodies. Registered last so real routes win.
        .route("/api/v1/{tail:.*}", web::route().to(unknown_route));
}

async fn unknown_route() -> HttpResponse {
    compat_error(StatusCode::NOT_FOUND, "The target couldn't be found")
}

#[derive(Debug, Deserialize)]
pub struct RepoPath {
    pub owner: String,
    pub repo: String,
}

/// Forgejo's error body: a human message and a documentation URL (may be empty).
#[derive(Debug, Serialize)]
struct CompatErrorBody<'a> {
    message: &'a str,
    url: &'a str,
}

pub fn compat_error(status: StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status).json(CompatErrorBody { message, url: "" })
}

/// Maps Twig's internal API errors onto Forgejo's body shape.
pub fn from_api_error(error: ApiError) -> HttpResponse {
    match error {
        ApiError::Unauthorized => compat_error(
            StatusCode::UNAUTHORIZED,
            "Valid Authorization credentials are required",
        ),
        ApiError::Forbidden => compat_error(
            StatusCode::FORBIDDEN,
            "The required permission is not granted",
        ),
        ApiError::NotFound => compat_error(StatusCode::NOT_FOUND, "The target couldn't be found"),
        ApiError::BadRequest(message) => compat_error(StatusCode::BAD_REQUEST, &message),
        ApiError::Conflict(message) => compat_error(StatusCode::CONFLICT, &message),
        ApiError::Gone(message) => compat_error(StatusCode::GONE, &message),
        ApiError::PayloadTooLarge => compat_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "The request payload is too large",
        ),
        ApiError::Internal => compat_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The request could not be completed",
        ),
    }
}

/// Maps request-rule failures from `crate::pulls` onto Forgejo's error body.
pub fn from_pull_error(error: PullError) -> HttpResponse {
    match error {
        PullError::NotFound => compat_error(StatusCode::NOT_FOUND, "The target couldn't be found"),
        PullError::Forbidden => compat_error(
            StatusCode::FORBIDDEN,
            "The required permission is not granted",
        ),
        PullError::Invalid(message) => compat_error(StatusCode::UNPROCESSABLE_ENTITY, &message),
        PullError::Conflict(message) => compat_error(StatusCode::CONFLICT, &message),
        PullError::Internal(detail) => {
            log::error!("Pull request operation failed: {detail}");
            compat_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            )
        }
    }
}

/// Repository access resolved for one request: who is calling, what they may do,
/// and the open repository.
pub struct CompatRepo {
    pub principal: AuthenticatedPrincipal,
    pub permissions: PermissionSet,
    pub handle: RepoHandle,
}

/// Authenticates the caller, requires `permission` in `owner`, and opens the
/// repository. Every compatibility repository route starts here.
pub async fn authorized_repo(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    owner: &str,
    repo: &str,
    permission: NamespacePermission,
) -> Result<CompatRepo, HttpResponse> {
    let principal = v1::authenticate(req, auth_state)
        .await
        .map_err(from_api_error)?;
    v1::validate_namespace_name(owner).map_err(from_api_error)?;
    v1::validate_repository_name(repo).map_err(from_api_error)?;
    let permissions =
        v1::require_namespace_permission(&principal, server, auth_state, owner, permission)
            .await
            .map_err(from_api_error)?;
    let handle = RepoHandle::open(server.project_root(), owner, repo)
        .map_err(|_| compat_error(StatusCode::NOT_FOUND, "The target couldn't be found"))?;
    Ok(CompatRepo {
        principal,
        permissions,
        handle,
    })
}

/// The numeric value of one query parameter, if it is present and numeric.
pub fn query_number(req: &HttpRequest, name: &str) -> Option<u32> {
    req.query_string().split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| value.parse().ok()).flatten()
    })
}

/// One page of a list held in memory. Clients page these lists until a page comes
/// back empty or without `rel="next"`, so every page must be cut from the same
/// ordered rows. Without `page` or `limit` the whole list is returned, which is what
/// single-call readers expect.
pub fn paginate<T: Serialize>(req: &HttpRequest, rows: Vec<T>) -> HttpResponse {
    let page = query_number(req, "page");
    let limit = query_number(req, "limit");
    let total = rows.len();
    // Every list reports its total, because clients compute page counts from it.
    let mut response = HttpResponse::Ok();
    response.insert_header(("X-Total-Count", total.to_string()));
    if page.is_none() && limit.is_none() {
        return response.json(rows);
    }
    let limit = limit.unwrap_or(30).clamp(1, 50) as usize;
    let page = page.unwrap_or(1).max(1);
    let start = (page as usize - 1) * limit;
    let slice: Vec<T> = rows.into_iter().skip(start).take(limit).collect();
    if start + limit < total {
        response.insert_header(("Link", next_page_link(req, page + 1)));
    }
    response.json(slice)
}

/// The `Link` header for paged lists. Keeps the caller's other query parameters
/// and replaces `page`, so a client can follow `rel="next"` without rebuilding it.
pub fn next_page_link(req: &HttpRequest, next_page: u32) -> String {
    let kept = req
        .query_string()
        .split('&')
        .filter(|pair| !pair.is_empty() && pair.split('=').next() != Some("page"))
        .collect::<Vec<_>>();
    let mut query = kept.join("&");
    if !query.is_empty() {
        query.push('&');
    }
    query.push_str(&format!("page={next_page}"));
    format!("<{}{}?{query}>; rel=\"next\"", origin(req), req.path())
}

/// Origin used for `html_url`, clone URLs, and `Link` headers. `PUBLIC_BASE_URL` wins
/// when configured; otherwise the request's scheme and host are used.
pub fn origin(req: &HttpRequest) -> String {
    if let Some(base) = req
        .app_data::<web::Data<config::Server>>()
        .and_then(|server| server.public_base_url())
    {
        return base.trim_end_matches('/').to_string();
    }
    let info = req.connection_info();
    format!("{}://{}", info.scheme(), info.host())
}
