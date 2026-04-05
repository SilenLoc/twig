use actix_web::{HttpRequest, HttpResponse, web};
use log::info;

use crate::auth::{AuthState, extract_basic_auth, verify_password};
use crate::config;
pub mod bare;
pub mod repo;

pub async fn git_handler(
    req: HttpRequest,
    body: web::Bytes,
    path: web::Path<(String, String, String)>, // (namespace,repo, endpoint)
    server: web::Data<config::Server>,
    auth_state: web::Data<AuthState>,
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
    match kind {
        crate::git_backend::GitRequestKind::Push
        | crate::git_backend::GitRequestKind::AdvertiseRefs(
            crate::git_backend::GitService::WriteRef,
        ) => match is_authenticated(&req, &auth_state, &namespace).await {
            Ok(true) => {}
            Ok(false) => {
                return actix_web::HttpResponse::Forbidden().body("Access denied to namespace");
            }
            Err(response) => return response,
        },
        _ => {}
    }

    // Run in blocking thread — xshell/process::Command is blocking
    let req = git_req.clone();
    let body_bytes = body.to_vec();
    let namespace_clone = namespace.clone();
    let result = web::block(move || {
        crate::git_backend::run_with_config(&git_backend_config, &namespace_clone, &req, body_bytes)
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
                response.status(actix_web::http::StatusCode::from_u16(code).unwrap());
            } else {
                response.insert_header((key.to_owned(), value.to_owned()));
            }
        }
    }
    response.body(body)
}

async fn is_authenticated(
    req: &actix_web::HttpRequest,
    auth_state: &web::Data<AuthState>,
    namespace_name: &str,
) -> Result<bool, HttpResponse> {
    // Extract basic auth credentials
    let (username, password) = match extract_basic_auth(req) {
        Some(creds) => creds,
        None => {
            log::warn!("Git auth failed: No basic auth credentials for namespace '{}'", namespace_name);
            return Err(actix_web::HttpResponse::Unauthorized()
                .insert_header(("WWW-Authenticate", "Basic realm=\"git\""))
                .body("Missing credentials"));
        }
    };

    log::debug!("Git auth attempt: user='{}' namespace='{}'", username, namespace_name);

    // Get user from database
    let user = match auth_state.db.get_user_by_username(&username).await {
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

    // Check if user has access to namespace
    match auth_state
        .db
        .user_has_namespace_access(&user.id, namespace_name)
        .await
    {
        Ok(true) => {
            log::info!("Git auth success: user='{}' has access to namespace='{}'", username, namespace_name);
            Ok(true)
        }
        Ok(false) => {
            log::warn!("Git auth failed: user='{}' does NOT have access to namespace='{}' (user_id='{}')", 
                username, namespace_name, user.id);
            Ok(false)
        }
        Err(e) => {
            log::error!("Database error checking namespace access for user '{}': {}", username, e);
            Err(actix_web::HttpResponse::InternalServerError().body("Database error"))
        }
    }
}
