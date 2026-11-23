//! GitHub-compatible pull request routes under `/api/v3/repos/{owner}/{repo}/pulls`:
//! list, create, read (including the diff), update, merge, files, commits, reviews,
//! and requested reviewers. Rules live in `crate::pulls`; this module shapes the
//! requests and responses.

use actix_web::{HttpRequest, HttpResponse, delete, get, http::StatusCode, patch, post, put, web};
use chrono::DateTime;
use serde::{Deserialize, Serialize};

use super::{IssuePath, Repo, RepoPath, UserJson, error, next_link, open_repo, paging, user_json};
use crate::{
    auth::NamespacePermission,
    auth::TwigContext,
    config,
    db::{
        Database,
        pull_requests::{
            PullRequest, PullRequestEdit, PullRequestFilter, PullRequestState, reviews::ReviewState,
        },
    },
    git::bare::{FileStatus, MergeStrategy, RepoHandle},
    http::api::compat::{json::is_work_in_progress, origin, pulls::change_size},
    pulls::{self, Actor, CreateInput, InlineInput, Merger, PullError, RepoRef},
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_pulls)
        .service(create_pull)
        .service(get_pull)
        .service(edit_pull)
        .service(get_merge_status)
        .service(merge_pull)
        .service(list_files)
        .service(list_commits)
        .service(list_reviews)
        .service(submit_review)
        .service(list_requested_reviewers)
        .service(request_reviewers)
        .service(remove_requested_reviewers);
}

/// Maps a request rule failure. `conflict` is the status for a `Conflict`, which
/// differs by route: `422` when opening, `409` when merging.
fn pull_error(failure: PullError, conflict: StatusCode) -> HttpResponse {
    match failure {
        PullError::NotFound => error(StatusCode::NOT_FOUND, "Not Found"),
        PullError::Forbidden => error(
            StatusCode::FORBIDDEN,
            "Must have push access to pull request",
        ),
        PullError::Invalid(message) => error(StatusCode::UNPROCESSABLE_ENTITY, &message),
        PullError::Conflict(message) => error(conflict, &message),
        PullError::Internal(detail) => {
            log::error!("Pull request operation failed: {detail}");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            )
        }
    }
}

fn internal(detail: impl std::fmt::Display) -> HttpResponse {
    log::error!("Pull request request failed: {detail}");
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "The request could not be completed",
    )
}

/// The repository object GitHub nests under a branch reference.
#[derive(Debug, Clone, Serialize)]
pub struct RepoRefJson {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub owner: UserJson,
    pub private: bool,
    pub fork: bool,
    pub html_url: String,
    pub url: String,
    pub clone_url: String,
}

/// One side of a pull request: the branch and the commit it names.
#[derive(Debug, Clone, Serialize)]
pub struct BranchRefJson {
    pub label: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub sha: String,
    pub user: Option<UserJson>,
    pub repo: Option<RepoRefJson>,
}

/// A pull request in GitHub's shape. Detail reads add the change size and commit count.
#[derive(Debug, Clone, Serialize)]
pub struct PullJson {
    pub id: i64,
    pub node_id: String,
    pub number: i64,
    pub state: &'static str,
    pub locked: bool,
    pub title: String,
    pub body: Option<String>,
    pub user: Option<UserJson>,
    pub draft: bool,
    pub merged: bool,
    pub mergeable: Option<bool>,
    pub mergeable_state: &'static str,
    pub merged_by: Option<UserJson>,
    pub merge_commit_sha: Option<String>,
    pub comments: i64,
    pub review_comments: i64,
    pub commits: Option<i64>,
    pub additions: Option<i64>,
    pub deletions: Option<i64>,
    pub changed_files: Option<i64>,
    pub labels: Vec<serde_json::Value>,
    pub assignees: Vec<UserJson>,
    pub requested_reviewers: Vec<UserJson>,
    pub requested_teams: Vec<serde_json::Value>,
    pub head: BranchRefJson,
    pub base: BranchRefJson,
    pub html_url: String,
    pub url: String,
    pub diff_url: String,
    pub patch_url: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub merged_at: Option<String>,
}

