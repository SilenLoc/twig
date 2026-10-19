use super::*;

use super::support::*;

use super::repo_access_and_routes::{basic_auth, init_private_repo};

#[actix_web::test]
async fn namespace_owner_can_invite_only_to_owned_namespace() {
    let (app, _, owner_session, _, _, _) = create_admin_test_service().await;
    let req = test::TestRequest::post()
        .uri("/manage/invites")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            owner_session.clone(),
        ))
        .set_form([
            ("email", "invitee@example.com"),
            ("namespace", "docs-team"),
            ("role", "owner"),
            ("expires", "never"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert!(
        String::from_utf8(body.to_vec())
            .unwrap()
            .contains("invitee@example.com")
    );

    let req = test::TestRequest::post()
        .uri("/manage/invites")
        .cookie(actix_web::cookie::Cookie::new("session", owner_session))
        .set_form([
            ("email", "other@example.com"),
            ("namespace", "acme"),
            ("role", "contributor"),
            ("expires", "7d"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
#[actix_web::test]
async fn contributor_cannot_create_namespaces_or_repositories_but_can_access_existing_repo() {
    let (app, _, _, contributor_session, _, project_root) = create_admin_test_service().await;
    let existing_repo = std::path::Path::new(&project_root).join("acme/existing");
    std::fs::create_dir_all(&existing_repo).unwrap();
    crate::git::repo::bare_init(&existing_repo, "main", "Twig Test", "twig@example.com").unwrap();

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
async fn configured_admin_has_all_git_permissions_without_namespace_membership() {
    let (app, admin_session, _, contributor_session, _, project_root) =
        create_admin_test_service().await;
    let private_repo = std::path::Path::new(&project_root).join("docs-team/private");
    init_private_repo(&private_repo);

    let req = test::TestRequest::get()
        .uri("/docs-team/private")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            admin_session.clone(),
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert!(String::from_utf8_lossy(&body).contains("docs-team/private"));

    let req = test::TestRequest::get()
        .uri("/docs-team/private")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            contributor_session.clone(),
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert!(
        String::from_utf8_lossy(&body).contains("do not have permission to view this namespace")
    );

    let req = test::TestRequest::get()
        .uri("/docs-team")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            contributor_session,
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert!(!String::from_utf8_lossy(&body).contains("private"));

    let req = test::TestRequest::get()
        .uri("/manage/users?tab=users")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            admin_session.clone(),
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("All namespaces"), "{body}");
    assert!(body.contains("Rewrite protected branches"), "{body}");

    let req = test::TestRequest::get()
        .uri("/docs-team/private/info/refs?service=git-upload-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .insert_header(("Authorization", basic_auth("admin", "password123")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = test::TestRequest::get()
        .uri("/docs-team/admin-created/info/refs?service=git-receive-pack")
        .insert_header(("User-Agent", "git/2.43.0"))
        .insert_header(("Authorization", basic_auth("admin", "password123")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        std::path::Path::new(&project_root)
            .join("docs-team/admin-created")
            .exists()
    );

    let req = test::TestRequest::post()
        .uri("/manage/invites")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            admin_session.clone(),
        ))
        .set_form([
            ("email", "admin-invite@example.com"),
            ("namespace", "docs-team"),
            ("role", "contributor"),
            ("expires", "7d"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = test::TestRequest::post()
        .uri("/settings/rename-repo")
        .cookie(actix_web::cookie::Cookie::new(
            "session",
            admin_session.clone(),
        ))
        .set_form([
            ("namespace", "docs-team"),
            ("repo_name", "admin-created"),
            ("new_name", "admin-renamed"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = test::TestRequest::post()
        .uri("/settings/rename-namespace")
        .cookie(actix_web::cookie::Cookie::new("session", admin_session))
        .set_form([("namespace", "docs-team"), ("new_name", "admin-docs")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let _ = std::fs::remove_dir_all(&project_root);
}
