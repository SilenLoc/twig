use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{NamespacePermission, TwigContext},
    config, git,
};

use super::{
    compat_error, from_api_error,
    json::{RepositoryJson, RepositoryLocation, UserJson},
    origin,
    repos::{permissions_json, repository_json},
    v1,
};
use crate::auth::PermissionSet;

/// `GET /api/v1/user`: the authenticated caller. Clients use it as "whoami".
#[get("/api/v1/user")]
pub async fn current_user(req: HttpRequest, auth_state: web::Data<TwigContext>) -> HttpResponse {
    match v1::authenticate(&req, &auth_state).await {
        Ok(principal) => HttpResponse::Ok().json(UserJson::own(&principal.user)),
        Err(error) => from_api_error(error),
    }
}

#[derive(Serialize)]
struct ServerVersion {
    version: &'static str,
}

/// `GET /api/v1/version`: the running Twig version in Forgejo's shape.
#[get("/api/v1/version")]
pub async fn server_version(req: HttpRequest, auth_state: web::Data<TwigContext>) -> HttpResponse {
    if let Err(error) = v1::authenticate(&req, &auth_state).await {
        return from_api_error(error);
    }
    HttpResponse::Ok().json(ServerVersion {
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Debug, Deserialize)]
pub struct CreateRepoBody {
    pub name: String,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub default_branch: Option<String>,
}

/// `POST /api/v1/user/repos`: creates a repository in the namespace named after the
/// caller's login. Privacy is set in `.twig.toml` in Twig, so `private` is refused.
#[post("/api/v1/user/repos")]
pub async fn create_user_repo(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    body: web::Json<CreateRepoBody>,
) -> HttpResponse {
    let principal = match v1::authenticate(&req, &auth_state).await {
        Ok(principal) => principal,
        Err(error) => return from_api_error(error),
    };
    let namespace = principal.user.username.clone();
    match auth_state.db().get_namespace_by_name(&namespace).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return compat_error(
                actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                &format!(
                    "Create a namespace named '{namespace}' first; repositories are created in it"
                ),
            );
        }
        Err(error) => {
            log::error!("Failed to look up namespace for user repository: {error}");
            return compat_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            );
        }
    }
    let permissions: PermissionSet = match v1::require_namespace_permission(
        &principal,
        &server,
        &auth_state,
        &namespace,
        NamespacePermission::CreateRepositories,
    )
    .await
    {
        Ok(permissions) => permissions,
        Err(error) => return from_api_error(error),
    };
    let name = body.name.trim().to_string();
    if let Err(error) = v1::validate_repository_name(&name) {
        return from_api_error(error);
    }
    if body.private {
        return compat_error(
            actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            "Set private = true in .twig.toml to make a Twig repository private",
        );
    }
    let Some(email) = principal.user.email.clone() else {
        return compat_error(
            actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            "Set an email address before creating repositories",
        );
    };
    let branch = body
        .default_branch
        .clone()
        .unwrap_or_else(git::repo::default_branch);
    let namespace_dir = match git::repo::ensure_namespace_directory(
        std::path::Path::new(server.project_root()),
        &namespace,
    ) {
        Ok(directory) => directory,
        Err(error) => {
            log::error!("Failed to prepare namespace for user repository: {error}");
            return compat_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            );
        }
    };
    let repo_path = namespace_dir.join(&name);
    if repo_path.exists() {
        return compat_error(
            actix_web::http::StatusCode::CONFLICT,
            "Repository already exists",
        );
    }
    if let Err(error) = std::fs::create_dir(&repo_path) {
        log::error!("Failed to create user repository directory: {error}");
        return compat_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "The request could not be completed",
        );
    }
    if let Err(error) = git::repo::bare_init(&repo_path, &branch, &principal.user.username, &email)
    {
        log::error!("Failed to initialize user repository: {error}");
        let _ = std::fs::remove_dir_all(&repo_path);
        return compat_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "The request could not be completed",
        );
    }
    let repo_id = match crate::git::bare::RepoHandle::open(server.project_root(), &namespace, &name)
        .and_then(|handle| handle.repo_id())
    {
        Ok(id) => id,
        Err(error) => {
            log::error!("Failed to read new repository identity: {error}");
            return compat_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            );
        }
    };
    let handle = match crate::git::bare::RepoHandle::open(server.project_root(), &namespace, &name)
    {
        Ok(handle) => handle,
        Err(_) => {
            return compat_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            );
        }
    };
    let location = RepositoryLocation {
        origin: &origin(&req),
        namespace: &namespace,
        name: &name,
        repo_id: &repo_id,
    };
    let json: Result<RepositoryJson, git2::Error> =
        repository_json(&location, &handle, permissions_json(&permissions));
    match json {
        Ok(json) => HttpResponse::Created().json(json),
        Err(error) => {
            log::error!("Failed to describe new repository: {error}");
            compat_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            )
        }
    }
}
