use super::*;

#[test]
fn test_server_new() {
    let server = Server::new(
        ("127.0.0.1".to_string(), 8080),
        "debug".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "mykey".to_string(),
        false,
    );
    assert_eq!(server.address(), ("127.0.0.1".to_string(), 8080));
    assert_eq!(server.log_level(), "debug");
    assert_eq!(server.project_root(), "/srv/git");
    assert_eq!(server.db_path(), "twig.db");
    assert_eq!(
        server.cache_control(),
        &HeaderValue::from_static(DEFAULT_CACHE_CONTROL)
    );
}

#[test]
fn test_server_cache_control_round_trip() {
    let server = Server::new(
        ("127.0.0.1".to_string(), 8080),
        "debug".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "mykey".to_string(),
        false,
    )
    .with_cache_control(HeaderValue::from_static("no-cache"));

    assert_eq!(
        server.cache_control(),
        &HeaderValue::from_static("no-cache")
    );
}

#[test]
fn test_server_clone() {
    let server = Server::new(
        ("0.0.0.0".to_string(), 80),
        "info".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "apikey".to_string(),
        false,
    );
    let cloned = server.clone();
    assert_eq!(cloned.project_root(), server.project_root());
    assert_eq!(cloned.log_level(), server.log_level());
}

#[test]
fn test_effective_api_key_with_set_key() {
    let server = Server::new(
        ("0.0.0.0".to_string(), 80),
        "info".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "my-secret-key".to_string(),
        false,
    );
    assert_eq!(server.effective_api_key(), "my-secret-key");
}

#[test]
fn test_effective_api_key_generates_when_empty() {
    let server = Server::new(
        ("0.0.0.0".to_string(), 80),
        "info".to_string(),
        "/srv/git".to_string(),
        String::new(),
        String::new(),
        false,
    );
    let key = server.effective_api_key();
    assert!(!key.is_empty());
    assert_eq!(key.len(), 64);
}

#[test]
fn test_generated_api_key_warning_omits_the_secret() {
    let mut warnings = Vec::new();
    let key = resolve_api_key("", |warning| warnings.push(warning.to_string()));

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("API_KEY"));
    assert!(warnings[0].contains("secret omitted"));
    assert!(!warnings[0].contains(&key));
    assert_eq!(key.len(), 64);

    let mut configured_key_was_logged = false;
    assert_eq!(
        resolve_api_key("configured-secret", |_| configured_key_was_logged = true),
        "configured-secret"
    );
    assert!(!configured_key_was_logged);
}

#[test]
fn test_missing_api_key_is_rejected_in_release_builds() {
    assert!(validate_api_key_mode("", true).is_ok());
    assert!(validate_api_key_mode("configured-secret", false).is_ok());
    assert!(validate_api_key_mode("", false).is_err());
}

#[test]
fn test_maybe_reset_database_no_flag() {
    let db_path = std::env::temp_dir().join(format!("twig_no_reset_{}.db", uuid::Uuid::new_v4()));
    std::fs::write(&db_path, "keep this database").unwrap();
    let server = Server::new(
        ("0.0.0.0".to_string(), 80),
        "info".to_string(),
        "/srv/git".to_string(),
        db_path.to_string_lossy().into_owned(),
        "key".to_string(),
        false,
    );
    server.maybe_reset_database().unwrap();
    assert_eq!(
        std::fs::read_to_string(&db_path).unwrap(),
        "keep this database"
    );
    let _ = std::fs::remove_file(db_path);
}

#[test]
fn test_maybe_reset_database_preserves_sentinel_for_unsafe_configuration() {
    for (bind_address, port) in [("0.0.0.0", 8080), ("127.0.0.1", 80)] {
        let db_path =
            std::env::temp_dir().join(format!("twig_reset_sentinel_{}.db", uuid::Uuid::new_v4()));
        std::fs::write(&db_path, "keep this database").unwrap();
        let server = Server::new(
            (bind_address.to_string(), port),
            "info".to_string(),
            "/srv/git".to_string(),
            db_path.to_string_lossy().into_owned(),
            "key".to_string(),
            true,
        );

        assert!(server.maybe_reset_database().is_err());

        assert_eq!(
            std::fs::read_to_string(&db_path).unwrap(),
            "keep this database",
            "reset mode must fail closed for bind {bind_address}:{port}"
        );
        let _ = std::fs::remove_file(db_path);
    }
}

