use std::path::Path;

use actix_web::{HttpRequest, HttpResponse, web};

use crate::auth::{
    AuthenticatedPrincipal, CredentialAuth, TwigContext, User,
    permissions::{self, NamespacePermission, PermissionSet},
};
use crate::config;
use crate::git::repo::bare_init;
pub mod bare;
pub mod repo;
pub mod reserved;

pub async fn git_handler(
    req: HttpRequest,
    body: web::Bytes,
    path: web::Path<(String, String, String)>, // (namespace,repo, endpoint)
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> HttpResponse {
    let (namespace, repo, endpoint) = path.into_inner();

    let clean_repo = if Path::new(server.project_root())
        .join(&namespace)
        .join(&repo)
        .exists()
    {
        repo.clone()
    } else if let Some(stripped) = repo.strip_suffix(".git") {
        if Path::new(server.project_root())
            .join(&namespace)
            .join(stripped)
            .exists()
        {
            stripped.to_string()
        } else {
            repo.clone()
        }
    } else {
        repo.clone()
    };

    let method = req.method().as_str();
    let query = req.query_string();
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let path_info = format!("/{clean_repo}/{endpoint}");

    let git_protocol = req
        .headers()
        .get("Git-Protocol")
        .and_then(|value| value.to_str().ok());
    let git_req = twig_git::backend::GitRequest::new(method, path_info, query, content_type)
        .with_git_protocol(git_protocol);
    let kind = git_req.kind();

    let authenticated_user =
        match authenticate_git_request(&req, &server, &auth_state, &namespace, &clean_repo, &kind)
            .await
        {
            Ok(authenticated_user) => authenticated_user,
            Err(response) => return response,
        };
    let username = authenticated_user
        .as_ref()
        .map(|authenticated_user| authenticated_user.username.clone());
    let namespace_role = authenticated_user
        .as_ref()
        .map(|authenticated_user| authenticated_user.namespace_role);

    let git_backend_config = twig_git::backend::Config::new(server.project_root());
    let namespace = namespace.clone();
    let body_bytes = body.to_vec();
    let git_req = git_req.clone();
    let result = web::block(move || {
        twig_git::backend::run_with_config(
            &git_backend_config,
            &namespace,
            &git_req,
            body_bytes,
            username.as_deref(),
            namespace_role.map(crate::auth::NamespaceRole::as_str),
        )
    })
    .await;

    match result {
        Ok(Ok((headers, response_body))) => build_cgi_response(&headers, response_body),
        Ok(Err(e)) => {
            log::error!("Git HTTP backend failed: {e}");
            HttpResponse::InternalServerError().body(e)
        }
        Err(e) => {
            log::error!("Git HTTP backend blocking task failed: {e}");
            HttpResponse::InternalServerError().body("blocking task failed")
        }
    }
}

fn build_cgi_response(headers: &str, body: Vec<u8>) -> HttpResponse {
    let mut response = HttpResponse::Ok();
    for line in headers.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if key.eq_ignore_ascii_case("Status") {
                let status = value
                    .split_whitespace()
                    .next()
                    .and_then(|code| code.parse::<u16>().ok())
                    .unwrap_or(200);
                response.status(
                    actix_web::http::StatusCode::from_u16(status)
                        .unwrap_or(actix_web::http::StatusCode::OK),
                );
            } else {
                response.insert_header((key.to_owned(), value.to_owned()));
            }
        }
    }
    response.body(body)
}

