use super::*;

#[actix_web::test]
async fn test_user_management_renders_admin_page_and_htmx_tabs() {
    let (app, session, owner_session, _, _, _) = create_admin_test_service().await;
    let req = test::TestRequest::get().uri("/manage/users").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(resp.headers().get("Location").unwrap(), "/auth/login");

    let req = test::TestRequest::get()
        .uri("/manage/users")
        .cookie(actix_web::cookie::Cookie::new("session", session.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("<title>User Management · Twig</title>"));
    assert!(body.contains("User Management"));
    assert!(body.contains("hx-post=\"/manage/invites\""));
    assert!(body.contains("value=\"contributor\" selected"));
    assert!(body.contains("value=\"acme\""));
    assert!(body.contains("value=\"docs-team\""));

    let req = test::TestRequest::get()
        .uri("/manage/users?tab=users")
        .cookie(actix_web::cookie::Cookie::new("session", session))
        .insert_header(("HX-Request", "true"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("contributor"));
    assert!(body.contains("owner"));
    assert!(body.contains("admin"));
    assert!(!body.contains("<title>"));

    let req = test::TestRequest::get()
        .uri("/manage/users?tab=users")
        .cookie(actix_web::cookie::Cookie::new("session", owner_session))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("owner@example.com"));
    assert!(body.contains("docs-team"));
    assert!(!body.contains("admin@example.com"));
    assert!(!body.contains("contributor@example.com"));
}

#[actix_web::test]
async fn test_tree_hub_and_admin_data_view_are_gated_and_paginated() {
    let db_path = format!("/tmp/test_twig_admin_data_{}.db", uuid::Uuid::new_v4());
    let server = config::Server::new(
        ("127.0.0.1".to_string(), 8080),
        "debug".to_string(),
        "/tmp/test_git".to_string(),
        db_path.clone(),
        "secure".to_string(),
        false,
    )
    .with_admin_user(Some("dbadmin".to_string()))
    .with_test_user(Some("testadmin".to_string()));
    let db = Database::new(&db_path);
    db.init_tables().await.expect("initialize tables");
    let auth_state = web::Data::new(auth::TwigContext::new(db.clone(), "secure".to_string()));

    let admin = auth::create_user(
        "dbadmin".to_string(),
        "dbadmin@example.com".to_string(),
        "password123",
    )
    .unwrap();
    db.create_user(&admin).await.unwrap();
    let admin_token = auth_state.create_session(admin.id).await.unwrap();
    let other = auth::create_user(
        "ordinary".to_string(),
        "ordinary@example.com".to_string(),
        "password123",
    )
    .unwrap();
    db.create_user(&other).await.unwrap();
    let other_token = auth_state.create_session(other.id).await.unwrap();

    db.conn()
        .await
        .unwrap()
        .execute("CREATE TABLE scroll_fixture (id INTEGER)", ())
        .await
        .unwrap();
    for id in 0..51 {
        db.conn()
            .await
            .unwrap()
            .execute(
                "INSERT INTO scroll_fixture (id) VALUES (?1)",
                crate::db::sqlite::params![id],
            )
            .await
            .unwrap();
    }

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(server))
            .app_data(auth_state)
            .wrap(actix_identity::IdentityMiddleware::default())
            .wrap(crate::auth::session_store::middleware(
                db.clone(),
                actix_web::cookie::Key::generate(),
            ))
            .configure(crate::http::routes::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/manage")
            .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(response.headers().get("Location").unwrap(), "/settings");

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/settings")
            .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("aria-current=\"page\" href=\"/settings\""));

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/manage/repositories")
            .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("aria-current=\"page\" href=\"/manage/repositories\">Repositories</a>"));
    assert!(html.contains("You don't have any repositories to manage."));

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/manage/data?table=scroll_fixture")
            .cookie(actix_web::cookie::Cookie::new("session", &other_token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/manage/data?table=scroll_fixture")
            .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("scroll_fixture"));
    assert!(html.contains(">49</td>"));
    assert!(!html.contains(">50</td>"));
    assert!(html.contains("hx-trigger=\"revealed\""));

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/manage/data/rows?table=scroll_fixture&offset=50")
            .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
            .to_request(),
    )
    .await;
    let body = test::read_body(response).await;
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains(">50</td>"));

    // An offset past the end of the table is an empty page with no load-more
    // link, so the scroll chain terminates instead of repeating rows.
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/manage/data/rows?table=scroll_fixture&offset=100")
            .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.is_empty(), "rows past the end render nothing: {html}");

    // A hand-crafted absurd offset is rejected rather than walked.
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/manage/data/rows?table=scroll_fixture&offset=99999999999")
            .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let _ = std::fs::remove_file(db_path);
}

#[actix_web::test]
async fn test_settings_page_without_login_is_an_htmx_recovery_fragment() {
    let app = create_test_service().await;
    let req = test::TestRequest::get()
        .uri("/settings")
        .insert_header(("HX-Request", "true"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(!body_str.contains("<!DOCTYPE html>"));
    assert!(body_str.contains("href=\"/auth/login\""));
    assert!(body_str.contains("Not logged in"));
}

#[actix_web::test]
async fn test_settings_page_with_expired_session_is_a_full_recovery_page() {
    let app = create_test_service().await;
    let req = test::TestRequest::get()
        .uri("/settings")
        .cookie(actix_web::cookie::Cookie::new("session", "expired"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("<!DOCTYPE html>"));
    assert!(body_str.contains("<title>Account · Twig</title>"));
    assert!(body_str.contains("href=\"/auth/login\""));
    assert!(body_str.contains("Session expired"));
}

// Settings tests - from settings.hurl
#[actix_web::test]
async fn test_settings_page_without_login() {
    let app = create_test_service().await;
    let req = test::TestRequest::get().uri("/settings").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("<!DOCTYPE html>"));
    assert!(body_str.contains("<title>Account · Twig</title>"));
    assert!(body_str.contains("href=\"/auth/login\""));
    assert!(body_str.contains("Not logged in"));
}

#[actix_web::test]
async fn test_update_email_invalid_format() {
    let app = create_test_service().await;
    let req = test::TestRequest::post()
        .uri("/settings/email")
        .set_form([("email", "invalid-email")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("valid email address"));
}

// ========== TICKET REPOSITORY PROVISIONING ==========
