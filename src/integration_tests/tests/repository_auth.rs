use super::*;

#[actix_web::test]
async fn test_private_repository_ui_visibility_gated_on_login() {
    let root = format!("/tmp/test_twig_private_ui_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let owner_session = signup_and_login(&app, "privowner").await;
    create_namespace(&app, owner_session.clone(), "privspace").await;

    let pub_repo = std::path::Path::new(&root).join("privspace/pubrepo");
    std::fs::create_dir_all(&pub_repo).unwrap();
    twig_git::repo::bare_init(&pub_repo, "main", "Test", "test@example.com").unwrap();

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
    twig_git::repo::bare_init(&pub_repo, "main", "Test", "test@example.com").unwrap();

    let priv_repo = std::path::Path::new(&root).join("gitspace/privaterepo");
    init_private_repo(&priv_repo);

    // Public repo git read (unauthenticated) -> 200 OK
    let req = test::TestRequest::get()
        .uri("/gitspace/pubrepo/info/refs?service=git-upload-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .insert_header(("Git-Protocol", "version=2"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert!(
        body.starts_with(b"000eversion 2\n"),
        "Git protocol v2 negotiation header must reach git-http-backend"
    );

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
