use actix_web::{HttpRequest, HttpResponse, get, put, web};
use semver::Version;
use serde::Deserialize;

use crate::{
    auth::{
        CredentialAuth, TwigContext,
        permissions::{self, NamespacePermission},
    },
    config,
    db::binaries::{BinaryBlob, PutBinaryOutcome},
    http::auth::session::get_username_from_request,
    http::body::{BodyClass, BodyReadError, RequestBodyLimits},
};
use twig_git::bare::RepoHandle;

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
    payload: web::Payload,
    params: web::Path<BinaryParams>,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    body_limits: web::Data<RequestBodyLimits>,
) -> HttpResponse {
    if params.version == "latest" {
        return HttpResponse::BadRequest().body("'latest' is reserved for downloads");
    }
    let Ok(version) = parse_release_version(&params.version) else {
        return HttpResponse::BadRequest().body("Invalid binary version");
    };
    if !is_safe_filename(&params.filename) {
        return HttpResponse::BadRequest().body("Invalid binary filename");
    }

    if let Err(response) =
        require_namespace_writer(&req, &server, &auth_state, &params.namespace).await
    {
        return response;
    }

    if RepoHandle::open(server.project_root(), &params.namespace, &params.repo).is_err() {
        return HttpResponse::NotFound().finish();
    }

    let max_bytes = body_limits.binary_upload_bytes();
    if RequestBodyLimits::declared_length_exceeds(&req, max_bytes) {
        return HttpResponse::PayloadTooLarge()
            .body(format!("Binary exceeds the {max_bytes}-byte upload limit"));
    }
    let mut body = match body_limits
        .read_payload(payload, BodyClass::Large, max_bytes)
        .await
    {
        Ok(body) => body,
        Err(BodyReadError::TooLarge) => {
            return HttpResponse::PayloadTooLarge()
                .body(format!("Binary exceeds the {max_bytes}-byte upload limit"));
        }
        Err(BodyReadError::Payload(error)) => {
            log::warn!("Failed to read binary upload body: {error}");
            return HttpResponse::BadRequest().body("Failed to read upload body");
        }
        Err(BodyReadError::CapacityClosed) => {
            return HttpResponse::ServiceUnavailable().body("Upload capacity is unavailable");
        }
    };

    match auth_state
        .db()
        .put_binary(
            &params.namespace,
            &params.repo,
            &version,
            &params.filename,
            std::mem::take(&mut body.bytes),
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
    let Ok(handle) = RepoHandle::open(server.project_root(), &params.namespace, &params.repo)
    else {
        return HttpResponse::NotFound().finish();
    };

    if handle.load_config_with_raw().config.private {
        if get_username_from_request(&req, &auth_state).await.is_none() {
            return HttpResponse::Unauthorized().finish();
        }
        let Some(user_id) = auth_state.user_id_from_request(&req).await else {
            return HttpResponse::Unauthorized().finish();
        };
        let user = match auth_state.db().get_user_by_id(&user_id).await {
            Ok(Some(user)) => user,
            Ok(None) => return HttpResponse::Unauthorized().finish(),
            Err(error) => {
                log::error!("Failed to load user for private binary download: {error}");
                return HttpResponse::InternalServerError().finish();
            }
        };
        match permissions::has(
            auth_state.db(),
            &user_id,
            &params.namespace,
            server.is_configured_admin(&user.username),
            NamespacePermission::ViewNamespace,
        )
        .await
        {
            Ok(true) => {}
            Ok(false) => return HttpResponse::Forbidden().finish(),
            Err(error) => {
                log::error!("Failed to check private binary download permission: {error}");
                return HttpResponse::InternalServerError().finish();
            }
        }
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
    server: &config::Server,
    auth_state: &TwigContext,
    namespace: &str,
) -> Result<(), HttpResponse> {
    let principal = match auth_state.authenticate_request(req).await {
        Ok(CredentialAuth::Authenticated(principal)) => principal,
        Ok(CredentialAuth::Missing) => {
            return Err(HttpResponse::Unauthorized()
                .insert_header(("WWW-Authenticate", "Basic realm=\"binary uploads\""))
                .body("Missing credentials"));
        }
        Ok(CredentialAuth::Invalid) => {
            return Err(HttpResponse::Unauthorized().body("Invalid credentials"));
        }
        Err(error) => {
            log::error!("Failed to authenticate binary uploader: {error}");
            return Err(HttpResponse::InternalServerError().body("Authentication failed"));
        }
    };

    match permissions::for_principal(
        auth_state.db(),
        &principal,
        namespace,
        server.is_configured_admin(&principal.user.username),
    )
    .await
    {
        Ok(Some(permissions)) if permissions.contains(NamespacePermission::PushChanges) => Ok(()),
        Ok(Some(_) | None) => {
            Err(HttpResponse::Forbidden().body("Push permission required for namespace"))
        }
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
        assert_eq!(RequestBodyLimits::default().binary_upload_bytes(), 1 << 29);
    }
}
