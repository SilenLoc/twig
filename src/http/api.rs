use actix_web::{HttpRequest, HttpResponse, get};
use serde::Serialize;

use crate::auth::TwigContext;

pub mod v1;

#[derive(Debug, Serialize)]
pub struct VersionResponse {
    pub version: &'static str,
}

/// Returns the current application version as JSON.
#[get("/api/version")]
pub async fn version_endpoint(
    req: HttpRequest,
    auth_state: actix_web::web::Data<TwigContext>,
) -> HttpResponse {
    if let Err(error) = v1::authenticate(&req, &auth_state).await {
        return error.into_response();
    }

    HttpResponse::Ok().json(VersionResponse {
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_response_serializes_as_json() {
        let body = serde_json::to_value(VersionResponse {
            version: env!("CARGO_PKG_VERSION"),
        })
        .expect("serialize version response");
        assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    }
}
