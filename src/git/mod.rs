use std::path::Path;

use actix_web::{HttpRequest, HttpResponse, web};
use log::info;

use crate::auth::{FigContext, User, extract_basic_auth, verify_password};
use crate::config;
use crate::git::repo::bare_init;
pub mod bare;
pub mod repo;

pub async fn git_handler(
    req: HttpRequest,
    body: web::Bytes,
    path: web::Path<(String, String, String)>, // (namespace,repo, endpoint)
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
) -> HttpResponse {
    let (namespace, repo, endpoint) = path.into_inner();

    let git_backend_config = crate::git_backend::Config::new(server.project_root());

    let method = req.method().as_str();
    let query = req.query_string();
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let path_info = format!("/{repo}/{endpoint}");

    let git_req = crate::git_backend::GitRequest::new(method, path_info, query, content_type);

    let kind = git_req.kind();

    match kind.clone() {
        crate::git_backend::GitRequestKind::AdvertiseRefs(git_service) => {
            info!(
                "handling advertise refs {}: {} kind: {}",
                repo, endpoint, git_service
            );
        }
        crate::git_backend::GitRequestKind::FetchClone => {
            info!("handling fetch or clone {}: {}", repo, endpoint);
        }
        crate::git_backend::GitRequestKind::Push => {
            info!("handling push {}: {}", repo, endpoint);
        }
        crate::git_backend::GitRequestKind::DumbGet => {
            info!("handling dumb get {}: {}", repo, endpoint);
        }
    }

    // Auth gate for write operations
    let username = match &kind {
        crate::git_backend::GitRequestKind::Push
        | crate::git_backend::GitRequestKind::AdvertiseRefs(
            crate::git_backend::GitService::WriteRef,
        ) => match is_authenticated(&req, &auth_state, &namespace).await {
            Ok(Some(user)) => {
                // Auto-create namespace if it doesn't exist
                if let Err(e) = ensure_namespace_exists(&auth_state, &user, &namespace).await {
                    log::error!("Failed to ensure namespace exists: {}", e);
                    return actix_web::HttpResponse::InternalServerError()
                        .body("Failed to create namespace");
                }
                // Auto-create repo if it doesn't exist
                if let Err(e) = ensure_repo_exists(server.project_root(), &namespace, &repo).await {
                    log::error!("Failed to ensure repo exists: {}", e);
                    return actix_web::HttpResponse::InternalServerError()
                        .body("Failed to create repository");
                }
                Some(user.username.clone())
            }
            Ok(None) => {
                return actix_web::HttpResponse::Forbidden().body("Access denied to namespace");
            }
            Err(response) => return response,
        },
        _ => None,
    };

    // Run in blocking thread — xshell/process::Command is blocking
    let req = git_req.clone();
    let body_bytes = body.to_vec();
    let namespace_clone = namespace.clone();
    let result = web::block(move || {
        crate::git_backend::run_with_config(
            &git_backend_config,
            &namespace_clone,
            &req,
            body_bytes,
            username.as_deref(),
        )
    })
    .await;

    match result {
        Ok(Ok(cgi_output)) => {
            let (headers, body) = cgi_output;
            build_response(headers, body)
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
fn build_response(headers: String, body: Vec<u8>) -> actix_web::HttpResponse {
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

async fn is_authenticated(
    req: &actix_web::HttpRequest,
    auth_state: &web::Data<FigContext>,
    namespace_name: &str,
) -> Result<Option<User>, HttpResponse> {
    // Extract basic auth credentials
    let (username, password) = match extract_basic_auth(req) {
        Some(creds) => creds,
        None => {
            log::warn!(
                "Git auth failed: No basic auth credentials for namespace '{}'",
                namespace_name
            );
            return Err(actix_web::HttpResponse::Unauthorized()
                .insert_header(("WWW-Authenticate", "Basic realm=\"git\""))
                .body("Missing credentials"));
        }
    };

    log::debug!(
        "Git auth attempt: user='{}' namespace='{}'",
        username,
        namespace_name
    );

    let db = match auth_state.db().await {
        Ok(db) => db,
        Err(e) => {
            log::error!("Database connection error for user '{}': {}", username, e);
            return Err(actix_web::HttpResponse::InternalServerError().body("Database error"));
        }
    };

    // Get user from database
    let user = match db.get_user_by_username(&username).await {
        Ok(Some(user)) => {
            log::debug!("Git auth: Found user '{}' with id '{}'", username, user.id);
            user
        }
        Ok(None) => {
            log::warn!("Git auth failed: User '{}' not found in database", username);
            return Err(actix_web::HttpResponse::Unauthorized().body("Invalid credentials"));
        }
        Err(e) => {
            log::error!("Database error looking up user '{}': {}", username, e);
            return Err(actix_web::HttpResponse::InternalServerError().body("Database error"));
        }
    };

    // Verify password
    match verify_password(&password, &user.password_hash) {
        Ok(true) => {
            log::debug!("Git auth: Password verified for user '{}'", username);
        }
        Ok(false) => {
            log::warn!("Git auth failed: Invalid password for user '{}'", username);
            return Err(actix_web::HttpResponse::Unauthorized().body("Invalid credentials"));
        }
        Err(e) => {
            log::error!("Password verification error for user '{}': {}", username, e);
            return Err(actix_web::HttpResponse::InternalServerError().body("Authentication error"));
        }
    }

    // Check if user has access to namespace (or if namespace doesn't exist yet, allow creation)
    log::debug!(
        "Checking namespace access: user_id='{}' namespace='{}'",
        user.id,
        namespace_name
    );
    match db.user_has_namespace_access(&user.id, namespace_name).await {
        Ok(true) => {
            log::info!(
                "Git auth success: user='{}' has access to namespace='{}'",
                username,
                namespace_name
            );
            Ok(Some(user))
        }
        Ok(false) => {
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
                Ok(None) => {
                    // Namespace doesn't exist - allow auto-creation by returning the user
                    log::info!(
                        "Git auth success: user='{}' can create namespace='{}' (doesn't exist)",
                        username,
                        namespace_name
                    );
                    Ok(Some(user))
                }
                Err(e) => {
                    log::error!(
                        "Database error checking namespace existence for user '{}': {}",
                        username,
                        e
                    );
                    Err(actix_web::HttpResponse::InternalServerError().body("Database error"))
                }
            }
        }
        Err(e) => {
            log::error!(
                "Database error checking namespace access for user '{}': {}",
                username,
                e
            );
            Err(actix_web::HttpResponse::InternalServerError().body("Database error"))
        }
    }
}

/// Ensures a namespace exists in the database, creating it if necessary
async fn ensure_namespace_exists(
    auth_state: &web::Data<FigContext>,
    user: &User,
    namespace_name: &str,
) -> Result<(), String> {
    let db = auth_state.db().await?;

    // Check if namespace exists
    match db.get_namespace_by_name(namespace_name).await {
        Ok(Some(_)) => {
            // Namespace already exists
            Ok(())
        }
        Ok(None) => {
            // Create the namespace
            log::info!(
                "Auto-creating namespace '{}' for user '{}'",
                namespace_name,
                user.username
            );
            let namespace =
                crate::auth::create_namespace(namespace_name.to_string(), user.id.clone());
            db.create_namespace(&namespace)
                .await
                .map_err(|e| format!("Failed to create namespace: {}", e))
        }
        Err(e) => Err(format!("Database error checking namespace: {}", e)),
    }
}

/// Ensures a bare repository exists on disk, creating it if necessary
async fn ensure_repo_exists(
    project_root: &str,
    namespace: &str,
    repo_name: &str,
) -> Result<(), String> {
    let repo_path = Path::new(project_root).join(namespace).join(repo_name);

    if repo_path.exists() {
        // Repo already exists
        return Ok(());
    }

    // Create namespace directory if needed
    let ns_path = Path::new(project_root).join(namespace);
    if !ns_path.exists() {
        std::fs::create_dir_all(&ns_path)
            .map_err(|e| format!("Failed to create namespace directory: {}", e))?;
    }

    // Create the bare repository
    log::info!("Auto-creating repository '{}/{}'", namespace, repo_name);

    std::fs::create_dir_all(&repo_path)
        .map_err(|e| format!("Failed to create repo directory: {}", e))?;

    bare_init(&repo_path, "main", "Fig", "fig@localhost")
        .map_err(|e| format!("Failed to initialize bare repo: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_response_status_header() {
        let headers = "Content-Type: text/html\r\nStatus: 401 Unauthorized".to_string();
        let body = b"Unauthorized".to_vec();
        let response = build_response(headers, body);
        assert_eq!(response.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_build_response_default_status() {
        let headers = "Content-Type: text/html".to_string();
        let body = b"OK".to_vec();
        let response = build_response(headers, body);
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
    }

    #[test]
    fn test_build_response_content_type() {
        let headers = "Content-Type: application/git-upload-pack-advertisement".to_string();
        let body = b"data".to_vec();
        let response = build_response(headers, body);
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
    }

    #[test]
    fn test_build_response_multiple_headers() {
        let headers = "Content-Type: text/plain\r\nCache-Control: no-cache".to_string();
        let body = b"test".to_vec();
        let response = build_response(headers, body);
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
    }
}
