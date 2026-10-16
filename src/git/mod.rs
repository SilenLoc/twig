use std::path::Path;

use actix_web::{HttpRequest, HttpResponse, web};

use crate::auth::{
    TwigContext, User, extract_basic_auth,
    permissions::{self, NamespacePermission, PermissionSet},
    verify_password,
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

    let kind = twig_http::classify_request(method, &path_info, query);

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
    let may_rewrite_protected_branches = authenticated_user
        .is_some_and(|authenticated_user| authenticated_user.may_rewrite_protected_branches);

    let project_root = server.project_root().to_owned();
    let namespace = namespace.clone();
    let method = method.to_owned();
    let path_info = path_info.clone();
    let query_string = query.to_owned();
    let content_type = content_type.to_owned();
    let body_bytes = body.to_vec();
    let username = username.clone();
    let receive_hooks = server.git_receive_hooks();
    let result = web::block(move || {
        let config = twig_http::Config::new(project_root);
        let request = twig_http::Request {
            method: &method,
            path_info: &path_info,
            query_string: &query_string,
            content_type: (!content_type.is_empty()).then_some(content_type.as_str()),
            body: &body_bytes,
            authenticated_user: username.as_deref(),
            may_rewrite_protected_branches,
            receive_hooks: &receive_hooks,
        };
        twig_http::handle(&config, &namespace, request)
    })
    .await;

    match result {
        Ok(Ok(response)) => build_library_response(response),
        Ok(Err(twig_http::Error::UnsupportedRequest)) => HttpResponse::NotFound().finish(),
        Ok(Err(e)) => {
            log::error!("Git HTTP library failed: {e}");
            HttpResponse::InternalServerError().body(e.to_string())
        }
        Err(e) => {
            log::error!("Git HTTP library blocking task failed: {e}");
            HttpResponse::InternalServerError().body("blocking task failed")
        }
    }
}

fn build_library_response(response: twig_http::Response) -> HttpResponse {
    let status = actix_web::http::StatusCode::from_u16(response.status)
        .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = HttpResponse::build(status);
    for (key, value) in response.headers {
        builder.insert_header((key, value));
    }
    builder.body(response.body)
}

async fn authenticate_git_request(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
    repo: &str,
    kind: &twig_http::RequestKind,
) -> Result<Option<AuthenticatedGitUser>, HttpResponse> {
    let is_write = matches!(
        kind,
        twig_http::RequestKind::ReceivePack
            | twig_http::RequestKind::AdvertiseRefs(twig_http::Service::ReceivePack)
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
                    may_rewrite_protected_branches: auth_result
                        .permissions
                        .contains(NamespacePermission::RewriteProtectedBranches),
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
                    may_rewrite_protected_branches: auth_result
                        .permissions
                        .contains(NamespacePermission::RewriteProtectedBranches),
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
    may_rewrite_protected_branches: bool,
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
    let user = authenticate_git_user(req, auth_state, namespace_name).await?;
    let username = user.username.clone();
    let db = auth_state.db();

    // Resolve effective permissions. Unknown namespaces can only be
    // bootstrapped by users who still have namespace-creation permission.
    log::debug!(
        "Checking namespace access: user_id='{}' namespace='{}'",
        user.id,
        namespace_name
    );
    match permissions::for_user(
        db,
        &user.id,
        namespace_name,
        server.is_configured_admin(&user.username),
    )
    .await
    {
        Ok(Some(permissions)) => {
            log::info!(
                "Git auth success: user='{username}' has access to namespace='{namespace_name}'"
            );
            Ok(Some(AuthResult {
                user,
                namespace_exists: true,
                permissions,
            }))
        }
        Ok(None) => match db.get_namespace_by_name(namespace_name).await {
            Ok(Some(_)) => {
                log::warn!(
                    "Git auth failed: user='{}' (id='{}') does NOT have access to namespace='{}'",
                    username,
                    user.id,
                    namespace_name
                );
                Ok(None)
            }
            Ok(None) => match db.user_can_create_namespace(&user.id).await {
                Ok(true) => {
                    log::info!(
                        "Git auth success: user='{username}' can create namespace='{namespace_name}' (doesn't exist)"
                    );
                    Ok(Some(AuthResult {
                        user,
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
) -> Result<User, HttpResponse> {
    let Some((username, password)) = extract_basic_auth(req) else {
        log::warn!("Git auth failed: No basic auth credentials for namespace '{namespace_name}'");
        return Err(HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", "Basic realm=\"git\""))
            .body("Missing credentials"));
    };

    log::debug!("Git auth attempt: user='{username}' namespace='{namespace_name}'");

    let db = auth_state.db();

    // Get user from database
    let user = match db.get_user_by_username(&username).await {
        Ok(Some(user)) => {
            log::debug!("Git auth: Found user '{}' with id '{}'", username, user.id);
            user
        }
        Ok(None) => {
            log::warn!("Git auth failed: User '{username}' not found in database");
            return Err(HttpResponse::Unauthorized().body("Invalid credentials"));
        }
        Err(e) => {
            log::error!("Database error looking up user '{username}': {e}");
            return Err(actix_web::HttpResponse::InternalServerError().body("Database error"));
        }
    };

    // Verify password
    match verify_password(&password, &user.password_hash) {
        Ok(true) => {
            log::debug!("Git auth: Password verified for user '{username}'");
        }
        Ok(false) => {
            log::warn!("Git auth failed: Invalid password for user '{username}'");
            return Err(HttpResponse::Unauthorized().body("Invalid credentials"));
        }
        Err(e) => {
            log::error!("Password verification error for user '{username}': {e}");
            return Err(HttpResponse::InternalServerError().body("Authentication error"));
        }
    }
    Ok(user)
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
        return Ok(());
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

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_response_keeps_status_and_headers() {
        let response = build_library_response(twig_http::Response {
            status: 401,
            headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
            body: b"Unauthorized".to_vec(),
        });
        assert_eq!(response.status(), actix_web::http::StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/plain"
        );
    }
}
