use actix_web::{HttpRequest, HttpResponse, web};
use log::info;

use crate::config;
pub mod bare;
pub mod repo;

pub async fn git_handler(
    req: HttpRequest,
    body: web::Bytes,
    path: web::Path<(String, String, String)>, // (namespace,repo, endpoint)
    server: web::Data<config::Server>,
) -> HttpResponse {
    let (namespace, repo, endpoint) = path.into_inner();

    let git_backend_config = git_backend::Config::new(server.project_root());

    let method = req.method().as_str();
    let query = req.query_string();
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let path_info = format!("/{repo}/{endpoint}");

    let git_req = git_backend::GitRequest::new(method, path_info, query, content_type);

    let kind = git_req.kind();

    match kind.clone() {
        git_backend::GitRequestKind::AdvertiseRefs(git_service) => {
            info!(
                "handling advertise refs {}: {} kind: {}",
                repo, endpoint, git_service
            );
        }
        git_backend::GitRequestKind::FetchClone => {
            info!("handling fetch or clone {}: {}", repo, endpoint);
        }
        git_backend::GitRequestKind::Push => {
            info!("handling push {}: {}", repo, endpoint);
        }
        git_backend::GitRequestKind::DumbGet => {
            info!("handling dumb get {}: {}", repo, endpoint);
        }
    }

    // Auth gate
    match kind {
        git_backend::GitRequestKind::Push
        | git_backend::GitRequestKind::AdvertiseRefs(git_backend::GitService::WriteRef) => {
            if !is_authenticated(&req) {
                return actix_web::HttpResponse::Unauthorized()
                    .insert_header(("WWW-Authenticate", "Basic realm=\"git\""))
                    .finish();
            }
        }
        _ => {}
    }

    // Run in blocking thread — xshell/process::Command is blocking
    let req = git_req.clone();
    let body_bytes = body.to_vec();
    let result = crate::web::block(move || {
        git_backend::run_with_config(&git_backend_config, &namespace, &req, body_bytes)
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

fn is_authenticated(_req: &actix_web::HttpRequest) -> bool {
    true
}
