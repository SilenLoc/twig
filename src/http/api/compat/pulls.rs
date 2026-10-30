use actix_web::{HttpRequest, HttpResponse, get, patch, post, web};
use serde::Deserialize;

use crate::{
    auth::{NamespacePermission, TwigContext},
    config,
    db::{
        Database,
        pull_requests::{PullRequest, PullRequestEdit, PullRequestFilter, PullRequestState},
    },
    git::bare::{MergeStrategy, RepoHandle},
    pulls::{self, Actor, CreateInput, RepoRef},
};

use super::{
    RepoPath, authorized_repo, compat_error, from_pull_error,
    json::{
        BranchRefJson, PermissionsJson, PullRequestJson, RepositoryLocation, UserJson,
        is_work_in_progress, numeric_id,
    },
    next_page_link, origin,
    repos::repository_json,
};

const DEFAULT_LIMIT: u32 = 30;
const MAX_LIMIT: u32 = 50;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub state: Option<String>,
    pub head: Option<String>,
    pub sort: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBody {
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub head: String,
    pub base: String,
}

#[derive(Debug, Deserialize)]
pub struct PullPath {
    pub owner: String,
    pub repo: String,
    pub index: i64,
}

/// `GET /api/v1/repos/{owner}/{repo}/pulls`
#[get("/api/v1/repos/{owner}/{repo}/pulls")]
pub async fn list_pulls(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
    query: web::Query<ListQuery>,
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
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let page = query.page.unwrap_or(1).max(1);
    let repo_id = match repo.handle.repo_id() {
        Ok(id) => id,
        Err(error) => return internal(&error.to_string()),
    };
    let scope = RepoRef { repo_id: &repo_id };
    let filter = PullRequestFilter {
        state,
        head_branch: query.head.as_deref().map(branch_from_head_filter),
        query: None,
        recently_updated: query.sort.as_deref() == Some("recentupdate"),
        limit,
        offset: (page - 1).saturating_mul(limit),
    };
    let (rows, has_more) = match pulls::list(auth_state.db(), &scope, &filter).await {
        Ok(result) => result,
        Err(error) => return from_pull_error(error),
    };

    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        match render(
            auth_state.db(),
            &repo.handle,
            &origin(&req),
            &path.owner,
            &path.repo,
            row,
        )
        .await
        {
            Ok(item) => items.push(item),
            Err(response) => return response,
        }
    }
    let total = match auth_state
        .db()
        .count_pull_requests(&repo_id, &filter_for_count(state, &query))
        .await
    {
        Ok(total) => total,
        Err(error) => return from_pull_error(crate::pulls::PullError::Internal(error)),
    };
    let mut response = HttpResponse::Ok();
    response.insert_header(("X-Total-Count", total.to_string()));
    if has_more {
        response.insert_header(("Link", next_page_link(&req, page + 1)));
    }
    response.json(items)
}

/// The filter a list's total is counted with: the same state and head, without paging.
fn filter_for_count(state: Option<PullRequestState>, query: &ListQuery) -> PullRequestFilter {
    PullRequestFilter {
        state,
        head_branch: query.head.as_deref().map(branch_from_head_filter),
        query: None,
        recently_updated: false,
        limit: 0,
        offset: 0,
    }
}

/// Accepts `branch` or `owner:branch`; only the branch name is used.
fn branch_from_head_filter(head: &str) -> String {
    head.rsplit_once(':')
        .map_or(head, |(_, branch)| branch)
        .to_string()
}

