//! GitHub-compatible REST routes under `/api/v3`, the root `gh` uses for GitHub
//! Enterprise hosts. `GH_HOST=<twig host> gh ...` reaches these paths with no extra
//! configuration. Responses and errors follow GitHub's REST shapes.
//!
//! This module is being built beside the Forgejo routes in `compat`. Issues come first;
//! pull requests, reviews, and GraphQL follow in later slices.

use actix_web::{HttpRequest, HttpResponse, get, http::StatusCode, patch, post, web};
use serde::{Deserialize, Serialize};

use super::compat::{json::numeric_id, origin};
use crate::{
    auth::{AuthenticatedPrincipal, NamespacePermission, PermissionSet, TwigContext},
    config,
    db::{
        Database,
        issues::{Issue, IssueFilter, IssueState},
    },
    git::bare::RepoHandle,
    issues::{self, AccountFilter, Actor, CreateInput, IssueChange, IssueError, RepoRef},
};

use super::v1::{self, ApiError};

/// The enterprise version reported by the API root. Twig answers as a GitHub Enterprise
/// host, so clients take the same path they take for one.
const GHES_VERSION: &str = "3.15.0";

/// GitHub's `documentation_url` for error bodies.
const DOCS_URL: &str = "https://docs.github.com/rest";

pub mod pulls;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(api_root_endpoint)
        .service(current_user)
        .service(get_repository)
        .service(list_issues)
        .service(create_issue)
        .service(get_issue)
        .service(edit_issue);
    pulls::configure(cfg);
}

/// `GET /api/v3/`: the API root. Like GitHub's, it needs no credentials; `gh` calls it
/// before it sends the token, to check that the host answers as an API.
#[get("/api/v3/")]
pub async fn api_root_endpoint(req: HttpRequest) -> HttpResponse {
    let origin = origin(&req);
    HttpResponse::Ok()
        .insert_header(("X-GitHub-Enterprise-Version", GHES_VERSION))
        .json(serde_json::json!({
            "current_user_url": format!("{origin}/api/v3/user"),
            "repository_url": format!("{origin}/api/v3/repos/{{owner}}/{{repo}}"),
            "issues_url": format!("{origin}/api/v3/issues"),
            "pulls_url": format!("{origin}/api/v3/repos/{{owner}}/{{repo}}/pulls"),
        }))
}

/// `GET /api/v3/user`: the account the token belongs to.
#[get("/api/v3/user")]
pub async fn current_user(req: HttpRequest, auth_state: web::Data<TwigContext>) -> HttpResponse {
    let principal = match v1::authenticate(&req, &auth_state).await {
        Ok(principal) => principal,
        Err(failure) => return from_api_error(failure),
    };
    let origin = origin(&req);
    let user = &principal.user;
    HttpResponse::Ok().json(serde_json::json!({
        "login": user.username,
        "id": numeric_id(&user.id),
        "node_id": format!("U_{}", user.id),
        "avatar_url": "",
        "html_url": format!("{origin}/{}", user.username),
        "url": format!("{origin}/api/v3/users/{}", user.username),
        "type": "User",
        "site_admin": false,
        "name": serde_json::Value::Null,
        "email": user.email,
    }))
}

/// `GET /api/v3/repos/{owner}/{repo}`
#[get("/api/v3/repos/{owner}/{repo}")]
pub async fn get_repository(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
) -> HttpResponse {
    let repo = match open_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    let origin = origin(&req);
    let full_name = format!("{}/{}", path.owner, path.repo);
    let default_branch = repo.handle.default_branch_name().ok().flatten();
    let can_push = repo.permissions.contains(NamespacePermission::PushChanges);
    let can_admin = repo
        .permissions
        .contains(NamespacePermission::ManageRepositories);
    HttpResponse::Ok().json(serde_json::json!({
        "id": numeric_id(&repo.repo_id),
        "node_id": format!("R_{}", repo.repo_id),
        "name": path.repo,
        "full_name": full_name,
        "owner": user_json(&origin, &path.owner, &format!("namespace:{}", path.owner)),
        "private": false,
        "fork": false,
        "archived": false,
        "html_url": format!("{origin}/{full_name}"),
        "url": format!("{origin}/api/v3/repos/{full_name}"),
        "clone_url": format!("{origin}/{full_name}.git"),
        "ssh_url": "",
        "description": serde_json::Value::Null,
        "default_branch": default_branch,
        "has_issues": true,
        "has_pull_requests": true,
        "allow_merge_commit": true,
        "allow_squash_merge": true,
        "allow_rebase_merge": true,
        "permissions": {
            "admin": can_admin,
            "push": can_push,
            "pull": true,
        },
    }))
}

