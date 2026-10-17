use super::*;
use actix_web::{http::header::HeaderValue, test as aw_test, web};

use crate::config;

fn body_of(filename: &str) -> &'static str {
    let (asset, _) = find(filename).unwrap_or_else(|| panic!("{filename} must be a known asset"));
    std::str::from_utf8(asset.body).expect("text asset should be valid UTF-8")
}

fn bytes_of(filename: &str) -> &'static [u8] {
    find(filename).map_or_else(
        || panic!("{filename} must be a known asset"),
        |(asset, _)| asset.body,
    )
}

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
    for filename in [
        "t.css",
        "twig.css",
        "theme.js",
        "h.js",
        "hx-live.js",
        "repository-editor.js",
        "quill.js",
        "quill.snow.css",
        "marked.js",
        "purify.js",
        "turndown.js",
        "twig.svg",
        "twig.schema.json",
    ] {
        assert!(
            !body_of(filename).is_empty(),
            "{filename} should not be empty"
        );
    }
    for filename in [
        "ibm-plex-mono-variable.woff2",
        "ibm-plex-sans-variable.woff2",
    ] {
        assert!(
            !bytes_of(filename).is_empty(),
            "{filename} should not be empty"
        );
    }
}

#[test]
fn test_urls_carry_the_version() {
    assert_eq!(url("t.css"), format!("/assets/t-{ASSET_VERSION}.css"));
    assert_eq!(url("twig.css"), format!("/assets/twig-{ASSET_VERSION}.css"));
    assert_eq!(
        url("hx-live.js"),
        format!("/assets/hx-live-{ASSET_VERSION}.js")
    );
    assert_eq!(
        url("quill.snow.css"),
        format!("/assets/quill.snow-{ASSET_VERSION}.css")
    );
    assert_eq!(
        url("repository-editor.js"),
        format!("/assets/repository-editor-{ASSET_VERSION}.js")
    );
    assert_eq!(url("twig.svg"), format!("/assets/twig-{ASSET_VERSION}.svg"));
    assert_eq!(
        url("ibm-plex-mono-variable.woff2"),
        format!("/assets/ibm-plex-mono-variable-{ASSET_VERSION}.woff2")
    );
    assert_eq!(
        url("ibm-plex-sans-variable.woff2"),
        format!("/assets/ibm-plex-sans-variable-{ASSET_VERSION}.woff2")
    );
    assert_eq!(url("nope.txt"), "/assets/nope.txt");
}

#[test]
fn test_twig_schema_lists_every_config_key() {
    let schema: serde_json::Value = serde_json::from_str(body_of("twig.schema.json"))
        .expect("twig.schema.json must be valid JSON");
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
        "scripts",
    ] {
        assert!(properties.contains_key(key), "schema missing {key}");
    }
}

#[test]
fn test_twig_schema_describes_nested_script_groups() {
    let schema: serde_json::Value = serde_json::from_str(body_of("twig.schema.json"))
        .expect("twig.schema.json must be valid JSON");
    let group = &schema["definitions"]["scriptGroup"];
    assert_eq!(group["properties"]["name"]["type"], "string");
    assert_eq!(
        group["properties"]["scripts"]["items"]["required"][0], "path",
        "a script entry must carry its repository path"
    );
    assert_eq!(
        group["additionalProperties"]["$ref"], "#/definitions/scriptGroup",
        "groups nest recursively, which is what builds the hierarchy"
    );
}

