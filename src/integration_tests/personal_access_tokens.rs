use super::repo_content::init_repo_with_files;
use super::support::*;
use super::*;
use crate::auth::NamespacePermission;

fn extract_personal_access_token(html: &str) -> String {
    html.split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .find(|value| value.starts_with("twigt_") && value.len() > "twigt_".len())
        .expect("new token secret in response")
        .to_string()
}

fn basic_authorization(username: &str, password: &str) -> String {
    use base64::Engine;
    let credentials =
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
    format!("Basic {credentials}")
}

async fn create_user_session(db: &Database, username: &str) -> (auth::User, String) {
    let user = auth::create_user(
        username.to_string(),
        format!("{username}@example.com"),
        "password123",
    )
    .expect("create test user");
    db.create_user(&user).await.expect("persist test user");
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let session = auth_state
        .create_session(user.id.clone())
        .await
        .expect("create test session");
    (user, session)
}

#[actix_web::test]
async fn account_tokens_create_list_and_revoke_real_scoped_token() {
    let root = format!("/tmp/test_twig_token_lifecycle_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, session) = create_user_session(&db, "token-lifecycle-owner").await;
    let namespace = auth::create_namespace("token-lifecycle-ns".to_string(), user.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create namespace");

    let anonymous = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/settings/tokens")
            .to_request(),
    )
    .await;
    assert_eq!(anonymous.status(), StatusCode::OK);
    assert!(String::from_utf8_lossy(&test::read_body(anonymous).await).contains("Not logged in"));

    let page = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/settings/tokens")
            .cookie(actix_web::cookie::Cookie::new("session", session.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(page.status(), StatusCode::OK);
    let body = String::from_utf8_lossy(&test::read_body(page).await).into_owned();
    for expected in [
        "Access Tokens",
        "token-lifecycle-ns",
        "All namespaces I can access",
        "Select all read permissions",
        "Select all write permissions",
    ] {
        assert!(body.contains(expected), "missing {expected}: {body}");
    }

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/settings/tokens")
            .cookie(actix_web::cookie::Cookie::new("session", session.clone()))
            .set_form([
                ("name".to_string(), "Build server".to_string()),
                (format!("namespace_{}", namespace.id), "true".to_string()),
                ("permission_view_namespace".to_string(), "true".to_string()),
                ("permission_push_changes".to_string(), "true".to_string()),
            ])
            .to_request(),
    )
    .await;
    let created_status = created.status();
    let body = String::from_utf8_lossy(&test::read_body(created).await).into_owned();
    assert_eq!(created_status, StatusCode::OK, "{body}");
    assert!(body.contains("COPY THIS TOKEN NOW"), "{body}");
    assert!(body.contains("Build server"), "{body}");
    let secret = extract_personal_access_token(&body);
    assert!(secret.starts_with("twigt_"));

    let tokens = db
        .list_personal_access_tokens(&user.id)
        .await
        .expect("list persisted token");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].name, "Build server");
    assert_eq!(tokens[0].token_hint, &secret[secret.len() - 4..]);
    assert_eq!(tokens[0].expires_at, None);
    assert_eq!(tokens[0].scopes.len(), 1);
    assert_eq!(tokens[0].scopes[0].namespace_id, namespace.id);
    assert_eq!(tokens[0].scopes[0].permissions.iter().count(), 2);
    let mut rows = db
        .conn()
        .await
        .expect("connect to token database")
        .query(
            "SELECT token_hash FROM personal_access_tokens WHERE id = ?1",
            crate::db::sqlite::params![tokens[0].id.clone()],
        )
        .await
        .expect("read token hash");
    let stored_hash = rows
        .next()
        .await
        .expect("read token hash row")
        .expect("token row exists")
        .get::<String>(0)
        .expect("stored hash");
    assert_eq!(stored_hash, auth::hash_personal_access_token(&secret));
    assert_ne!(stored_hash, secret);

    let listing = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/settings/tokens")
            .cookie(actix_web::cookie::Cookie::new("session", session.clone()))
            .to_request(),
    )
    .await;
    let body = String::from_utf8_lossy(&test::read_body(listing).await).into_owned();
    assert!(
        !body.contains(&secret),
        "secret must not be shown again: {body}"
    );
    assert!(body.contains(&tokens[0].token_hint));
    assert!(body.contains("View namespace"));
    assert!(body.contains("Push changes"));

    let (other_user, other_session) = create_user_session(&db, "token-lifecycle-other").await;
    let other_listing = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/settings/tokens")
            .cookie(actix_web::cookie::Cookie::new("session", other_session))
            .to_request(),
    )
    .await;
    let body = String::from_utf8_lossy(&test::read_body(other_listing).await).into_owned();
    assert!(body.contains("NO TOKENS"));
    assert!(!body.contains(&tokens[0].token_hint));
    assert_ne!(user.id, other_user.id);

    let revoked = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/settings/tokens/{}/revoke", tokens[0].id))
            .cookie(actix_web::cookie::Cookie::new("session", session))
            .to_request(),
    )
    .await;
    assert_eq!(revoked.status(), StatusCode::OK);
    let body = String::from_utf8_lossy(&test::read_body(revoked).await).into_owned();
    assert!(body.contains("Access token revoked"));
    assert!(body.contains("NO TOKENS"));
    assert!(
        db.list_personal_access_tokens(&user.id)
            .await
            .expect("list after revoke")
            .is_empty()
    );

    drop(app);
    let _ = std::fs::remove_dir_all(root);
}

