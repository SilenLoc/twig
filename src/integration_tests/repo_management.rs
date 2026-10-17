use super::*;

use super::support::*;

use super::auth_and_invitations::{create_namespace, signup_and_login};

#[actix_web::test]
async fn test_move_repo_between_owned_namespaces() {
    let root = format!("/tmp/test_twig_move_repo_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "moveowner").await;
    create_namespace(&app, session.clone(), "source").await;
    create_namespace(&app, session.clone(), "target").await;

    let source = std::path::Path::new(&root).join("source/repo");
    std::fs::create_dir_all(&source).unwrap();
    crate::git::repo::bare_init(&source, "main", "Test", "test@example.com").unwrap();

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
    assert!(git2::Repository::open(destination).is_ok());

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
    crate::git::repo::bare_init(&source, "main", "Test", "test@example.com").unwrap();

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
    crate::git::repo::bare_init(&source, "main", "Test", "test@example.com").unwrap();

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
    assert!(git2::Repository::open(destination).is_ok());

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
    crate::git::repo::bare_init(&repo, "main", "Test", "test@example.com").unwrap();
    let taken = std::path::Path::new(&root).join("owned/taken");
    std::fs::create_dir_all(&taken).unwrap();
    crate::git::repo::bare_init(&taken, "main", "Test", "test@example.com").unwrap();

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
    crate::git::repo::bare_init(&repo, "main", "Test", "test@example.com").unwrap();

    let req = test::TestRequest::post()
        .uri("/settings/rename-namespace")
        .cookie(session.clone())
        .set_form([("namespace", "oldns"), ("new_name", "newns")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!source.exists());
    assert!(git2::Repository::open(std::path::Path::new(&root).join("newns/repo")).is_ok());

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
