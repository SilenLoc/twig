//! Code-review reads: a request's commits and diff, commit reads and diffs, and
//! file contents at a branch or commit. Forgejo clients fetch these to show a
//! request's changes.

use actix_web::{HttpRequest, HttpResponse, get, web};
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{NamespacePermission, TwigContext},
    config,
    db::Database,
    git::bare::{CommitSummary, RepoHandle},
    pulls::{self, RepoRef},
};

use super::{authorized_repo, compat_error, from_pull_error, json::UserJson, origin};

const DEFAULT_LIMIT: u32 = 30;
const MAX_LIMIT: u32 = 50;

#[derive(Debug, Deserialize)]
pub struct PullPath {
    pub owner: String,
    pub repo: String,
    pub index: i64,
}

#[derive(Debug, Deserialize)]
pub struct PullDiffPath {
    pub owner: String,
    pub repo: String,
    pub index: i64,
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct RefQuery {
    #[serde(rename = "ref")]
    pub reference: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CommitPath {
    pub owner: String,
    pub repo: String,
    pub sha: String,
}

#[derive(Debug, Deserialize)]
pub struct ContentsPath {
    pub owner: String,
    pub repo: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct PersonJson {
    pub name: String,
    pub email: String,
    pub date: String,
}

#[derive(Debug, Serialize)]
pub struct CommitInfoJson {
    pub message: String,
    pub url: String,
    pub author: PersonJson,
    pub committer: PersonJson,
    pub verification: Option<()>,
}

#[derive(Debug, Serialize)]
pub struct ParentJson {
    pub sha: String,
    pub url: String,
    pub html_url: String,
}

#[derive(Debug, Serialize)]
pub struct StatsJson {
    pub additions: i64,
    pub deletions: i64,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct CommitJson {
    pub sha: String,
    pub url: String,
    pub html_url: String,
    pub created: String,
    pub commit: CommitInfoJson,
    pub author: Option<UserJson>,
    pub committer: Option<UserJson>,
    pub parents: Vec<ParentJson>,
    pub stats: Option<StatsJson>,
}

#[derive(Debug, Serialize)]
pub struct ContentsJson {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub name: String,
    pub path: String,
    pub sha: String,
    pub size: usize,
    pub encoding: &'static str,
    pub content: String,
    pub url: String,
    pub html_url: String,
    pub download_url: String,
}

fn person(name: &str, email: &str, seconds: i64) -> PersonJson {
    PersonJson {
        name: name.to_string(),
        email: email.to_string(),
        date: chrono::DateTime::from_timestamp(seconds, 0)
            .map(|time| time.to_rfc3339())
            .unwrap_or_default(),
    }
}

/// Maps a commit author to a Twig user by email, the way Forgejo links commits.
async fn author_for(db: &Database, email: &str) -> Option<UserJson> {
    db.get_user_by_email(email)
        .await
        .ok()
        .flatten()
        .map(|user| UserJson::public(&user))
}

async fn commit_json(
    db: &Database,
    base: &str,
    owner: &str,
    repo: &str,
    summary: &CommitSummary,
) -> CommitJson {
    let sha = summary.id.to_string();
    let url = format!("{base}/api/v1/repos/{owner}/{repo}/git/commits/{sha}");
    let author = author_for(db, &summary.author_email).await;
    CommitJson {
        html_url: format!("{base}/{owner}/{repo}/commit/{sha}"),
        created: person(
            &summary.author_name,
            &summary.author_email,
            summary.committed_at,
        )
        .date,
        commit: CommitInfoJson {
            message: if summary.body.is_empty() {
                summary.summary.clone()
            } else {
                format!("{}\n\n{}", summary.summary, summary.body)
            },
            url: url.clone(),
            author: person(
                &summary.author_name,
                &summary.author_email,
                summary.committed_at,
            ),
            committer: person(
                &summary.author_name,
                &summary.author_email,
                summary.committed_at,
            ),
            verification: None,
        },
        committer: author.clone(),
        author,
        parents: summary
            .parents
            .iter()
            .map(|parent| ParentJson {
                sha: parent.to_string(),
                url: format!("{base}/api/v1/repos/{owner}/{repo}/git/commits/{parent}"),
                html_url: format!("{base}/{owner}/{repo}/commit/{parent}"),
            })
            .collect(),
        stats: None,
        url,
        sha,
    }
}

fn internal(detail: &str) -> HttpResponse {
    log::error!("Compatibility code request failed: {detail}");
    compat_error(
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
        "The request could not be completed",
    )
}

fn not_found() -> HttpResponse {
    compat_error(
        actix_web::http::StatusCode::NOT_FOUND,
        "The target couldn't be found",
    )
}

/// Resolves a branch name or a full commit ID to a commit.
fn resolve(handle: &RepoHandle, reference: &str) -> Option<git2::Oid> {
    if let Ok(Some(tip)) = handle.branch_tip(reference) {
        return Some(tip);
    }
    let oid = git2::Oid::from_str(reference).ok()?;
    handle.commit_summary(oid).ok().map(|summary| summary.id)
}

/// `GET /api/v1/repos/{owner}/{repo}/pulls/{index}/commits`: the request's commits, oldest first.
#[get("/api/v1/repos/{owner}/{repo}/pulls/{index}/commits")]
pub async fn list_pull_commits(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    query: web::Query<PageQuery>,
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
    let pull = match pulls::get(auth_state.db(), &RepoRef { repo_id: &repo_id }, path.index).await {
        Ok(pull) => pull,
        Err(error) => return from_pull_error(error),
    };
    let (Some(base), Some(head)) = (
        repo.handle.branch_tip(&pull.base_branch).ok().flatten(),
        repo.handle.branch_tip(&pull.head_branch).ok().flatten(),
    ) else {
        return not_found();
    };
    let ids = match repo.handle.commit_ids_in_range(base, head) {
        Ok(ids) => ids,
        Err(error) => return internal(&error.to_string()),
    };
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as usize;
    let page = query.page.unwrap_or(1).max(1) as usize;
    let start = (page - 1) * limit;
    let mut items = Vec::new();
    for oid in ids.iter().skip(start).take(limit) {
        let summary = match repo.handle.commit_summary(*oid) {
            Ok(summary) => summary,
            Err(error) => return internal(&error.to_string()),
        };
        items.push(
            commit_json(
                auth_state.db(),
                &origin(&req),
                &path.owner,
                &path.repo,
                &summary,
            )
            .await,
        );
    }
    let mut response = HttpResponse::Ok();
    if start + limit < ids.len() {
        let next = format!("{}?page={}&limit={limit}", req.path(), page + 1);
        response.insert_header(("Link", format!("<{}{next}>; rel=\"next\"", origin(&req))));
    }
    response.json(items)
}

/// The text of a request's diff: the merge base against its head.
fn request_patch(handle: &RepoHandle, base_branch: &str, head_branch: &str) -> Option<String> {
    let base = handle.branch_tip(base_branch).ok().flatten()?;
    let head = handle.branch_tip(head_branch).ok().flatten()?;
    let from = handle.merge_base(base, head).ok().flatten().unwrap_or(base);
    handle.diff_patch(from, head).ok()
}

async fn patch_response(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    owner: String,
    repo: String,
    index: i64,
    format: &'static str,
) -> HttpResponse {
    let repository = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &owner,
        &repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repository) => repository,
        Err(response) => return response,
    };
    let repo_id = match repository.handle.repo_id() {
        Ok(id) => id,
        Err(error) => return internal(&error.to_string()),
    };
    let pull = match pulls::get(auth_state.db(), &RepoRef { repo_id: &repo_id }, index).await {
        Ok(pull) => pull,
        Err(error) => return from_pull_error(error),
    };
    match request_patch(&repository.handle, &pull.base_branch, &pull.head_branch) {
        Some(text) => HttpResponse::Ok()
            .content_type(format!("text/{format}; charset=utf-8"))
            .body(text),
        None => not_found(),
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/pulls/{index}.diff`
#[get("/api/v1/repos/{owner}/{repo}/pulls/{index}.diff")]
pub async fn pull_diff(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullDiffPath>,
) -> HttpResponse {
    patch_response(
        req,
        server,
        auth_state,
        path.owner.clone(),
        path.repo.clone(),
        path.index,
        "plain",
    )
    .await
}

/// `GET /api/v1/repos/{owner}/{repo}/pulls/{index}.patch`
#[get("/api/v1/repos/{owner}/{repo}/pulls/{index}.patch")]
pub async fn pull_patch(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullDiffPath>,
) -> HttpResponse {
    patch_response(
        req,
        server,
        auth_state,
        path.owner.clone(),
        path.repo.clone(),
        path.index,
        "plain",
    )
    .await
}

/// `GET /api/v1/repos/{owner}/{repo}/git/commits/{sha}`
#[get("/api/v1/repos/{owner}/{repo}/git/commits/{sha}")]
pub async fn get_commit(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CommitPath>,
) -> HttpResponse {
    let repository = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repository) => repository,
        Err(response) => return response,
    };
    let Ok(oid) = git2::Oid::from_str(&path.sha) else {
        return not_found();
    };
    let summary = match repository.handle.commit_summary(oid) {
        Ok(summary) => summary,
        Err(_) => return not_found(),
    };
    let json = commit_json(
        auth_state.db(),
        &origin(&req),
        &path.owner,
        &path.repo,
        &summary,
    )
    .await;
    HttpResponse::Ok().json(json)
}

/// `GET /api/v1/repos/{owner}/{repo}/git/commits/{sha}.diff`
#[get("/api/v1/repos/{owner}/{repo}/git/commits/{sha}.diff")]
pub async fn get_commit_diff(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CommitPath>,
) -> HttpResponse {
    let repository = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repository) => repository,
        Err(response) => return response,
    };
    let Ok(oid) = git2::Oid::from_str(&path.sha) else {
        return not_found();
    };
    match repository.handle.commit_patch(oid) {
        Ok(text) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body(text),
        Err(_) => not_found(),
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/contents/{path}?ref=`: one file's bytes, base64.
#[get("/api/v1/repos/{owner}/{repo}/contents/{path:.*}")]
pub async fn get_contents(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<ContentsPath>,
    query: web::Query<RefQuery>,
) -> HttpResponse {
    let repository = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repository) => repository,
        Err(response) => return response,
    };
    if !crate::git::bare::is_safe_repo_path(&path.path) {
        return not_found();
    }
    let reference = match query.reference.clone() {
        Some(reference) => reference,
        None => match repository.handle.default_branch_name() {
            Ok(Some(branch)) => branch,
            _ => return not_found(),
        },
    };
    let Some(commit) = resolve(&repository.handle, &reference) else {
        return not_found();
    };
    let (blob, bytes) = match repository.handle.blob_at_commit(commit, &path.path) {
        Ok(Some(found)) => found,
        Ok(None) => return not_found(),
        Err(error) => return internal(&error.to_string()),
    };
    let base = origin(&req);
    let name = path.path.rsplit('/').next().unwrap_or_default().to_string();
    HttpResponse::Ok().json(ContentsJson {
        kind: "file",
        name,
        path: path.path.clone(),
        sha: blob.to_string(),
        size: bytes.len(),
        encoding: "base64",
        content: base64::engine::general_purpose::STANDARD.encode(&bytes),
        url: format!(
            "{base}/api/v1/repos/{}/{}/contents/{}",
            path.owner, path.repo, path.path
        ),
        html_url: format!("{base}/{}/{}/src/{}", path.owner, path.repo, path.path),
        download_url: format!("{base}/{}/{}/raw/{}", path.owner, path.repo, path.path),
    })
}

/// `GET /api/v1/repos/{owner}/{repo}/raw/{filepath}?ref=`: a file's raw bytes.
#[get("/api/v1/repos/{owner}/{repo}/raw/{path:.*}")]
pub async fn get_raw_file(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<ContentsPath>,
    query: web::Query<RefQuery>,
) -> HttpResponse {
    let repository = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repository) => repository,
        Err(response) => return response,
    };
    if !crate::git::bare::is_safe_repo_path(&path.path) {
        return not_found();
    }
    let reference = match query.reference.clone() {
        Some(reference) => reference,
        None => match repository.handle.default_branch_name() {
            Ok(Some(branch)) => branch,
            _ => return not_found(),
        },
    };
    let Some(commit) = resolve(&repository.handle, &reference) else {
        return not_found();
    };
    match repository.handle.blob_at_commit(commit, &path.path) {
        Ok(Some((_, bytes))) => HttpResponse::Ok()
            .content_type("application/octet-stream")
            .body(bytes),
        Ok(None) => not_found(),
        Err(error) => internal(&error.to_string()),
    }
}