fn repo_ref_json(origin: &str, owner: &str, repo: &str, repo_id: &str) -> RepoRefJson {
    let full_name = format!("{owner}/{repo}");
    RepoRefJson {
        id: crate::http::api::compat::json::numeric_id(repo_id),
        name: repo.to_string(),
        full_name: full_name.clone(),
        owner: user_json(origin, owner, &format!("namespace:{owner}")),
        private: false,
        fork: false,
        html_url: format!("{origin}/{full_name}"),
        url: format!("{origin}/api/v3/repos/{full_name}"),
        clone_url: format!("{origin}/{full_name}.git"),
    }
}

/// Builds a pull request in GitHub's shape from its row and the current branch tips.
/// `detail` adds the reads that need the Git history: change size and commit count.
async fn render(
    db: &Database,
    handle: &RepoHandle,
    origin: &str,
    owner: &str,
    repo: &str,
    pull: &PullRequest,
    detail: bool,
) -> Result<PullJson, String> {
    let author = match db.get_user_by_id(&pull.author_user_id).await? {
        Some(user) => Some(user_json(origin, &user.username, &user.id)),
        None => None,
    };
    let merged_by = match &pull.merged_by_user_id {
        Some(user_id) => db
            .get_user_by_id(user_id)
            .await?
            .map(|user| user_json(origin, &user.username, &user.id)),
        None => None,
    };
    let base_tip = handle
        .branch_tip(&pull.base_branch)
        .map_err(|error| error.to_string())?;
    let head_tip = handle
        .branch_tip(&pull.head_branch)
        .map_err(|error| error.to_string())?;
    let comments = db.list_pull_request_comments(&pull.id).await?.len();
    let mut requested_reviewers = Vec::new();
    for username in db.list_requested_reviewers(&pull.id).await? {
        if let Some(user) = db.get_user_by_username(&username).await? {
            requested_reviewers.push(user_json(origin, &user.username, &user.id));
        }
    }
    let mergeable = pulls::mergeable(handle, pull);
    let open = pull.state == PullRequestState::Open;
    let (commits, additions, deletions, changed_files) = if detail {
        let commits = match (base_tip, head_tip) {
            (Some(base), Some(head)) => handle
                .commit_ids_in_range(base, head)
                .map(|ids| ids.len() as i64)
                .map_err(|error| error.to_string())?,
            _ => 0,
        };
        let (added, removed, files) = change_size(handle, pull);
        (Some(commits), Some(added), Some(removed), Some(files))
    } else {
        (None, None, None, None)
    };
    let api = format!("{origin}/api/v3/repos/{owner}/{repo}/pulls/{}", pull.number);
    let repo_id = pull.repo_id.as_str();
    let branch = |name: &str, tip: Option<git2::Oid>, user: Option<UserJson>| BranchRefJson {
        label: format!("{owner}:{name}"),
        reference: name.to_string(),
        sha: tip.map(|oid| oid.to_string()).unwrap_or_default(),
        user,
        repo: Some(repo_ref_json(origin, owner, repo, repo_id)),
    };
    Ok(PullJson {
        id: crate::http::api::compat::json::numeric_id(&pull.id),
        node_id: format!("PR_{}", pull.id),
        number: pull.number,
        state: if open { "open" } else { "closed" },
        locked: false,
        title: pull.title.clone(),
        body: Some(pull.body.clone()),
        user: author.clone(),
        draft: is_work_in_progress(&pull.title),
        merged: pull.merged_at.is_some(),
        mergeable,
        mergeable_state: match mergeable {
            Some(true) => "clean",
            Some(false) => "dirty",
            None => "unknown",
        },
        merged_by,
        merge_commit_sha: pull.merge_commit_oid.clone(),
        comments: comments as i64,
        review_comments: 0,
        commits,
        additions,
        deletions,
        changed_files,
        labels: Vec::new(),
        assignees: Vec::new(),
        requested_reviewers,
        requested_teams: Vec::new(),
        head: branch(&pull.head_branch, head_tip, author.clone()),
        base: branch(&pull.base_branch, base_tip, author),
        html_url: format!("{origin}/{owner}/{repo}/pulls/{}", pull.number),
        url: api.clone(),
        diff_url: format!("{origin}/{owner}/{repo}/pulls/{}.diff", pull.number),
        patch_url: format!("{origin}/{owner}/{repo}/pulls/{}.patch", pull.number),
        created_at: pull.created_at.clone(),
        updated_at: pull.updated_at.clone(),
        closed_at: pull.closed_at.clone(),
        merged_at: pull.merged_at.clone(),
    })
}

