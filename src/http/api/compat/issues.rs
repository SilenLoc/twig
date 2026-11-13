//! `GET|POST /repos/{owner}/{repo}/issues` and the issue shape Forgejo clients use.
//! `type=issues` lists and creates issues. `type=pulls` (and a missing `type`) lists
//! pull requests in the issue shape, with a `pull_request` marker. Templates and config
//! answer with blank issues allowed.

use actix_web::{HttpRequest, HttpResponse, get, patch, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{NamespacePermission, TwigContext},
    config,
    db::{
        Database,
        issues::{
            Issue, IssueFilter, IssueState,
            comments::{IssueComment, issue_comment_api_id, issue_comment_row_id},
        },
        pull_requests::{PullRequestFilter, PullRequestState},
    },
    issues::{self, Actor, CreateInput, IssueChange, IssueError},
    pulls::{self, RepoRef},
};

use super::{
    CompatRepo, RepoPath, authorized_repo,
    comments::CommentJson,
    compat_error, from_issue_error, from_pull_error,
    json::{PullRequestJson, UserJson, numeric_id},
    origin,
};

#[derive(Debug, Deserialize)]
pub struct IssueListQuery {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub state: Option<String>,
    pub q: Option<String>,
    pub created_by: Option<String>,
    pub assigned_by: Option<String>,
    pub labels: Option<String>,
    pub milestones: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIssueBody {
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub assignees: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct PullRequestMarker {
    pub merged: bool,
    pub merged_at: Option<String>,
    pub draft: bool,
    pub html_url: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct IssueJson {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub state: &'static str,
    pub user: Option<UserJson>,
    pub html_url: String,
    pub url: String,
    pub comments: i64,
    pub is_locked: bool,
    pub labels: Vec<serde_json::Value>,
    pub assignees: Vec<UserJson>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub pull_request: Option<PullRequestMarker>,
}

impl IssueJson {
    fn from_pull(pull: PullRequestJson) -> Self {
        Self {
            id: pull.id,
            number: pull.number,
            title: pull.title,
            body: pull.body,
            state: pull.state,
            user: pull.user,
            html_url: pull.html_url.clone(),
            url: pull.url.clone(),
            comments: pull.comments,
            is_locked: pull.is_locked,
            labels: pull.labels,
            assignees: pull.assignees,
            created_at: pull.created_at,
            updated_at: pull.updated_at,
            closed_at: pull.closed_at,
            pull_request: Some(PullRequestMarker {
                merged: pull.merged,
                merged_at: pull.merged_at,
                draft: pull.draft,
                html_url: pull.html_url,
                url: pull.url,
            }),
        }
    }
}

/// The Forgejo issue shape for a native issue. Author and assignees are looked up
/// by ID; an account that no longer exists is left out rather than failing the read.
async fn render_issue(
    db: &Database,
    origin: &str,
    owner: &str,
    repo: &str,
    issue: &Issue,
) -> IssueJson {
    let user = match db.get_user_by_id(&issue.author_user_id).await {
        Ok(Some(user)) => Some(UserJson::public(&user)),
        _ => None,
    };
    let mut assignees = Vec::with_capacity(issue.assignee_user_ids.len());
    for user_id in &issue.assignee_user_ids {
        if let Ok(Some(user)) = db.get_user_by_id(user_id).await {
            assignees.push(UserJson::public(&user));
        }
    }
    IssueJson {
        id: numeric_id(&issue.id),
        number: issue.number,
        title: issue.title.clone(),
        body: issue.body.clone(),
        state: issue.state.as_str(),
        user,
        html_url: format!("{origin}/{owner}/{repo}/issues/{}", issue.number),
        url: format!(
            "{origin}/api/v1/repos/{owner}/{repo}/issues/{}",
            issue.number
        ),
        comments: db
            .count_issue_comments(&issue.id)
            .await
            .ok()
            .and_then(|count| i64::try_from(count).ok())
            .unwrap_or_default(),
        is_locked: false,
        labels: Vec::new(),
        assignees,
        created_at: issue.created_at.clone(),
        updated_at: issue.updated_at.clone(),
        closed_at: issue.closed_at.clone(),
        pull_request: None,
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/issues`
#[get("/api/v1/repos/{owner}/{repo}/issues")]
pub async fn list_issues(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
    query: web::Query<IssueListQuery>,
) -> HttpResponse {
    let repo = match authorized_repo(
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
    let repo_id = match repo.handle.repo_id() {
        Ok(id) => id,
        Err(error) => {
            log::error!("Failed to read repository identity: {error}");
            return compat_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            );
        }
    };
    if query.kind.as_deref() == Some("issues") {
        return list_native(&req, &auth_state, &path, &query, &repo_id).await;
    }
    let state = match query.state.as_deref().unwrap_or("open") {
        "all" => None,
        other => match PullRequestState::parse(other) {
            Some(state) => Some(state),
            None => {
                return compat_error(
                    actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                    "Invalid state",
                );
            }
        },
    };
    let limit = query.limit.unwrap_or(30).clamp(1, 50);
    let page = query.page.unwrap_or(1).max(1);
    let filter = PullRequestFilter {
        state,
        head_branch: None,
        query: query.q.clone(),
        recently_updated: false,
        limit,
        offset: (page - 1).saturating_mul(limit),
    };
    let (rows, has_more) =
        match pulls::list(auth_state.db(), &RepoRef { repo_id: &repo_id }, &filter).await {
            Ok(found) => found,
            Err(error) => return from_pull_error(error),
        };
    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        match super::pulls::render_for_list(
            auth_state.db(),
            &repo.handle,
            &origin(&req),
            &path.owner,
            &path.repo,
            row,
        )
        .await
        {
            Ok(pull) => items.push(IssueJson::from_pull(pull)),
            Err(response) => return response,
        }
    }
    let total = match auth_state.db().count_pull_requests(&repo_id, &filter).await {
        Ok(total) => total,
        Err(error) => return from_pull_error(crate::pulls::PullError::Internal(error)),
    };
    let mut response = HttpResponse::Ok();
    response.insert_header(("X-Total-Count", total.to_string()));
    if has_more {
        response.insert_header(("Link", super::next_page_link(&req, page + 1)));
    }
    response.json(items)
}

/// The `type=issues` list: native issues, newest first, with Forgejo's paging headers.
async fn list_native(
    req: &HttpRequest,
    auth_state: &web::Data<TwigContext>,
    path: &web::Path<RepoPath>,
    query: &web::Query<IssueListQuery>,
    repo_id: &str,
) -> HttpResponse {
    let state = match query.state.as_deref().unwrap_or("open") {
        "all" => None,
        other => match IssueState::parse(other) {
            Some(state) => Some(state),
            None => {
                return compat_error(
                    actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                    "Invalid state",
                );
            }
        },
    };
    // Twig has no labels or milestones. Asking for them is refused, not answered with an
    // empty list that would look like a real result.
    if has_filter(query.labels.as_deref()) {
        return compat_error(
            actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            "Labels are not supported on Twig",
        );
    }
    if has_filter(query.milestones.as_deref()) {
        return compat_error(
            actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            "Milestones are not supported on Twig",
        );
    }
    let author = match issues::account_filter(auth_state.db(), query.created_by.as_deref()).await {
        Ok(issues::AccountFilter::Anyone) => None,
        Ok(issues::AccountFilter::Only(id)) => Some(id),
        Ok(issues::AccountFilter::NoOne) => return empty_list(),
        Err(error) => return from_issue_error(error),
    };
    let assignee = match issues::account_filter(auth_state.db(), query.assigned_by.as_deref()).await
    {
        Ok(issues::AccountFilter::Anyone) => None,
        Ok(issues::AccountFilter::Only(id)) => Some(id),
        Ok(issues::AccountFilter::NoOne) => return empty_list(),
        Err(error) => return from_issue_error(error),
    };
    let limit = query.limit.unwrap_or(30).clamp(1, 50);
    let page = query.page.unwrap_or(1).max(1);
    let filter = IssueFilter {
        state,
        query: query.q.clone().filter(|text| !text.trim().is_empty()),
        author_user_id: author,
        assignee_user_id: assignee,
        limit,
        offset: (page - 1).saturating_mul(limit),
    };
    let scope = issues::RepoRef {
        repo_id,
        namespace: &path.owner,
    };
    let page_of = match issues::list(auth_state.db(), &scope, &filter).await {
        Ok(page_of) => page_of,
        Err(error) => return from_issue_error(error),
    };
    let base = origin(req);
    let mut items = Vec::with_capacity(page_of.items.len());
    for issue in &page_of.items {
        items.push(render_issue(auth_state.db(), &base, &path.owner, &path.repo, issue).await);
    }
    let mut response = HttpResponse::Ok();
    response.insert_header(("X-Total-Count", page_of.total.to_string()));
    if page_of.has_more {
        response.insert_header(("Link", super::next_page_link(req, page + 1)));
    }
    response.json(items)
}

/// `POST /api/v1/repos/{owner}/{repo}/issues`
#[post("/api/v1/repos/{owner}/{repo}/issues")]
pub async fn create_issue(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
    body: web::Json<CreateIssueBody>,
) -> HttpResponse {
    let repo = match authorized_repo(
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
    let repo_id = match repo.handle.repo_id() {
        Ok(id) => id,
        Err(error) => {
            log::error!("Failed to read repository identity: {error}");
            return compat_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            );
        }
    };
    let scope = issues::RepoRef {
        repo_id: &repo_id,
        namespace: &path.owner,
    };
    let actor = Actor {
        user_id: &repo.principal.user.id,
    };
    let assignees = body.assignees.clone().unwrap_or_default();
    let input = CreateInput {
        title: &body.title,
        body: body.body.as_deref().unwrap_or(""),
        assignees: &assignees,
    };
    match issues::create(auth_state.db(), &scope, &actor, &repo.permissions, input).await {
        Ok(issue) => {
            let json = render_issue(
                auth_state.db(),
                &origin(&req),
                &path.owner,
                &path.repo,
                &issue,
            )
            .await;
            HttpResponse::Created().json(json)
        }
        Err(error) => from_issue_error(error),
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/issues/{index}`. A pull request's number answers
/// with the pull request in issue shape, so `fj issue view` works on either.
#[get("/api/v1/repos/{owner}/{repo}/issues/{index}")]
pub async fn get_issue(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<super::comments::IssuePath>,
) -> HttpResponse {
    let repo = match authorized_repo(
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
    let repo_id = match repo.handle.repo_id() {
        Ok(id) => id,
        Err(error) => {
            log::error!("Failed to read repository identity: {error}");
            return compat_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            );
        }
    };
    let scope = issues::RepoRef {
        repo_id: &repo_id,
        namespace: &path.owner,
    };
    match issues::get(auth_state.db(), &scope, path.index).await {
        Ok(issue) => {
            let json = render_issue(
                auth_state.db(),
                &origin(&req),
                &path.owner,
                &path.repo,
                &issue,
            )
            .await;
            HttpResponse::Ok().json(json)
        }
        Err(IssueError::NotFound) => {
            let scope = RepoRef { repo_id: &repo_id };
            match pulls::get(auth_state.db(), &scope, path.index).await {
                Ok(pull) => match super::pulls::render_for_list(
                    auth_state.db(),
                    &repo.handle,
                    &origin(&req),
                    &path.owner,
                    &path.repo,
                    &pull,
                )
                .await
                {
                    Ok(json) => HttpResponse::Ok().json(IssueJson::from_pull(json)),
                    Err(response) => response,
                },
                Err(_) => from_issue_error(IssueError::NotFound),
            }
        }
        Err(error) => from_issue_error(error),
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/issue_templates`. Twig has no templates, and
/// Forgejo answers `404` for a repository without any. `fj` reads that as "blank
/// issues allowed" and creates an issue without `--no-template`; an empty `200` list
/// makes it refuse.
#[get("/api/v1/repos/{owner}/{repo}/issue_templates")]
pub async fn issue_templates(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
) -> HttpResponse {
    match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(_) => compat_error(
            actix_web::http::StatusCode::NOT_FOUND,
            "The target couldn't be found",
        ),
        Err(response) => response,
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/issue_config`: blank issues are allowed, with no contact links.
#[get("/api/v1/repos/{owner}/{repo}/issue_config")]
pub async fn issue_config(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
) -> HttpResponse {
    match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "blank_issues_enabled": true,
            "contact_links": [],
        })),
        Err(response) => response,
    }
}

/// The caller's access to one repository, with its ID, for issue routes.
pub(super) struct IssueScope {
    pub compat: CompatRepo,
    pub repo_id: String,
    pub owner: String,
    pub name: String,
}

impl IssueScope {
    fn rules(&self) -> issues::RepoRef<'_> {
        issues::RepoRef {
            repo_id: &self.repo_id,
            namespace: &self.owner,
        }
    }
}

/// Authenticates and resolves the repository every issue route starts with.
pub(super) async fn issue_scope(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    owner: &str,
    repo: &str,
) -> Result<IssueScope, HttpResponse> {
    let compat = authorized_repo(
        req,
        server,
        auth_state,
        owner,
        repo,
        NamespacePermission::ViewNamespace,
    )
    .await?;
    let repo_id = compat.handle.repo_id().map_err(|error| {
        log::error!("Failed to read repository identity: {error}");
        compat_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "The request could not be completed",
        )
    })?;
    Ok(IssueScope {
        compat,
        repo_id,
        owner: owner.to_string(),
        name: repo.to_string(),
    })
}

/// The Forgejo shape of one issue comment. `pull_request_url` is empty, and `issue_url`
/// names the issue.
async fn comment_json(
    db: &Database,
    origin: &str,
    owner: &str,
    repo: &str,
    number: i64,
    comment: &IssueComment,
) -> CommentJson {
    let user = match db.get_user_by_id(&comment.author_user_id).await {
        Ok(Some(user)) => Some(UserJson::public(&user)),
        _ => None,
    };
    let api_id = issue_comment_api_id(comment.row_id);
    CommentJson {
        id: api_id,
        html_url: format!("{origin}/{owner}/{repo}/issues/{number}#issuecomment-{api_id}"),
        pull_request_url: String::new(),
        issue_url: format!("{origin}/api/v1/repos/{owner}/{repo}/issues/{number}"),
        original_author_id: user.as_ref().map_or(0, |user| user.id),
        original_author: user
            .as_ref()
            .map_or_else(String::new, |user| user.login.clone()),
        user,
        body: comment.body.clone(),
        created_at: comment.created_at.clone(),
        updated_at: comment
            .updated_at
            .clone()
            .unwrap_or_else(|| comment.created_at.clone()),
        assets: Vec::new(),
    }
}

/// `GET .../issues/{n}/comments` for an issue.
pub(super) async fn list_comments(
    req: &HttpRequest,
    auth_state: &web::Data<TwigContext>,
    scope: &IssueScope,
    issue: &Issue,
) -> HttpResponse {
    let db = auth_state.db();
    let comments = match db.list_issue_comments(&issue.id).await {
        Ok(comments) => comments,
        Err(error) => return from_issue_error(IssueError::Internal(error)),
    };
    let base = origin(req);
    let mut items = Vec::with_capacity(comments.len());
    for comment in &comments {
        items.push(comment_json(db, &base, &scope.owner, &scope.name, issue.number, comment).await);
    }
    super::paginate(req, items)
}

/// `POST .../issues/{n}/comments` for an issue.
pub(super) async fn create_comment(
    req: &HttpRequest,
    auth_state: &web::Data<TwigContext>,
    scope: &IssueScope,
    issue: &Issue,
    body: &str,
) -> HttpResponse {
    let actor = Actor {
        user_id: &scope.compat.principal.user.id,
    };
    match issues::add_comment(
        auth_state.db(),
        issue,
        &actor,
        &scope.compat.permissions,
        body,
    )
    .await
    {
        Ok(comment) => {
            let json = comment_json(
                auth_state.db(),
                &origin(req),
                &scope.owner,
                &scope.name,
                issue.number,
                &comment,
            )
            .await;
            HttpResponse::Created().json(json)
        }
        Err(error) => from_issue_error(error),
    }
}

/// The issue a comment row belongs to, within the scope's repository. A comment of
/// another repository is not found.
async fn comment_issue(
    db: &Database,
    scope: &IssueScope,
    row_id: i64,
) -> Result<Issue, IssueError> {
    let comment = db
        .get_issue_comment(row_id)
        .await
        .map_err(IssueError::Internal)?
        .ok_or(IssueError::NotFound)?;
    db.get_issue_by_id(&comment.issue_id)
        .await
        .map_err(IssueError::Internal)?
        .filter(|issue| issue.repo_id == scope.repo_id)
        .ok_or(IssueError::NotFound)
}

/// `PATCH /api/v1/repos/{owner}/{repo}/issues/comments/{id}` for an issue comment.
pub(super) async fn edit_comment(
    req: &HttpRequest,
    auth_state: &web::Data<TwigContext>,
    scope: &IssueScope,
    row_id: i64,
    body: &str,
) -> HttpResponse {
    let db = auth_state.db();
    let issue = match comment_issue(db, scope, row_id).await {
        Ok(issue) => issue,
        Err(error) => return from_issue_error(error),
    };
    let actor = Actor {
        user_id: &scope.compat.principal.user.id,
    };
    match issues::edit_comment(db, &issue, row_id, &actor, &scope.compat.permissions, body).await {
        Ok(comment) => {
            let json = comment_json(
                db,
                &origin(req),
                &scope.owner,
                &scope.name,
                issue.number,
                &comment,
            )
            .await;
            HttpResponse::Ok().json(json)
        }
        Err(error) => from_issue_error(error),
    }
}

/// `DELETE /api/v1/repos/{owner}/{repo}/issues/comments/{id}` for an issue comment.
pub(super) async fn delete_comment(
    auth_state: &web::Data<TwigContext>,
    scope: &IssueScope,
    row_id: i64,
) -> HttpResponse {
    let db = auth_state.db();
    let issue = match comment_issue(db, scope, row_id).await {
        Ok(issue) => issue,
        Err(error) => return from_issue_error(error),
    };
    let actor = Actor {
        user_id: &scope.compat.principal.user.id,
    };
    match issues::delete_comment(db, &issue, row_id, &actor, &scope.compat.permissions).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(error) => from_issue_error(error),
    }
}

