use actix_web::{HttpRequest, HttpResponse, Responder, get, http::header::CACHE_CONTROL, web};

use crate::config;

const TCSS: &str = include_str!("../assets/t.css");
const TWIGCSS: &str = include_str!("../assets/twig.css");
const THEME_JS: &str = include_str!("../assets/theme.js");
const HTMX: &str = include_str!("../assets/h.js");
const HX_LIVE: &str = include_str!("../assets/hx-live.js");
const TWIG_SVG: &str = include_str!("../assets/twig.svg");
const TWIG_SCHEMA: &str = include_str!("../assets/twig.schema.json");

#[get("/assets/{filename:.*}")]
pub async fn assets(req: HttpRequest, config: web::Data<config::Server>) -> impl Responder {
    let path = req.match_info().query("filename");

    match path {
        "t.css" => HttpResponse::Ok()
            .content_type("text/css; charset=utf-8")
            .insert_header((CACHE_CONTROL, config.cache_control().clone()))
            .body(TCSS),
        "twig.css" => HttpResponse::Ok()
            .content_type("text/css; charset=utf-8")
            .insert_header((CACHE_CONTROL, config.cache_control().clone()))
            .body(TWIGCSS),
        "theme.js" => HttpResponse::Ok()
            .content_type("application/javascript; charset=utf-8")
            .insert_header((CACHE_CONTROL, config.cache_control().clone()))
            .body(THEME_JS),
        "h.js" => HttpResponse::Ok()
            .content_type("application/javascript; charset=utf-8")
            .insert_header((CACHE_CONTROL, config.cache_control().clone()))
            .body(HTMX),
        "hx-live.js" => HttpResponse::Ok()
            .content_type("application/javascript; charset=utf-8")
            .insert_header((CACHE_CONTROL, config.cache_control().clone()))
            .body(HX_LIVE),
        "twig.svg" => HttpResponse::Ok()
            .content_type("image/svg+xml")
            .insert_header((CACHE_CONTROL, config.cache_control().clone()))
            .body(TWIG_SVG),
        "twig.schema.json" => HttpResponse::Ok()
            .content_type("application/schema+json; charset=utf-8")
            .insert_header((CACHE_CONTROL, config.cache_control().clone()))
            .body(TWIG_SCHEMA),
        _ => HttpResponse::NotFound().body("Not found"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http::header::HeaderValue, test as aw_test, web};

    use crate::config;

    fn css_without_comments(css: &str) -> String {
        let mut out = String::with_capacity(css.len());
        let mut rest = css;
        while let Some(start) = rest.find("/*") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            let end = after.find("*/").expect("css comment must be closed");
            rest = &after[end + 2..];
        }
        out.push_str(rest);
        out
    }

    fn css_outside_root_blocks(css: &str) -> String {
        let mut out = String::with_capacity(css.len());
        let mut rest = css;
        while let Some(start) = rest.find(":root") {
            out.push_str(&rest[..start]);
            let after = &rest[start..];
            let open = after.find('{').expect(":root must open a block");
            let close = after.find('}').expect(":root block must be closed");
            out.push_str(&after[..open]);
            rest = &after[close + 1..];
        }
        out.push_str(rest);
        out
    }

    fn starts_a_hex_colour(rest: &str) -> bool {
        rest.chars().take(3).filter(char::is_ascii_hexdigit).count() == 3
    }

    #[test]
    fn test_asset_constants_are_non_empty() {
        assert!(!TCSS.is_empty(), "t.css should not be empty");
        assert!(!TWIGCSS.is_empty(), "twig.css should not be empty");
        assert!(!THEME_JS.is_empty(), "theme.js should not be empty");
        assert!(!HTMX.is_empty(), "h.js should not be empty");
        assert!(!HX_LIVE.is_empty(), "hx-live.js should not be empty");
        assert!(!TWIG_SVG.is_empty(), "twig.svg should not be empty");
        assert!(
            !TWIG_SCHEMA.is_empty(),
            "twig.schema.json should not be empty"
        );
    }

    #[test]
    fn test_twig_schema_lists_every_config_key() {
        let schema: serde_json::Value =
            serde_json::from_str(TWIG_SCHEMA).expect("twig.schema.json must be valid JSON");
        let properties = schema["properties"]
            .as_object()
            .expect("schema must define top-level properties");
        for key in [
            "ignore_for_view",
            "tabs",
            "deleteable",
            "private",
            "present",
            "paper",
        ] {
            assert!(properties.contains_key(key), "schema missing {key}");
        }
    }

    #[test]
    fn test_twig_css_confines_raw_colour_values_to_the_token_block() {
        let scanned = css_outside_root_blocks(&css_without_comments(TWIGCSS));
        for (offset, _) in scanned.match_indices('#') {
            assert!(
                !starts_a_hex_colour(&scanned[offset + 1..]),
                "hex colour outside :root near {:?}",
                &scanned[offset..(offset + 40).min(scanned.len())]
            );
        }
        for function in ["rgb(", "rgba(", "hsl(", "hsla("] {
            assert!(
                !scanned.contains(function),
                "{function} outside :root; every colour is a var(--twig-*) token"
            );
        }
        assert!(
            scanned.contains("var(--twig-ink-primary)"),
            "primitives must reference tokens"
        );
    }

    #[test]
    fn test_twig_css_honours_the_depth_and_motion_bans() {
        let css = css_without_comments(TWIGCSS);
        for banned in ["box-shadow", "backdrop-filter", "text-shadow", "100vh"] {
            assert!(!css.contains(banned), "{banned} is banned by DESIGN.md");
        }
        for (offset, _) in css.match_indices("border-radius:") {
            let value = css[offset..].split(';').next().unwrap_or_default();
            assert!(
                value.contains('0'),
                "every corner is square, found {value:?}"
            );
        }
        assert!(css.contains("100dvh"), "full-height regions use dvh");
        assert!(
            css.contains("prefers-reduced-motion: reduce"),
            "reduced motion must be honoured"
        );
        assert!(
            css.contains(":focus-visible"),
            "the shared focus ring must exist"
        );
    }

    #[test]
    fn test_twig_css_uses_only_the_two_authoritative_breakpoints() {
        let css = css_without_comments(TWIGCSS);
        let marker = "min-width:";
        let widths: Vec<String> = css
            .match_indices(marker)
            .map(|(offset, _)| {
                let rest = &css[offset + marker.len()..];
                let end = rest.find(')').expect("media query must be closed");
                rest[..end].trim().to_owned()
            })
            .collect();
        assert!(!widths.is_empty(), "responsive structure lives in twig.css");
        for width in &widths {
            assert!(
                width == "48rem" || width == "80rem",
                "unexpected breakpoint {width}"
            );
        }
    }

    #[test]
    fn test_twig_css_drops_the_outgoing_system() {
        let css = css_without_comments(TWIGCSS);
        for outgoing in ["tf-", "Anton", "Bricolage", "markdown-body"] {
            assert!(!css.contains(outgoing), "{outgoing} is superseded");
        }
        assert!(css.contains("Space Grotesk"), "Space Grotesk is the sans");
    }

    #[actix_web::test]
    async fn test_assets_handler_uses_configured_cache_control() {
        let config = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "key".to_string(),
            false,
            1.0,
        )
        .with_cache_control(HeaderValue::from_static("no-cache"));
        let app = aw_test::init_service(
            actix_web::App::new()
                .app_data(web::Data::new(config))
                .service(assets),
        )
        .await;

        let req = aw_test::TestRequest::get()
            .uri("/assets/t.css")
            .to_request();
        let resp = aw_test::call_service(&app, req).await;

        assert_eq!(
            resp.headers().get("cache-control"),
            Some(&HeaderValue::from_static("no-cache"))
        );
    }

    #[actix_web::test]
    async fn test_assets_handler_known_files() {
        let config = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "key".to_string(),
            false,
            1.0,
        );
        let app = aw_test::init_service(
            actix_web::App::new()
                .app_data(web::Data::new(config))
                .service(assets),
        )
        .await;

        for (path, expected_type) in [
            ("/assets/t.css", "text/css"),
            ("/assets/twig.css", "text/css"),
            ("/assets/theme.js", "application/javascript"),
            ("/assets/h.js", "application/javascript"),
            ("/assets/hx-live.js", "application/javascript"),
            ("/assets/twig.svg", "image/svg+xml"),
            ("/assets/twig.schema.json", "application/schema+json"),
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
                config::DEFAULT_CACHE_CONTROL
            );
        }
    }

    #[actix_web::test]
    async fn test_assets_handler_unknown_file_returns_404() {
        let config = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "key".to_string(),
            false,
            1.0,
        );
        let app = aw_test::init_service(
            actix_web::App::new()
                .app_data(web::Data::new(config))
                .service(assets),
        )
        .await;
        let req = aw_test::TestRequest::get()
            .uri("/assets/unknown.txt")
            .to_request();
        let resp = aw_test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }
}