/// Shared setup for pull request routes: the repository, its ID, and the request's
/// pull request row. `permission` is what the route needs in the namespace.
async fn load_pull(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    owner: &str,
    repo: &str,
    number: i64,
    permission: NamespacePermission,
) -> Result<(Repo, PullRequest), HttpResponse> {
    let repo_access = open_repo(req, server, auth_state, owner, repo, permission).await?;
    let scope = RepoRef {
        repo_id: &repo_access.repo_id,
    };
    let pull = pulls::get(auth_state.db(), &scope, number)
        .await
        .map_err(|failure| pull_error(failure, StatusCode::CONFLICT))?;
    Ok((repo_access, pull))
}

fn actor_of(repo: &Repo) -> Actor<'_> {
    Actor {
        user_id: &repo.principal.user.id,
    }
}

#[derive(Debug, Deserialize)]
pub struct ListPullsQuery {
    pub state: Option<String>,
    pub head: Option<String>,
    pub base: Option<String>,
    pub sort: Option<String>,
    pub direction: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// `GET /api/v3/repos/{owner}/{repo}/pulls`
#[get("/api/v3/repos/{owner}/{repo}/pulls")]
pub async fn list_pulls(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
    query: web::Query<ListPullsQuery>,
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
        "open" => Some(PullRequestState::Open),
        "closed" => Some(PullRequestState::Closed),
        "all" => None,
        _ => return error(StatusCode::UNPROCESSABLE_ENTITY, "Invalid state"),
    };
    if query.base.is_some() {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Filtering pull requests by base is not supported on Twig",
        );
    }
    let direction = query.direction.as_deref().unwrap_or("desc");
    if direction != "desc" {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Only descending order is supported on Twig",
        );
    }
    let head_branch = query
        .head
        .as_deref()
        .map(|head| head.rsplit(':').next().unwrap_or(head).to_string());
    let (page, per_page) = paging(query.page, query.per_page);
    let filter = PullRequestFilter {
        state,
        head_branch,
        query: None,
        recently_updated: query.sort.as_deref() == Some("updated"),
        limit: per_page,
        offset: (page - 1).saturating_mul(per_page),
    };
    let db = auth_state.db();
    let scope = RepoRef {
        repo_id: &repo.repo_id,
    };
    let (rows, has_more) = match pulls::list(db, &scope, &filter).await {
        Ok(found) => found,
        Err(failure) => return pull_error(failure, StatusCode::CONFLICT),
    };
    let origin = origin(&req);
    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        match render(
            db,
            &repo.handle,
            &origin,
            &path.owner,
            &path.repo,
            row,
            false,
        )
        .await
        {
            Ok(item) => items.push(item),
            Err(detail) => return internal(detail),
        }
    }
    let mut response = HttpResponse::Ok();
    if has_more {
        response.insert_header(("Link", next_link(&req, page + 1)));
    }
    response.json(items)
}

#[derive(Debug, Deserialize)]
pub struct CreatePullBody {
    pub title: String,
    pub body: Option<String>,
    pub head: String,
    pub base: String,
    pub draft: Option<bool>,
}