#[actix_web::test]
async fn all_namespace_and_permission_group_choices_expand_to_explicit_grants() {
    let root = format!("/tmp/test_twig_token_all_scopes_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, session) = create_user_session(&db, "token-all-scopes-owner").await;
    for name in ["token-all-one", "token-all-two"] {
        let namespace = auth::create_namespace(name.to_string(), user.id.clone());
        db.create_namespace(&namespace)
            .await
            .expect("create namespace");
    }

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/settings/tokens")
            .cookie(actix_web::cookie::Cookie::new("session", session))
            .set_form([
                ("name", "All namespace token"),
                ("all_namespaces", "true"),
                ("all_read", "true"),
                ("all_write", "true"),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let tokens = db
        .list_personal_access_tokens(&user.id)
        .await
        .expect("list token");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].scopes.len(), 2);
    for scope in &tokens[0].scopes {
        assert_eq!(
            scope.permissions.iter().count(),
            NamespacePermission::ALL.len()
        );
    }

    drop(app);
    let _ = std::fs::remove_dir_all(root);
}

#[actix_web::test]
async fn token_creation_rejects_unavailable_permissions_and_foreign_namespaces() {
    let root = format!("/tmp/test_twig_token_denied_scope_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, _) = create_user_session(&db, "token-scope-owner").await;
    let (contributor, session) = create_user_session(&db, "token-scope-contributor").await;
    let namespace = auth::create_namespace("token-scope-ns".to_string(), owner.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create namespace");
    db.conn()
        .await
        .expect("connect to membership database")
        .execute(
            "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
             VALUES (?1, ?2, 'contributor', ?3)",
            crate::db::sqlite::params![
                namespace.id.clone(),
                contributor.id.clone(),
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .await
        .expect("add contributor membership");

    let forbidden_permission = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/settings/tokens")
            .cookie(actix_web::cookie::Cookie::new("session", session.clone()))
            .set_form([
                ("name".to_string(), "Too powerful".to_string()),
                (format!("namespace_{}", namespace.id), "true".to_string()),
                ("permission_manage_members".to_string(), "true".to_string()),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(forbidden_permission.status(), StatusCode::BAD_REQUEST);

    let foreign_namespace = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/settings/tokens")
            .cookie(actix_web::cookie::Cookie::new("session", session))
            .set_form([
                ("name".to_string(), "Foreign namespace".to_string()),
                ("namespace_not-accessible".to_string(), "true".to_string()),
                ("permission_view_namespace".to_string(), "true".to_string()),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(foreign_namespace.status(), StatusCode::BAD_REQUEST);
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
async fn token_create_and_revoke_require_a_browser_session() {
    let app = create_test_service().await;
    let create = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/settings/tokens")
            .set_form([("name", "Build server")])
            .to_request(),
    )
    .await;
    assert_eq!(create.status(), StatusCode::UNAUTHORIZED);

    let revoke = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/settings/tokens/demo-token/revoke")
            .to_request(),
    )
    .await;
    assert_eq!(revoke.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn bearer_and_basic_token_password_resolve_owner_and_never_fall_back() {
    use crate::auth::{CredentialAuth, NamespacePermission, PermissionSet};

    let db_path = format!(
        "/tmp/test_twig_token_credentials_{}.db",
        uuid::Uuid::new_v4()
    );
    let db = Database::new(&db_path);
    db.init_tables().await.expect("initialize database");
    let user = auth::create_user(
        "credential-owner".to_string(),
        "credential-owner@example.com".to_string(),
        "password123",
    )
    .expect("create token owner");
    db.create_user(&user).await.expect("store token owner");
    let namespace = auth::create_namespace("credential-space".to_string(), user.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create namespace");

    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let (_, token) = auth_state
        .issue_personal_access_token(
            &user.id,
            "Git push",
            &[namespace.id],
            &PermissionSet::new([NamespacePermission::PushChanges]),
            None,
        )
        .await
        .expect("create token");

    let bearer = format!("Bearer {token}");
    let bearer_req = test::TestRequest::default()
        .insert_header(("Authorization", bearer.as_str()))
        .to_http_request();
    let bearer_auth = auth_state
        .authenticate_request(&bearer_req)
        .await
        .expect("resolve bearer");
    let CredentialAuth::Authenticated(principal) = bearer_auth else {
        panic!("valid bearer should authenticate");
    };
    assert_eq!(principal.user.id, user.id);
    assert!(
        principal.token_permissions.as_ref().unwrap()["credential-space"]
            .contains(NamespacePermission::PushChanges)
    );

    let basic = basic_authorization("not-the-owner", &token);
    let basic_req = test::TestRequest::default()
        .insert_header(("Authorization", basic.as_str()))
        .to_http_request();
    let basic_auth = auth_state
        .authenticate_request(&basic_req)
        .await
        .expect("resolve token password");
    let CredentialAuth::Authenticated(basic_principal) = basic_auth else {
        panic!("valid Basic token password should authenticate");
    };
    assert_eq!(basic_principal.user.id, user.id);

    let password_user = auth::create_user(
        "credential-fallback".to_string(),
        "credential-fallback@example.com".to_string(),
        "twigt_not_registered",
    )
    .expect("create fallback test user");
    db.create_user(&password_user)
        .await
        .expect("store fallback test user");
    let invalid_token_password = basic_authorization("credential-fallback", "twigt_not_registered");
    let invalid_password_req = test::TestRequest::default()
        .insert_header(("Authorization", invalid_token_password.as_str()))
        .to_http_request();
    assert!(matches!(
        auth_state
            .authenticate_request(&invalid_password_req)
            .await
            .expect("reject unknown token password"),
        CredentialAuth::Invalid
    ));

    let regular_password = basic_authorization("credential-owner", "password123");
    let regular_password_req = test::TestRequest::default()
        .insert_header(("Authorization", regular_password.as_str()))
        .to_http_request();
    let regular_auth = auth_state
        .authenticate_request(&regular_password_req)
        .await
        .expect("preserve ordinary Basic auth");
    let CredentialAuth::Authenticated(regular_principal) = regular_auth else {
        panic!("existing password auth should continue to work");
    };
    assert!(regular_principal.token_permissions.is_none());

    let _ = std::fs::remove_file(db_path);
}

#[actix_web::test]
async fn repository_init_and_binary_upload_enforce_token_permissions() {
    use crate::auth::PermissionSet;

    let root = format!(
        "/tmp/test_twig_token_protected_ops_{}",
        uuid::Uuid::new_v4()
    );
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, _) = create_user_session(&db, "token-operations-owner").await;
    let namespace = auth::create_namespace("token-operations".to_string(), user.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create namespace");
    init_repo_with_files(
        &std::path::Path::new(&root).join("token-operations/binaries"),
        "\n",
        &[("README.md", "Binary uploads are enabled.\n")],
    );

    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let namespace_ids = [namespace.id.clone()];
    let (_, create_token) = auth_state
        .issue_personal_access_token(
            &user.id,
            "Repository initialization",
            &namespace_ids,
            &PermissionSet::new([NamespacePermission::CreateRepositories]),
            None,
        )
        .await
        .expect("create repository token");
    let (_, push_token) = auth_state
        .issue_personal_access_token(
            &user.id,
            "Binary upload",
            &namespace_ids,
            &PermissionSet::new([NamespacePermission::PushChanges]),
            None,
        )
        .await
        .expect("create push token");
    let (_, read_token) = auth_state
        .issue_personal_access_token(
            &user.id,
            "Read only",
            &namespace_ids,
            &PermissionSet::new([NamespacePermission::ViewNamespace]),
            None,
        )
        .await
        .expect("create read token");

    let initialized = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/init")
            .insert_header(("Authorization", format!("Bearer {create_token}")))
            .set_form([
                ("namespace", "token-operations"),
                ("repo", "initialized-bearer"),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(initialized.status(), StatusCode::OK);

    let basic_create = basic_authorization("not-the-owner", &create_token);
    let initialized_with_password_token = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/init")
            .insert_header(("Authorization", basic_create.as_str()))
            .set_form([
                ("namespace", "token-operations"),
                ("repo", "initialized-basic"),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(initialized_with_password_token.status(), StatusCode::OK);

    let denied_init = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/init")
            .insert_header(("Authorization", format!("Bearer {read_token}")))
            .set_form([("namespace", "token-operations"), ("repo", "not-allowed")])
            .to_request(),
    )
    .await;
    assert_eq!(denied_init.status(), StatusCode::FORBIDDEN);

    let allowed_upload = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/token-operations/binaries/binaries/1.0.0/package")
            .insert_header(("Authorization", format!("Bearer {push_token}")))
            .set_payload(b"binary payload".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(allowed_upload.status(), StatusCode::CREATED);

    let denied_upload = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/token-operations/binaries/binaries/2.0.0/package")
            .insert_header(("Authorization", format!("Bearer {read_token}")))
            .set_payload(b"should be denied".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(denied_upload.status(), StatusCode::FORBIDDEN);

    drop(app);
    let _ = std::fs::remove_dir_all(root);
}