#[derive(Debug, Deserialize)]
pub struct RepoPath {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Deserialize)]
pub struct IssuePath {
    pub owner: String,
    pub repo: String,
    pub number: i64,
}

/// GitHub's error body. Validation errors may add `errors`; Twig sends the message only.
#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    message: &'a str,
    documentation_url: &'static str,
}

pub fn error(status: StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status).json(ErrorBody {
        message,
        documentation_url: DOCS_URL,
    })
}

fn from_api_error(failure: ApiError) -> HttpResponse {
    match failure {
        ApiError::Unauthorized => error(StatusCode::UNAUTHORIZED, "Requires authentication"),
        ApiError::Forbidden => error(
            StatusCode::FORBIDDEN,
            "Must have admin rights to Repository.",
        ),
        ApiError::NotFound => error(StatusCode::NOT_FOUND, "Not Found"),
        ApiError::BadRequest(message) => error(StatusCode::BAD_REQUEST, &message),
        ApiError::Conflict(message) => error(StatusCode::CONFLICT, &message),
        ApiError::Gone(message) => error(StatusCode::GONE, &message),
        ApiError::PayloadTooLarge => error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "The request payload is too large",
        ),
        ApiError::Internal => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The request could not be completed",
        ),
    }
}

fn from_issue_error(failure: IssueError) -> HttpResponse {
    match failure {
        IssueError::NotFound => error(StatusCode::NOT_FOUND, "Not Found"),
        IssueError::Forbidden => {
            error(StatusCode::FORBIDDEN, "Must have push access to the issue.")
        }
        IssueError::Invalid(message) => error(StatusCode::UNPROCESSABLE_ENTITY, &message),
        IssueError::Internal(detail) => {
            log::error!("Issue operation failed: {detail}");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            )
        }
    }
}

/// Repository access resolved for one request.
pub(super) struct Repo {
    pub(super) principal: AuthenticatedPrincipal,
    pub(super) permissions: PermissionSet,
    pub(super) handle: RepoHandle,
    pub(super) repo_id: String,
}

/// Authenticates the caller, requires `permission` in `owner`, and opens the repository.
async fn open_repo(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    owner: &str,
    repo: &str,
    permission: NamespacePermission,
) -> Result<Repo, HttpResponse> {
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
        .map_err(|_| error(StatusCode::NOT_FOUND, "Not Found"))?;
    let repo_id = handle.repo_id().map_err(|detail| {
        log::error!("Failed to read repository identity: {detail}");
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The request could not be completed",
        )
    })?;
    Ok(Repo {
        principal,
        permissions,
        handle,
        repo_id,
    })
}

/// A user in GitHub's shape. Email addresses are never disclosed here.
#[derive(Debug, Clone, Serialize)]
pub struct UserJson {
    pub login: String,
    pub id: i64,
    pub node_id: String,
    pub avatar_url: String,
    pub html_url: String,
    pub url: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub site_admin: bool,
}

fn user_json(origin: &str, username: &str, user_id: &str) -> UserJson {
    UserJson {
        login: username.to_string(),
        id: numeric_id(user_id),
        node_id: format!("U_{user_id}"),
        avatar_url: String::new(),
        html_url: format!("{origin}/{username}"),
        url: format!("{origin}/api/v3/users/{username}"),
        kind: "User",
        site_admin: false,
    }
}

/// An issue in GitHub's shape. `state_reason` follows GitHub: `completed` on a closed
/// issue and `null` while it is open.
#[derive(Debug, Clone, Serialize)]
pub struct IssueJson {
    pub id: i64,
    pub node_id: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: &'static str,
    pub state_reason: Option<&'static str>,
    pub locked: bool,
    pub user: Option<UserJson>,
    pub labels: Vec<serde_json::Value>,
    pub assignees: Vec<UserJson>,
    pub comments: i64,
    pub html_url: String,
    pub url: String,
    pub comments_url: String,
    pub repository_url: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
}