#[derive(Debug, Serialize)]
pub struct CompareJson {
    pub total_commits: i64,
    pub commits: Vec<CommitJson>,
    pub files: Vec<serde_json::Value>,
}

/// `GET /api/v1/repos/{owner}/{repo}/compare/{base}...{head}`: the commits `head` adds to
/// `base`, oldest first. Clients call it before creating a request to suggest its title.
#[get("/api/v1/repos/{owner}/{repo}/compare/{spec:.*}")]
pub async fn compare(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CompareSpecPath>,
) -> HttpResponse {
    let repository = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repository) => repository,
        Err(response) => return response,
    };
    let Some((base_ref, head_ref)) = path.spec.split_once("...") else {
        return compat_error(
            actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            "Compare with base...head",
        );
    };
    let (Some(base), Some(head)) = (
        resolve(&repository.handle, base_ref),
        resolve(&repository.handle, head_ref),
    ) else {
        return not_found();
    };
    let ids = match repository.handle.commit_ids_in_range(base, head) {
        Ok(ids) => ids,
        Err(error) => return internal(&error.to_string()),
    };
    let mut commits = Vec::with_capacity(ids.len());
    for oid in &ids {
        let summary = match repository.handle.commit_summary(*oid) {
            Ok(summary) => summary,
            Err(error) => return internal(&error.to_string()),
        };
        commits.push(
            commit_json(
                auth_state.db(),
                &origin(&req),
                &path.owner,
                &path.repo,
                &summary,
            )
            .await,
        );
    }
    HttpResponse::Ok().json(CompareJson {
        total_commits: i64::try_from(commits.len()).unwrap_or_default(),
        commits,
        files: Vec::new(),
    })
}

