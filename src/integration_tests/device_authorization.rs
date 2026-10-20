use std::collections::HashMap;

use super::support::*;
use super::*;
use crate::auth::NamespacePermission;

fn extract_token(body: &serde_json::Value) -> String {
    body["access_token"]
        .as_str()
        .expect("device poll returns token")
        .to_string()
}

fn extract_hidden_value(html: &str, name: &str) -> String {
    let marker = format!("name=\"{name}\" value=\"");
    let start = html
        .find(&marker)
        .unwrap_or_else(|| panic!("missing hidden field {name}: {html}"))
        + marker.len();
    let end = start
        + html[start..]
            .find('"')
            .unwrap_or_else(|| panic!("unterminated hidden field {name}: {html}"));
    html[start..end].to_string()
}

fn basic_authorization(username: &str, password: &str) -> String {
    use base64::Engine;
    let credentials =
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
    format!("Basic {credentials}")
}

async fn create_owner_and_login<S>(
    app: &S,
    db: &Database,
    username: &str,
) -> (
    auth::User,
    auth::Namespace,
    actix_web::cookie::Cookie<'static>,
)
where
    S: actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let user = auth::create_user(
        username.to_string(),
        format!("{username}@example.com"),
        "password123",
    )
    .expect("create device test user");
    db.create_user(&user).await.expect("persist test user");
    let namespace = auth::create_namespace("device-space".to_string(), user.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create test namespace");

    let identity = login_user(app, username).await;
    (user, namespace, identity)
}

async fn login_user<S>(app: &S, username: &str) -> actix_web::cookie::Cookie<'static>
where
    S: actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let login = test::call_service(
        app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_form([("username", username), ("password", "password123")])
            .to_request(),
    )
    .await;
    assert!(login.status().is_success(), "login should succeed");
    login
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "id")
        .expect("login should issue the identity session cookie")
        .into_owned()
}

async fn start_device_authorization<S>(app: &S) -> serde_json::Value
where
    S: actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let response = test::call_service(
        app,
        test::TestRequest::post()
            .uri("/api/auth/device")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    test::read_body_json(response).await
}

async fn open_approval<S>(
    app: &S,
    user_code: &str,
    identity: &actix_web::cookie::Cookie<'static>,
) -> (String, actix_web::cookie::Cookie<'static>)
where
    S: actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let page = test::call_service(
        app,
        test::TestRequest::get()
            .uri(&format!("/auth/device?user_code={user_code}"))
            .cookie(identity.clone())
            .to_request(),
    )
    .await;
    assert_eq!(page.status(), StatusCode::OK);
    assert_eq!(
        page.headers()
            .get("Referrer-Policy")
            .and_then(|value| value.to_str().ok()),
        Some("no-referrer")
    );
    let updated_identity = page
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "id")
        .map_or_else(|| identity.clone(), actix_web::cookie::Cookie::into_owned);
    let body = String::from_utf8(test::read_body(page).await.to_vec()).unwrap();
    for permission in [
        "View namespace and repositories",
        "Push changes",
        "Create repositories",
        "Manage repositories",
        "Manage namespace",
        "Manage members",
        "Rewrite protected branches",
    ] {
        assert!(body.contains(permission), "missing {permission}: {body}");
    }
    (extract_hidden_value(&body, "csrf_token"), updated_identity)
}

