use crate::auth::{Invite, generate_token};
use crate::db::Database;

#[test]
fn test_generate_token_format() {
    let token = generate_token();
    // Token should be 64 hex characters (32 bytes * 2)
    assert_eq!(token.len(), 64);
    // Token should only contain hex characters
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_invite_struct_with_none_user_id() {
    let invite = Invite {
        id: generate_token(),
        user_id: None,
        used: false,
        created_at: chrono::Utc::now().to_rfc3339(),
        used_at: None,
    };

    assert!(invite.user_id.is_none());
    assert!(!invite.used);
}

#[test]
fn test_invite_struct_with_some_user_id() {
    let invite = Invite {
        id: generate_token(),
        user_id: Some("user-123".to_string()),
        used: true,
        created_at: chrono::Utc::now().to_rfc3339(),
        used_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    assert_eq!(invite.user_id, Some("user-123".to_string()));
    assert!(invite.used);
}

#[tokio::test]
async fn test_create_invite_with_null_user_id() {
    // Create a temporary database for testing
    let db_path = format!("/tmp/test_twig_db_{}.db", generate_token());
    let db = Database::new(&db_path);

    // Initialize tables for testing
    db.init_tables()
        .await
        .expect("Failed to initialize database tables");

    // Create a invite with NULL user_id (for signup)
    let invite = Invite {
        id: generate_token(),
        user_id: None,
        used: false,
        created_at: chrono::Utc::now().to_rfc3339(),
        used_at: None,
    };

    // This should succeed now with the migration
    let result = db.create_invite(&invite).await;
    assert!(
        result.is_ok(),
        "Failed to create invite with NULL user_id: {:?}",
        result.err()
    );

    // Verify we can retrieve the invite
    let retrieved = db
        .get_invite(&invite.id)
        .await
        .expect("Failed to get invite");
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.id, invite.id);
    assert!(retrieved.user_id.is_none());
    assert!(!retrieved.used);

    // Cleanup
    let _ = std::fs::remove_file(&db_path);
}