async fn authenticate_git_request(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
    repo: &str,
    kind: &twig_git::backend::GitRequestKind,
) -> Result<Option<AuthenticatedGitUser>, HttpResponse> {
    let is_write = matches!(
        kind,
        twig_git::backend::GitRequestKind::Push
            | twig_git::backend::GitRequestKind::AdvertiseRefs(
                twig_git::backend::GitService::WriteRef
            )
    );

    if is_write {
        match is_authenticated(req, server, auth_state, namespace).await {
            Ok(Some(auth_result)) => {
                if !auth_result.namespace_exists
                    && let Err(e) =
                        ensure_namespace_exists(auth_state, &auth_result.user, namespace).await
                {
                    log::error!("Failed to ensure namespace exists: {e}");
                    return Err(
                        HttpResponse::InternalServerError().body("Failed to create namespace")
                    );
                }
                let repo_exists = Path::new(server.project_root())
                    .join(namespace)
                    .join(repo)
                    .exists();
                let required_permission = if repo_exists {
                    NamespacePermission::PushChanges
                } else {
                    NamespacePermission::CreateRepositories
                };
                if !auth_result.permissions.contains(required_permission) {
                    if required_permission == NamespacePermission::CreateRepositories {
                        return Err(HttpResponse::Forbidden()
                            .body("Only namespace owners can create repositories"));
                    }
                    return Err(
                        HttpResponse::Forbidden().body("Push permission required for namespace")
                    );
                }
                if let Err(e) = ensure_repo_exists(server.project_root(), namespace, repo) {
                    log::error!("Failed to ensure repo exists: {e}");
                    return Err(
                        HttpResponse::InternalServerError().body("Failed to create repository")
                    );
                }
                Ok(Some(AuthenticatedGitUser {
                    username: auth_result.user.username,
                    namespace_role: if auth_result
                        .permissions
                        .contains(NamespacePermission::RewriteProtectedBranches)
                    {
                        crate::auth::NamespaceRole::Owner
                    } else {
                        crate::auth::NamespaceRole::Contributor
                    },
                }))
            }
            Ok(None) => Err(HttpResponse::Forbidden().body("Access denied to namespace")),
            Err(response) => Err(response),
        }
    } else if bare::is_repo_private(server.project_root(), namespace, repo) {
        match is_authenticated(req, server, auth_state, namespace).await {
            Ok(Some(auth_result)) => {
                if !auth_result.namespace_exists {
                    return Err(HttpResponse::NotFound().body("Repository not found"));
                }
                if !auth_result
                    .permissions
                    .contains(NamespacePermission::ViewNamespace)
                {
                    return Err(HttpResponse::Forbidden().body("Read permission required"));
                }
                Ok(Some(AuthenticatedGitUser {
                    username: auth_result.user.username,
                    namespace_role: if auth_result
                        .permissions
                        .contains(NamespacePermission::RewriteProtectedBranches)
                    {
                        crate::auth::NamespaceRole::Owner
                    } else {
                        crate::auth::NamespaceRole::Contributor
                    },
                }))
            }
            Ok(None) => Err(HttpResponse::Forbidden().body("Access denied to namespace")),
            Err(response) => Err(response),
        }
    } else {
        Ok(None)
    }
}

struct AuthenticatedGitUser {
    username: String,
    namespace_role: crate::auth::NamespaceRole,
}

struct AuthResult {
    user: User,
    namespace_exists: bool,
    permissions: PermissionSet,
}

async fn is_authenticated(
    req: &actix_web::HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace_name: &str,
) -> Result<Option<AuthResult>, HttpResponse> {
    let principal = authenticate_git_user(req, auth_state, namespace_name).await?;
    let username = principal.user.username.clone();
    let db = auth_state.db();

    // Resolve effective permissions. Unknown namespaces can only be
    // bootstrapped by users who still have namespace-creation permission.
    log::debug!(
        "Checking namespace access: user_id='{}' namespace='{}'",
        principal.user.id,
        namespace_name
    );
    match permissions::for_principal(
        db,
        &principal,
        namespace_name,
        server.is_configured_admin(&principal.user.username),
    )
    .await
    {
        Ok(Some(permissions)) => {
            log::info!(
                "Git auth success: user='{username}' has access to namespace='{namespace_name}'"
            );
            Ok(Some(AuthResult {
                user: principal.user,
                namespace_exists: true,
                permissions,
            }))
        }
        Ok(None) => match db.get_namespace_by_name(namespace_name).await {
            Ok(Some(_)) => {
                log::warn!(
                    "Git auth failed: user='{}' (id='{}') does NOT have access to namespace='{}'",
                    username,
                    principal.user.id,
                    namespace_name
                );
                Ok(None)
            }
            Ok(None) if principal.token_permissions.is_some() => Ok(None),
            Ok(None) => match db.user_can_create_namespace(&principal.user.id).await {
                Ok(true) => {
                    log::info!(
                        "Git auth success: user='{username}' can create namespace='{namespace_name}' (doesn't exist)"
                    );
                    Ok(Some(AuthResult {
                        user: principal.user,
                        namespace_exists: false,
                        permissions: PermissionSet::from_role(crate::auth::NamespaceRole::Owner),
                    }))
                }
                Ok(false) => {
                    log::warn!("Git auth denied namespace creation for contributor '{username}'");
                    Ok(None)
                }
                Err(error) => {
                    log::error!(
                        "Database error checking namespace creation for '{username}': {error}"
                    );
                    Err(HttpResponse::InternalServerError().body("Database error"))
                }
            },
            Err(error) => {
                log::error!(
                    "Database error checking namespace existence for user '{username}': {error}"
                );
                Err(HttpResponse::InternalServerError().body("Database error"))
            }
        },
        Err(error) => {
            log::error!("Database error checking namespace access for user '{username}': {error}");
            Err(HttpResponse::InternalServerError().body("Database error"))
        }
    }
}

