use super::*;

use super::support::*;

use super::auth_and_invitations::{create_namespace, signup_and_login};
use super::repo_content::{init_repo_with_files, init_repo_with_paper};

pub(super) fn init_private_repo(repo_path: &std::path::Path) {
    std::fs::create_dir_all(repo_path).unwrap();
    crate::git::repo::bare_init(repo_path, "main", "Test", "test@example.com").unwrap();
    let sh = xshell::Shell::new().unwrap();
    let _p = sh.push_dir(repo_path);
    let blob = xshell::cmd!(sh, "git hash-object -w --stdin")
        .stdin("private = true\n")
        .read()
        .unwrap();
    let tree = xshell::cmd!(sh, "git mktree")
        .stdin(format!("100644 blob {blob}\t.twig.toml\n"))
        .read()
        .unwrap();
    let commit = xshell::cmd!(sh, "git commit-tree {tree} -m 'make private'")
        .read()
        .unwrap();
    xshell::cmd!(sh, "git update-ref refs/heads/main {commit}")
        .run()
        .unwrap();
}
pub(super) fn basic_auth(username: &str, password: &str) -> String {
    use base64::Engine;
    let creds = format!("{username}:{password}");
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(creds.as_bytes())
    )
}
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
            ("docs/guide.MARKDOWN", "# Guide\n"),
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

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/toolbox/raw/docs/guide.MARKDOWN")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("Content-Type").unwrap(),
        "text/plain; charset=utf-8"
    );
    assert_eq!(test::read_body(response).await.as_ref(), b"# Guide\n");

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
#[actix_web::test]
async fn test_markdown_source_actions_are_only_in_content() {
    let root = format!("/tmp/test_twig_markdown_raw_{}", uuid::Uuid::new_v4());
    let repo_path = std::path::Path::new(&root).join("pub/guide");
    let source = "# Nested guide\n\nCopy this exact Markdown.\n";
    init_repo_with_files(
        &repo_path,
        "",
        &[("README.md", "# Root guide\n"), ("docs/guide.md", source)],
    );
    let app = create_test_service_in(&root).await;

    let (status, root_html) = body_of(&app, "/pub/guide").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !root_html.contains("/pub/guide/raw/README.md")
            && !root_html.contains("data-twig-copy-markdown"),
        "Documentation keeps source actions out of the rendered document: {root_html}"
    );

    let (status, html) = body_of(&app, "/pub/guide/md/docs/guide.md").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !html.contains("/pub/guide/raw/docs/guide.md") && !html.contains("data-twig-copy-markdown"),
        "the Documentation view does not include source actions: {html}"
    );
    assert!(
        html.contains("Copy this exact Markdown."),
        "the selected page is rendered: {html}"
    );

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/guide/md/docs/guide.md")
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let htmx_body = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();
    assert!(
        !htmx_body.contains("/pub/guide/raw/docs/guide.md")
            && !htmx_body.contains("data-twig-copy-markdown"),
        "an HTMX Documentation change also omits source actions: {htmx_body}"
    );

    let (status, content_html) = body_of(&app, "/pub/guide/content/docs/guide.md").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        content_html.contains(r#"href="/pub/guide/raw/docs/guide.md""#)
            && content_html.contains("data-twig-copy-markdown")
            && content_html.contains("Copy source"),
        "the Content view offers actions for the selected Markdown file: {content_html}"
    );
    assert!(
        !content_html.contains("Edit in workspace") && content_html.contains(">Open workspace</a>"),
        "the workspace is opened from the single repository-level action: {content_html}"
    );

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/guide/raw/docs/guide.md")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("Content-Type").unwrap(),
        "text/plain; charset=utf-8"
    );
    assert_eq!(test::read_body(response).await.as_ref(), source.as_bytes());

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
async fn test_private_repository_ui_visibility_gated_on_login() {
    let root = format!("/tmp/test_twig_private_ui_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let owner_session = signup_and_login(&app, "privowner").await;
    create_namespace(&app, owner_session.clone(), "privspace").await;

    let pub_repo = std::path::Path::new(&root).join("privspace/pubrepo");
    std::fs::create_dir_all(&pub_repo).unwrap();
    crate::git::repo::bare_init(&pub_repo, "main", "Test", "test@example.com").unwrap();

    let priv_repo = std::path::Path::new(&root).join("privspace/secretrepo");
    init_private_repo(&priv_repo);

    // Anonymous namespace listing: only public repo should appear
    let req = test::TestRequest::get().uri("/privspace").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("pubrepo"), "public repo must be visible");
    assert!(
        !body.contains("secretrepo"),
        "private repo must NOT be visible to anonymous visitors"
    );

    // Logged-in namespace listing: both public and private repos appear
    let req = test::TestRequest::get()
        .uri("/privspace")
        .cookie(owner_session.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("pubrepo"), "public repo must be visible");
    assert!(
        body.contains("secretrepo"),
        "private repo must be visible to logged-in users"
    );

    // Anonymous direct access to private repo: auth error
    let req = test::TestRequest::get()
        .uri("/privspace/secretrepo")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(
        body.contains("Not logged in. Please log in first."),
        "must show auth error notice"
    );
    assert!(body.contains("href=\"/auth/login\""));

    // Anonymous tab access: auth error
    let req = test::TestRequest::get()
        .uri("/privspace/secretrepo/commits")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("Not logged in. Please log in first."));

    // Anonymous content access: auth error
    let req = test::TestRequest::get()
        .uri("/privspace/secretrepo/content/")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(body.contains("Not logged in. Please log in first."));

    // Logged-in direct access to private repo: succeeds with repo page
    let req = test::TestRequest::get()
        .uri("/privspace/secretrepo")
        .cookie(owner_session)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(!body.contains("Not logged in. Please log in first."));
    assert!(body.contains("privspace/secretrepo"));

    let _ = std::fs::remove_dir_all(&root);
}
#[actix_web::test]
async fn test_private_repository_git_read_gated_behind_auth() {
    let root = format!("/tmp/test_twig_private_git_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let owner_session = signup_and_login(&app, "gitprivowner").await;
    let other_session = signup_and_login(&app, "gitprivother").await;
    create_namespace(&app, owner_session, "gitspace").await;
    create_namespace(&app, other_session, "otherspace").await;

    let pub_repo = std::path::Path::new(&root).join("gitspace/pubrepo");
    std::fs::create_dir_all(&pub_repo).unwrap();
    crate::git::repo::bare_init(&pub_repo, "main", "Test", "test@example.com").unwrap();

    let priv_repo = std::path::Path::new(&root).join("gitspace/privaterepo");
    init_private_repo(&priv_repo);

    // Public repo git read (unauthenticated) -> 200 OK
    let req = test::TestRequest::get()
        .uri("/gitspace/pubrepo/info/refs?service=git-upload-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // The regular Git smart-push handshake remains available and still
    // requires credentials, independently of the binary HTTP endpoints.
    let req = test::TestRequest::get()
        .uri("/gitspace/pubrepo/info/refs?service=git-receive-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let req = test::TestRequest::get()
        .uri("/gitspace/pubrepo/info/refs?service=git-receive-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .insert_header(("Authorization", basic_auth("gitprivowner", "password123")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Private repo git read (unauthenticated) -> 401 Unauthorized with WWW-Authenticate
    let req = test::TestRequest::get()
        .uri("/gitspace/privaterepo/info/refs?service=git-upload-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let auth_header = resp
        .headers()
        .get("WWW-Authenticate")
        .expect("must contain WWW-Authenticate header")
        .to_str()
        .unwrap();
    assert!(auth_header.contains("Basic"));

    // Private repo git read with .git suffix (unauthenticated) -> 401 Unauthorized
    let req = test::TestRequest::get()
        .uri("/gitspace/privaterepo.git/info/refs?service=git-upload-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Private repo git read with wrong password -> 401 Unauthorized
    let req = test::TestRequest::get()
        .uri("/gitspace/privaterepo/info/refs?service=git-upload-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .insert_header((
            "Authorization",
            basic_auth("gitprivowner", "wrong-password"),
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Private repo git read with valid user but no namespace access -> 403 Forbidden
    let req = test::TestRequest::get()
        .uri("/gitspace/privaterepo/info/refs?service=git-upload-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .insert_header(("Authorization", basic_auth("gitprivother", "password123")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Private repo git read with owner credentials -> 200 OK
    let req = test::TestRequest::get()
        .uri("/gitspace/privaterepo/info/refs?service=git-upload-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .insert_header(("Authorization", basic_auth("gitprivowner", "password123")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Private repo git read with .git suffix with owner credentials -> 200 OK
    let req = test::TestRequest::get()
        .uri("/gitspace/privaterepo.git/info/refs?service=git-upload-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .insert_header(("Authorization", basic_auth("gitprivowner", "password123")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let _ = std::fs::remove_dir_all(&root);
}