#[derive(Debug, Deserialize)]
pub struct EditIssueBody {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    /// Usernames. When present, the list replaces the current assignees.
    #[serde(default)]
    pub assignees: Option<Vec<String>>,
}

/// `PATCH /api/v1/repos/{owner}/{repo}/issues/{index}`. Forgejo answers `201` here,
/// and `fj` expects it. Changes the state; other fields follow in a later slice.
#[patch("/api/v1/repos/{owner}/{repo}/issues/{index}")]
pub async fn edit_issue(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<super::comments::IssuePath>,
    body: web::Json<EditIssueBody>,
) -> HttpResponse {
    let scope = match issue_scope(&req, &server, &auth_state, &path.owner, &path.repo).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let issue = match issues::get(auth_state.db(), &scope.rules(), path.index).await {
        Ok(issue) => issue,
        Err(IssueError::NotFound) => {
            let pull_scope = RepoRef {
                repo_id: &scope.repo_id,
            };
            if pulls::get(auth_state.db(), &pull_scope, path.index)
                .await
                .is_ok()
            {
                return compat_error(
                    actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                    &format!("Use pulls/{} to change a pull request", path.index),
                );
            }
            return from_issue_error(IssueError::NotFound);
        }
        Err(error) => return from_issue_error(error),
    };
    let state = match body.state.as_deref() {
        None => None,
        Some(text) => match IssueState::parse(text) {
            Some(state) => Some(state),
            None => {
                return compat_error(
                    actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                    "Invalid state",
                );
            }
        },
    };
    let actor = Actor {
        user_id: &scope.compat.principal.user.id,
    };
    match issues::change(
        auth_state.db(),
        &scope.rules(),
        &issue,
        &actor,
        &scope.compat.permissions,
        IssueChange {
            title: body.title.clone(),
            body: body.body.clone(),
            state,
            assignees: body.assignees.clone(),
        },
    )
    .await
    {
        Ok(updated) => {
            let json = render_issue(
                auth_state.db(),
                &origin(&req),
                &path.owner,
                &path.repo,
                &updated,
            )
            .await;
            HttpResponse::Created().json(json)
        }
        Err(error) => from_issue_error(error),
    }
}