#[actix_web::test]
async fn device_start_returns_hashed_codes_and_expected_poll_timing() {
    let root = format!("/tmp/test_twig_device_start_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/device")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("Cache-Control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    let body: serde_json::Value = test::read_body_json(response).await;
    let device_code = body["device_code"].as_str().expect("device code");
    let user_code = body["user_code"].as_str().expect("user code");
    assert_eq!(device_code.len(), 64);
    assert!(device_code.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(user_code.len(), 9);
    assert_eq!(user_code.as_bytes()[4], b'-');
    assert_eq!(body["verification_uri"], "/auth/device");
    assert_eq!(body["expires_in"], 600);
    assert_eq!(body["interval"], 5);

    let mut rows = db
        .conn()
        .await
        .expect("connect to test database")
        .query(
            "SELECT device_code_hash, user_code_hash FROM device_authorizations",
            (),
        )
        .await
        .expect("read challenge hashes");
    let row = rows.next().await.expect("read challenge row").unwrap();
    let stored_device_hash: String = row.get(0).expect("device hash");
    let stored_user_hash: String = row.get(1).expect("user hash");
    assert_eq!(
        stored_device_hash,
        auth::device::hash_device_code(device_code)
    );
    assert_eq!(
        stored_user_hash,
        auth::device::hash_device_code(&auth::device::normalize_user_code(user_code).unwrap())
    );
    assert_ne!(stored_device_hash, device_code);
    assert_ne!(stored_user_hash, user_code);

    drop(app);
    let _ = std::fs::remove_dir_all(root);
}

#[actix_web::test]
async fn anonymous_device_page_returns_through_login_to_the_same_code() {
    let root = format!(
        "/tmp/test_twig_device_login_return_{}",
        uuid::Uuid::new_v4()
    );
    let (app, db) = create_test_service_with_db_in(&root).await;
    let challenge = start_device_authorization(&app).await;
    let user_code = challenge["user_code"].as_str().unwrap().to_string();

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/auth/device?user_code={user_code}"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(
        response
            .headers()
            .get("Referrer-Policy")
            .and_then(|value| value.to_str().ok()),
        Some("no-referrer")
    );
    let location = response
        .headers()
        .get("Location")
        .and_then(|value| value.to_str().ok())
        .expect("login redirect")
        .to_string();
    assert_eq!(
        location,
        format!("/auth/login?return_to=%2Fauth%2Fdevice%3Fuser_code%3D{user_code}")
    );

    let login =
        test::call_service(&app, test::TestRequest::get().uri(&location).to_request()).await;
    assert_eq!(login.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(login).await.to_vec()).unwrap();
    assert!(body.contains(&format!(
        "name=\"return_to\" value=\"/auth/device?user_code={user_code}\""
    )));

    let username = "device-login-return";
    let user = auth::create_user(
        username.to_string(),
        "device-login-return@example.com".to_string(),
        "password123",
    )
    .expect("create login return user");
    db.create_user(&user)
        .await
        .expect("persist login return user");
    let return_to = format!("/auth/device?user_code={user_code}");
    let authenticated = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_form([
                ("username", username),
                ("password", "password123"),
                ("return_to", return_to.as_str()),
            ])
            .to_request(),
    )
    .await;
    assert!(authenticated.status().is_success());
    assert_eq!(
        authenticated
            .headers()
            .get("HX-Redirect")
            .and_then(|value| value.to_str().ok()),
        Some(return_to.as_str())
    );

    let unsafe_redirect = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_form([
                ("username", username),
                ("password", "password123"),
                ("return_to", "https://attacker.example/"),
            ])
            .to_request(),
    )
    .await;
    assert!(unsafe_redirect.status().is_success());
    assert!(unsafe_redirect.headers().get("HX-Redirect").is_none());

    drop(app);
    let _ = std::fs::remove_dir_all(root);
}

