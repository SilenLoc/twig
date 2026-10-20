use crate::auth::{Invite, User};
use crate::db::Database;

#[derive(Debug, PartialEq, Eq)]
pub enum SignupInviteError {
    InviteUnavailable,
    UsernameExists,
    Database(String),
}

impl Database {
    pub async fn create_invite(&self, invite: &Invite) -> Result<(), String> {
        self.conn().await?
            .execute(
                "INSERT INTO invites (id, user_id, used, created_at, used_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                turso::params![
                    invite.id.clone(),
                    invite.user_id.clone(),
                    invite.used,
                    invite.created_at.clone(),
                    invite.used_at.clone()
                ],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_invite(&self, invite_id: &str) -> Result<Option<Invite>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT id, user_id, used, created_at, used_at FROM invites WHERE id = ?1",
                turso::params![invite_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(Invite {
                id: row.get(0).map_err(|e| e.to_string())?,
                user_id: row.get(1).map_err(|e| e.to_string())?,
                used: row.get(2).map_err(|e| e.to_string())?,
                created_at: row.get(3).map_err(|e| e.to_string())?,
                used_at: row.get(4).map_err(|e| e.to_string())?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Atomically claims an unused signup invite and inserts the corresponding
    /// user. A failed insert rolls back the invite claim.
    pub async fn create_user_with_invite(
        &self,
        invite_id: &str,
        user: &User,
    ) -> Result<(), SignupInviteError> {
        let conn = self.conn().await.map_err(SignupInviteError::Database)?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|error| SignupInviteError::Database(error.to_string()))?;

        let result = create_user_with_invite_in_transaction(&conn, invite_id, user).await;
        match result {
            Ok(()) => match conn.execute("COMMIT", ()).await {
                Ok(_) => Ok(()),
                Err(error) => {
                    let _ = conn.execute("ROLLBACK", ()).await;
                    Err(SignupInviteError::Database(error.to_string()))
                }
            },
            Err(error) => {
                if let Err(rollback_error) = conn.execute("ROLLBACK", ()).await {
                    return Err(SignupInviteError::Database(format!(
                        "{error:?} (rollback failed: {rollback_error})"
                    )));
                }
                Err(error)
            }
        }
    }
}

async fn create_user_with_invite_in_transaction(
    conn: &turso::Connection,
    invite_id: &str,
    user: &User,
) -> Result<(), SignupInviteError> {
    let username_exists = {
        let mut rows = conn
            .query(
                "SELECT 1 FROM users WHERE username = ?1 LIMIT 1",
                turso::params![user.username.as_str()],
            )
            .await
            .map_err(|error| SignupInviteError::Database(error.to_string()))?;
        rows.next()
            .await
            .map_err(|error| SignupInviteError::Database(error.to_string()))?
            .is_some()
    };
    if username_exists {
        return Err(SignupInviteError::UsernameExists);
    }

    let used_at = chrono::Utc::now().to_rfc3339();
    let claimed = conn
        .execute(
            "UPDATE invites
             SET used = TRUE, used_at = ?1, user_id = ?2
             WHERE id = ?3 AND used = FALSE",
            turso::params![used_at, user.id.as_str(), invite_id],
        )
        .await
        .map_err(|error| SignupInviteError::Database(error.to_string()))?;
    if claimed != 1 {
        return Err(SignupInviteError::InviteUnavailable);
    }

    conn.execute(
        "INSERT INTO users (id, username, email, password_hash, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        turso::params![
            user.id.as_str(),
            user.username.as_str(),
            user.email.clone(),
            user.password_hash.as_str(),
            user.created_at.as_str(),
        ],
    )
    .await
    .map_err(|error| SignupInviteError::Database(error.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Invite;

    async fn setup_db() -> (Database, String) {
        let db_path = format!("/tmp/test_twig_invites_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("init tables");
        (db, db_path)
    }

    fn test_invite() -> Invite {
        Invite {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: None,
            used: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            used_at: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_invite() {
        let (db, db_path) = setup_db().await;
        let invite = test_invite();

        db.create_invite(&invite).await.expect("create invite");

        let retrieved = db.get_invite(&invite.id).await.expect("get invite");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, invite.id);
        assert!(retrieved.user_id.is_none());
        assert!(!retrieved.used);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_get_invite_not_found() {
        let (db, db_path) = setup_db().await;

        let retrieved = db
            .get_invite("nonexistent-invite")
            .await
            .expect("query should not fail");
        assert!(retrieved.is_none());

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn concurrent_signup_claims_consume_one_invite_once() {
        let (db, db_path) = setup_db().await;
        let invite = test_invite();
        db.create_invite(&invite).await.unwrap();
        let first = crate::auth::create_user(
            "signup-first".to_string(),
            "first@example.test".to_string(),
            "password123",
        )
        .unwrap();
        let second = crate::auth::create_user(
            "signup-second".to_string(),
            "second@example.test".to_string(),
            "password123",
        )
        .unwrap();

        let (first_result, second_result) = tokio::join!(
            db.create_user_with_invite(&invite.id, &first),
            db.create_user_with_invite(&invite.id, &second),
        );
        assert_ne!(first_result.is_ok(), second_result.is_ok());

        let first_saved = db
            .get_user_by_username(&first.username)
            .await
            .unwrap()
            .is_some();
        let second_saved = db
            .get_user_by_username(&second.username)
            .await
            .unwrap()
            .is_some();
        assert_ne!(first_saved, second_saved);
        let consumed = db.get_invite(&invite.id).await.unwrap().unwrap();
        assert!(consumed.used);
        assert_eq!(
            consumed.user_id.as_deref(),
            Some(if first_saved {
                first.id.as_str()
            } else {
                second.id.as_str()
            })
        );

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn failed_user_insert_rolls_back_invite_claim() {
        let (db, db_path) = setup_db().await;
        let existing = crate::auth::create_user(
            "existing-user".to_string(),
            "existing@example.test".to_string(),
            "password123",
        )
        .unwrap();
        db.create_user(&existing).await.unwrap();
        let invite = test_invite();
        db.create_invite(&invite).await.unwrap();
        let mut conflicting_id = crate::auth::create_user(
            "new-user".to_string(),
            "new@example.test".to_string(),
            "password123",
        )
        .unwrap();
        conflicting_id.id.clone_from(&existing.id);

        let result = db
            .create_user_with_invite(&invite.id, &conflicting_id)
            .await;
        assert!(matches!(result, Err(SignupInviteError::Database(_))));
        assert!(db.get_user_by_username("new-user").await.unwrap().is_none());
        assert!(!db.get_invite(&invite.id).await.unwrap().unwrap().used);

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn username_conflict_does_not_consume_signup_invite() {
        let (db, db_path) = setup_db().await;
        let existing = crate::auth::create_user(
            "existing-user".to_string(),
            "existing@example.test".to_string(),
            "password123",
        )
        .unwrap();
        db.create_user(&existing).await.unwrap();
        let invite = test_invite();
        db.create_invite(&invite).await.unwrap();
        let duplicate_name = crate::auth::create_user(
            "existing-user".to_string(),
            "another@example.test".to_string(),
            "password123",
        )
        .unwrap();

        assert!(matches!(
            db.create_user_with_invite(&invite.id, &duplicate_name)
                .await,
            Err(SignupInviteError::UsernameExists)
        ));
        assert!(!db.get_invite(&invite.id).await.unwrap().unwrap().used);

        let _ = std::fs::remove_file(db_path);
    }
}