/// A conversation-comment request on a number that may name an issue.
pub(super) enum IssueCommentRequest<'a> {
    List,
    Create(&'a str),
}

/// Answers a conversation-comment request when `number` names an issue. `None` means it
/// names a pull request (or nothing), so the pull request handler answers instead.
pub(super) async fn comment_request_for_issue(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    owner: &str,
    repo: &str,
    number: i64,
    request: IssueCommentRequest<'_>,
) -> Option<HttpResponse> {
    let scope = match issue_scope(req, server, auth_state, owner, repo).await {
        Ok(scope) => scope,
        Err(response) => return Some(response),
    };
    let issue = match issues::get(auth_state.db(), &scope.rules(), number).await {
        Ok(issue) => issue,
        Err(IssueError::NotFound) => return None,
        Err(error) => return Some(from_issue_error(error)),
    };
    Some(match request {
        IssueCommentRequest::List => list_comments(req, auth_state, &scope, &issue).await,
        IssueCommentRequest::Create(body) => {
            create_comment(req, auth_state, &scope, &issue, body).await
        }
    })
}

/// `GET /api/v1/repos/{owner}/{repo}/issues/comments/{id}`. `fj issue edit comment` reads
/// a comment before it changes it. Pull request comment IDs are not served here.
#[get("/api/v1/repos/{owner}/{repo}/issues/comments/{id}")]
pub async fn get_comment(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<super::comments::CommentPath>,
) -> HttpResponse {
    let Some(row_id) = issue_comment_row_id(path.id) else {
        return from_issue_error(IssueError::NotFound);
    };
    let scope = match issue_scope(&req, &server, &auth_state, &path.owner, &path.repo).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let db = auth_state.db();
    let issue = match comment_issue(db, &scope, row_id).await {
        Ok(issue) => issue,
        Err(error) => return from_issue_error(error),
    };
    let comment = match db.get_issue_comment(row_id).await {
        Ok(Some(comment)) => comment,
        Ok(None) => return from_issue_error(IssueError::NotFound),
        Err(error) => return from_issue_error(IssueError::Internal(error)),
    };
    let json = comment_json(
        db,
        &origin(&req),
        &scope.owner,
        &scope.name,
        issue.number,
        &comment,
    )
    .await;
    HttpResponse::Ok().json(json)
}