/// Renders one issue. An account that no longer exists is left out rather than failing.
async fn render_issue(
    db: &Database,
    origin: &str,
    owner: &str,
    repo: &str,
    issue: &Issue,
) -> IssueJson {
    let user = match db.get_user_by_id(&issue.author_user_id).await {
        Ok(Some(user)) => Some(user_json(origin, &user.username, &user.id)),
        _ => None,
    };
    let mut assignees = Vec::with_capacity(issue.assignee_user_ids.len());
    for user_id in &issue.assignee_user_ids {
        if let Ok(Some(user)) = db.get_user_by_id(user_id).await {
            assignees.push(user_json(origin, &user.username, &user.id));
        }
    }
    let api_root = format!("{origin}/api/v3/repos/{owner}/{repo}");
    let closed = issue.state == IssueState::Closed;
    IssueJson {
        id: numeric_id(&issue.id),
        node_id: format!("I_{}", issue.id),
        number: issue.number,
        title: issue.title.clone(),
        body: Some(issue.body.clone()),
        state: issue.state.as_str(),
        state_reason: closed.then_some("completed"),
        locked: false,
        user,
        labels: Vec::new(),
        assignees,
        comments: db
            .count_issue_comments(&issue.id)
            .await
            .ok()
            .and_then(|count| i64::try_from(count).ok())
            .unwrap_or_default(),
        html_url: format!("{origin}/{owner}/{repo}/issues/{}", issue.number),
        url: format!("{api_root}/issues/{}", issue.number),
        comments_url: format!("{api_root}/issues/{}/comments", issue.number),
        repository_url: api_root,
        created_at: issue.created_at.clone(),
        updated_at: issue.updated_at.clone(),
        closed_at: issue.closed_at.clone(),
    }
}

fn scope_for<'a>(repo: &'a Repo, owner: &'a str) -> RepoRef<'a> {
    RepoRef {
        repo_id: &repo.repo_id,
        namespace: owner,
    }
}

/// `page` and `per_page`, with GitHub's defaults (30 per page, at most 100).
fn paging(page: Option<u32>, per_page: Option<u32>) -> (u32, u32) {
    let per_page = per_page.unwrap_or(30).clamp(1, 100);
    let page = page.unwrap_or(1).max(1);
    (page, per_page)
}

/// The `Link` header for `rel="next"`, keeping the caller's other query parameters.
fn next_link(req: &HttpRequest, next_page: u32) -> String {
    super::compat::next_page_link(req, next_page)
}

