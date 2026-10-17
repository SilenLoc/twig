use super::*;

#[actix_web::test]
async fn test_paper_tab_renders_lazily_loaded_pages() {
    let root = format!("/tmp/test_twig_paper_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    init_repo_with_paper(
        &std::path::Path::new(&root).join("pub/book"),
        "[paper]\ndir = \"paper\"\n",
        &[("01.md", "# First\n"), ("02.md", "# Second\n")],
    );

    let body = get_body(&app, "/pub/book").await;
    assert!(body.contains(">Paper<"), "paper tab missing: {body}");
    assert!(
        body.contains("href=\"/pub/book/paper\""),
        "the tab points at the paper view: {body}"
    );

    let body = get_body(&app, "/pub/book/paper").await;
    assert!(body.contains("paper-container"), "{body}");
    assert!(body.contains("data-twig-paper-font=\"sans\""), "{body}");
    assert!(
        body.contains(r#"hx-trigger="revealed""#),
        "pages must lazy load on scroll: {body}"
    );
    assert!(
        body.contains("/pub/book/paper/paper/01.md"),
        "the page endpoint is wired: {body}"
    );
    assert!(
        body.contains(r#"id="paper-01.md""#),
        "each page carries an anchor: {body}"
    );

    let body = get_body(&app, "/pub/book/paper/paper/01.md").await;
    assert!(
        body.contains(r#"<h1 id="paper-01.md--first">First</h1>"#),
        "headings are anchored: {body}"
    );

    let body = get_body(&app, "/pub/book/paper/.twig.toml").await;
    assert!(
        body.contains("Paper page not found."),
        "files outside the paper dir are refused: {body}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_scripts_tab_is_hidden_until_configured() {
    let root = format!("/tmp/test_twig_scripts_hidden_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    init_repo_with_files(
        &std::path::Path::new(&root).join("pub/plain"),
        "tabs = []\n",
        &[("README.md", "# Plain\n")],
    );

    let body = get_body(&app, "/pub/plain").await;
    assert!(
        !body.contains(">Scripts<"),
        "no [scripts] section means no Scripts tab: {body}"
    );

    // Even a direct visit stays inert: no groups render.
    let body = get_body(&app, "/pub/plain/scripts").await;
    assert!(body.contains("NO SCRIPTS"), "{body}");

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_scripts_tab_shows_groups_with_curl_commands() {
    let root = format!("/tmp/test_twig_scripts_tab_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    init_repo_with_files(
        &std::path::Path::new(&root).join("pub/toolbox"),
        r#"
[scripts.linux]
name = "Linux"
scripts = [{ name = "Install", path = "scripts/install.sh", shell = "sh" }]

[scripts.linux.maintenance]
scripts = [{ name = "Cleanup", path = "scripts/cleanup.sh" }]
"#,
        &[
            ("README.md", "# Toolbox\n"),
            ("scripts/install.sh", "#!/bin/sh\necho install\n"),
            ("scripts/cleanup.sh", "#!/bin/bash\necho cleanup\n"),
        ],
    );

    // The tab appears on the repository home and points at its own route.
    let body = get_body(&app, "/pub/toolbox").await;
    assert!(body.contains(">Scripts<"), "Scripts tab missing: {body}");
    assert!(
        body.contains("href=\"/pub/toolbox/scripts\""),
        "the tab points at the scripts view: {body}"
    );

    // The default group renders with its sub-tab bar and run command.
    let body = get_body(&app, "/pub/toolbox/scripts").await;
    assert!(body.contains("scripts-container"), "{body}");
    assert!(body.contains(">Linux<"), "the group is a sub-tab: {body}");
    assert!(
        body.contains("href=\"/pub/toolbox/scripts/linux\""),
        "sub-tabs link by group key: {body}"
    );
    assert!(
        body.contains("curl -fsSL 'http://localhost:8080/pub/toolbox/raw/scripts/install.sh' | sh"),
        "the run command names the raw route and the configured shell: {body}"
    );
    assert!(
        body.contains("data-twig-copy"),
        "every entry carries a copy button: {body}"
    );

    // A nested group renders its own scripts under its key.
    let body = get_body(&app, "/pub/toolbox/scripts/linux/maintenance").await;
    assert!(
        body.contains(
            "curl -fsSL 'http://localhost:8080/pub/toolbox/raw/scripts/cleanup.sh' | bash"
        ),
        "nested groups resolve and default to bash: {body}"
    );

    // An unknown group key falls back to the first configured group.
    let body = get_body(&app, "/pub/toolbox/scripts/bogus").await;
    assert!(
        body.contains("curl -fsSL 'http://localhost:8080/pub/toolbox/raw/scripts/install.sh'"),
        "unknown keys fall back to the first group: {body}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_legacy_tab_url_redirects_to_the_tab_route() {
    let root = format!("/tmp/test_twig_tab_redirect_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    init_repo_with_paper(
        &std::path::Path::new(&root).join("pub/book"),
        "[paper]\ndir = \"paper\"\n",
        &[("01.md", "# First\n")],
    );

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/book/tab/paper")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response.headers().get("Location").unwrap(),
        "/pub/book/paper",
        "the legacy tab URL points at the tab's own route"
    );

    // An unknown tab name falls back to the repository home.
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/book/tab/bogus")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(response.headers().get("Location").unwrap(), "/pub/book");

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_invalid_twig_toml_shows_a_diagnostic() {
    let root = format!("/tmp/test_twig_badconfig_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    init_repo_with_paper(
        &std::path::Path::new(&root).join("pub/broken"),
        "this = = not valid\n",
        &[],
    );

    let body = get_body(&app, "/pub/broken").await;
    assert!(body.contains("CONFIG ERROR"), "{body}");
    assert!(
        body.contains("error: could not parse configuration"),
        "the parse failure is surfaced, not swallowed: {body}"
    );
    assert!(body.contains("help: fix the syntax"), "{body}");
    assert!(
        body.contains(".twig.toml"),
        "the offending file is named: {body}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