#[test]
fn test_reset_db_requires_debug_build_loopback_and_non_80_port() {
    assert!(validate_reset_db_mode(false, "0.0.0.0", 80, false).is_ok());
    assert!(validate_reset_db_mode(true, "127.0.0.1", 8080, true).is_ok());
    assert!(validate_reset_db_mode(true, "::1", 8080, true).is_ok());
    assert!(validate_reset_db_mode(true, "127.0.0.1", 80, true).is_err());
    assert!(validate_reset_db_mode(true, "0.0.0.0", 8080, true).is_err());
    assert!(validate_reset_db_mode(true, "127.0.0.1", 8080, false).is_err());
}

#[test]
fn test_maybe_reset_database_runs_only_in_allowed_local_development_mode() {
    let db_path =
        std::env::temp_dir().join(format!("twig_reset_allowed_{}.db", uuid::Uuid::new_v4()));
    std::fs::write(&db_path, "temporary database").unwrap();
    let server = Server::new(
        ("127.0.0.1".to_string(), 8080),
        "info".to_string(),
        "/srv/git".to_string(),
        db_path.to_string_lossy().into_owned(),
        "key".to_string(),
        true,
    );

    if cfg!(debug_assertions) {
        server.maybe_reset_database().unwrap();
        assert!(!db_path.exists());
    } else {
        assert!(server.maybe_reset_database().is_err());
        assert_eq!(
            std::fs::read_to_string(&db_path).unwrap(),
            "temporary database"
        );
        let _ = std::fs::remove_file(db_path);
    }
}

#[test]
fn test_address_returns_clone() {
    let server = Server::new(
        ("0.0.0.0".to_string(), 3000),
        "info".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "key".to_string(),
        false,
    );
    let addr = server.address();
    assert_eq!(addr, ("0.0.0.0".to_string(), 3000));
}

#[test]
fn test_server_test_user() {
    let server = Server::new(
        ("0.0.0.0".to_string(), 8080),
        "info".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "key".to_string(),
        false,
    );
    assert_eq!(server.test_user(), None);
    assert!(!server.is_test_user_enabled());
    assert!(!server.is_test_user("admin"));

    let server_with_admin = server.with_test_user(Some("admin".to_string()));
    assert_eq!(server_with_admin.test_user(), Some("admin"));
    assert!(server_with_admin.is_test_user_enabled());
    assert!(server_with_admin.is_test_user("admin"));
    assert!(!server_with_admin.is_test_user("alice"));
}

#[test]
fn test_server_admin_user_is_independent_from_test_user() {
    let server = Server::new(
        ("0.0.0.0".to_string(), 8080),
        "info".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "key".to_string(),
        false,
    )
    .with_admin_user(Some("root-user".to_string()));

    assert_eq!(server.admin_user(), Some("root-user"));
    assert!(server.is_configured_admin("root-user"));
    assert!(!server.is_configured_admin("admin"));
    assert!(!server.is_test_user_enabled());
}

#[test]
fn test_blank_admin_user_is_not_configured() {
    let server = Server::new(
        ("0.0.0.0".to_string(), 8080),
        "info".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "key".to_string(),
        false,
    )
    .with_admin_user(None);

    assert_eq!(server.admin_user(), None);
}

#[test]
fn test_resend_settings_are_optional_and_keep_sender_configuration() {
    let server = Server::new(
        ("0.0.0.0".to_string(), 8080),
        "info".to_string(),
        "/srv/git".to_string(),
        "twig.db".to_string(),
        "key".to_string(),
        false,
    );
    assert_eq!(server.resend_api_key(), None);
    assert_eq!(server.resend_from(), DEFAULT_RESEND_FROM);
    assert_eq!(server.public_base_url(), None);

    let configured = server.with_resend_settings(
        Some("re_example_secret".to_string()),
        "Twig <invites@example.com>".to_string(),
        Some("https://twig.example".to_string()),
    );
    assert_eq!(configured.resend_api_key(), Some("re_example_secret"));
    assert_eq!(configured.resend_from(), "Twig <invites@example.com>");
    assert_eq!(configured.public_base_url(), Some("https://twig.example"));

    let blank = configured.with_resend_settings(
        Some("  ".to_string()),
        DEFAULT_RESEND_FROM.to_string(),
        None,
    );
    assert_eq!(blank.resend_api_key(), None);
}