/// `POST /api/v3/repos/{owner}/{repo}/pulls`. Answers `201`. A draft is recorded as a
/// `WIP:` title, the form Twig reads as a draft.
#[post("/api/v3/repos/{owner}/{repo}/pulls")]
pub async fn create_pull(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
    body: web::Json<CreatePullBody>,
) -> HttpResponse {
    let repo = match open_repo(
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
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Pull requests must come from a branch in the same repository",
        );
    }
    let title = if body.draft == Some(true) && !is_work_in_progress(&body.title) {
        format!("WIP: {}", body.title)
    } else {
        body.title.clone()
    };
    let db = auth_state.db();
    let scope = RepoRef {
        repo_id: &repo.repo_id,
    };
    let actor = actor_of(&repo);
    let created = pulls::create(
        db,
        &repo.handle,
        &scope,
        &actor,
        &repo.permissions,
        CreateInput {
            base: &body.base,
            head,
            title: &title,
            body: body.body.as_deref().unwrap_or(""),
        },
    )
    .await;
    match created {
        Ok(pull) => match render(
            db,
            &repo.handle,
            &origin(&req),
            &path.owner,
            &path.repo,
            &pull,
            true,
        )
        .await
        {
            Ok(item) => HttpResponse::Created().json(item),
            Err(detail) => internal(detail),
        },
        Err(failure) => pull_error(failure, StatusCode::UNPROCESSABLE_ENTITY),
    }
}

/// `GET /api/v3/repos/{owner}/{repo}/pulls/{number}`. With
/// `Accept: application/vnd.github.diff` it answers the diff as plain text.
#[get("/api/v3/repos/{owner}/{repo}/pulls/{number}")]
pub async fn get_pull(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
) -> HttpResponse {
    let (repo, pull) = match load_pull(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.number,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let wants_diff = req
        .headers()
        .get("accept")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|accept| accept.contains(".diff") || accept.contains(".patch"));
    if wants_diff {
        let tips = repo
            .handle
            .branch_tip(&pull.base_branch)
            .ok()
            .flatten()
            .zip(repo.handle.branch_tip(&pull.head_branch).ok().flatten());
        let Some((base, head)) = tips else {
            return error(StatusCode::NOT_FOUND, "Not Found");
        };
        let from = repo
            .handle
            .merge_base(base, head)
            .ok()
            .flatten()
            .unwrap_or(base);
        return match repo.handle.diff_patch(from, head) {
            Ok(text) => HttpResponse::Ok()
                .content_type("text/plain; charset=utf-8")
                .body(text),
            Err(detail) => internal(detail),
        };
    }
    match render(
        auth_state.db(),
        &repo.handle,
        &origin(&req),
        &path.owner,
        &path.repo,
        &pull,
        true,
    )
    .await
    {
        Ok(item) => HttpResponse::Ok().json(item),
        Err(detail) => internal(detail),
    }
}

#[derive(Debug, Deserialize)]
pub struct EditPullBody {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<String>,
}