#[actix_web::test]
async fn device_approval_requires_session_csrf_and_issues_one_scoped_token() {
    let root = format!("/tmp/test_twig_device_approve_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, identity) = create_owner_and_login(&app, &db, "device-owner").await;
    let challenge = start_device_authorization(&app).await;
    let device_code = challenge["device_code"].as_str().unwrap().to_string();
    let user_code = challenge["user_code"].as_str().unwrap().to_string();
    let (csrf_token, identity) = open_approval(&app, &user_code, &identity).await;

    let invalid_form = HashMap::from([
        ("user_code".to_string(), user_code.clone()),
        ("csrf_token".to_string(), "invalid".to_string()),
        ("decision".to_string(), "approve".to_string()),
        (format!("namespace_{}", namespace.id), "true".to_string()),
        ("permission_view_namespace".to_string(), "true".to_string()),
        ("permission_push_changes".to_string(), "true".to_string()),
    ]);
    let invalid_csrf = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/device/approve")
            .cookie(identity.clone())
            .set_form(&invalid_form)
            .to_request(),
    )
    .await;
    assert_eq!(invalid_csrf.status(), StatusCode::FORBIDDEN);

    let approved_form = HashMap::from([
        ("user_code".to_string(), user_code.clone()),
        ("csrf_token".to_string(), csrf_token),
        ("decision".to_string(), "approve".to_string()),
        (format!("namespace_{}", namespace.id), "true".to_string()),
        ("permission_view_namespace".to_string(), "true".to_string()),
        ("permission_push_changes".to_string(), "true".to_string()),
    ]);
    let approved = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/device/approve")
            .cookie(identity)
            .set_form(&approved_form)
            .to_request(),
    )
    .await;
    assert_eq!(approved.status(), StatusCode::OK);

    let poll = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/device/token")
            .set_json(serde_json::json!({ "device_code": device_code }))
            .to_request(),
    )
    .await;
    assert_eq!(poll.status(), StatusCode::OK);
    let token_response: serde_json::Value = test::read_body_json(poll).await;
    assert_eq!(token_response["status"], "authorized");
    let secret = extract_token(&token_response);
    assert!(secret.starts_with("twigt_"));

    let tokens = db
        .list_personal_access_tokens(&user.id)
        .await
        .expect("list device-issued token");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].name, "Tree CLI");
    assert_eq!(tokens[0].expires_at, None);
    assert_eq!(tokens[0].scopes.len(), 1);
    assert_eq!(tokens[0].scopes[0].namespace_id, namespace.id);
    assert_eq!(
        tokens[0].scopes[0].permissions.iter().collect::<Vec<_>>(),
        vec![
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges
        ]
    );

    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let req = test::TestRequest::default()
        .insert_header(("Authorization", basic_authorization("ignored", &secret)))
        .to_http_request();
    let principal = match auth_state
        .authenticate_request(&req)
        .await
        .expect("authenticate device token")
    {
        auth::CredentialAuth::Authenticated(principal) => principal,
        other => panic!("expected authenticated token, got {other:?}"),
    };
    let effective = auth::permissions::for_principal(&db, &principal, &namespace.name, false)
        .await
        .expect("resolve effective permissions")
        .expect("namespace permission set");
    assert!(effective.contains(NamespacePermission::ViewNamespace));
    assert!(effective.contains(NamespacePermission::PushChanges));
    assert!(!effective.contains(NamespacePermission::ManageMembers));

    let second_poll = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/device/token")
            .set_json(serde_json::json!({ "device_code": challenge["device_code"] }))
            .to_request(),
    )
    .await;
    let second_body: serde_json::Value = test::read_body_json(second_poll).await;
    assert_eq!(second_body["status"], "expired");

    drop(app);
    let _ = std::fs::remove_dir_all(root);
}

