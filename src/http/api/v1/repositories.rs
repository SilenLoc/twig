use std::path::Path;

use actix_web::{HttpRequest, HttpResponse, delete, get, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedPrincipal, NamespacePermission, TwigContext},
    config,
    git::{
        self,
        bare::{BranchInfo, Commit, RepoHandle},
    },
};

use super::{ApiError, RepositoryPath, authenticate, json_error, require_namespace_permission};

#[derive(Debug, Serialize)]
struct RepositoryResponse {
    namespace: String,
    name: String,
    private: bool,
    default_branch: Option<String>,
    head: Option<String>,
}

#[derive(Debug, Serialize)]
struct BranchResponse {
    name: String,
    tip: Option<String>,
    is_default: bool,
}

#[derive(Debug, Serialize)]
struct CommitResponse {
    id: String,
    author: String,
    date: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct MutationResponse {
    resource: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct UpdateRepositoryRequest {
    name: Option<String>,
    target_namespace: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CommitQuery {
    #[serde(rename = "ref")]
    reference: String,
    limit: Option<usize>,
}

async fn open_authorized_repo(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    path: &RepositoryPath,
    permission: NamespacePermission,
) -> Result<(AuthenticatedPrincipal, RepoHandle), HttpResponse> {
    let principal = authenticate(req, auth_state).await.map_err(json_error)?;
    super::validate_namespace_name(&path.namespace).map_err(json_error)?;
    super::validate_repository_name(&path.repo).map_err(json_error)?;
    require_namespace_permission(&principal, server, auth_state, &path.namespace, permission)
        .await
        .map_err(json_error)?;
    let handle = RepoHandle::open(server.project_root(), &path.namespace, &path.repo)
        .map_err(|_| json_error(ApiError::NotFound))?;
    Ok((principal, handle))
}

fn repository_response(
    handle: &RepoHandle,
    namespace: &str,
    repo: &str,
) -> Result<RepositoryResponse, ApiError> {
    let config = handle.load_config_with_raw().config;
    let default_branch = handle.default_branch_name().map_err(|error| {
        log::error!("Failed to read API default branch: {error}");
        ApiError::Internal
    })?;
    let head = handle
        .head_oid()
        .map_err(|error| {
            log::error!("Failed to read API repository HEAD: {error}");
            ApiError::Internal
        })?
        .map(|oid| oid.to_string());
    Ok(RepositoryResponse {
        namespace: namespace.to_string(),
        name: repo.to_string(),
        private: config.private,
        default_branch,
        head,
    })
}

#[get("/api/v1/namespaces/{namespace}/repositories/{repo}")]
pub async fn get_repository(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepositoryPath>,
) -> HttpResponse {
    let (_, handle) = match open_authorized_repo(
        &req,
        &server,
        &auth_state,
        &path,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    match repository_response(&handle, &path.namespace, &path.repo) {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(error) => json_error(error),
    }
}

fn repository_path(
    server: &config::Server,
    namespace: &str,
    repo: &str,
) -> Result<std::path::PathBuf, ApiError> {
    super::validate_namespace_name(namespace)?;
    super::validate_repository_name(repo)?;
    let namespace_path =
        git::repo::checked_namespace_path(Path::new(server.project_root()), namespace).map_err(
            |error| {
                log::error!("Invalid API repository namespace path: {error}");
                ApiError::BadRequest("Invalid namespace path".to_string())
            },
        )?;
    let repo_path = namespace_path.join(repo);
    if std::fs::symlink_metadata(&repo_path).is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(ApiError::NotFound);
    }
    if !repo_path.exists() {
        return Err(ApiError::NotFound);
    }
    if git2::Repository::open(&repo_path).is_err() {
        return Err(ApiError::BadRequest(
            "Path is not a Git repository".to_string(),
        ));
    }
    Ok(repo_path)
}

#[actix_web::patch("/api/v1/namespaces/{namespace}/repositories/{repo}")]
pub async fn update_repository(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepositoryPath>,
    body: web::Json<UpdateRepositoryRequest>,
) -> HttpResponse {
    let principal = match authenticate(&req, &auth_state).await {
        Ok(principal) => principal,
        Err(error) => return json_error(error),
    };
    if body.name.is_some() == body.target_namespace.is_some() {
        return json_error(ApiError::BadRequest(
            "Provide exactly one of name or target_namespace".to_string(),
        ));
    }
    if let Err(error) = require_namespace_permission(
        &principal,
        &server,
        &auth_state,
        &path.namespace,
        NamespacePermission::ManageRepositories,
    )
    .await
    {
        return json_error(error);
    }
    let source = match repository_path(&server, &path.namespace, &path.repo) {
        Ok(source) => source,
        Err(error) => return json_error(error),
    };

    let (destination_namespace, destination_name) = if let Some(name) = &body.name {
        let name = name.trim();
        if name != body.name.as_deref().unwrap_or_default() {
            return json_error(ApiError::BadRequest("Invalid repository name".to_string()));
        }
        if let Err(message) = git::reserved::validate_repo_name(name) {
            return json_error(ApiError::BadRequest(message));
        }
        if name == path.repo {
            return json_error(ApiError::BadRequest(
                "Choose a different repository name".to_string(),
            ));
        }
        (path.namespace.clone(), name.to_string())
    } else {
        let target = body.target_namespace.as_deref().unwrap_or_default().trim();
        if target != body.target_namespace.as_deref().unwrap_or_default() {
            return json_error(ApiError::BadRequest("Invalid namespace name".to_string()));
        }
        if let Err(message) = git::reserved::validate_namespace_name(target) {
            return json_error(ApiError::BadRequest(message));
        }
        if target == path.namespace {
            return json_error(ApiError::BadRequest(
                "Choose a different destination namespace".to_string(),
            ));
        }
        if let Err(error) = require_namespace_permission(
            &principal,
            &server,
            &auth_state,
            target,
            NamespacePermission::ManageRepositories,
        )
        .await
        {
            return json_error(error);
        }
        (target.to_string(), path.repo.clone())
    };

    let destination_dir = match git::repo::ensure_namespace_directory(
        Path::new(server.project_root()),
        &destination_namespace,
    ) {
        Ok(directory) => directory,
        Err(error) => {
            log::error!("Failed to prepare API repository destination: {error}");
            return json_error(ApiError::Internal);
        }
    };
    let destination = destination_dir.join(&destination_name);
    if destination.exists() {
        return json_error(ApiError::Conflict(
            "A repository already exists at the destination".to_string(),
        ));
    }
    if let Err(error) = std::fs::rename(&source, &destination) {
        log::error!("Failed to move/rename API repository: {error}");
        return json_error(ApiError::Internal);
    }
    HttpResponse::Ok().json(MutationResponse {
        resource: format!(
            "/api/v1/namespaces/{destination_namespace}/repositories/{destination_name}"
        ),
        message: "Repository moved or renamed".to_string(),
    })
}

#[delete("/api/v1/namespaces/{namespace}/repositories/{repo}")]
pub async fn delete_repository(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepositoryPath>,
) -> HttpResponse {
    let principal = match authenticate(&req, &auth_state).await {
        Ok(principal) => principal,
        Err(error) => return json_error(error),
    };
    if let Err(error) = require_namespace_permission(
        &principal,
        &server,
        &auth_state,
        &path.namespace,
        NamespacePermission::ManageRepositories,
    )
    .await
    {
        return json_error(error);
    }
    let repo_path = match repository_path(&server, &path.namespace, &path.repo) {
        Ok(path) => path,
        Err(error) => return json_error(error),
    };
    if !git::bare::TwigConfig::load(server.project_root(), &path.namespace, &path.repo).deletable {
        return json_error(ApiError::Forbidden);
    }
    let repo_id =
        crate::git::bare::RepoHandle::open(server.project_root(), &path.namespace, &path.repo)
            .and_then(|handle| handle.repo_id())
            .ok();
    if let Err(error) = std::fs::remove_dir_all(&repo_path) {
        log::error!("Failed to delete API repository: {error}");
        return json_error(ApiError::Internal);
    }
    if let Some(repo_id) = repo_id
        && let Err(error) = auth_state
            .db()
            .delete_pull_requests_for_repo(&repo_id)
            .await
    {
        log::error!("Failed to remove pull requests of deleted repository: {error}");
    }
    HttpResponse::Ok().json(MutationResponse {
        resource: format!(
            "/api/v1/namespaces/{}/repositories/{}",
            path.namespace, path.repo
        ),
        message: "Repository deleted".to_string(),
    })
}

fn branch_response(branch: &BranchInfo) -> BranchResponse {
    BranchResponse {
        name: branch.name.clone(),
        tip: branch.tip.map(|tip| tip.to_string()),
        is_default: branch.is_default,
    }
}

#[get("/api/v1/namespaces/{namespace}/repositories/{repo}/branches")]
pub async fn list_branches(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepositoryPath>,
) -> HttpResponse {
    let (_, handle) = match open_authorized_repo(
        &req,
        &server,
        &auth_state,
        &path,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    match handle.list_branches() {
        Ok(branches) => HttpResponse::Ok().json(
            branches
                .into_iter()
                .map(|branch| branch_response(&branch))
                .collect::<Vec<_>>(),
        ),
        Err(error) => {
            log::error!("Failed to list API branches: {error}");
            json_error(ApiError::Internal)
        }
    }
}

fn commit_response(commit: &Commit) -> CommitResponse {
    CommitResponse {
        id: commit.hash().to_string(),
        author: commit.author().to_string(),
        date: commit.date().to_rfc3339(),
        message: commit.message().to_string(),
    }
}

#[get("/api/v1/namespaces/{namespace}/repositories/{repo}/commits")]
pub async fn list_commits(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepositoryPath>,
    query: web::Query<CommitQuery>,
) -> HttpResponse {
    let (_, handle) = match open_authorized_repo(
        &req,
        &server,
        &auth_state,
        &path,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    if handle.branch_tip(&query.reference).is_err() {
        return json_error(ApiError::BadRequest("Invalid branch name".to_string()));
    }
    if handle.branch_tip(&query.reference).ok().flatten().is_none() {
        return json_error(ApiError::NotFound);
    }
    let limit = query.limit.unwrap_or(100).min(git::bare::MAX_COMMITS);
    if limit == 0 {
        return json_error(ApiError::BadRequest("limit must be at least 1".to_string()));
    }
    match handle.get_commits_at(&query.reference, limit) {
        Ok(commits) => {
            HttpResponse::Ok().json(commits.iter().map(commit_response).collect::<Vec<_>>())
        }
        Err(error) => {
            log::error!("Failed to load API commits: {error}");
            json_error(ApiError::Internal)
        }
    }
}
