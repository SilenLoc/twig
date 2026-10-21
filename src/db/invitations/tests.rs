use super::*;
use crate::auth::{Namespace, User};

async fn setup_db() -> (Database, String, Namespace) {
    let db_path = format!(
        "/tmp/test_twig_namespace_invites_{}.db",
        uuid::Uuid::new_v4()
    );
    let db = Database::new(&db_path);
    db.init_tables().await.expect("init tables");
    let user = User {
        id: uuid::Uuid::new_v4().to_string(),
        username: "invite-owner".to_string(),
        email: Some("owner@example.com".to_string()),
        password_hash: "hash".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    db.create_user(&user).await.expect("create owner");
    let namespace = crate::auth::create_namespace("invites-test".to_string(), user.id);
    db.create_namespace(&namespace)
        .await
        .expect("create namespace");
    (db, db_path, namespace)
}

#[tokio::test]
async fn create_get_and_scope_list_namespace_invitations() {
    let (db, db_path, namespace) = setup_db().await;
    let invite = db
        .create_namespace_invitation(&NewNamespaceInvitation {
            email: "sam@example.com".to_string(),
            namespace_id: namespace.id.clone(),
            role: NamespaceRole::Contributor,
            expires_at: None,
        })
        .await
        .expect("create invitation");

    assert_eq!(invite.email, "sam@example.com");
    assert_eq!(invite.namespace_name, "invites-test");
    assert_eq!(invite.role, NamespaceRole::Contributor);
    assert!(invite.expires_at.is_none());
    assert!(invite.accepted_at.is_none());
    assert_eq!(invite.token.len(), 64);

    let loaded = db
        .get_namespace_invitation(&invite.token)
        .await
        .expect("get invitation")
        .expect("invitation exists");
    assert_eq!(loaded.token, invite.token);

    let global = db
        .list_namespace_invitations(None)
        .await
        .expect("list all invitations");
    assert_eq!(global.len(), 1);
    let scoped = db
        .list_namespace_invitations(Some(std::slice::from_ref(&namespace.id)))
        .await
        .expect("list scoped invitations");
    assert_eq!(scoped.len(), 1);
    assert!(
        db.list_namespace_invitations(Some(&[]))
            .await
            .expect("empty namespace scope")
            .is_empty()
    );
    assert!(
        db.get_namespace_invitation("not-a-token")
            .await
            .expect("missing invite lookup")
            .is_none()
    );

    let _ = std::fs::remove_file(db_path);
}

async fn pending_invitation(
    db: &Database,
    namespace: &Namespace,
    email: &str,
    role: NamespaceRole,
    expires_at: Option<String>,
) -> NamespaceInvitation {
    db.create_namespace_invitation(&NewNamespaceInvitation {
        email: email.to_string(),
        namespace_id: namespace.id.clone(),
        role,
        expires_at,
    })
    .await
    .expect("create pending invitation")
}

fn invited_user(username: &str, email: &str) -> User {
    crate::auth::create_user(username.to_string(), email.to_string(), "password123")
        .expect("prepare invited user")
}

#[tokio::test]
async fn acceptance_creates_account_membership_and_consumes_invite_atomically() {
    let (db, db_path, namespace) = setup_db().await;
    let invitation = pending_invitation(
        &db,
        &namespace,
        "sam@example.com",
        NamespaceRole::Owner,
        None,
    )
    .await;
    let user = invited_user("sammy", "sam@example.com");

    db.accept_namespace_invitation(&invitation.token, &user)
        .await
        .expect("accept invitation");

    let loaded_user = db
        .get_user_by_id(&user.id)
        .await
        .expect("load account")
        .expect("account created");
    assert_eq!(loaded_user.email.as_deref(), Some("sam@example.com"));
    assert!(crate::auth::verify_password("password123", &loaded_user.password_hash).unwrap());

    let mut rows = db
        .conn()
        .await
        .unwrap()
        .query(
            "SELECT role FROM namespace_members WHERE namespace_id = ?1 AND user_id = ?2",
            crate::db::sqlite::params![namespace.id.clone(), user.id.clone()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("membership created");
    assert_eq!(row.get::<String>(0).unwrap(), "owner");

    let consumed = db
        .get_namespace_invitation(&invitation.token)
        .await
        .expect("load consumed invite")
        .expect("invite retained");
    assert!(consumed.accepted_at.is_some());
    assert_eq!(consumed.accepted_user_id.as_deref(), Some(user.id.as_str()));
    assert!(
        db.accept_namespace_invitation(
            &invitation.token,
            &invited_user("other", "sam@example.com")
        )
        .await
        .unwrap_err()
        .contains("already been accepted")
    );

    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
async fn failed_acceptance_rolls_back_account_membership_and_invite_state() {
    let (db, db_path, namespace) = setup_db().await;
    let invitation = pending_invitation(
        &db,
        &namespace,
        "sam@example.com",
        NamespaceRole::Contributor,
        None,
    )
    .await;
    let existing = invited_user("taken", "other@example.com");
    db.create_user(&existing)
        .await
        .expect("create existing user");
    let duplicate = invited_user("taken", "sam@example.com");

    assert!(
        db.accept_namespace_invitation(&invitation.token, &duplicate)
            .await
            .unwrap_err()
            .contains("Username already exists")
    );

    let still_pending = db
        .get_namespace_invitation(&invitation.token)
        .await
        .expect("reload pending invite")
        .expect("invitation still exists");
    assert!(still_pending.accepted_at.is_none());
    assert!(still_pending.accepted_user_id.is_none());
    let mut rows = db
        .conn()
        .await
        .unwrap()
        .query(
            "SELECT 1 FROM namespace_members WHERE namespace_id = ?1 AND user_id = ?2",
            crate::db::sqlite::params![namespace.id, duplicate.id],
        )
        .await
        .unwrap();
    assert!(rows.next().await.unwrap().is_none());

    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
async fn expired_or_mismatched_invitation_cannot_be_accepted() {
    let (db, db_path, namespace) = setup_db().await;
    let expired = pending_invitation(
        &db,
        &namespace,
        "sam@example.com",
        NamespaceRole::Contributor,
        Some((chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339()),
    )
    .await;
    assert!(
        db.accept_namespace_invitation(&expired.token, &invited_user("sammy", "sam@example.com"))
            .await
            .unwrap_err()
            .contains("expired")
    );

    let valid = pending_invitation(
        &db,
        &namespace,
        "sam@example.com",
        NamespaceRole::Contributor,
        None,
    )
    .await;
    assert!(
        db.accept_namespace_invitation(&valid.token, &invited_user("sammy", "other@example.com"))
            .await
            .unwrap_err()
            .contains("does not match")
    );
    let pending = db
        .get_namespace_invitation(&valid.token)
        .await
        .unwrap()
        .unwrap();
    assert!(pending.accepted_at.is_none());

    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
async fn concurrent_acceptance_only_creates_one_account_and_membership() {
    let (db, db_path, namespace) = setup_db().await;
    let invitation = pending_invitation(
        &db,
        &namespace,
        "sam@example.com",
        NamespaceRole::Contributor,
        None,
    )
    .await;
    let left_user = invited_user("sam-left", "sam@example.com");
    let right_user = invited_user("sam-right", "sam@example.com");

    let (left, right) = tokio::join!(
        db.accept_namespace_invitation(&invitation.token, &left_user),
        db.accept_namespace_invitation(&invitation.token, &right_user),
    );
    assert_ne!(left.is_ok(), right.is_ok());
    assert_eq!(
        db.list_namespace_invitations(Some(std::slice::from_ref(&namespace.id)))
            .await
            .unwrap()
            .iter()
            .filter(|invite| invite.accepted_at.is_some())
            .count(),
        1
    );

    let _ = std::fs::remove_file(db_path);
}