#[actix_web::test]
async fn denied_device_authorization_never_creates_a_token() {
    let root = format!("/tmp/test_twig_device_deny_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, _namespace, identity) = create_owner_and_login(&app, &db, "device-deny-owner").await;
    let challenge = start_device_authorization(&app).await;
    let device_code = challenge["device_code"].as_str().unwrap().to_string();
    let user_code = challenge["user_code"].as_str().unwrap().to_string();
    let (csrf_token, identity) = open_approval(&app, &user_code, &identity).await;

    let denied = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/device/approve")
            .cookie(identity)
            .set_form([
                ("user_code", user_code.as_str()),
                ("csrf_token", csrf_token.as_str()),
                ("decision", "deny"),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::OK);

    let poll = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/device/token")
            .set_json(serde_json::json!({ "device_code": device_code }))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(poll).await;
    assert_eq!(body["status"], "denied");
    assert!(
        db.list_personal_access_tokens(&user.id)
            .await
            .expect("list tokens after denial")
            .is_empty()
    );

    drop(app);
    let _ = std::fs::remove_dir_all(root);
}

#[actix_web::test]
async fn device_approval_rejects_permissions_the_user_does_not_have() {
    let root = format!("/tmp/test_twig_device_permissions_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let owner = auth::create_user(
        "device-permission-owner".to_string(),
        "device-permission-owner@example.com".to_string(),
        "password123",
    )
    .expect("create namespace owner");
    db.create_user(&owner)
        .await
        .expect("persist namespace owner");
    let namespace = auth::create_namespace("device-permission-space".to_string(), owner.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create namespace");
    let contributor = auth::create_user(
        "device-permission-contributor".to_string(),
        "device-permission-contributor@example.com".to_string(),
        "password123",
    )
    .expect("create contributor");
    db.create_user(&contributor)
        .await
        .expect("persist contributor");
    db.conn()
        .await
        .expect("connect to membership database")
        .execute(
            "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
             VALUES (?1, ?2, 'contributor', ?3)",
            turso::params![
                namespace.id.clone(),
                contributor.id.clone(),
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .await
        .expect("add contributor membership");
    let identity = login_user(&app, "device-permission-contributor").await;

    let challenge = start_device_authorization(&app).await;
    let device_code = challenge["device_code"].as_str().unwrap().to_string();
    let user_code = challenge["user_code"].as_str().unwrap().to_string();
    let (csrf_token, identity) = open_approval(&app, &user_code, &identity).await;
    let form = HashMap::from([
        ("user_code".to_string(), user_code),
        ("csrf_token".to_string(), csrf_token),
        ("decision".to_string(), "approve".to_string()),
        (format!("namespace_{}", namespace.id), "true".to_string()),
        ("permission_manage_members".to_string(), "true".to_string()),
    ]);
    let approval = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/device/approve")
            .cookie(identity)
            .set_form(&form)
            .to_request(),
    )
    .await;
    assert_eq!(approval.status(), StatusCode::BAD_REQUEST);

    let poll = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/device/token")
            .set_json(serde_json::json!({ "device_code": device_code }))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(poll).await;
    assert_eq!(body["status"], "pending");
    assert!(
        db.list_personal_access_tokens(&contributor.id)
            .await
            .expect("list contributor tokens")
            .is_empty()
    );

    drop(app);
    let _ = std::fs::remove_dir_all(root);
}

#[actix_web::test]
async fn device_poll_denies_a_grant_if_namespace_access_was_revoked() {
    let root = format!(
        "/tmp/test_twig_device_revoked_scope_{}",
        uuid::Uuid::new_v4()
    );
    let (app, db) = create_test_service_with_db_in(&root).await;
    let owner = auth::create_user(
        "device-revoked-owner".to_string(),
        "device-revoked-owner@example.com".to_string(),
        "password123",
    )
    .expect("create namespace owner");
    db.create_user(&owner)
        .await
        .expect("persist namespace owner");
    let namespace = auth::create_namespace("device-revoked-space".to_string(), owner.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create namespace");
    let contributor = auth::create_user(
        "device-revoked-contributor".to_string(),
        "device-revoked-contributor@example.com".to_string(),
        "password123",
    )
    .expect("create contributor");
    db.create_user(&contributor)
        .await
        .expect("persist contributor");
    db.conn()
        .await
        .expect("connect to membership database")
        .execute(
            "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
             VALUES (?1, ?2, 'contributor', ?3)",
            turso::params![
                namespace.id.clone(),
                contributor.id.clone(),
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .await
        .expect("add contributor membership");
    let identity = login_user(&app, "device-revoked-contributor").await;
    let challenge = start_device_authorization(&app).await;
    let device_code = challenge["device_code"].as_str().unwrap().to_string();
    let user_code = challenge["user_code"].as_str().unwrap().to_string();
    let (csrf_token, identity) = open_approval(&app, &user_code, &identity).await;
    let form = HashMap::from([
        ("user_code".to_string(), user_code),
        ("csrf_token".to_string(), csrf_token),
        ("decision".to_string(), "approve".to_string()),
        (format!("namespace_{}", namespace.id), "true".to_string()),
        ("permission_view_namespace".to_string(), "true".to_string()),
    ]);
    let approved = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/device/approve")
            .cookie(identity)
            .set_form(&form)
            .to_request(),
    )
    .await;
    assert_eq!(approved.status(), StatusCode::OK);

    db.conn()
        .await
        .expect("connect to membership database")
        .execute(
            "DELETE FROM namespace_members WHERE namespace_id = ?1 AND user_id = ?2",
            turso::params![namespace.id.clone(), contributor.id.clone()],
        )
        .await
        .expect("revoke contributor membership");

    let poll = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/device/token")
            .set_json(serde_json::json!({ "device_code": device_code }))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(poll).await;
    assert_eq!(body["status"], "denied");
    assert!(
        db.list_personal_access_tokens(&contributor.id)
            .await
            .expect("list contributor tokens")
            .is_empty()
    );

    drop(app);
    let _ = std::fs::remove_dir_all(root);
}

#[actix_web::test]
async fn device_start_is_rate_limited_per_peer() {
    let app = create_test_service().await;
    for attempt in 0..10 {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/auth/device")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "attempt {attempt}");
    }
    let limited = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/device")
            .to_request(),
    )
    .await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[actix_web::test]
async fn invalid_device_code_lookups_are_rate_limited() {
    let app = create_test_service().await;
    for attempt in 0..10 {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/auth/device?user_code=ABCD-EFGH")
                .to_request(),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "attempt {attempt}"
        );
    }
    let limited = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/auth/device?user_code=ABCD-EFGH")
            .to_request(),
    )
    .await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
}