#[derive(Debug, Deserialize)]
pub struct CompareSpecPath {
    pub owner: String,
    pub repo: String,
    pub spec: String,
}

#[derive(Debug, Serialize)]
pub struct ChangedFileJson {
    pub filename: String,
    pub previous_filename: Option<String>,
    pub status: &'static str,
    pub additions: i64,
    pub deletions: i64,
    pub changes: i64,
    pub raw_url: String,
    pub contents_url: String,
    pub html_url: String,
}

/// `GET /api/v1/repos/{owner}/{repo}/pulls/{index}/files`: the files the request changes.
#[get("/api/v1/repos/{owner}/{repo}/pulls/{index}/files")]
pub async fn list_pull_files(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
) -> HttpResponse {
    let repository = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repository) => repository,
        Err(response) => return response,
    };
    let repo_id = match repository.handle.repo_id() {
        Ok(id) => id,
        Err(error) => return internal(&error.to_string()),
    };
    let pull = match pulls::get(auth_state.db(), &RepoRef { repo_id: &repo_id }, path.index).await {
        Ok(pull) => pull,
        Err(error) => return from_pull_error(error),
    };
    let Some((base, head)) = repository
        .handle
        .branch_tip(&pull.base_branch)
        .ok()
        .flatten()
        .zip(
            repository
                .handle
                .branch_tip(&pull.head_branch)
                .ok()
                .flatten(),
        )
    else {
        return not_found();
    };
    let from = repository
        .handle
        .merge_base(base, head)
        .ok()
        .flatten()
        .unwrap_or(base);
    let files = match repository.handle.diff_files(from, head) {
        Ok(files) => files,
        Err(error) => return internal(&error.to_string()),
    };
    let base_url = origin(&req);
    let rows: Vec<ChangedFileJson> = files
        .iter()
        .map(|file| {
            let status = match file.status {
                crate::git::bare::FileStatus::Added => "added",
                crate::git::bare::FileStatus::Deleted => "removed",
                crate::git::bare::FileStatus::Renamed => "renamed",
                crate::git::bare::FileStatus::Modified => "modified",
            };
            let name = file.path().to_string();
            let changes = i64::try_from(file.additions + file.deletions).unwrap_or_default();
            ChangedFileJson {
                previous_filename: file
                    .old_path
                    .clone()
                    .filter(|old| Some(old) != file.new_path.as_ref()),
                status,
                additions: i64::try_from(file.additions).unwrap_or_default(),
                deletions: i64::try_from(file.deletions).unwrap_or_default(),
                changes,
                raw_url: format!("{base_url}/{}/{}/raw/{name}", path.owner, path.repo),
                contents_url: format!(
                    "{base_url}/api/v1/repos/{}/{}/contents/{name}",
                    path.owner, path.repo
                ),
                html_url: format!(
                    "{base_url}/{}/{}/pulls/{}/files",
                    path.owner, path.repo, path.index
                ),
                filename: name,
            }
        })
        .collect();
    super::paginate(&req, rows)
}
