use crate::auth::Invite;
use crate::db::Database;

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

    pub async fn mark_invite_used(&self, invite_id: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn()
            .await?
            .execute(
                "UPDATE invites SET used = TRUE, used_at = ?1 WHERE id = ?2",
                turso::params![now, invite_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Invite;

    async fn setup_db() -> (Database, String) {
        let db_path = format!("/tmp/test_fig_invites_{}.db", uuid::Uuid::new_v4());
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
    async fn test_mark_invite_used() {
        let (db, db_path) = setup_db().await;
        let invite = test_invite();

        db.create_invite(&invite).await.expect("create invite");
        db.mark_invite_used(&invite.id).await.expect("mark used");

        let retrieved = db
            .get_invite(&invite.id)
            .await
            .expect("get invite")
            .unwrap();
        assert!(retrieved.used);
        assert!(retrieved.used_at.is_some());

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
}
