use super::*;

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
