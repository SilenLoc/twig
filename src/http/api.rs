use actix_web::{Responder, get, web};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct VersionResponse {
    pub version: &'static str,
}

/// Returns the current application version as JSON.
#[get("/api/version")]
pub async fn version_endpoint() -> impl Responder {
    web::Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, test as aw_test};

    #[actix_web::test]
    async fn test_version_endpoint_returns_package_version() {
        let app = aw_test::init_service(App::new().service(version_endpoint)).await;
        let response = aw_test::call_service(
            &app,
            aw_test::TestRequest::get().uri("/api/version").to_request(),
        )
        .await;

        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
        let body: serde_json::Value = aw_test::read_body_json(response).await;
        assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    }
}
