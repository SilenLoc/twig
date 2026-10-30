use actix_web::{HttpRequest, HttpResponse, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::auth::{
    AuthenticatedPrincipal, CredentialAuth, NamespacePermission, PermissionSet, TwigContext,
    permissions,
};
use crate::config;

pub mod binaries;
pub mod contents;
pub mod namespaces;
pub mod repositories;

#[derive(Debug, Deserialize)]
pub struct NamespacePath {
    pub namespace: String,
}

#[derive(Debug, Deserialize)]
pub struct RepositoryPath {
    pub namespace: String,
    pub repo: String,
}

#[derive(Debug, Deserialize)]
pub struct BinaryPath {
    pub namespace: String,
    pub repo: String,
    pub version: String,
    pub filename: String,
}

#[derive(Debug, Deserialize)]
pub struct InvitationPath {
    pub namespace: String,
    pub id: String,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    error: ApiErrorDetail,
}

#[derive(Debug, Serialize)]
struct ApiErrorDetail {
    code: String,
    message: String,
}

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    Forbidden,
    NotFound,
    BadRequest(String),
    Conflict(String),
    Gone(String),
    PayloadTooLarge,
    Internal,
}

impl ApiError {
    pub fn into_response(self) -> HttpResponse {
        let (status, code, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Valid Authorization credentials are required".to_string(),
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "The required permission is not granted".to_string(),
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "The requested resource was not found".to_string(),
            ),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
            Self::Gone(message) => (StatusCode::GONE, "gone", message),
            Self::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                "The request payload exceeds the allowed size".to_string(),
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "The request could not be completed".to_string(),
            ),
        };
        HttpResponse::build(status).json(ApiErrorBody {
            error: ApiErrorDetail {
                code: code.to_string(),
                message,
            },
        })
    }
}

pub async fn authenticate(
    req: &HttpRequest,
    auth_state: &TwigContext,
) -> Result<AuthenticatedPrincipal, ApiError> {
    match auth_state.authenticate_request(req).await {
        Ok(CredentialAuth::Authenticated(principal)) => Ok(principal),
        Ok(CredentialAuth::Missing | CredentialAuth::Invalid) => Err(ApiError::Unauthorized),
        Err(error) => {
            log::error!("Failed to authenticate API request: {error}");
            Err(ApiError::Internal)
        }
    }
}

pub async fn require_namespace_permission(
    principal: &AuthenticatedPrincipal,
    server: &config::Server,
    auth_state: &TwigContext,
    namespace: &str,
    required: NamespacePermission,
) -> Result<PermissionSet, ApiError> {
    match permissions::for_principal(
        auth_state.db(),
        principal,
        namespace,
        server.is_configured_admin(&principal.user.username),
    )
    .await
    {
        Ok(Some(current)) if current.contains(required) => Ok(current),
        Ok(Some(_) | None) => Err(ApiError::Forbidden),
        Err(error) => {
            log::error!("Failed to resolve API permissions for namespace '{namespace}': {error}");
            Err(ApiError::Internal)
        }
    }
}

pub fn json_error(error: ApiError) -> HttpResponse {
    error.into_response()
}

pub fn json_payload_error(
    error: actix_web::error::JsonPayloadError,
    request: &HttpRequest,
) -> actix_web::Error {
    let overflow = matches!(&error, actix_web::error::JsonPayloadError::Overflow { .. });
    // Forgejo-compatible routes answer in Forgejo's error shape; Twig's own routes keep theirs.
    let forgejo_route =
        request.path().starts_with("/api/v1/repos/") || request.path().starts_with("/api/v1/user");
    let response = match (overflow, forgejo_route) {
        (true, false) => ApiError::PayloadTooLarge.into_response(),
        (false, false) => {
            ApiError::BadRequest("Invalid JSON request body".to_string()).into_response()
        }
        (true, true) => super::compat::compat_error(
            actix_web::http::StatusCode::PAYLOAD_TOO_LARGE,
            "The request payload is too large",
        ),
        (false, true) => super::compat::compat_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Invalid JSON request body",
        ),
    };
    actix_web::error::InternalError::from_response(error, response).into()
}

pub fn validate_namespace_name(name: &str) -> Result<(), ApiError> {
    crate::git::reserved::validate_namespace_name(name).map_err(ApiError::BadRequest)
}

pub fn validate_repository_name(name: &str) -> Result<(), ApiError> {
    crate::git::reserved::validate_repo_name(name).map_err(ApiError::BadRequest)
}

#[derive(Debug, Serialize)]
struct UserResponse {
    id: String,
    username: String,
    email: Option<String>,
}

#[actix_web::get("/api/v1/me")]
pub async fn me(req: HttpRequest, auth_state: actix_web::web::Data<TwigContext>) -> HttpResponse {
    let principal = match authenticate(&req, &auth_state).await {
        Ok(principal) => principal,
        Err(error) => return error.into_response(),
    };

    HttpResponse::Ok().json(UserResponse {
        id: principal.user.id,
        username: principal.user.username,
        email: principal.user.email,
    })
}
