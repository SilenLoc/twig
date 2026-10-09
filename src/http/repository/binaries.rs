use actix_web::{HttpRequest, HttpResponse, get, put, web};
use semver::Version;
use serde::Deserialize;

use crate::{
    auth::{TwigContext, extract_basic_auth, verify_password},
    config,
    db::binaries::{BinaryBlob, PutBinaryOutcome},
    git::bare::RepoHandle,
    http::auth::session::get_username_from_request,
};

/// Match the Actix request payload ceiling configured in `main`.
const MAX_BINARY_BYTES: usize = 1 << 29;

#[derive(Deserialize)]
struct BinaryParams {
    namespace: String,
    repo: String,
    version: String,
    filename: String,
}

#[put("/{namespace}/{repo}/binaries/{version}/{filename}")]
pub async fn upload_binary(
    req: HttpRequest,
    body: web::Bytes,
    params: web::Path<BinaryParams>,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> HttpResponse {
    if !is_binary_size_allowed(body.len()) {
        return HttpResponse::build(actix_web::http::StatusCode::PAYLOAD_TOO_LARGE)
            .body("Binary exceeds the 512 MiB upload limit");
    }
    if params.version == "latest" {
        return HttpResponse::BadRequest().body("'latest' is reserved for downloads");
    }
    let Ok(version) = parse_release_version(&params.version) else {
        return HttpResponse::BadRequest().body("Invalid binary version");
    };
    if !is_safe_filename(&params.filename) {
        return HttpResponse::BadRequest().body("Invalid binary filename");
    }

    if RepoHandle::open(server.project_root(), &params.namespace, &params.repo).is_err() {
        return HttpResponse::NotFound().finish();
    }

    if let Err(response) = require_namespace_writer(&req, &auth_state, &params.namespace).await {
        return response;
    }

    match auth_state
        .db()
        .put_binary(
            &params.namespace,
            &params.repo,
            &version,
            &params.filename,
            &body,
        )
        .await
    {
        Ok(PutBinaryOutcome::Stored) => HttpResponse::Created()
            .insert_header((
                "Location",
                format!(
                    "/{}/{}/binaries/{}/{}",
                    params.namespace, params.repo, version, params.filename
                ),
            ))
            .finish(),
        Ok(PutBinaryOutcome::Replaced) => HttpResponse::Ok().finish(),
        Ok(PutBinaryOutcome::TooOld) => {
            HttpResponse::Conflict().body("Version is older than the three retained releases")
        }
        Err(error) => {
            log::error!(
                "Failed to store binary for {}/{}: {error}",
                params.namespace,
                params.repo
            );
            HttpResponse::InternalServerError().body("Failed to store binary")
        }
    }
}

#[get("/{namespace}/{repo}/binaries/{version}/{filename}")]
pub async fn download_binary(
    req: HttpRequest,
    params: web::Path<BinaryParams>,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> HttpResponse {
    let handle = match RepoHandle::open(server.project_root(), &params.namespace, &params.repo) {
        Ok(handle) => handle,
        Err(_) => return HttpResponse::NotFound().finish(),
    };

    if handle.load_config_with_raw().config.private
        && get_username_from_request(&req, &auth_state).await.is_none()
    {
        return HttpResponse::Unauthorized().finish();
    }

    if !is_safe_filename(&params.filename) {
        return HttpResponse::BadRequest().body("Invalid binary filename");
    }

    let binary = if params.version == "latest" {
        auth_state
            .db()
            .get_latest_binary(&params.namespace, &params.repo, &params.filename)
            .await
    } else {
        let Ok(version) = parse_release_version(&params.version) else {
            return HttpResponse::BadRequest().body("Invalid binary version");
        };
        auth_state
            .db()
            .get_binary(&params.namespace, &params.repo, &version, &params.filename)
            .await
    };

    match binary {
        Ok(Some(binary)) => binary_response(binary),
        Ok(None) => HttpResponse::NotFound().finish(),
        Err(error) => {
            log::error!(
                "Failed to read binary for {}/{}: {error}",
                params.namespace,
                params.repo
            );
            HttpResponse::InternalServerError().body("Failed to read binary")
        }
    }
}

async fn require_namespace_writer(
    req: &HttpRequest,
    auth_state: &TwigContext,
    namespace: &str,
) -> Result<(), HttpResponse> {
    let Some((username, password)) = extract_basic_auth(req) else {
        return Err(HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", "Basic realm=\"binary uploads\""))
            .body("Missing credentials"));
    };

    let user = match auth_state.db().get_user_by_username(&username).await {
        Ok(Some(user)) => user,
        Ok(None) => return Err(HttpResponse::Unauthorized().body("Invalid credentials")),
        Err(error) => {
            log::error!("Failed to look up binary uploader '{username}': {error}");
            return Err(HttpResponse::InternalServerError().body("Authentication failed"));
        }
    };

    match verify_password(&password, &user.password_hash) {
        Ok(true) => {}
        Ok(false) | Err(_) => return Err(HttpResponse::Unauthorized().body("Invalid credentials")),
    }

    match auth_state
        .db()
        .user_has_namespace_access(&user.id, namespace)
        .await
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(HttpResponse::Forbidden().body("Access denied to namespace")),
        Err(error) => {
            log::error!("Failed to check namespace access for binary upload: {error}");
            Err(HttpResponse::InternalServerError().body("Authorization failed"))
        }
    }
}

fn parse_release_version(input: &str) -> Result<Version, ()> {
    let version = input.strip_prefix('v').unwrap_or(input);
    Version::parse(version).map_err(|_| ())
}

fn is_safe_filename(filename: &str) -> bool {
    !filename.is_empty()
        && filename != "."
        && filename != ".."
        && filename.len() <= 255
        && filename.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | '+')
        })
}

fn is_binary_size_allowed(size_bytes: usize) -> bool {
    size_bytes <= MAX_BINARY_BYTES
}

fn binary_response(binary: BinaryBlob) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Content-Type", "application/octet-stream"))
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", binary.filename),
        ))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .body(binary.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_versions_accept_optional_v_and_reject_latest_for_upload() {
        assert_eq!(
            parse_release_version("v1.2.3").unwrap(),
            parse_release_version("1.2.3").unwrap()
        );
        assert!(parse_release_version("latest").is_err());
        assert!(parse_release_version("1.2").is_err());
    }

    #[test]
    fn binary_filename_and_size_validation_reject_unsafe_inputs() {
        assert!(is_safe_filename("twig-linux-x86_64.tar.gz"));
        assert!(!is_safe_filename("../outside"));
        assert!(!is_safe_filename("bad\r\nname"));
        assert!(!is_safe_filename("."));
        assert!(is_binary_size_allowed(MAX_BINARY_BYTES));
        assert!(!is_binary_size_allowed(MAX_BINARY_BYTES + 1));
    }
}
