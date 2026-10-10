use std::path::Path;

use actix_web::{HttpRequest, HttpResponse, web};

use crate::auth::{NamespaceRole, TwigContext, User, extract_basic_auth, verify_password};
use crate::config;
use crate::git::repo::bare_init;
pub mod backend;
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

    let git_backend_config = crate::git::backend::Config::new(server.project_root());

    let method = req.method().as_str();
    let query = req.query_string();
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let path_info = format!("/{clean_repo}/{endpoint}");

    let git_req = crate::git::backend::GitRequest::new(method, path_info, query, content_type);

    let kind = git_req.kind();

    let authenticated_user = match authenticate_git_request(
        &req,
        &server,
        &auth_state,
        &namespace,
        &repo,
        &kind,
    )
    .await
    {
        Ok(authenticated_user) => authenticated_user,
        Err(response) => return response,
    };
    let username = authenticated_user
        .as_ref()
        .map(|authenticated_user| authenticated_user.username.clone());
    let namespace_role = authenticated_user.map(|authenticated_user| authenticated_user.role);

    // Run in blocking thread — xshell/process::Command is blocking
    let req = git_req.clone();
    let body_bytes = body.to_vec();
    let namespace_clone = namespace.clone();
    let result = web::block(move || {
        crate::git::backend::run_with_config(
            &git_backend_config,
            &namespace_clone,
            &req,
            body_bytes,
            username.as_deref(),
            namespace_role,
        )
    })
    .await;

    match result {
        Ok(Ok(cgi_output)) => {
            let (headers, body) = cgi_output;
            build_response(&headers, body)
        }
        Ok(Err(e)) => {
            log::error!("{e:?}");
            actix_web::HttpResponse::InternalServerError().body(e)
        }
        Err(e) => {
            log::error!("{e:?}");
            actix_web::HttpResponse::InternalServerError().body("blocking task failed")
        }
    }
}

// Parse CGI headers into an actix HttpResponse
fn build_response(headers: &str, body: Vec<u8>) -> actix_web::HttpResponse {
    let mut response = actix_web::HttpResponse::Ok();
    for line in headers.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            // Pick up the status line if git sets it (e.g. "Status: 401 Unauthorized")
            if key.eq_ignore_ascii_case("Status") {
                let code = value
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<u16>().ok())
                    .unwrap_or(200);
                response.status(
                    actix_web::http::StatusCode::from_u16(code)
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
    kind: &crate::git::backend::GitRequestKind,
) -> Result<Option<AuthenticatedGitUser>, HttpResponse> {
    let is_write = matches!(
        kind,
        crate::git::backend::GitRequestKind::Push
            | crate::git::backend::GitRequestKind::AdvertiseRefs(
                crate::git::backend::GitService::WriteRef,
            )
    );

    if is_write {
        match is_authenticated(req, auth_state, namespace).await {
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
                if !repo_exists && auth_result.role != NamespaceRole::Owner {
                    return Err(HttpResponse::Forbidden()
                        .body("Only namespace owners can create repositories"));
                }
                if let Err(e) = ensure_repo_exists(server.project_root(), namespace, repo) {
                    log::error!("Failed to ensure repo exists: {e}");
                    return Err(
                        HttpResponse::InternalServerError().body("Failed to create repository")
                    );
                }
                Ok(Some(AuthenticatedGitUser {
                    username: auth_result.user.username,
                    role: auth_result.role,
                }))
            }
            Ok(None) => Err(HttpResponse::Forbidden().body("Access denied to namespace")),
            Err(response) => Err(response),
        }
    } else if bare::is_repo_private(server.project_root(), namespace, repo) {
        match is_authenticated(req, auth_state, namespace).await {
            Ok(Some(auth_result)) => {
                if !auth_result.namespace_exists {
                    return Err(HttpResponse::NotFound().body("Repository not found"));
                }
                Ok(Some(AuthenticatedGitUser {
                    username: auth_result.user.username,
                    role: auth_result.role,
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
    role: NamespaceRole,
}

struct AuthResult {
    user: User,
    namespace_exists: bool,
    role: NamespaceRole,
}

async fn is_authenticated(
    req: &actix_web::HttpRequest,
    auth_state: &web::Data<TwigContext>,
    namespace_name: &str,
) -> Result<Option<AuthResult>, HttpResponse> {
    // Extract basic auth credentials
    let Some((username, password)) = extract_basic_auth(req) else {
        log::warn!("Git auth failed: No basic auth credentials for namespace '{namespace_name}'");
        return Err(actix_web::HttpResponse::Unauthorized()
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
            return Err(actix_web::HttpResponse::Unauthorized().body("Invalid credentials"));
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
            return Err(actix_web::HttpResponse::Unauthorized().body("Invalid credentials"));
        }
        Err(e) => {
            log::error!("Password verification error for user '{username}': {e}");
            return Err(actix_web::HttpResponse::InternalServerError().body("Authentication error"));
        }
    }

    // Resolve the user's namespace role. Unknown namespaces can only be
    // bootstrapped by users who still have namespace-creation permission.
    log::debug!(
        "Checking namespace access: user_id='{}' namespace='{}'",
        user.id,
        namespace_name
    );
    match db.get_namespace_role(&user.id, namespace_name).await {
        Ok(Some(role)) => {
            log::info!(
                "Git auth success: user='{username}' has access to namespace='{namespace_name}'"
            );
            Ok(Some(AuthResult {
                user,
                namespace_exists: true,
                role,
            }))
        }
        Ok(None) => {
            // Check if namespace exists at all
            match db.get_namespace_by_name(namespace_name).await {
                Ok(Some(_)) => {
                    // Namespace exists but user doesn't have access
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
                            role: NamespaceRole::Owner,
                        }))
                    }
                    Ok(false) => {
                        log::warn!(
                            "Git auth denied namespace creation for contributor '{username}'"
                        );
                        Ok(None)
                    }
                    Err(error) => {
                        log::error!(
                            "Database error checking namespace creation for '{username}': {error}"
                        );
                        Err(HttpResponse::InternalServerError().body("Database error"))
                    }
                },
                Err(e) => {
                    log::error!(
                        "Database error checking namespace existence for user '{username}': {e}"
                    );
                    Err(actix_web::HttpResponse::InternalServerError().body("Database error"))
                }
            }
        }
        Err(e) => {
            log::error!("Database error checking namespace access for user '{username}': {e}");
            Err(actix_web::HttpResponse::InternalServerError().body("Database error"))
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
        // Install/update policy hooks on repositories created by older Twig versions.
        return repo::install_pre_receive_hook(&repo_path);
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
    fn test_build_response_status_header() {
        let headers = "Content-Type: text/html\r\nStatus: 401 Unauthorized".to_string();
        let body = b"Unauthorized".to_vec();
        let response = build_response(&headers, body);
        assert_eq!(response.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_build_response_default_status() {
        let headers = "Content-Type: text/html".to_string();
        let body = b"OK".to_vec();
        let response = build_response(&headers, body);
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
    }

    #[test]
    fn test_build_response_content_type() {
        let headers = "Content-Type: application/git-upload-pack-advertisement".to_string();
        let body = b"data".to_vec();
        let response = build_response(&headers, body);
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
    }

    #[test]
    fn test_build_response_multiple_headers() {
        let headers = "Content-Type: text/plain\r\nCache-Control: no-cache".to_string();
        let body = b"test".to_vec();
        let response = build_response(&headers, body);
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
    }
}
