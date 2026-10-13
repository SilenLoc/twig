use super::*;

#[test]
fn test_render_stopped_content() {
    let markup = render_stopped_content().into_string();
    assert!(markup.contains("id=\"test-runner-stopped\""));
    assert!(markup.contains("Test run interrupted"));
}

#[test]
fn test_render_test_page_is_idle_until_started() {
    let role = CallerRole::Admin {
        username: "admin".to_string(),
    };
    let markup = render_test_page(&role, None, "http://localhost:8080", false).into_string();
    assert!(markup.contains("id=\"test-runner-container\""));
    assert!(markup.contains("id=\"btn-start\""));
    assert!(markup.contains("id=\"btn-stop\""));
    assert!(markup.contains("hx-get=\"/_test/runner\""));
    assert!(markup.contains("hx-get=\"/_test/stopped\""));
    assert!(markup.contains("id=\"test-runner-idle\""));
    assert!(!markup.contains("id=\"test-runner\""));
    assert!(!markup.contains("hx-trigger=\"every 25ms\""));
}

#[test]
fn test_endpoint_catalog_uses_current_api_and_info_routes() {
    let endpoints = list_test_endpoints();
    let urls: Vec<_> = endpoints
        .iter()
        .map(|endpoint| endpoint.url.as_str())
        .collect();
    assert!(urls.contains(&"/api/tree"));
    assert!(urls.contains(&"/_info?tab=docs&page=git-backend"));
    assert!(urls.contains(&"/_info?tab=docs&page=ui"));
    assert!(urls.contains(&"/_info?tab=docs&page=environment-variables"));
    assert!(
        !urls
            .iter()
            .any(|url| url.contains("/api/v1/") || url.contains("doc="))
    );
}
