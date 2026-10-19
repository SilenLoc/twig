use super::*;

#[actix_web::test]
async fn test_test_page_404_when_test_user_unset() {
    let (app, _) = create_test_service_with_test_user(None).await;
    let req = test::TestRequest::get().uri("/_test").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn test_test_page_forbidden_when_unauthenticated() {
    let (app, _) = create_test_service_with_test_user(Some("admin")).await;
    let req = test::TestRequest::get().uri("/_test").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn test_test_page_and_rapid_runner_when_authenticated_as_admin() {
    let (app, auth_state) = create_test_service_with_test_user(Some("admin")).await;
    let user = auth::create_user(
        "admin".to_string(),
        "admin@example.com".to_string(),
        "password123",
    )
    .unwrap();
    auth_state.db().create_user(&user).await.unwrap();
    let token = auth_state.create_session(user.id).await.unwrap();

    // 1. Load main test page
    let req = test::TestRequest::get()
        .uri("/_test")
        .cookie(actix_web::cookie::Cookie::new("session", &token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("Test Suite"));
    assert!(body_str.contains("id=\"btn-start\""));
    assert!(body_str.contains("id=\"btn-stop\""));
    assert!(body_str.contains("id=\"test-runner-container\""));
    assert!(body_str.contains("id=\"test-runner-idle\""));
    assert!(!body_str.contains("id=\"test-runner\""));

    // 2. Load runner content (very low trigger on page, even smaller on contents)
    let req = test::TestRequest::get()
        .uri("/_test/runner")
        .cookie(actix_web::cookie::Cookie::new("session", &token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("hx-trigger=\"every 25ms\""));
    assert!(body_str.contains("hx-trigger=\"load, every 5ms\""));
    assert!(body_str.contains("/health"));
    assert!(!body_str.contains("/api/tree"));
    assert!(!body_str.contains("/settings"));
    assert!(!body_str.contains("delete-namespace"));

    // 3. Load stopped content
    let req = test::TestRequest::get()
        .uri("/_test/stopped")
        .cookie(actix_web::cookie::Cookie::new("session", &token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("id=\"test-runner-stopped\""));
    assert!(body_str.contains("Test run interrupted"));

    // 4. Ping and feature-check endpoints
    let req = test::TestRequest::get()
        .uri("/_test/ping")
        .cookie(actix_web::cookie::Cookie::new("session", &token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = test::TestRequest::get()
        .uri("/_test/feature-check")
        .cookie(actix_web::cookie::Cookie::new("session", &token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn test_test_page_pin_and_qr_code_multi_user_lifecycle() {
    let (app, auth_state) = create_test_service_with_test_user(Some("admin")).await;
    let admin = auth::create_user(
        "admin".to_string(),
        "admin@example.com".to_string(),
        "password123",
    )
    .unwrap();
    auth_state.db().create_user(&admin).await.unwrap();
    let admin_token = auth_state.create_session(admin.id).await.unwrap();

    // 1. Non-admin cannot create a PIN -> 403 Forbidden
    let req = test::TestRequest::post()
        .uri("/_test/pin/create")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 2. Admin creates PIN -> 200 OK with PIN and QR code SVG
    let req = test::TestRequest::post()
        .uri("/_test/pin/create")
        .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("id=\"active-pin-display\""));
    assert!(body_str.contains("<svg"));
    assert!(body_str.contains("Remove PIN"));

    let active_pin = auth_state
        .get_test_pin()
        .await
        .expect("active pin must be set");
    assert_eq!(active_pin.len(), 6);
    // Verify PIN is saved in the database directly
    assert_eq!(
        auth_state.db().get_test_pin().await.unwrap().as_deref(),
        Some(active_pin.as_str())
    );

    // 3. Guest visits with PIN in query param -> 200 OK, sets test_pin cookie
    let req = test::TestRequest::get()
        .uri(&format!("/_test?pin={active_pin}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify test_pin cookie was attached
    let pin_cookie = resp
        .response()
        .cookies()
        .find(|c| c.name() == "test_pin")
        .expect("must set test_pin cookie");
    assert_eq!(pin_cookie.value(), active_pin);

    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("GUEST (PIN:"));

    // 4. Guest runs the test runner with test_pin cookie -> 200 OK
    let req = test::TestRequest::get()
        .uri("/_test/runner")
        .cookie(actix_web::cookie::Cookie::new("test_pin", &active_pin))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("hx-trigger=\"every 25ms\""));
    assert!(body_str.contains("hx-trigger=\"load, every 5ms\""));

    // 5. Guest requests QR endpoint -> 200 OK with SVG
    let req = test::TestRequest::get()
        .uri("/_test/pin/qr")
        .cookie(actix_web::cookie::Cookie::new("test_pin", &active_pin))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap(), "image/svg+xml");

    // 6. Admin removes PIN -> 200 OK
    let req = test::TestRequest::post()
        .uri("/_test/pin/remove")
        .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(auth_state.get_test_pin().await, None);
    assert_eq!(auth_state.db().get_test_pin().await.unwrap(), None);

    // 7. Guest trying to run runner with now-revoked PIN -> 403 Forbidden!
    let req = test::TestRequest::get()
        .uri("/_test/runner")
        .cookie(actix_web::cookie::Cookie::new("test_pin", &active_pin))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 8. Guest trying to access main test page with old PIN -> 403 Forbidden!
    let req = test::TestRequest::get()
        .uri(&format!("/_test?pin={active_pin}"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