async fn authenticate_git_user(
    req: &HttpRequest,
    auth_state: &web::Data<TwigContext>,
    namespace_name: &str,
) -> Result<AuthenticatedPrincipal, HttpResponse> {
    match auth_state.authenticate_request(req).await {
        Ok(CredentialAuth::Authenticated(principal)) => Ok(principal),
        Ok(CredentialAuth::Missing) => {
            log::warn!("Git auth failed: No credentials for namespace '{namespace_name}'");
            Err(HttpResponse::Unauthorized()
                .insert_header(("WWW-Authenticate", "Basic realm=\"git\""))
                .body("Missing credentials"))
        }
        Ok(CredentialAuth::Invalid) => {
            log::warn!("Git auth failed: Invalid credentials for namespace '{namespace_name}'");
            Err(HttpResponse::Unauthorized().body("Invalid credentials"))
        }
        Err(error) => {
            log::error!("Git authentication failed for namespace '{namespace_name}': {error}");
            Err(HttpResponse::InternalServerError().body("Authentication error"))
        }
    }
}

/// Ensures a namespace exists in the database, creating it if necessary.
async fn ensure_namespace_exists(
    auth_state: &web::Data<TwigContext>,
    user: &User,
    namespace_name: &str,
) -> Result<(), String> {
    let db = auth_state.db();

    log::info!(
        "Auto-creating namespace '{}' for user '{}'",
        namespace_name,
        user.username
    );
    let namespace = crate::auth::create_namespace(namespace_name.to_string(), user.id.clone());
    db.create_namespace(&namespace)
        .await
        .map_err(|e| format!("Failed to create namespace: {e}"))?;

    Ok(())
}

/// Ensures a bare repository exists on disk, creating it if necessary
fn ensure_repo_exists(project_root: &str, namespace: &str, repo_name: &str) -> Result<(), String> {
    // User-created repositories must not shadow namespace-level UI routes.
    if crate::git::reserved::is_reserved_repo_name(repo_name) {
        return Err(format!("'{repo_name}' is a reserved repository name"));
    }

    let repo_path = Path::new(project_root).join(namespace).join(repo_name);

    if repo_path.exists() {
        return twig_git::repo::install_pre_receive_hook(&repo_path);
    }

    // Create namespace directory if needed
    let ns_path = Path::new(project_root).join(namespace);
    if !ns_path.exists() {
        std::fs::create_dir_all(&ns_path)
            .map_err(|e| format!("Failed to create namespace directory: {e}"))?;
    }

    // Create the bare repository
    log::info!("Auto-creating repository '{namespace}/{repo_name}'");

    std::fs::create_dir_all(&repo_path)
        .map_err(|e| format!("Failed to create repo directory: {e}"))?;

    bare_init(&repo_path, "main", "Twig", "twig@localhost")
        .map_err(|e| format!("Failed to initialize bare repo: {e}"))?;

    twig_git::repo::install_pre_receive_hook(&repo_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cgi_response_keeps_status_and_headers() {
        let response = build_cgi_response(
            "Status: 401 Unauthorized\r\nContent-Type: text/plain",
            b"Unauthorized".to_vec(),
        );
        assert_eq!(response.status(), actix_web::http::StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/plain"
        );
    }
}