#[test]
fn test_twig_css_confines_raw_colour_values_to_the_token_block() {
    let scanned = css_outside_root_blocks(&css_without_comments(body_of("twig.css")));
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
fn test_twig_css_has_grayscale_dark_and_light_palettes() {
    let css = css_without_comments(body_of("twig.css"));
    assert!(css.contains(":root {"), "dark is the default palette");
    assert!(
        css.contains(":root[data-twig-theme=\"light\"] {"),
        "light has its own palette"
    );

    let mut color_count = 0;
    for line in css.lines() {
        let Some((_, value)) = line.trim().split_once(':') else {
            continue;
        };
        let value = value.trim().trim_end_matches(';');
        let Some(hex) = value.strip_prefix('#') else {
            continue;
        };
        if hex.len() != 6 {
            continue;
        }
        let red = u8::from_str_radix(&hex[0..2], 16).expect("valid red channel");
        let green = u8::from_str_radix(&hex[2..4], 16).expect("valid green channel");
        let blue = u8::from_str_radix(&hex[4..6], 16).expect("valid blue channel");
        assert_eq!(red, green, "non-grayscale token: {line}");
        assert_eq!(green, blue, "non-grayscale token: {line}");
        color_count += 1;
    }
    assert!(color_count > 0, "theme tokens should define palette values");
}

#[test]
fn test_theme_asset_uses_existing_two_mode_preference_contract() {
    let script = body_of("theme.js");
    for contract in [
        "var key = \"twig-theme\"",
        "root.dataset.twigTheme = \"light\"",
        "Switch to E-ink Dark",
        "Switch to E-ink Light",
        "localStorage.setItem(key, theme)",
    ] {
        assert!(
            script.contains(contract),
            "missing theme contract {contract}"
        );
    }
}

#[test]
fn test_twig_css_honours_the_depth_and_motion_bans() {
    let css = css_without_comments(body_of("twig.css"));
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
    let css = css_without_comments(body_of("twig.css"));
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
fn test_twig_css_uses_local_plex_font_faces() {
    let css = css_without_comments(body_of("twig.css"));
    for outgoing in ["tf-", "Anton", "Bricolage", "markdown-body"] {
        assert!(!css.contains(outgoing), "{outgoing} is superseded");
    }
    assert!(
        css.contains("font-family: \"IBM Plex Mono\"")
            && css.contains("/assets/ibm-plex-mono-variable.woff2"),
        "interface uses the bundled IBM Plex Mono face"
    );
    assert!(
        css.contains("font-family: \"IBM Plex Sans\"")
            && css.contains("/assets/ibm-plex-sans-variable.woff2"),
        "prose uses the bundled IBM Plex Sans face"
    );
    assert!(css.contains("--twig-sans: \"IBM Plex Sans\""));
    assert!(css.contains("--twig-mono:"));
    assert!(!css.contains("fonts.googleapis.com"), "font face is local");
}

#[test]
fn test_font_assets_include_open_font_license() {
    let license = include_str!("../../../assets/fonts/OFL.txt");
    assert!(
        license.contains("SIL OPEN FONT LICENSE Version 1.1"),
        "bundled fonts must carry their SIL OFL 1.1 license"
    );
}

#[test]
fn test_twig_css_distinguishes_monochrome_syntax_and_repository_rows() {
    let css = css_without_comments(body_of("twig.css"));
    for rule in [
        ".twig-syn-keyword {\n    color: var(--twig-syntax-keyword);\n    font-weight: 700;",
        ".twig-syn-type {\n    color: var(--twig-syntax-type);\n    text-decoration: underline dotted;",
        ".twig-syn-comment {\n    color: var(--twig-syntax-comment);\n    font-style: italic;",
        ".twig-notice--warning {\n    border-inline-start-color: var(--twig-signal-warn);\n    border-inline-start-style: dashed;",
        ".twig-colhead.twig-colhead--content {\n    display: grid;",
        ".twig-list--content .twig-row {\n    display: grid;",
    ] {
        assert!(
            css.contains(rule),
            "missing e-ink presentation rule: {rule}"
        );
    }
}

#[actix_web::test]
async fn test_versioned_assets_handler_uses_configured_cache_control() {
    let config = config::Server::new(
        ("127.0.0.1".to_string(), 8080),
        "debug".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "key".to_string(),
        false,
    )
    .with_cache_control(HeaderValue::from_static("no-cache"));
    let app = aw_test::init_service(
        actix_web::App::new()
            .app_data(web::Data::new(config))
            .service(assets),
    )
    .await;

    let req = aw_test::TestRequest::get().uri(&url("t.css")).to_request();
    let resp = aw_test::call_service(&app, req).await;

    assert_eq!(
        resp.headers().get("cache-control"),
        Some(&HeaderValue::from_static("no-cache"))
    );
}

#[actix_web::test]
async fn test_unversioned_assets_must_revalidate() {
    let config = config::Server::new(
        ("127.0.0.1".to_string(), 8080),
        "debug".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "key".to_string(),
        false,
    );
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
        Some(&HeaderValue::from_static(UNVERSIONED_CACHE_CONTROL)),
        "an unversioned path cannot be cached for a whole release"
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
    );
    let app = aw_test::init_service(
        actix_web::App::new()
            .app_data(web::Data::new(config))
            .service(assets),
    )
    .await;

    for (name, expected_type) in [
        ("t.css", "text/css"),
        ("twig.css", "text/css"),
        ("theme.js", "application/javascript"),
        ("h.js", "application/javascript"),
        ("hx-live.js", "application/javascript"),
        ("repository-editor.js", "application/javascript"),
        ("quill.js", "application/javascript"),
        ("quill.snow.css", "text/css"),
        ("marked.js", "application/javascript"),
        ("purify.js", "application/javascript"),
        ("turndown.js", "application/javascript"),
        ("twig.svg", "image/svg+xml"),
        ("twig.schema.json", "application/schema+json"),
        ("ibm-plex-mono-variable.woff2", "font/woff2"),
        ("ibm-plex-sans-variable.woff2", "font/woff2"),
    ] {
        for (path, expected_cache) in [
            (url(name), config::DEFAULT_CACHE_CONTROL),
            (format!("/assets/{name}"), UNVERSIONED_CACHE_CONTROL),
        ] {
            let req = aw_test::TestRequest::get().uri(&path).to_request();
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
                expected_cache,
                "{path} should carry {expected_cache}"
            );
        }
    }
}

#[actix_web::test]
async fn test_assets_handler_serves_bundled_font_bytes() {
    let config = config::Server::new(
        ("127.0.0.1".to_string(), 8080),
        "debug".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "key".to_string(),
        false,
    );
    let app = aw_test::init_service(
        actix_web::App::new()
            .app_data(web::Data::new(config))
            .service(assets),
    )
    .await;

    for name in [
        "ibm-plex-mono-variable.woff2",
        "ibm-plex-sans-variable.woff2",
    ] {
        let req = aw_test::TestRequest::get().uri(&url(name)).to_request();
        let body = aw_test::call_and_read_body(&app, req).await;
        assert_eq!(
            body.as_ref(),
            bytes_of(name),
            "{name} bytes must be unchanged"
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

#[actix_web::test]
async fn test_assets_handler_rejects_another_version() {
    let config = config::Server::new(
        ("127.0.0.1".to_string(), 8080),
        "debug".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "key".to_string(),
        false,
    );
    let app = aw_test::init_service(
        actix_web::App::new()
            .app_data(web::Data::new(config))
            .service(assets),
    )
    .await;
    let req = aw_test::TestRequest::get()
        .uri("/assets/t-0.0.1.css")
        .to_request();
    let resp = aw_test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::NOT_FOUND,
        "a version from another release must not be served"
    );
}
