use actix_web::{HttpRequest, HttpResponse, get, put, web};
use futures_util::StreamExt;
use semver::Version;
use serde::Serialize;

use crate::{
    auth::{NamespacePermission, TwigContext},
    config,
    db::binaries::{BinaryBlob, PutBinaryOutcome},
    git::bare::RepoHandle,
};

use super::{
    ApiError, BinaryPath, RepositoryPath, authenticate, json_error, require_namespace_permission,
};

const MAX_BINARY_BYTES: usize = 1 << 29;

#[derive(Debug, Serialize)]
struct BinaryReleaseResponse {
    version: String,
    assets: Vec<BinaryAssetResponse>,
}

#[derive(Debug, Serialize)]
struct BinaryAssetResponse {
    filename: String,
    size_bytes: u64,
}

#[derive(Debug, Serialize)]
struct BinaryUploadResponse {
    namespace: String,
    repository: String,
    version: String,
    filename: String,
    outcome: &'static str,
}

async fn open_authorized_repo(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    namespace: &str,
    repo: &str,
    permission: NamespacePermission,
) -> Result<RepoHandle, HttpResponse> {
    let principal = authenticate(req, auth_state).await.map_err(json_error)?;
    super::validate_namespace_name(namespace).map_err(json_error)?;
    super::validate_repository_name(repo).map_err(json_error)?;
    require_namespace_permission(&principal, server, auth_state, namespace, permission)
        .await
        .map_err(json_error)?;
    RepoHandle::open(server.project_root(), namespace, repo)
        .map_err(|_| json_error(ApiError::NotFound))
}

#[get("/api/v1/namespaces/{namespace}/repositories/{repo}/binaries")]
pub async fn list_binaries(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepositoryPath>,
) -> HttpResponse {
    if let Err(response) = open_authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        return response;
    }
    match auth_state
        .db()
        .list_binary_releases(&path.namespace, &path.repo)
        .await
    {
        Ok(releases) => HttpResponse::Ok().json(
            releases
                .into_iter()
                .map(|release| BinaryReleaseResponse {
                    version: release.version,
                    assets: release
                        .assets
                        .into_iter()
                        .map(|asset| BinaryAssetResponse {
                            filename: asset.filename,
                            size_bytes: asset.size_bytes,
                        })
                        .collect(),
                })
                .collect::<Vec<_>>(),
        ),
        Err(error) => {
            log::error!("Failed to list API binary releases: {error}");
            json_error(ApiError::Internal)
        }
    }
}

#[get("/api/v1/namespaces/{namespace}/repositories/{repo}/binaries/{version}/{filename}")]
pub async fn get_binary(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<BinaryPath>,
) -> HttpResponse {
    if let Err(response) = open_authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        return response;
    }
    if !is_safe_filename(&path.filename) {
        return json_error(ApiError::BadRequest("Invalid binary filename".to_string()));
    }
    let binary = if path.version == "latest" {
        auth_state
            .db()
            .get_latest_binary(&path.namespace, &path.repo, &path.filename)
            .await
    } else {
        let version = match parse_version(&path.version) {
            Ok(version) => version,
            Err(error) => return json_error(error),
        };
        auth_state
            .db()
            .get_binary(&path.namespace, &path.repo, &version, &path.filename)
            .await
    };
    match binary {
        Ok(Some(binary)) => binary_response(binary),
        Ok(None) => json_error(ApiError::NotFound),
        Err(error) => {
            log::error!("Failed to download API binary: {error}");
            json_error(ApiError::Internal)
        }
    }
}

#[put("/api/v1/namespaces/{namespace}/repositories/{repo}/binaries/{version}/{filename}")]
pub async fn put_binary(
    req: HttpRequest,
    mut body: web::Payload,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<BinaryPath>,
) -> HttpResponse {
    if let Err(response) = open_authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        return response;
    }
    if path.version == "latest" {
        return json_error(ApiError::BadRequest(
            "'latest' is reserved for downloads".to_string(),
        ));
    }
    let version = match parse_version(&path.version) {
        Ok(version) => version,
        Err(error) => return json_error(error),
    };
    if !is_safe_filename(&path.filename) {
        return json_error(ApiError::BadRequest("Invalid binary filename".to_string()));
    }
    let mut bytes = web::BytesMut::new();
    while let Some(chunk) = body.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                log::warn!("Failed to read API binary upload body: {error}");
                return json_error(ApiError::BadRequest(
                    "Could not read binary upload body".to_string(),
                ));
            }
        };
        if bytes.len().saturating_add(chunk.len()) > MAX_BINARY_BYTES {
            return json_error(ApiError::PayloadTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    match auth_state
        .db()
        .put_binary(
            &path.namespace,
            &path.repo,
            &version,
            &path.filename,
            bytes.freeze(),
        )
        .await
    {
        Ok(PutBinaryOutcome::Stored) => HttpResponse::Created().json(BinaryUploadResponse {
            namespace: path.namespace.clone(),
            repository: path.repo.clone(),
            version: version.to_string(),
            filename: path.filename.clone(),
            outcome: "stored",
        }),
        Ok(PutBinaryOutcome::Replaced) => HttpResponse::Ok().json(BinaryUploadResponse {
            namespace: path.namespace.clone(),
            repository: path.repo.clone(),
            version: version.to_string(),
            filename: path.filename.clone(),
            outcome: "replaced",
        }),
        Ok(PutBinaryOutcome::TooOld) => json_error(ApiError::Conflict(
            "Version is older than the three retained releases".to_string(),
        )),
        Err(error) => {
            log::error!("Failed to store API binary: {error}");
            json_error(ApiError::Internal)
        }
    }
}

fn parse_version(input: &str) -> Result<Version, ApiError> {
    Version::parse(input.strip_prefix('v').unwrap_or(input))
        .map_err(|_| ApiError::BadRequest("Invalid binary version".to_string()))
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