#[derive(Debug, Deserialize)]
pub struct ListIssuesQuery {
    pub state: Option<String>,
    pub creator: Option<String>,
    pub assignee: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// `GET /api/v3/repos/{owner}/{repo}/issues`
#[get("/api/v3/repos/{owner}/{repo}/issues")]
pub async fn list_issues(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
    query: web::Query<ListIssuesQuery>,
) -> HttpResponse {
    let repo = match open_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    let state = match query.state.as_deref().unwrap_or("open") {
        "all" => None,
        "open" => Some(IssueState::Open),
        "closed" => Some(IssueState::Closed),
        _ => return error(StatusCode::UNPROCESSABLE_ENTITY, "Invalid state"),
    };
    let db = auth_state.db();
    let creator = match issues::account_filter(db, query.creator.as_deref()).await {
        Ok(filter) => filter,
        Err(error) => return from_issue_error(error),
    };
    let assignee = match issues::account_filter(db, query.assignee.as_deref()).await {
        Ok(filter) => filter,
        Err(error) => return from_issue_error(error),
    };
    let author_user_id = match creator {
        AccountFilter::Anyone => None,
        AccountFilter::Only(id) => Some(id),
        AccountFilter::NoOne => return HttpResponse::Ok().json(Vec::<IssueJson>::new()),
    };
    let assignee_user_id = match assignee {
        AccountFilter::Anyone => None,
        AccountFilter::Only(id) => Some(id),
        AccountFilter::NoOne => return HttpResponse::Ok().json(Vec::<IssueJson>::new()),
    };
    let (page, per_page) = paging(query.page, query.per_page);
    let filter = IssueFilter {
        state,
        query: None,
        author_user_id,
        assignee_user_id,
        limit: per_page,
        offset: (page - 1).saturating_mul(per_page),
    };
    let scope = scope_for(&repo, &path.owner);
    let found = match issues::list(db, &scope, &filter).await {
        Ok(found) => found,
        Err(error) => return from_issue_error(error),
    };
    let origin = origin(&req);
    let mut items = Vec::with_capacity(found.items.len());
    for issue in &found.items {
        items.push(render_issue(db, &origin, &path.owner, &path.repo, issue).await);
    }
    let mut response = HttpResponse::Ok();
    response.insert_header(("X-Total-Count", found.total.to_string()));
    if found.has_more {
        response.insert_header(("Link", next_link(&req, page + 1)));
    }
    response.json(items)
}

#[derive(Debug, Deserialize)]
pub struct CreateIssueBody {
    pub title: String,
    pub body: Option<String>,
    pub assignees: Option<Vec<String>>,
    pub labels: Option<Vec<String>>,
}

/// `POST /api/v3/repos/{owner}/{repo}/issues`. Answers `201` with the new issue.
#[post("/api/v3/repos/{owner}/{repo}/issues")]
pub async fn create_issue(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
    body: web::Json<CreateIssueBody>,
) -> HttpResponse {
    let repo = match open_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    if body
        .labels
        .as_ref()
        .is_some_and(|labels| !labels.is_empty())
    {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Labels are not supported on Twig",
        );
    }
    let db = auth_state.db();
    let scope = scope_for(&repo, &path.owner);
    let actor = Actor {
        user_id: &repo.principal.user.id,
    };
    let assignees = body.assignees.clone().unwrap_or_default();
    let input = CreateInput {
        title: &body.title,
        body: body.body.as_deref().unwrap_or(""),
        assignees: &assignees,
    };
    match issues::create(db, &scope, &actor, &repo.permissions, input).await {
        Ok(issue) => {
            let json = render_issue(db, &origin(&req), &path.owner, &path.repo, &issue).await;
            HttpResponse::Created().json(json)
        }
        Err(error) => from_issue_error(error),
    }
}

/// `GET /api/v3/repos/{owner}/{repo}/issues/{number}`
#[get("/api/v3/repos/{owner}/{repo}/issues/{number}")]
pub async fn get_issue(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
) -> HttpResponse {
    let repo = match open_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    let db = auth_state.db();
    let scope = scope_for(&repo, &path.owner);
    match issues::get(db, &scope, path.number).await {
        Ok(issue) => HttpResponse::Ok()
            .json(render_issue(db, &origin(&req), &path.owner, &path.repo, &issue).await),
        Err(error) => from_issue_error(error),
    }
}

#[derive(Debug, Deserialize)]
pub struct EditIssueBody {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<String>,
    pub assignees: Option<Vec<String>>,
    pub labels: Option<Vec<String>>,
}

/// `PATCH /api/v3/repos/{owner}/{repo}/issues/{number}`
#[patch("/api/v3/repos/{owner}/{repo}/issues/{number}")]
pub async fn edit_issue(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    body: web::Json<EditIssueBody>,
) -> HttpResponse {
    let repo = match open_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    if body
        .labels
        .as_ref()
        .is_some_and(|labels| !labels.is_empty())
    {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Labels are not supported on Twig",
        );
    }
    let state = match body.state.as_deref() {
        None => None,
        Some(value) => match IssueState::parse(value) {
            Some(state) => Some(state),
            None => return error(StatusCode::UNPROCESSABLE_ENTITY, "Invalid state"),
        },
    };
    let db = auth_state.db();
    let scope = scope_for(&repo, &path.owner);
    let issue = match issues::get(db, &scope, path.number).await {
        Ok(issue) => issue,
        Err(error) => return from_issue_error(error),
    };
    let actor = Actor {
        user_id: &repo.principal.user.id,
    };
    let change = IssueChange {
        title: body.title.clone(),
        body: body.body.clone(),
        state,
        assignees: body.assignees.clone(),
    };
    match issues::change(db, &scope, &issue, &actor, &repo.permissions, change).await {
        Ok(updated) => HttpResponse::Ok()
            .json(render_issue(db, &origin(&req), &path.owner, &path.repo, &updated).await),
        Err(error) => from_issue_error(error),
    }
}
