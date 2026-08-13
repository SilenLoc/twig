use actix_web::{HttpRequest, HttpResponse, Responder, get};

const TCSS: &str = include_str!("../assets/t.css");
const HTMX: &str = include_str!("../assets/h.js");
const HTMX_RESPONSE_TARGETS: &str = include_str!("../assets/hx-response-targets.js");
const FIG_SVG: &str = include_str!("../assets/fig.svg");

// Cache static assets for 1 year (in seconds)
const CACHE_CONTROL_VALUE: &str = "public, max-age=31536000, immutable";

#[get("/assets/{filename:.*}")]
pub async fn assets(req: HttpRequest) -> impl Responder {
    let path = req.match_info().query("filename");

    match path {
        "t.css" => HttpResponse::Ok()
            .content_type("text/css; charset=utf-8")
            .insert_header(("Cache-Control", CACHE_CONTROL_VALUE))
            .body(TCSS),
        "h.js" => HttpResponse::Ok()
            .content_type("application/javascript; charset=utf-8")
            .insert_header(("Cache-Control", CACHE_CONTROL_VALUE))
            .body(HTMX),
        "hx-response-targets.js" => HttpResponse::Ok()
            .content_type("application/javascript; charset=utf-8")
            .insert_header(("Cache-Control", CACHE_CONTROL_VALUE))
            .body(HTMX_RESPONSE_TARGETS),
        "fig.svg" => HttpResponse::Ok()
            .content_type("image/svg+xml")
            .insert_header(("Cache-Control", CACHE_CONTROL_VALUE))
            .body(FIG_SVG),
        _ => HttpResponse::NotFound().body("Not found"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test as aw_test;

    #[test]
    fn test_asset_constants_are_non_empty() {
        assert!(!TCSS.is_empty(), "t.css should not be empty");
        assert!(!HTMX.is_empty(), "h.js should not be empty");
        assert!(
            !HTMX_RESPONSE_TARGETS.is_empty(),
            "hx-response-targets.js should not be empty"
        );
        assert!(!FIG_SVG.is_empty(), "fig.svg should not be empty");
    }

    #[actix_web::test]
    async fn test_assets_handler_known_files() {
        let app = aw_test::init_service(actix_web::App::new().service(assets)).await;

        for (path, expected_type) in [
            ("/assets/t.css", "text/css"),
            ("/assets/h.js", "application/javascript"),
            ("/assets/hx-response-targets.js", "application/javascript"),
            ("/assets/fig.svg", "image/svg+xml"),
        ] {
            let req = aw_test::TestRequest::get().uri(path).to_request();
            let resp = aw_test::call_service(&app, req).await;
            assert!(resp.status().is_success(), "{path} should succeed");
            assert!(
                resp.headers()
                    .get("content-type")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with(expected_type),
                "{path} should have content type {expected_type}"
            );
            assert_eq!(
                resp.headers()
                    .get("cache-control")
                    .unwrap()
                    .to_str()
                    .unwrap(),
                CACHE_CONTROL_VALUE
            );
        }
    }

    #[actix_web::test]
    async fn test_assets_handler_unknown_file_returns_404() {
        let app = aw_test::init_service(actix_web::App::new().service(assets)).await;
        let req = aw_test::TestRequest::get()
            .uri("/assets/unknown.txt")
            .to_request();
        let resp = aw_test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }
}
