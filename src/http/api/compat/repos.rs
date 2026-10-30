//! Repository metadata that Forgejo clients read before and around pull requests:
//! the repository itself, branches, and the empty lists of features Twig does not
//! model (commit statuses, labels). Assignees are the namespace's members.

use actix_web::{HttpRequest, HttpResponse, get, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{NamespacePermission, PermissionSet, TwigContext},
    config,
    git::bare::RepoHandle,
};

use super::{
    RepoPath, authorized_repo, compat_error,
    json::{PermissionsJson, RepositoryJson, RepositoryLocation, UserJson},
    origin,
};

#[derive(Debug, Deserialize)]
pub struct BranchPath {
    pub owner: String,
    pub repo: String,
    pub branch: String,
}

#[derive(Debug, Serialize)]
struct CommitRefJson {
    id: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct BranchJson {
    name: String,
    commit: CommitRefJson,
    protected: bool,
    user_can_push: bool,
    user_can_merge: bool,
    enable_status_check: bool,
    status_check_contexts: Vec<String>,
}

pub fn permissions_json(permissions: &PermissionSet) -> PermissionsJson {
    PermissionsJson {
        admin: permissions.contains(NamespacePermission::ManageRepositories),
        push: permissions.contains(NamespacePermission::PushChanges),
        pull: permissions.contains(NamespacePermission::ViewNamespace),
    }
}

/// Builds the repository object for `repo`. Reading the configuration and the
/// branch heads is the only Git work involved.
pub fn repository_json(
    location: &RepositoryLocation<'_>,
    handle: &RepoHandle,
    permissions: PermissionsJson,
) -> Result<RepositoryJson, git2::Error> {
    let private = handle.load_config_with_raw().config.private;
    let default_branch = handle.default_branch_name()?;
    let empty = handle.head_oid()?.is_none();
    Ok(RepositoryJson::new(
        location,
        private,
        default_branch,
        empty,
        permissions,
    ))
}

/// `GET /api/v1/repos/{owner}/{repo}`
#[get("/api/v1/repos/{owner}/{repo}")]
pub async fn get_repository(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
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
    let location = RepositoryLocation {
        origin: &origin(&req),
        namespace: &path.owner,
        name: &path.repo,
        repo_id: &repo_id,
    };
    match repository_json(&location, &repo.handle, permissions_json(&repo.permissions)) {
        Ok(json) => HttpResponse::Ok().json(json),
        Err(error) => internal(&error.to_string()),
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/branches/{branch}`. The branch may contain
/// slashes, which clients send unescaped.
#[get("/api/v1/repos/{owner}/{repo}/branches/{branch:.*}")]
pub async fn get_branch(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<BranchPath>,
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
    let tip = match repo.handle.branch_tip(&path.branch) {
        Ok(Some(tip)) => tip,
        Ok(None) | Err(_) => return not_found(),
    };
    let summary = match repo.handle.commit_summary(tip) {
        Ok(summary) => summary,
        Err(error) => return internal(&error.to_string()),
    };
    let permissions = permissions_json(&repo.permissions);
    HttpResponse::Ok().json(BranchJson {
        name: path.branch.clone(),
        commit: CommitRefJson {
            id: tip.to_string(),
            message: summary.summary,
        },
        protected: false,
        user_can_push: permissions.push,
        user_can_merge: permissions.push,
        enable_status_check: false,
        status_check_contexts: Vec::new(),
    })
}

/// `GET /api/v1/repos/{owner}/{repo}/statuses/{sha}`: Twig runs no CI, so no statuses.
#[get("/api/v1/repos/{owner}/{repo}/statuses/{sha}")]
pub async fn list_statuses(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
) -> HttpResponse {
    if let Err(response) = authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        return response;
    }
    HttpResponse::Ok().json(Vec::<serde_json::Value>::new())
}

/// `GET /api/v1/repos/{owner}/{repo}/labels`: labels are not part of Twig's model.
#[get("/api/v1/repos/{owner}/{repo}/labels")]
pub async fn list_labels(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
) -> HttpResponse {
    if let Err(response) = authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        return response;
    }
    HttpResponse::Ok().json(Vec::<serde_json::Value>::new())
}

/// `GET /api/v1/repos/{owner}/{repo}/assignees`: everyone who may be asked to review.
#[get("/api/v1/repos/{owner}/{repo}/assignees")]
pub async fn list_assignees(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
) -> HttpResponse {
    if let Err(response) = authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        return response;
    }
    match auth_state.db().list_namespace_members(&path.owner).await {
        Ok(members) => super::paginate(
            &req,
            members.iter().map(UserJson::public).collect::<Vec<_>>(),
        ),
        Err(error) => internal(&error),
    }
}

fn not_found() -> HttpResponse {
    compat_error(
        actix_web::http::StatusCode::NOT_FOUND,
        "The target couldn't be found",
    )
}

fn internal(detail: &str) -> HttpResponse {
    log::error!("Compatibility repository request failed: {detail}");
    compat_error(
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
        "The request could not be completed",
    )
}

#[derive(Debug, Serialize)]
pub struct CombinedStatusJson {
    pub state: &'static str,
    pub sha: String,
    pub total_count: i64,
    pub statuses: Vec<serde_json::Value>,
}

/// `GET /api/v1/repos/{owner}/{repo}/commits/{sha}/status`: the combined state of a
/// commit's statuses. Twig reports none, so the state is `pending`, as Forgejo does.
#[get("/api/v1/repos/{owner}/{repo}/commits/{sha}/status")]
pub async fn combined_status(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<StatusPath>,
) -> HttpResponse {
    if let Err(response) = authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        return response;
    }
    HttpResponse::Ok()
        .insert_header(("X-Total-Count", "0"))
        .json(CombinedStatusJson {
            state: "pending",
            sha: path.sha.clone(),
            total_count: 0,
            statuses: Vec::new(),
        })
}

#[derive(Debug, Deserialize)]
pub struct StatusPath {
    pub owner: String,
    pub repo: String,
    pub sha: String,
}
