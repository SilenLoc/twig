use super::*;

#[test]
fn test_base_url_schemes_follow_the_request() {
    use actix_web::test as actix_test;

    let local = actix_test::TestRequest::get()
        .uri("http://localhost:8080/ns/repo")
        .to_http_request();
    assert_eq!(
        script_base_url(&local),
        "http://localhost:8080",
        "local hosts stay on http so dev commands run as printed"
    );

    let proxied = actix_test::TestRequest::get()
        .uri("https://git.example.com/ns/repo")
        .insert_header(("X-Forwarded-Proto", "https"))
        .to_http_request();
    assert_eq!(script_base_url(&proxied), "https://git.example.com");

    let plain = actix_test::TestRequest::get()
        .uri("http://git.example.com/ns/repo")
        .to_http_request();
    assert_eq!(
        script_base_url(&plain),
        "https://git.example.com",
        "without a proxy header, remote hosts default to https"
    );
}

#[test]
fn test_raw_content_type_is_text_for_scripts_and_neutral_otherwise() {
    assert_eq!(
        raw_content_type("scripts/install.sh"),
        "text/plain; charset=utf-8"
    );
    assert_eq!(raw_content_type("README.md"), "text/plain; charset=utf-8");
    assert_eq!(
        raw_content_type("assets/logo.bin"),
        "application/octet-stream"
    );
}
