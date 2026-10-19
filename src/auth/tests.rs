use super::*;

#[test]
fn test_hash_and_verify_password() {
    let hash = hash_password("testpassword").unwrap();
    assert!(verify_password("testpassword", &hash).unwrap());
    assert!(!verify_password("wrongpassword", &hash).unwrap());
}

#[test]
fn test_verify_password_invalid_hash() {
    assert!(verify_password("test", "not-a-hash").is_err());
}

#[test]
fn test_generate_token_is_hex() {
    let token = generate_token();
    assert_eq!(token.len(), 64);
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_generate_invite_is_uuid() {
    let invite = generate_invite();
    assert!(uuid::Uuid::parse_str(&invite).is_ok());
}

#[test]
fn test_generate_tokens_are_unique() {
    let t1 = generate_token();
    let t2 = generate_token();
    assert_ne!(t1, t2);
}

#[test]
fn personal_access_token_has_prefix_unique_secret_and_stable_hash() {
    let first = generate_personal_access_token();
    let second = generate_personal_access_token();

    assert!(first.starts_with(PERSONAL_ACCESS_TOKEN_PREFIX));
    assert!(second.starts_with(PERSONAL_ACCESS_TOKEN_PREFIX));
    assert_ne!(first, second);

    let hash = hash_personal_access_token(&first);
    assert_ne!(hash, first);
    assert_eq!(hash, hash_personal_access_token(&first));
    assert_eq!(hash.len(), 64);
}

#[test]
fn test_create_user_fields() {
    let user = create_user(
        "testuser".to_string(),
        "test@example.com".to_string(),
        "password123",
    )
    .unwrap();
    assert_eq!(user.username, "testuser");
    assert_eq!(user.email, Some("test@example.com".to_string()));
    assert!(!user.password_hash.is_empty());
    assert!(!user.id.is_empty());
}

#[test]
fn test_create_user_password_is_hashed() {
    let user = create_user(
        "testuser".to_string(),
        "test@example.com".to_string(),
        "password123",
    )
    .unwrap();
    assert_ne!(user.password_hash, "password123");
    assert!(verify_password("password123", &user.password_hash).unwrap());
}

#[test]
fn test_create_namespace_fields() {
    let ns = create_namespace("myns".to_string(), "user-1".to_string());
    assert_eq!(ns.name, "myns");
    assert_eq!(ns.owner_id, "user-1");
    assert!(!ns.id.is_empty());
}

#[test]
fn test_base64_decode_valid() {
    let result = base64_decode("dXNlcjpwYXNz");
    assert_eq!(result, Some("user:pass".to_string()));
}

#[test]
fn test_base64_decode_invalid() {
    let result = base64_decode("not-valid-base64!!!");
    assert!(result.is_none());
}

#[test]
fn test_base64_decode_empty() {
    let result = base64_decode("");
    assert_eq!(result, Some(String::new()));
}

#[tokio::test]
async fn test_twig_context_validate_api_key() {
    let db = Database::new("/tmp/test_twig_ctx_validate.db");
    let ctx = TwigContext::new(db, "my-api-key".to_string());
    assert!(ctx.validate_api_key("my-api-key"));
    assert!(!ctx.validate_api_key("wrong-key"));
}

#[tokio::test]
async fn test_twig_context_test_pin() {
    let db_path = format!("/tmp/test_twig_ctx_pin_{}.db", Uuid::new_v4());
    let db = Database::new(&db_path);
    db.init_tables().await.expect("init tables");
    let ctx = TwigContext::new(db, "key".to_string());
    assert_eq!(ctx.get_test_pin().await, None);
    assert!(!ctx.validate_test_pin("123456").await);

    ctx.set_test_pin("123456").await.expect("set test pin");
    assert_eq!(ctx.get_test_pin().await.as_deref(), Some("123456"));
    assert!(ctx.validate_test_pin("123456").await);
    assert!(!ctx.validate_test_pin("654321").await);
    assert!(!ctx.validate_test_pin("").await);

    ctx.clear_test_pin().await.expect("clear test pin");
    assert_eq!(ctx.get_test_pin().await, None);
    assert!(!ctx.validate_test_pin("123456").await);

    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn test_seed_dev_admin_creates_user_namespace_and_session() {
    let db_path = format!("/tmp/test_twig_seed_dev_admin_{}.db", Uuid::new_v4());
    let project_root = format!("/tmp/test_twig_seed_dev_admin_root_{}", Uuid::new_v4());
    let db = Database::new(&db_path);
    db.init_tables().await.expect("init tables");
    let ctx = TwigContext::new(db.clone(), "key".to_string());

    let token = seed_dev_admin(&ctx, &project_root)
        .await
        .expect("seed dev admin");

    let user_id = ctx
        .validate_token(&token)
        .await
        .expect("session token should be valid");
    let user = db
        .get_user_by_id(&user_id)
        .await
        .expect("get user")
        .expect("user should exist");
    assert_eq!(user.username, "admin");
    assert!(verify_password("admin", &user.password_hash).unwrap());

    let namespace = db
        .get_namespace_by_name("admin")
        .await
        .expect("get namespace")
        .expect("namespace should exist");
    assert_eq!(namespace.owner_id, user_id);

    // Cleanup
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(&project_root);
}

#[tokio::test]
async fn test_seed_dev_admin_is_idempotent() {
    let db_path = format!("/tmp/test_twig_seed_dev_admin_idem_{}.db", Uuid::new_v4());
    let project_root = format!("/tmp/test_twig_seed_dev_admin_idem_root_{}", Uuid::new_v4());
    let db = Database::new(&db_path);
    db.init_tables().await.expect("init tables");
    let ctx = TwigContext::new(db.clone(), "key".to_string());

    let token1 = seed_dev_admin(&ctx, &project_root).await.expect("seed 1");
    let token2 = seed_dev_admin(&ctx, &project_root).await.expect("seed 2");

    // Both tokens should be valid sessions for the same, single admin user.
    let user_id1 = ctx.validate_token(&token1).await.expect("token1 valid");
    let user_id2 = ctx.validate_token(&token2).await.expect("token2 valid");
    assert_eq!(user_id1, user_id2);

    let all_namespaces = db
        .get_all_namespaces_with_owners()
        .await
        .expect("list namespaces");
    assert_eq!(all_namespaces.len(), 1);

    // Cleanup
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(&project_root);
}