/// `PATCH /api/v3/repos/{owner}/{repo}/pulls/{number}`: title, body, or state.
#[patch("/api/v3/repos/{owner}/{repo}/pulls/{number}")]
pub async fn edit_pull(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    body: web::Json<EditPullBody>,
) -> HttpResponse {
    let (repo, pull) = match load_pull(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.number,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let state = match body.state.as_deref() {
        None => None,
        Some("open") => Some(PullRequestState::Open),
        Some("closed") => Some(PullRequestState::Closed),
        Some(_) => return error(StatusCode::UNPROCESSABLE_ENTITY, "Invalid state"),
    };
    let db = auth_state.db();
    let actor = actor_of(&repo);
    let change = PullRequestEdit {
        title: body.title.clone(),
        body: body.body.clone(),
        state,
    };
    let updated = match pulls::edit(db, &pull, &actor, &repo.permissions, change).await {
        Ok(updated) => updated,
        Err(failure) => return pull_error(failure, StatusCode::CONFLICT),
    };
    match render(
        db,
        &repo.handle,
        &origin(&req),
        &path.owner,
        &path.repo,
        &updated,
        true,
    )
    .await
    {
        Ok(item) => HttpResponse::Ok().json(item),
        Err(detail) => internal(detail),
    }
}

/// `GET /api/v3/repos/{owner}/{repo}/pulls/{number}/merge`: `204` once merged, `404` before.
#[get("/api/v3/repos/{owner}/{repo}/pulls/{number}/merge")]
pub async fn get_merge_status(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
) -> HttpResponse {
    match load_pull(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.number,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok((_, pull)) if pull.merged_at.is_some() => HttpResponse::NoContent().finish(),
        Ok(_) => error(StatusCode::NOT_FOUND, "Not Found"),
        Err(response) => response,
    }
}

#[derive(Debug, Deserialize)]
pub struct MergeBody {
    pub commit_title: Option<String>,
    pub commit_message: Option<String>,
    pub merge_method: Option<String>,
    /// When given, the merge only proceeds if the head branch still names this commit.
    pub sha: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MergeResultJson {
    pub sha: String,
    pub merged: bool,
    pub message: &'static str,
}

/// `PUT /api/v3/repos/{owner}/{repo}/pulls/{number}/merge`. Merges with the chosen method
/// (`merge`, `squash`, or `rebase`) and answers `200` with the merge commit.
#[put("/api/v3/repos/{owner}/{repo}/pulls/{number}/merge")]
pub async fn merge_pull(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    body: web::Json<MergeBody>,
) -> HttpResponse {
    let (repo, pull) = match load_pull(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.number,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let strategy = match body.merge_method.as_deref().unwrap_or("merge") {
        "merge" => MergeStrategy::Merge,
        "squash" => MergeStrategy::Squash,
        "rebase" => MergeStrategy::Rebase,
        _ => {
            return error(StatusCode::UNPROCESSABLE_ENTITY, "Unsupported merge_method");
        }
    };
    if let Some(expected) = body.sha.as_deref() {
        let head = repo
            .handle
            .branch_tip(&pull.head_branch)
            .ok()
            .flatten()
            .map(|oid| oid.to_string());
        if head.as_deref() != Some(expected) {
            return error(
                StatusCode::CONFLICT,
                "Head branch was modified. Review and try the merge again.",
            );
        }
    }
    let email = repo
        .principal
        .user
        .email
        .clone()
        .unwrap_or_else(|| format!("{}@localhost", repo.principal.user.username));
    let merger = Merger {
        name: &repo.principal.user.username,
        email: &email,
    };
    let actor = actor_of(&repo);
    match pulls::merge(
        auth_state.db(),
        &repo.handle,
        &pull,
        &actor,
        &repo.permissions,
        strategy,
        body.commit_title.as_deref(),
        body.commit_message.as_deref(),
        &merger,
    )
    .await
    {
        Ok(sha) => HttpResponse::Ok().json(MergeResultJson {
            sha,
            merged: true,
            message: "Pull Request successfully merged",
        }),
        Err(failure) => pull_error(failure, StatusCode::CONFLICT),
    }
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct FileJson {
    pub sha: String,
    pub filename: String,
    pub status: &'static str,
    pub additions: i64,
    pub deletions: i64,
    pub changes: i64,
    pub previous_filename: Option<String>,
}

/// `GET /api/v3/repos/{owner}/{repo}/pulls/{number}/files`: files changed since the merge base.
#[get("/api/v3/repos/{owner}/{repo}/pulls/{number}/files")]
pub async fn list_files(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    query: web::Query<PageQuery>,
) -> HttpResponse {
    let (repo, pull) = match load_pull(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.number,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Some((from, to)) = pulls::change_range(&repo.handle, &pull) else {
        return error(StatusCode::NOT_FOUND, "Not Found");
    };
    let files = match repo.handle.diff_files(from, to) {
        Ok(files) => files,
        Err(detail) => return internal(detail),
    };
    let rows: Vec<FileJson> = files
        .iter()
        .map(|file| {
            let status = match file.status {
                FileStatus::Added => "added",
                FileStatus::Deleted => "removed",
                FileStatus::Renamed => "renamed",
                FileStatus::Modified => "modified",
            };
            let additions = i64::try_from(file.additions).unwrap_or_default();
            let deletions = i64::try_from(file.deletions).unwrap_or_default();
            FileJson {
                sha: String::new(),
                filename: file.path().to_string(),
                status,
                additions,
                deletions,
                changes: additions + deletions,
                previous_filename: match file.status {
                    FileStatus::Renamed => file.old_path.clone(),
                    _ => None,
                },
            }
        })
        .collect();
    paginate_rows(&req, rows, &query)
}

/// Pages an in-memory list with `page` and `per_page`, adding `Link` when more follow.
fn paginate_rows<T: Serialize>(req: &HttpRequest, rows: Vec<T>, query: &PageQuery) -> HttpResponse {
    let (page, per_page) = paging(query.page, query.per_page);
    let total = rows.len();
    let start = ((page - 1) as usize).saturating_mul(per_page as usize);
    let mut response = HttpResponse::Ok();
    if start + (per_page as usize) < total {
        response.insert_header(("Link", next_link(req, page + 1)));
    }
    let slice: Vec<T> = rows
        .into_iter()
        .skip(start)
        .take(per_page as usize)
        .collect();
    response.json(slice)
}

#[derive(Debug, Serialize)]
pub struct CommitPersonJson {
    pub name: String,
    pub email: String,
    pub date: String,
}

#[derive(Debug, Serialize)]
pub struct CommitDetailJson {
    pub message: String,
    pub author: CommitPersonJson,
    pub committer: CommitPersonJson,
}

#[derive(Debug, Serialize)]
pub struct CommitJson {
    pub sha: String,
    pub commit: CommitDetailJson,
    pub html_url: String,
    pub url: String,
    pub parents: Vec<serde_json::Value>,
}

/// `GET /api/v3/repos/{owner}/{repo}/pulls/{number}/commits`: commits in the request,
/// oldest first.
#[get("/api/v3/repos/{owner}/{repo}/pulls/{number}/commits")]
pub async fn list_commits(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    query: web::Query<PageQuery>,
) -> HttpResponse {
    let (repo, pull) = match load_pull(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.number,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let ids = pulls::change_commits(&repo.handle, &pull);
    let origin = origin(&req);
    let mut rows = Vec::with_capacity(ids.len());
    for oid in ids.iter().rev() {
        let summary = match repo.handle.commit_summary(*oid) {
            Ok(summary) => summary,
            Err(detail) => return internal(detail),
        };
        let sha = oid.to_string();
        let date = DateTime::from_timestamp(summary.committed_at, 0)
            .map(|time| time.to_rfc3339())
            .unwrap_or_default();
        let person = |name: &str, email: &str| CommitPersonJson {
            name: name.to_string(),
            email: email.to_string(),
            date: date.clone(),
        };
        rows.push(CommitJson {
            html_url: format!("{origin}/{}/{}/commit/{sha}", path.owner, path.repo),
            url: format!(
                "{origin}/api/v3/repos/{}/{}/commits/{sha}",
                path.owner, path.repo
            ),
            commit: CommitDetailJson {
                message: if summary.body.is_empty() {
                    summary.summary.clone()
                } else {
                    format!("{}\n\n{}", summary.summary, summary.body)
                },
                author: person(&summary.author_name, &summary.author_email),
                committer: person(&summary.author_name, &summary.author_email),
            },
            parents: summary
                .parents
                .iter()
                .map(|parent| {
                    serde_json::json!({
                        "sha": parent.to_string(),
                        "url": format!(
                            "{origin}/api/v3/repos/{}/{}/commits/{parent}",
                            path.owner, path.repo
                        ),
                    })
                })
                .collect(),
            sha,
        });
    }
    paginate_rows(&req, rows, &query)
}

#[derive(Debug, Serialize)]
pub struct ReviewJson {
    pub id: i64,
    pub node_id: String,
    pub user: Option<UserJson>,
    pub body: String,
    pub state: &'static str,
    pub commit_id: String,
    pub submitted_at: String,
    pub html_url: String,
    pub pull_request_url: String,
}

/// GitHub's name for a stored review state.
fn review_state_name(state: ReviewState) -> &'static str {
    match state {
        ReviewState::Approved => "APPROVED",
        ReviewState::RequestChanges => "CHANGES_REQUESTED",
        ReviewState::Comment => "COMMENTED",
    }
}

/// `GET /api/v3/repos/{owner}/{repo}/pulls/{number}/reviews`
#[get("/api/v3/repos/{owner}/{repo}/pulls/{number}/reviews")]
pub async fn list_reviews(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    query: web::Query<PageQuery>,
) -> HttpResponse {
    let (_repo, pull) = match load_pull(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.number,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let db = auth_state.db();
    let origin = origin(&req);
    let reviews = match db.list_pull_request_reviews(&pull.id).await {
        Ok(reviews) => reviews,
        Err(detail) => return internal(detail),
    };
    let mut rows = Vec::with_capacity(reviews.len());
    for review in &reviews {
        let user = match db.get_user_by_id(&review.author_user_id).await {
            Ok(Some(user)) => Some(user_json(&origin, &user.username, &user.id)),
            _ => None,
        };
        rows.push(ReviewJson {
            id: review.id,
            node_id: format!("PRR_{}", review.id),
            user,
            body: review.body.clone(),
            state: review_state_name(review.state),
            commit_id: review.commit_oid.clone(),
            submitted_at: review.submitted_at.clone(),
            html_url: format!(
                "{origin}/{}/{}/pulls/{}#review-{}",
                path.owner, path.repo, pull.number, review.id
            ),
            pull_request_url: format!(
                "{origin}/api/v3/repos/{}/{}/pulls/{}",
                path.owner, path.repo, pull.number
            ),
        });
    }
    paginate_rows(&req, rows, &query)
}

#[derive(Debug, Deserialize)]
pub struct ReviewCommentInput {
    pub path: String,
    pub body: String,
    /// The line in the file: the new side for `RIGHT` (the default), the old for `LEFT`.
    pub line: Option<i64>,
    pub side: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitReviewBody {
    pub event: String,
    pub body: Option<String>,
    pub commit_id: Option<String>,
    pub comments: Option<Vec<ReviewCommentInput>>,
}

/// `POST /api/v3/repos/{owner}/{repo}/pulls/{number}/reviews`. `event` is `APPROVE`,
/// `REQUEST_CHANGES`, or `COMMENT`. Answers `200` with the review.
#[post("/api/v3/repos/{owner}/{repo}/pulls/{number}/reviews")]
pub async fn submit_review(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    body: web::Json<SubmitReviewBody>,
) -> HttpResponse {
    let (repo, pull) = match load_pull(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.number,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let state = match body.event.as_str() {
        "APPROVE" => ReviewState::Approved,
        "REQUEST_CHANGES" => ReviewState::RequestChanges,
        "COMMENT" => ReviewState::Comment,
        _ => return error(StatusCode::UNPROCESSABLE_ENTITY, "Invalid review event"),
    };
    let Some(head) = repo
        .handle
        .branch_tip(&pull.head_branch)
        .ok()
        .flatten()
        .map(|oid| oid.to_string())
    else {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Head branch does not exist",
        );
    };
    if body
        .commit_id
        .as_deref()
        .is_some_and(|commit| commit != head)
    {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "commit_id must name the head of the pull request",
        );
    }
    // Each line comment becomes one inline row: `LEFT` lines use the old position,
    // everything else the new one.
    let mut owned: Vec<(String, String, i64, i64)> = Vec::new();
    for comment in body.comments.as_deref().unwrap_or_default() {
        let Some(line) = comment.line else {
            return error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Inline comments need a line",
            );
        };
        let (old_position, new_position) = match comment.side.as_deref().unwrap_or("RIGHT") {
            "LEFT" => (line, 0),
            "RIGHT" => (0, line),
            _ => return error(StatusCode::UNPROCESSABLE_ENTITY, "Invalid side"),
        };
        owned.push((
            comment.path.clone(),
            comment.body.clone(),
            old_position,
            new_position,
        ));
    }
    let inline: Vec<InlineInput<'_>> = owned
        .iter()
        .map(|(path, body, old_position, new_position)| InlineInput {
            path,
            body,
            old_position: *old_position,
            new_position: *new_position,
        })
        .collect();
    let actor = actor_of(&repo);
    match pulls::submit_review(
        auth_state.db(),
        &pull,
        &actor,
        &repo.permissions,
        state,
        body.body.as_deref().unwrap_or(""),
        &head,
        &inline,
    )
    .await
    {
        Ok(review) => {
            let origin = origin(&req);
            let user = match auth_state.db().get_user_by_id(&review.author_user_id).await {
                Ok(Some(user)) => Some(user_json(&origin, &user.username, &user.id)),
                _ => None,
            };
            HttpResponse::Ok().json(ReviewJson {
                id: review.id,
                node_id: format!("PRR_{}", review.id),
                user,
                body: review.body,
                state: review_state_name(review.state),
                commit_id: review.commit_oid,
                submitted_at: review.submitted_at,
                html_url: format!(
                    "{origin}/{}/{}/pulls/{}#review-{}",
                    path.owner, path.repo, pull.number, review.id
                ),
                pull_request_url: format!(
                    "{origin}/api/v3/repos/{}/{}/pulls/{}",
                    path.owner, path.repo, pull.number
                ),
            })
        }
        Err(failure) => pull_error(failure, StatusCode::UNPROCESSABLE_ENTITY),
    }
}

#[derive(Debug, Serialize)]
pub struct RequestedReviewersJson {
    pub users: Vec<UserJson>,
    pub teams: Vec<serde_json::Value>,
}

/// `GET /api/v3/repos/{owner}/{repo}/pulls/{number}/requested_reviewers`
#[get("/api/v3/repos/{owner}/{repo}/pulls/{number}/requested_reviewers")]
pub async fn list_requested_reviewers(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
) -> HttpResponse {
    let (_repo, pull) = match load_pull(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.number,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let db = auth_state.db();
    let origin = origin(&req);
    let usernames = match db.list_requested_reviewers(&pull.id).await {
        Ok(names) => names,
        Err(detail) => return internal(detail),
    };
    let mut users = Vec::new();
    for username in usernames {
        if let Ok(Some(user)) = db.get_user_by_username(&username).await {
            users.push(user_json(&origin, &user.username, &user.id));
        }
    }
    HttpResponse::Ok().json(RequestedReviewersJson {
        users,
        teams: Vec::new(),
    })
}

#[derive(Debug, Deserialize)]
pub struct ReviewersBody {
    pub reviewers: Vec<String>,
}

/// Asks or withdraws reviewers by username, then answers with the request.
async fn change_reviewers(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    path: &IssuePath,
    reviewers: &[String],
    requested: bool,
) -> HttpResponse {
    let (repo, pull) = match load_pull(
        req,
        server,
        auth_state,
        &path.owner,
        &path.repo,
        path.number,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let db = auth_state.db();
    let actor = actor_of(&repo);
    for name in reviewers {
        if let Err(failure) = pulls::set_requested_reviewer(
            db,
            &path.owner,
            &pull,
            &actor,
            &repo.permissions,
            name,
            requested,
        )
        .await
        {
            return pull_error(failure, StatusCode::CONFLICT);
        }
    }
    match render(
        db,
        &repo.handle,
        &origin(req),
        &path.owner,
        &path.repo,
        &pull,
        true,
    )
    .await
    {
        Ok(item) => HttpResponse::Created().json(item),
        Err(detail) => internal(detail),
    }
}

/// `POST /api/v3/repos/{owner}/{repo}/pulls/{number}/requested_reviewers`. Answers `201`.
#[post("/api/v3/repos/{owner}/{repo}/pulls/{number}/requested_reviewers")]
pub async fn request_reviewers(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    body: web::Json<ReviewersBody>,
) -> HttpResponse {
    change_reviewers(&req, &server, &auth_state, &path, &body.reviewers, true).await
}

/// `DELETE /api/v3/repos/{owner}/{repo}/pulls/{number}/requested_reviewers`. Answers `200`.
#[delete("/api/v3/repos/{owner}/{repo}/pulls/{number}/requested_reviewers")]
pub async fn remove_requested_reviewers(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    body: web::Json<ReviewersBody>,
) -> HttpResponse {
    let response =
        change_reviewers(&req, &server, &auth_state, &path, &body.reviewers, false).await;
    if response.status() == StatusCode::CREATED {
        // Withdrawing answers `200`, not `201`, so the body is rebuilt with the same content.
        let body = actix_web::body::to_bytes(response.into_body())
            .await
            .unwrap_or_default();
        return HttpResponse::Ok()
            .content_type("application/json")
            .body(body);
    }
    response
}