/// `POST /api/v1/repos/{owner}/{repo}/pulls`
#[post("/api/v1/repos/{owner}/{repo}/pulls")]
pub async fn create_pull(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
    body: web::Json<CreateBody>,
) -> HttpResponse {
    let repo = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    let (owner_part, head) = match body.head.split_once(':') {
        Some((owner, branch)) => (Some(owner), branch),
        None => (None, body.head.as_str()),
    };
    if owner_part.is_some_and(|owner| owner != path.owner) {
        return compat_error(
            actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            "Pull requests must come from a branch in the same repository",
        );
    }
    let repo_id = match repo.handle.repo_id() {
        Ok(id) => id,
        Err(error) => return internal(&error.to_string()),
    };
    let scope = RepoRef { repo_id: &repo_id };
    let actor = Actor {
        user_id: &repo.principal.user.id,
    };
    let created = pulls::create(
        auth_state.db(),
        &repo.handle,
        &scope,
        &actor,
        &repo.permissions,
        CreateInput {
            base: &body.base,
            head,
            title: &body.title,
            body: &body.body,
        },
    )
    .await;
    let pull_request = match created {
        Ok(pull_request) => pull_request,
        Err(error) => return from_pull_error(error),
    };
    match render(
        auth_state.db(),
        &repo.handle,
        &origin(&req),
        &path.owner,
        &path.repo,
        &pull_request,
    )
    .await
    {
        Ok(item) => HttpResponse::Created().json(item),
        Err(response) => response,
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/pulls/{index}`
#[get("/api/v1/repos/{owner}/{repo}/pulls/{index}")]
pub async fn get_pull(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
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
        Err(error) => return internal(&error.to_string()),
    };
    let scope = RepoRef { repo_id: &repo_id };
    match pulls::get(auth_state.db(), &scope, path.index).await {
        Ok(pull_request) => {
            match render(
                auth_state.db(),
                &repo.handle,
                &origin(&req),
                &path.owner,
                &path.repo,
                &pull_request,
            )
            .await
            {
                Ok(mut item) => {
                    // Change size and mergeability are only computed for one request, not for every row of a list.
                    let (additions, deletions, files) = change_size(&repo.handle, &pull_request);
                    item.additions = Some(additions);
                    item.deletions = Some(deletions);
                    item.changed_files = Some(files);
                    item.mergeable = pulls::mergeable(&repo.handle, &pull_request);
                    HttpResponse::Ok().json(item)
                }
                Err(response) => response,
            }
        }
        Err(error) => from_pull_error(error),
    }
}

fn internal(detail: &str) -> HttpResponse {
    log::error!("Compatibility pull request request failed: {detail}");
    compat_error(
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
        "The request could not be completed",
    )
}

/// Builds the Forgejo representation of one request from its row and the
/// repository's current branch tips.
async fn render(
    db: &Database,
    handle: &RepoHandle,
    origin: &str,
    owner: &str,
    repo: &str,
    pull_request: &PullRequest,
) -> Result<PullRequestJson, HttpResponse> {
    let author = db
        .get_user_by_id(&pull_request.author_user_id)
        .await
        .map_err(|error| internal(&error))?
        .map(|user| UserJson::public(&user));
    let base_tip = handle
        .branch_tip(&pull_request.base_branch)
        .map_err(|error| internal(&error.to_string()))?;
    let head_tip = handle
        .branch_tip(&pull_request.head_branch)
        .map_err(|error| internal(&error.to_string()))?;
    let merge_base = match (base_tip, head_tip) {
        (Some(base), Some(head)) => handle
            .merge_base(base, head)
            .map_err(|error| internal(&error.to_string()))?,
        _ => None,
    };
    let repo_id = pull_request.repo_id.as_str();
    let location = RepositoryLocation {
        origin,
        namespace: owner,
        name: repo,
        repo_id,
    };
    let repository = repository_json(
        &location,
        handle,
        PermissionsJson {
            admin: false,
            push: false,
            pull: true,
        },
    )
    .map_err(|error| internal(&error.to_string()))?;
    let branch = |name: &str, tip: Option<git2::Oid>| BranchRefJson {
        label: name.to_string(),
        reference: name.to_string(),
        sha: tip.map(|oid| oid.to_string()).unwrap_or_default(),
        repo_id: numeric_id(repo_id),
        repo: Some(repository.clone()),
    };
    let merged_by = match &pull_request.merged_by_user_id {
        Some(user_id) => db
            .get_user_by_id(user_id)
            .await
            .map_err(|error| internal(&error))?
            .map(|user| UserJson::public(&user)),
        None => None,
    };
    let comment_count = db
        .list_pull_request_comments(&pull_request.id)
        .await
        .map_err(|error| internal(&error))?
        .len();
    let comment_count = i64::try_from(comment_count).unwrap_or_default();
    let requested_reviewers = super::reviews::requested_reviewers(db, pull_request)
        .await
        .map_err(|error| internal(&error))?;
    let number = pull_request.number;
    let html_url = format!("{origin}/{owner}/{repo}/pulls/{number}");
    let api_url = format!("{origin}/api/v1/repos/{owner}/{repo}/pulls/{number}");
    let merged = pull_request.merged_at.is_some();
    Ok(PullRequestJson {
        id: number,
        number,
        title: pull_request.title.clone(),
        body: pull_request.body.clone(),
        html_url,
        url: api_url.clone(),
        diff_url: format!("{api_url}.diff"),
        patch_url: format!("{api_url}.patch"),
        user: author,
        state: if pull_request.state == PullRequestState::Open {
            "open"
        } else {
            "closed"
        },
        is_locked: false,
        draft: is_work_in_progress(&pull_request.title),
        merged,
        mergeable: None,
        head: branch(&pull_request.head_branch, head_tip),
        base: branch(&pull_request.base_branch, base_tip),
        merge_base: merge_base.map(|oid| oid.to_string()),
        merge_commit_sha: pull_request.merge_commit_oid.clone(),
        merged_by,
        created_at: pull_request.created_at.clone(),
        updated_at: pull_request.updated_at.clone(),
        closed_at: pull_request.closed_at.clone(),
        merged_at: pull_request.merged_at.clone(),
        additions: None,
        deletions: None,
        changed_files: None,
        comments: comment_count,
        labels: Vec::new(),
        requested_reviewers,
        assignees: Vec::new(),
        allow_maintainer_edit: false,
    })
}

#[derive(Debug, Deserialize)]
pub struct UpdatePullBody {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<String>,
}

/// `PATCH /api/v1/repos/{owner}/{repo}/pulls/{index}`: title, description, or state.
#[patch("/api/v1/repos/{owner}/{repo}/pulls/{index}")]
pub async fn update_pull(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    body: web::Json<UpdatePullBody>,
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
        Err(error) => return internal(&error.to_string()),
    };
    let scope = RepoRef { repo_id: &repo_id };
    let current = match pulls::get(auth_state.db(), &scope, path.index).await {
        Ok(pull_request) => pull_request,
        Err(error) => return from_pull_error(error),
    };
    let state = match body.state.as_deref() {
        None => None,
        Some("open") => Some(PullRequestState::Open),
        Some("closed") => Some(PullRequestState::Closed),
        Some(_) => {
            return compat_error(
                actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                "Invalid state",
            );
        }
    };
    let actor = Actor {
        user_id: &repo.principal.user.id,
    };
    let change = PullRequestEdit {
        title: body.title.clone(),
        body: body.body.clone(),
        state,
    };
    let updated =
        match pulls::edit(auth_state.db(), &current, &actor, &repo.permissions, change).await {
            Ok(pull_request) => pull_request,
            Err(error) => return from_pull_error(error),
        };
    match render(
        auth_state.db(),
        &repo.handle,
        &origin(&req),
        &path.owner,
        &path.repo,
        &updated,
    )
    .await
    {
        Ok(item) => HttpResponse::Ok().json(item),
        Err(response) => response,
    }
}

/// Lines added, lines removed, and files changed by a request, from its merge base.
pub fn change_size(handle: &RepoHandle, pull_request: &PullRequest) -> (i64, i64, i64) {
    let range = (|| {
        let base = handle
            .branch_tip(&pull_request.base_branch)
            .ok()
            .flatten()?;
        let head = handle
            .branch_tip(&pull_request.head_branch)
            .ok()
            .flatten()?;
        let from = handle.merge_base(base, head).ok().flatten().unwrap_or(base);
        handle.diff_files(from, head).ok()
    })();
    let files = range.unwrap_or_default();
    let additions: usize = files.iter().map(|file| file.additions).sum();
    let deletions: usize = files.iter().map(|file| file.deletions).sum();
    (
        i64::try_from(additions).unwrap_or_default(),
        i64::try_from(deletions).unwrap_or_default(),
        i64::try_from(files.len()).unwrap_or_default(),
    )
}

#[derive(Debug, Deserialize)]
pub struct MergeBody {
    /// Forgejo's name for the merge style: `merge`, `squash`, or `rebase`.
    #[serde(rename = "Do", default = "default_merge_style")]
    pub style: String,
    #[serde(rename = "MergeTitleField", default)]
    pub title: Option<String>,
    #[serde(rename = "MergeMessageField", default)]
    pub message: Option<String>,
}

fn default_merge_style() -> String {
    "merge".to_string()
}

/// `POST /api/v1/repos/{owner}/{repo}/pulls/{index}/merge`
#[post("/api/v1/repos/{owner}/{repo}/pulls/{index}/merge")]
pub async fn merge_pull(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    body: web::Json<MergeBody>,
) -> HttpResponse {
    let repo = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    let strategy = match body.style.as_str() {
        "merge" => MergeStrategy::Merge,
        "squash" => MergeStrategy::Squash,
        "rebase" => MergeStrategy::Rebase,
        _ => {
            return compat_error(
                actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                "Unsupported merge style",
            );
        }
    };
    let repo_id = match repo.handle.repo_id() {
        Ok(id) => id,
        Err(error) => return internal(&error.to_string()),
    };
    let scope = RepoRef { repo_id: &repo_id };
    let pull_request = match pulls::get(auth_state.db(), &scope, path.index).await {
        Ok(pull_request) => pull_request,
        Err(error) => return from_pull_error(error),
    };
    let actor = Actor {
        user_id: &repo.principal.user.id,
    };
    let email = repo
        .principal
        .user
        .email
        .clone()
        .unwrap_or_else(|| format!("{}@localhost", repo.principal.user.username));
    let merger = pulls::Merger {
        name: &repo.principal.user.username,
        email: &email,
    };
    match pulls::merge(
        auth_state.db(),
        &repo.handle,
        &pull_request,
        &actor,
        &repo.permissions,
        strategy,
        body.title.as_deref(),
        body.message.as_deref(),
        &merger,
    )
    .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(error) => from_pull_error(error),
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/pulls/{index}/merge`: 204 once merged, 404 before.
#[get("/api/v1/repos/{owner}/{repo}/pulls/{index}/merge")]
pub async fn merge_status(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
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
        Err(error) => return internal(&error.to_string()),
    };
    match pulls::get(auth_state.db(), &RepoRef { repo_id: &repo_id }, path.index).await {
        Ok(pull_request) if pull_request.merged_at.is_some() => HttpResponse::NoContent().finish(),
        Ok(_) => from_pull_error(crate::pulls::PullError::NotFound),
        Err(error) => from_pull_error(error),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateQuery {
    pub style: Option<String>,
}

/// `POST /api/v1/repos/{owner}/{repo}/pulls/{index}/update`: merges the base into the head.
#[post("/api/v1/repos/{owner}/{repo}/pulls/{index}/update")]
pub async fn update_branch_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    query: web::Query<UpdateQuery>,
) -> HttpResponse {
    if query.style.as_deref().is_some_and(|style| style != "merge") {
        return compat_error(
            actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            "Only merge updates are supported",
        );
    }
    let repo = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    let repo_id = match repo.handle.repo_id() {
        Ok(id) => id,
        Err(error) => return internal(&error.to_string()),
    };
    let pull_request =
        match pulls::get(auth_state.db(), &RepoRef { repo_id: &repo_id }, path.index).await {
            Ok(pull_request) => pull_request,
            Err(error) => return from_pull_error(error),
        };
    let email = repo
        .principal
        .user
        .email
        .clone()
        .unwrap_or_else(|| format!("{}@localhost", repo.principal.user.username));
    let merger = pulls::Merger {
        name: &repo.principal.user.username,
        email: &email,
    };
    match pulls::update_branch(&repo.handle, &pull_request, &repo.permissions, &merger) {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(error) => from_pull_error(error),
    }
}

/// The request representation used by list views, without the per-request extras
/// (change size and mergeability) that single reads compute.
pub async fn render_for_list(
    db: &Database,
    handle: &RepoHandle,
    origin: &str,
    owner: &str,
    repo: &str,
    pull_request: &PullRequest,
) -> Result<PullRequestJson, HttpResponse> {
    render(db, handle, origin, owner, repo, pull_request).await
}