/// Whether a list filter has a value. An empty parameter, which `fj` sends by default, does not.
fn has_filter(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

/// The empty list, for a filter that names an account no one has.
fn empty_list() -> HttpResponse {
    let mut response = HttpResponse::Ok();
    response.insert_header(("X-Total-Count", "0"));
    response.json(Vec::<IssueJson>::new())
}

/// Answers a Forgejo feature Twig does not have. The caller must still be signed in and
/// able to view the repository, so an anonymous request gets the usual 401.
async fn refuse(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    owner: &str,
    repo: &str,
    message: &str,
) -> HttpResponse {
    match authorized_repo(
        req,
        server,
        auth_state,
        owner,
        repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(_) => compat_error(actix_web::http::StatusCode::UNPROCESSABLE_ENTITY, message),
        Err(response) => response,
    }
}

/// `GET|POST|DELETE .../issues/{index}/dependencies`
pub async fn dependencies_refused(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<super::comments::IssuePath>,
) -> HttpResponse {
    refuse(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        "Issue dependencies are not supported on Twig",
    )
    .await
}

/// `GET|POST|DELETE .../issues/{index}/blocks`
pub async fn blocks_refused(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<super::comments::IssuePath>,
) -> HttpResponse {
    refuse(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        "Blocking is not supported on Twig",
    )
    .await
}

/// `POST|PUT|DELETE .../issues/{index}/labels` and `DELETE .../labels/{identifier}`
pub async fn labels_refused(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<super::comments::IssuePath>,
) -> HttpResponse {
    refuse(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        "Labels are not supported on Twig",
    )
    .await
}

/// `GET .../issues/{index}/labels`. Twig has no labels, so an issue has none.
pub async fn labels_list(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<super::comments::IssuePath>,
) -> HttpResponse {
    match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(_) => HttpResponse::Ok().json(Vec::<serde_json::Value>::new()),
        Err(response) => response,
    }
}

/// Reactions exist on pull requests only. A reaction on an issue, or on an issue comment,
/// is refused with the reason. `None` lets the pull request handler answer.
pub(super) async fn refuse_reactions(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    owner: &str,
    repo: &str,
    number: i64,
    comment_id: Option<i64>,
) -> Option<HttpResponse> {
    let scope = match issue_scope(req, server, auth_state, owner, repo).await {
        Ok(scope) => scope,
        Err(response) => return Some(response),
    };
    let on_issue = match comment_id {
        Some(id) => issue_comment_row_id(id).is_some(),
        None => match issues::get(auth_state.db(), &scope.rules(), number).await {
            Ok(_) => true,
            Err(IssueError::NotFound) => false,
            Err(error) => return Some(from_issue_error(error)),
        },
    };
    on_issue.then(|| {
        compat_error(
            actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            "Reactions on issues are not supported yet",
        )
    })
}
