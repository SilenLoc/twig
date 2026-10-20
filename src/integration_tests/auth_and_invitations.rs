use super::*;

use super::support::*;

/// Pulls the 64-hex-character invite token out of the success fragment.
pub(super) fn extract_invite(html: &str) -> String {
    html.split(|c: char| !c.is_ascii_hexdigit())
        .find(|token| token.len() == 64)
        .expect("invite token in response")
        .to_string()
}
/// Signs a user up and logs them in, returning the identity session cookie.
pub(super) async fn signup_and_login<S>(
    app: &S,
    username: &str,
) -> actix_web::cookie::Cookie<'static>
where
    S: actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let req = test::TestRequest::post()
        .uri("/auth/invite")
        .set_form([("api_key", "secure")])
        .to_request();
    let resp = test::call_service(app, req).await;
    assert!(
        resp.status().is_success(),
        "invite generation should succeed"
    );
    let body = test::read_body(resp).await;
    let invite = extract_invite(&String::from_utf8_lossy(&body));

    let email = format!("{username}@example.com");
    let req = test::TestRequest::post()
        .uri("/auth/signup")
        .set_form([
            ("invite", invite.as_str()),
            ("username", username),
            ("email", email.as_str()),
            ("password", "password123"),
        ])
        .to_request();
    let resp = test::call_service(app, req).await;
    assert!(resp.status().is_success(), "signup should succeed");

    let req = test::TestRequest::post()
        .uri("/auth/login")
        .set_form([("username", username), ("password", "password123")])
        .to_request();
    let resp = test::call_service(app, req).await;
    assert!(resp.status().is_success(), "login should succeed");

    let id_cookie = resp
        .response()
        .cookies()
        .find(|c| c.name() == "id")
        .expect("login should set an identity cookie");
    assert_eq!(
        id_cookie.same_site(),
        Some(actix_web::cookie::SameSite::Lax),
        "session cookie SameSite policy is the CSRF fallback"
    );
    id_cookie.into_owned()
}
pub(super) async fn create_namespace<S>(
    app: &S,
    session: actix_web::cookie::Cookie<'static>,
    name: &str,
) where
    S: actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let req = test::TestRequest::post()
        .uri("/auth/namespace")
        .cookie(session)
        .set_form([("name", name)])
        .to_request();
    let resp = test::call_service(app, req).await;
    assert!(
        resp.status().is_success(),
        "namespace creation should succeed"
    );
}
#[actix_web::test]
async fn test_health_endpoint() {
    let app = create_test_service().await;
    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}
