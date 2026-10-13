use super::*;

use super::support::*;

// ========== CONTENT TAB TRAVERSAL SECURITY TESTS ==========

/// Marker embedded in a secret file placed OUTSIDE the project root.
/// No response may ever contain this string.
const SECRET_MARKER: &str = "TOP_SECRET_TRAVERSAL_MARKER_9f3a";

struct TraversalFixture {
    root: std::path::PathBuf,
    outside: std::path::PathBuf,
    db_path: String,
}

impl Drop for TraversalFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
        let _ = std::fs::remove_dir_all(&self.outside);
        let _ = std::fs::remove_file(&self.db_path);
    }
}

/// Creates a project root containing `public/repo` (a real bare repo with
/// an initial commit) plus a secret directory OUTSIDE the project root.
fn setup_traversal_fixture() -> TraversalFixture {
    use std::path::PathBuf;

    let id = uuid::Uuid::new_v4().to_string();
    let root = PathBuf::from(format!("/tmp/twig_trav_root_{id}"));
    let repo_dir = root.join("public").join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    crate::git::repo::bare_init(&repo_dir, "main", "Test", "t@example.com")
        .expect("bare_init failed");

    // Secret file outside the project root, reachable only via traversal
    let outside = PathBuf::from(format!("/tmp/twig_trav_secret_{id}"));
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(
        outside.join("secret.txt"),
        format!("{SECRET_MARKER}\nroot:x:0:0:root\n"),
    )
    .unwrap();

    TraversalFixture {
        root,
        outside,
        db_path: format!("/tmp/twig_trav_{id}.db"),
    }
}
async fn create_traversal_service(
    fixture: &TraversalFixture,
) -> impl actix_web::dev::Service<
    Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    let config = config::Server::new(
        ("127.0.0.1".to_string(), 8080),
        "error".to_string(),
        fixture.root.to_string_lossy().into_owned(),
        fixture.db_path.clone(),
        "secure".to_string(),
        true,
    );

    let db = Database::new(config.db_path());
    let auth_state = web::Data::new(auth::TwigContext::new(db, "secure".to_string()));
    auth_state.db().init_tables().await.expect("init tables");

    test::init_service(
        App::new()
            .app_data(web::Data::new(config))
            .app_data(auth_state)
            .service(repo_pages::handler)
            .service(repo_pages::tab_handler)
            .service(repo_pages::content_tab_handler)
            .service(repo_pages::commits_tab_handler)
            .service(repo_pages::content_handler)
            .service(repo_pages::markdown_handler),
    )
    .await
}
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
    crate::git::repo::bare_init(&sibling, "main", "Test", "t@example.com").unwrap();

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
