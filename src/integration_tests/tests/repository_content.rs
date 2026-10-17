use super::*;

#[actix_web::test]
async fn test_content_tab_lists_repo_files() {
    let fixture = setup_traversal_fixture();
    let app = create_traversal_service(&fixture).await;

    let (status, body) = body_of(&app, "/public/repo/content").await;
    assert_eq!(status, StatusCode::OK);
    // Positive control: repo's own file is listed
    assert!(
        body.contains(".twig.toml"),
        "expected .twig.toml in listing: {body}"
    );
    assert!(!body.contains(SECRET_MARKER));
}

#[actix_web::test]
async fn test_content_file_view_serves_repo_files_only() {
    let fixture = setup_traversal_fixture();
    let app = create_traversal_service(&fixture).await;

    let (status, body) = body_of(&app, "/public/repo/content/.twig.toml").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Created with Twig"), "{body}");
    assert!(
        body.contains(r#"<nav class="twig-crumbs twig-crumbs--page" aria-label="Breadcrumb">"#),
        "direct file views must retain the repository breadcrumb: {body}"
    );
    assert!(
        body.contains(r#"<h1 class="twig-crumb-current" aria-current="page">repo</h1>"#),
        "direct file views must identify the repository in the breadcrumb: {body}"
    );
    assert!(!body.contains(SECRET_MARKER));
}

#[actix_web::test]
async fn test_content_rejects_dotdot_segments() {
    let fixture = setup_traversal_fixture();
    let app = create_traversal_service(&fixture).await;

    for uri in [
        "/public/repo/content/../../../../../etc/passwd",
        "/public/repo/content/src/../../../etc/passwd",
        "/public/repo/content/..%2f..%2f..%2fetc%2fpasswd",
        "/public/repo/content/%2e%2e/%2e%2e/secret.txt",
        "/public/repo/content/..%5c..%5csecret.txt",
        "/public/repo/content/%2fetc%2fpasswd",
        "/public/repo/content/..",
        "/public/repo/content/a//b",
    ] {
        let (_status, body) = body_of(&app, uri).await;
        assert!(
            !body.contains(SECRET_MARKER),
            "SECRET LEAKED via {uri}: {body}"
        );
        assert!(!body.contains("root:x:0:0"), "passwd leaked via {uri}");
        assert!(
            body.contains("Path not found"),
            "expected rejection page for {uri}: {body}"
        );
    }
}

#[actix_web::test]
async fn test_content_rejects_namespace_and_repo_traversal() {
    let fixture = setup_traversal_fixture();
    let app = create_traversal_service(&fixture).await;

    for uri in [
        // namespace tries to climb out of the project root
        "/..%2f..%2ftmp%2fnonsense/repo/content",
        "/public%2f..%2f..%2fsecret/repo/content",
        // repo name tries to climb out of the namespace dir
        "/public/..%2f..%2fsecret/content",
        "/public/../secret/content",
    ] {
        let (_status, body) = body_of(&app, uri).await;
        assert!(
            !body.contains(SECRET_MARKER),
            "SECRET LEAKED via {uri}: {body}"
        );
        assert!(!body.contains("root:x:0:0"), "passwd leaked via {uri}");
    }
}

#[actix_web::test]
async fn test_content_rejects_nul_byte_paths() {
    let fixture = setup_traversal_fixture();
    let app = create_traversal_service(&fixture).await;

    for uri in [
        "/public/repo/content/.twig.toml%00",
        "/public/repo/content/%00.twig.toml",
        "/public/repo/content/a%00b",
    ] {
        let (_status, body) = body_of(&app, uri).await;
        assert!(
            !body.contains("Created with Twig") || uri.contains(".twig"),
            "unexpected content served for {uri}",
        );
        assert!(!body.contains(SECRET_MARKER), "leak via {uri}");
    }
}

#[actix_web::test]
async fn test_markdown_view_rejects_traversal() {
    let fixture = setup_traversal_fixture();
    let app = create_traversal_service(&fixture).await;

    for uri in [
        "/public/repo/md/../../secret.txt",
        "/public/repo/md/..%2f..%2fsecret.txt",
        "/public/repo/md/%2e%2e%2f%2e%2e%2fsecret.txt",
    ] {
        let (_status, body) = body_of(&app, uri).await;
        assert!(
            !body.contains(SECRET_MARKER),
            "SECRET LEAKED via {uri}: {body}"
        );
    }
}

#[actix_web::test]
async fn test_content_cannot_reach_sibling_namespace_via_dots() {
    let fixture = setup_traversal_fixture();
    // A sibling namespace with its own repo inside the same root
    let sibling = fixture.root.join("other").join("vault");
    std::fs::create_dir_all(&sibling).unwrap();
    twig_git::repo::bare_init(&sibling, "main", "Test", "t@example.com").unwrap();

    let app = create_traversal_service(&fixture).await;

    // Normal access to a public path still works...
    let (status, body) = body_of(&app, "/public/repo/content/.twig.toml").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Created with Twig"));

    // ...but dot-segment navigation must not escape into other namespaces
    let (_status, body) = body_of(&app, "/public/repo/content/../../other/vault/.twig.toml").await;
    assert!(
        !body.contains("Created with Twig"),
        "escaped into sibling namespace: {body}"
    );
    assert!(body.contains("Path not found"), "{body}");
}

// ========== PRIVATE REPOSITORY TESTS ==========

#[actix_web::test]
async fn test_raw_route_serves_repo_files_and_refuses_ignored_paths() {
    let root = format!("/tmp/test_twig_scripts_raw_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    init_repo_with_files(
        &std::path::Path::new(&root).join("pub/toolbox"),
        r#"
ignore_for_view = ["secrets/"]
[scripts.linux]
scripts = [{ name = "Install", path = "scripts/install.sh" }]
"#,
        &[
            ("scripts/install.sh", "#!/bin/bash\necho install\n"),
            ("secrets/key.txt", "DO NOT SERVE\n"),
        ],
    );

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/toolbox/raw/scripts/install.sh")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("Content-Type").unwrap(),
        "text/plain; charset=utf-8"
    );
    let body = test::read_body(response).await;
    assert_eq!(body, "#!/bin/bash\necho install\n");

    for uri in [
        "/pub/toolbox/raw/secrets/key.txt",
        "/pub/toolbox/raw/missing.sh",
        "/pub/toolbox/raw/../../secrets/key.txt",
        "/pub/toolbox/raw/%2e%2e/secrets/key.txt",
        "/pub/toolbox/raw/",
    ] {
        let response =
            test::call_service(&app, test::TestRequest::get().uri(uri).to_request()).await;
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "expected 404 for {uri}"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
}