#[actix_web::test]
async fn test_invite_page_loads() {
    let app = create_test_service().await;
    let req = test::TestRequest::get().uri("/auth/invite").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("<title>Get Signup Invite · Twig</title>"));
    assert!(body_str.contains("Get Signup Invite"));
    assert!(body_str.contains("API Key"));
}
#[actix_web::test]
async fn test_signup_page_loads() {
    let app = create_test_service().await;
    let req = test::TestRequest::get().uri("/auth/signup").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("<title>Create Account · Twig</title>"));
    assert!(body_str.contains("Create Account"));
    assert!(body_str.contains("Signup Invite"));
}
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
    assert_eq!(
        body.matches("href=\"/manage/users?tab=users\"").count(),
        1,
        "full page should contain one Users tab"
    );
    assert_eq!(
        body.matches("href=\"/manage/users?tab=invites\"").count(),
        1,
        "full page should contain one Invites tab"
    );
    assert!(body.contains("hx-post=\"/manage/invites\""));
    assert!(body.contains("value=\"contributor\" selected"));
    assert!(body.contains("value=\"acme\""));
    assert!(body.contains("value=\"docs-team\""));

    let req = test::TestRequest::get()
        .uri("/manage/users?tab=users")
        .cookie(actix_web::cookie::Cookie::new("session", session.clone()))
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
    assert!(body.contains("id=\"user-management-tabs\" hx-swap-oob=\"outerHTML\""));
    assert!(body.contains("href=\"/manage/users?tab=users\""));
    assert!(body.contains("aria-current=\"page\""));

    let req = test::TestRequest::get()
        .uri("/manage/users?tab=invites")
        .cookie(actix_web::cookie::Cookie::new("session", session))
        .insert_header(("HX-Request", "true"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("Create invitation"));
    assert!(body.contains("No invitations yet."));
    assert!(body.contains("id=\"user-management-tabs\" hx-swap-oob=\"outerHTML\""));
    assert!(body.contains("href=\"/manage/users?tab=invites\""));
    assert!(body.contains("aria-current=\"page\""));

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
async fn test_created_invitation_completes_account_and_membership_setup() {
    let (app, session, _, _, db, _) = create_admin_test_service().await;
    let req = test::TestRequest::post()
        .uri("/manage/invites")
        .cookie(actix_web::cookie::Cookie::new("session", session.clone()))
        .set_form([
            ("email", "sam@example.com"),
            ("namespace", "acme"),
            ("role", "contributor"),
            ("expires", "7d"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("/auth/accept-invite/"));
    assert!(body.contains("No email was sent"));
    assert!(body.contains("INVITATION CREATED"));
    assert!(body.contains("hx-swap-oob=\"outerHTML\""));
    let link_start = body
        .find("href=\"/auth/accept-invite/")
        .map(|index| index + "href=\"".len())
        .expect("accept link should be present");
    let link_end = link_start + body[link_start..].find('"').unwrap();
    let invite_path = &body[link_start..link_end];
    let token = invite_path.rsplit('/').next().unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/manage/invites/{token}/resend"))
        .cookie(actix_web::cookie::Cookie::new("session", session))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("RESEND_API_KEY is not set"));
    assert!(body.contains("hx-swap-oob=\"outerHTML\""));

    let req = test::TestRequest::get().uri(invite_path).to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("value=\"sam@example.com\" readonly"));
    assert!(body.contains("Join the acme namespace"));
    assert!(body.contains("invited as a Contributor"));
    assert!(body.contains("name=\"username\""));
    assert!(body.contains("name=\"password\""));

    let req = test::TestRequest::post()
        .uri(invite_path)
        .set_form([
            ("username", "sa"),
            ("password", "short"),
            ("email", "attacker@example.com"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = test::read_body(resp).await;
    assert!(
        String::from_utf8(body.to_vec())
            .unwrap()
            .contains("Username must be at least 3 characters")
    );

    let req = test::TestRequest::post()
        .uri(invite_path)
        .set_form([
            ("username", "sammy"),
            ("password", "password123"),
            ("email", "attacker@example.com"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("ACCOUNT CREATED"));
    assert!(body.contains("sammy"));
    assert!(body.contains("acme namespace"));
    assert!(body.contains("href=\"/auth/login\""));

    let user = db
        .get_user_by_username("sammy")
        .await
        .expect("load created user")
        .expect("user was created");
    assert_eq!(user.email.as_deref(), Some("sam@example.com"));
    assert!(auth::verify_password("password123", &user.password_hash).unwrap());
    let accepted = db
        .get_namespace_invitation(token)
        .await
        .expect("load accepted invitation")
        .expect("invitation retained");
    assert!(accepted.accepted_at.is_some());
    assert_eq!(accepted.accepted_user_id.as_deref(), Some(user.id.as_str()));
    let mut membership_rows = db
        .conn()
        .await
        .unwrap()
        .query(
            "SELECT role FROM namespace_members WHERE namespace_id = ?1 AND user_id = ?2",
            turso::params![accepted.namespace_id, user.id],
        )
        .await
        .unwrap();
    assert_eq!(
        membership_rows
            .next()
            .await
            .unwrap()
            .expect("namespace membership")
            .get::<String>(0)
            .unwrap(),
        "contributor"
    );

    let req = test::TestRequest::get().uri(invite_path).to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::GONE);
    let body = test::read_body(resp).await;
    assert!(
        String::from_utf8(body.to_vec())
            .unwrap()
            .contains("already been accepted")
    );
}
#[actix_web::test]
async fn expired_namespace_invitation_link_returns_gone() {
    let (app, _, _, _, db, _) = create_admin_test_service().await;
    let namespace = db
        .get_namespace_by_name("acme")
        .await
        .expect("load namespace")
        .expect("namespace exists");
    let invitation = db
        .create_namespace_invitation(&auth::NewNamespaceInvitation {
            email: "expired@example.com".to_string(),
            namespace_id: namespace.id,
            role: auth::NamespaceRole::Contributor,
            expires_at: Some((chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339()),
        })
        .await
        .expect("create expired invitation");

    let req = test::TestRequest::get()
        .uri(&format!("/auth/accept-invite/{}", invitation.token))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::GONE);
    let body = test::read_body(resp).await;
    assert!(
        String::from_utf8(body.to_vec())
            .unwrap()
            .contains("This invitation has expired")
    );
}
#[actix_web::test]
async fn test_login_page_loads() {
    let app = create_test_service().await;
    let req = test::TestRequest::get().uri("/auth/login").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("<title>Log In · Twig</title>"));
    assert!(body_str.contains("Log In"));
    assert!(body_str.contains("Username"));
}
#[actix_web::test]
async fn test_namespace_page_loads() {
    let app = create_test_service().await;
    let req = test::TestRequest::get().uri("/auth/namespace").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("<title>Create Namespace · Twig</title>"));
    assert!(body_str.contains("Create Namespace"));
}
#[actix_web::test]
async fn test_generate_invite_with_invalid_api_key() {
    let app = create_test_service().await;
    let req = test::TestRequest::post()
        .uri("/auth/invite")
        .set_form([("api_key", "invalid_key")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("Invalid API key"));
}
#[actix_web::test]
async fn test_generate_invite_with_valid_api_key() {
    let app = create_test_service().await;
    let req = test::TestRequest::post()
        .uri("/auth/invite")
        .set_form([("api_key", "secure")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("Invite Generated!"));
}
#[actix_web::test]
async fn test_signup_with_invalid_invite() {
    let app = create_test_service().await;
    let req = test::TestRequest::post()
        .uri("/auth/signup")
        .set_form([
            ("invite", "invalid-invite-code"),
            ("username", "uiuser"),
            ("email", "uiuser@example.com"),
            ("password", "password123"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("Invalid invite"));
}

#[actix_web::test]
async fn signup_invite_is_consumed_once_and_replay_creates_no_second_account() {
    let root = format!("/tmp/test_twig_signup_invite_once_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let generated = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/invite")
            .set_form([("api_key", "secure")])
            .to_request(),
    )
    .await;
    assert_eq!(generated.status(), StatusCode::OK);
    let invite_html = String::from_utf8(test::read_body(generated).await.to_vec()).unwrap();
    let invite = extract_invite(&invite_html);

    let first = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/signup")
            .set_form([
                ("invite", invite.as_str()),
                ("username", "firstsignup"),
                ("email", "firstsignup@example.test"),
                ("password", "password123"),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let replay = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/signup")
            .set_form([
                ("invite", invite.as_str()),
                ("username", "secondsignup"),
                ("email", "secondsignup@example.test"),
                ("password", "password123"),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
    let first_user = db
        .get_user_by_username("firstsignup")
        .await
        .unwrap()
        .expect("first signup should create an account");
    assert!(
        db.get_user_by_username("secondsignup")
            .await
            .unwrap()
            .is_none()
    );
    let invite = db.get_invite(&invite).await.unwrap().unwrap();
    assert!(invite.used);
    assert_eq!(invite.user_id.as_deref(), Some(first_user.id.as_str()));

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn login_and_invite_generation_are_rate_limited() {
    let app = create_test_service().await;

    for _ in 0..10 {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/auth/login")
                .set_form([("username", "unknown-user"), ("password", "wrong")])
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let throttled_login = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_form([("username", "unknown-user"), ("password", "wrong")])
            .to_request(),
    )
    .await;
    assert_eq!(throttled_login.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(throttled_login.headers().get("retry-after").unwrap(), "60");

    for _ in 0..10 {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/auth/invite")
                .set_form([("api_key", "wrong-key")])
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let throttled_invite = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/invite")
            .set_form([("api_key", "wrong-key")])
            .to_request(),
    )
    .await;
    assert_eq!(throttled_invite.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(throttled_invite.headers().get("retry-after").unwrap(), "60");
}

#[actix_web::test]
async fn same_origin_guard_rejects_cross_origin_login_and_namespace_changes() {
    let project_root = format!("/tmp/test_twig_csrf_root_{}", uuid::Uuid::new_v4());
    let db_path = format!("/tmp/test_twig_csrf_{}.db", uuid::Uuid::new_v4());
    let config = config::Server::new(
        ("127.0.0.1".to_string(), 8080),
        "debug".to_string(),
        project_root.clone(),
        db_path.clone(),
        "secure".to_string(),
        false,
    )
    .with_resend_settings(
        None,
        config::DEFAULT_RESEND_FROM.to_string(),
        Some("http://localhost".to_string()),
    );
    let db = Database::new(&db_path);
    db.init_tables().await.unwrap();
    let user = auth::create_user(
        "csrfowner".to_string(),
        "csrfowner@example.test".to_string(),
        "password123",
    )
    .unwrap();
    db.create_user(&user).await.unwrap();
    let auth_state = web::Data::new(auth::TwigContext::new(db.clone(), "secure".to_string()));
    let session = auth_state.create_session(user.id).await.unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(config))
            .app_data(auth_state)
            .app_data(web::Data::new(
                crate::http::body::RequestBodyLimits::default(),
            ))
            .app_data(web::Data::new(
                crate::http::auth::device::DeviceAuthorizationRateLimiter::default(),
            ))
            .app_data(web::Data::new(
                crate::http::auth::rate_limit::AuthRateLimiter::default(),
            ))
            .app_data(web::PayloadConfig::new(
                crate::http::body::DEFAULT_SMALL_BODY_LIMIT,
            ))
            .wrap(actix_identity::IdentityMiddleware::default())
            .wrap(crate::auth::session_store::middleware(
                db.clone(),
                actix_web::cookie::Key::generate(),
            ))
            .wrap(actix_web::middleware::from_fn(
                crate::http::security::same_origin_guard,
            ))
            .wrap(actix_web::middleware::from_fn(
                crate::http::security::security_headers,
            ))
            .configure(crate::http::routes::configure_routes),
    )
    .await;

    let cross_origin_login = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .insert_header(("Origin", "https://attacker.example"))
            .set_form([("username", "victim"), ("password", "invalid")])
            .to_request(),
    )
    .await;
    assert_eq!(cross_origin_login.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        cross_origin_login
            .headers()
            .get("x-content-type-options")
            .unwrap(),
        "nosniff"
    );
    assert_eq!(
        cross_origin_login.headers().get("cache-control").unwrap(),
        "no-store"
    );
    assert!(
        cross_origin_login
            .headers()
            .contains_key("content-security-policy")
    );

    let cross_origin_change = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/namespace")
            .cookie(actix_web::cookie::Cookie::new("session", session.clone()))
            .insert_header(("Origin", "https://attacker.example"))
            .set_form([("name", "cross-origin-space")])
            .to_request(),
    )
    .await;
    assert_eq!(cross_origin_change.status(), StatusCode::FORBIDDEN);
    assert!(
        db.get_namespace_by_name("cross-origin-space")
            .await
            .unwrap()
            .is_none()
    );

    let same_origin_change = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/namespace")
            .cookie(actix_web::cookie::Cookie::new("session", session))
            .insert_header(("Origin", "http://localhost"))
            .set_form([("name", "same-origin-space")])
            .to_request(),
    )
    .await;
    assert_eq!(same_origin_change.status(), StatusCode::OK);
    assert!(
        db.get_namespace_by_name("same-origin-space")
            .await
            .unwrap()
            .is_some()
    );

    let _ = std::fs::remove_dir_all(project_root);
    let _ = std::fs::remove_file(db_path);
}

#[actix_web::test]
async fn login_response_does_not_distinguish_unknown_user_from_wrong_password() {
    let root = format!("/tmp/test_twig_login_enumeration_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let user = auth::create_user(
        "known-login-user".to_string(),
        "known@example.test".to_string(),
        "correct password",
    )
    .unwrap();
    db.create_user(&user).await.unwrap();

    let unknown = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_form([("username", "unknown-login-user"), ("password", "wrong")])
            .to_request(),
    )
    .await;
    let unknown_status = unknown.status();
    let unknown_body = test::read_body(unknown).await;
    let wrong_password = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_form([("username", "known-login-user"), ("password", "wrong")])
            .to_request(),
    )
    .await;

    assert_eq!(unknown_status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_password.status(), unknown_status);
    assert_eq!(test::read_body(wrong_password).await, unknown_body);
    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn dynamic_responses_receive_security_headers_without_overriding_asset_cache() {
    let app = create_test_service().await;

    let page = test::call_service(
        &app,
        test::TestRequest::get().uri("/auth/login").to_request(),
    )
    .await;
    assert_eq!(page.status(), StatusCode::OK);
    assert_eq!(
        page.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert_eq!(page.headers().get("x-frame-options").unwrap(), "DENY");
    assert_eq!(
        page.headers().get("referrer-policy").unwrap(),
        "no-referrer"
    );
    assert_eq!(page.headers().get("cache-control").unwrap(), "no-store");
    let csp = page
        .headers()
        .get("content-security-policy")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(csp.contains("object-src 'none'"), "{csp}");
    assert!(csp.contains("frame-ancestors 'none'"), "{csp}");

    let asset = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/assets/twig.svg")
            .to_request(),
    )
    .await;
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(
        asset.headers().get("cache-control").unwrap(),
        "no-cache",
        "route-specific asset cache policy takes precedence over the default"
    );
    assert_eq!(
        asset.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
}

#[actix_web::test]
async fn test_login_with_wrong_password() {
    let app = create_test_service().await;
    let req = test::TestRequest::post()
        .uri("/auth/login")
        .set_form([("username", "nonexistent"), ("password", "wrongpassword")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("Invalid credentials"));
}
#[actix_web::test]
async fn test_create_namespace_without_session() {
    let app = create_test_service().await;
    let req = test::TestRequest::post()
        .uri("/auth/namespace")
        .set_form([("name", "shouldfail_ns")])
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("Not logged in"));
}
