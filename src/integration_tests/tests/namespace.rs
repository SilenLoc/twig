use super::*;

#[actix_web::test]
async fn contributor_cannot_create_namespaces_or_repositories_but_can_access_existing_repo() {
    let (app, _, _, contributor_session, _, project_root) = create_admin_test_service().await;
    let existing_repo = std::path::Path::new(&project_root).join("acme/existing");
    std::fs::create_dir_all(&existing_repo).unwrap();
    twig_git::repo::bare_init(&existing_repo, "main", "Twig Test", "twig@example.com").unwrap();

    let req = test::TestRequest::get()
        .uri("/acme")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            contributor_session.clone(),
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(!body.contains(">Create repo</button>"));
    assert!(!body.contains("create-repo-container"));

    let req = test::TestRequest::get()
        .uri("/acme/create-repo-form")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            contributor_session.clone(),
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let req = test::TestRequest::post()
        .uri("/acme/create-repo")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            contributor_session.clone(),
        ))
        .set_form([("repo_name", "ui-created")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(
        !std::path::Path::new(&project_root)
            .join("acme/ui-created")
            .exists()
    );

    let req = test::TestRequest::post()
        .uri("/auth/namespace")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            contributor_session.clone(),
        ))
        .set_form([("name", "contrib-new-space")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let req = test::TestRequest::post()
        .uri("/init")
        .insert_header(("Authorization", basic_auth("contributor", "password123")))
        .set_form([("namespace", "acme"), ("repo", "init-created")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(
        !std::path::Path::new(&project_root)
            .join("acme/init-created")
            .exists()
    );

    let req = test::TestRequest::get()
        .uri("/acme/from-git/info/refs?service=git-receive-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .insert_header(("Authorization", basic_auth("contributor", "password123")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(
        !std::path::Path::new(&project_root)
            .join("acme/from-git")
            .exists()
    );

    let req = test::TestRequest::get()
        .uri("/contrib-new-space/repo/info/refs?service=git-receive-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .insert_header(("Authorization", basic_auth("contributor", "password123")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let req = test::TestRequest::get()
        .uri("/acme/existing/info/refs?service=git-receive-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .insert_header(("Authorization", basic_auth("contributor", "password123")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = test::TestRequest::post()
        .uri("/settings/rename-namespace")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            contributor_session.clone(),
        ))
        .set_form([("namespace", "acme"), ("new_name", "renamed-acme")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let req = test::TestRequest::post()
        .uri("/settings/rename-repo")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            contributor_session.clone(),
        ))
        .set_form([
            ("namespace", "acme"),
            ("repo_name", "existing"),
            ("new_name", "renamed"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let req = test::TestRequest::get()
        .uri("/acme/existing")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            contributor_session.clone(),
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert!(
        String::from_utf8(body.to_vec())
            .unwrap()
            .contains("acme/existing")
    );

    let req = test::TestRequest::get()
        .uri("/manage/users")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            contributor_session,
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let _ = std::fs::remove_dir_all(&project_root);
}

#[actix_web::test]
async fn test_move_repo_between_owned_namespaces() {
    let root = format!("/tmp/test_twig_move_repo_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "moveowner").await;
    create_namespace(&app, session.clone(), "source").await;
    create_namespace(&app, session.clone(), "target").await;

    let source = std::path::Path::new(&root).join("source/repo");
    std::fs::create_dir_all(&source).unwrap();
    twig_git::repo::bare_init(&source, "main", "Test", "test@example.com").unwrap();

    let req = test::TestRequest::post()
        .uri("/settings/move-repo")
        .cookie(session)
        .set_form([
            ("source_namespace", "source"),
            ("repo_name", "repo"),
            ("target_namespace", "target"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!source.exists());
    let destination = std::path::Path::new(&root).join("target/repo");
    assert!(twig_git::is_repository(&destination));

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_move_repo_rejects_namespace_owned_by_another_user() {
    let root = format!("/tmp/test_twig_move_denied_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let owner_session = signup_and_login(&app, "sourceowner").await;
    create_namespace(&app, owner_session.clone(), "ownedsource").await;
    let other_session = signup_and_login(&app, "targetowner").await;
    create_namespace(&app, other_session, "foreigntarget").await;

    let source = std::path::Path::new(&root).join("ownedsource/repo");
    std::fs::create_dir_all(&source).unwrap();
    twig_git::repo::bare_init(&source, "main", "Test", "test@example.com").unwrap();

    let req = test::TestRequest::post()
        .uri("/settings/move-repo")
        .cookie(owner_session)
        .set_form([
            ("source_namespace", "ownedsource"),
            ("repo_name", "repo"),
            ("target_namespace", "foreigntarget"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(source.exists());
    assert!(
        !std::path::Path::new(&root)
            .join("foreigntarget/repo")
            .exists()
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_rename_repo_within_owned_namespace() {
    let root = format!("/tmp/test_twig_rename_repo_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "renameowner").await;
    create_namespace(&app, session.clone(), "acme").await;

    let source = std::path::Path::new(&root).join("acme/old-name");
    std::fs::create_dir_all(&source).unwrap();
    twig_git::repo::bare_init(&source, "main", "Test", "test@example.com").unwrap();

    let req = test::TestRequest::post()
        .uri("/settings/rename-repo")
        .cookie(session)
        .set_form([
            ("namespace", "acme"),
            ("repo_name", "old-name"),
            ("new_name", "new-name"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!source.exists());
    let destination = std::path::Path::new(&root).join("acme/new-name");
    assert!(twig_git::is_repository(&destination));

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_rename_repo_rejects_foreign_namespace_and_existing_name() {
    let root = format!("/tmp/test_twig_rename_denied_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let owner_session = signup_and_login(&app, "renameowner").await;
    create_namespace(&app, owner_session.clone(), "owned").await;
    let other_session = signup_and_login(&app, "renameother").await;
    create_namespace(&app, other_session, "foreign").await;

    let repo = std::path::Path::new(&root).join("owned/repo");
    std::fs::create_dir_all(&repo).unwrap();
    twig_git::repo::bare_init(&repo, "main", "Test", "test@example.com").unwrap();
    let taken = std::path::Path::new(&root).join("owned/taken");
    std::fs::create_dir_all(&taken).unwrap();
    twig_git::repo::bare_init(&taken, "main", "Test", "test@example.com").unwrap();

    // Another user's namespace is off limits.
    let req = test::TestRequest::post()
        .uri("/settings/rename-repo")
        .cookie(owner_session.clone())
        .set_form([
            ("namespace", "foreign"),
            ("repo_name", "repo"),
            ("new_name", "renamed"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Renaming onto an existing repository name conflicts.
    let req = test::TestRequest::post()
        .uri("/settings/rename-repo")
        .cookie(owner_session)
        .set_form([
            ("namespace", "owned"),
            ("repo_name", "repo"),
            ("new_name", "taken"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert!(repo.exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_rename_namespace_moves_directory_and_record() {
    let root = format!("/tmp/test_twig_rename_ns_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "nsrenameowner").await;
    create_namespace(&app, session.clone(), "oldns").await;

    let source = std::path::Path::new(&root).join("oldns");
    let repo = source.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    twig_git::repo::bare_init(&repo, "main", "Test", "test@example.com").unwrap();

    let req = test::TestRequest::post()
        .uri("/settings/rename-namespace")
        .cookie(session.clone())
        .set_form([("namespace", "oldns"), ("new_name", "newns")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!source.exists());
    assert!(twig_git::is_repository(
        &std::path::Path::new(&root).join("newns/repo")
    ));

    // The database record moved too: renaming again from the new name works.
    let req = test::TestRequest::post()
        .uri("/settings/rename-namespace")
        .cookie(session)
        .set_form([("namespace", "newns"), ("new_name", "final")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(std::path::Path::new(&root).join("final/repo").is_dir());

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_rename_namespace_rejects_foreign_owner_and_existing_name() {
    let root = format!("/tmp/test_twig_rename_ns_denied_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let owner_session = signup_and_login(&app, "nsrenameowner").await;
    create_namespace(&app, owner_session.clone(), "ownedns").await;
    create_namespace(&app, owner_session.clone(), "taken").await;
    let other_session = signup_and_login(&app, "nsrenameother").await;
    create_namespace(&app, other_session, "foreignns").await;

    // Another user's namespace is off limits.
    let req = test::TestRequest::post()
        .uri("/settings/rename-namespace")
        .cookie(owner_session.clone())
        .set_form([("namespace", "foreignns"), ("new_name", "renamedns")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(std::path::Path::new(&root).join("foreignns").is_dir());

    // Renaming onto an existing namespace name conflicts.
    let req = test::TestRequest::post()
        .uri("/settings/rename-namespace")
        .cookie(owner_session)
        .set_form([("namespace", "ownedns"), ("new_name", "taken")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert!(std::path::Path::new(&root).join("ownedns").is_dir());

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_cannot_create_namespace_with_reserved_prefix() {
    let root = format!("/tmp/test_twig_ns_reserved_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "underscoreuser").await;

    let req = test::TestRequest::post()
        .uri("/auth/namespace")
        .cookie(session)
        .set_form([("name", "_admin")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_client_error(),
        "namespace names starting with '_' must be refused"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// ========== CONTENT TAB TRAVERSAL SECURITY TESTS ==========
