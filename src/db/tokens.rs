use crate::db::Database;

impl Database {
    pub async fn create_token(&self, token: &str, user_id: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn()
            .await?
            .execute(
                "INSERT INTO tokens (token, user_id, created_at) VALUES (?1, ?2, ?3)",
                turso::params![token, user_id, now],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_token_user(&self, token: &str) -> Result<Option<String>, String> {
        let mut rows = self
            .conn().await?
            .query(
                "SELECT user_id FROM tokens WHERE token = ?1 AND created_at > datetime('now', '-30 days')",
                turso::params![token],
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(row.get(0).map_err(|e| e.to_string())?))
        } else {
            Ok(None)
        }
    }

    pub async fn delete_token(&self, token: &str) -> Result<(), String> {
        self.conn()
            .await?
            .execute("DELETE FROM tokens WHERE token = ?1", turso::params![token])
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn cleanup_expired_tokens(&self) -> Result<(), String> {
        self.conn()
            .await?
            .execute(
                "DELETE FROM tokens WHERE created_at < datetime('now', '-30 days')",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::User;

    async fn setup_db_with_user() -> (Database, String, String) {
        let db_path = format!("/tmp/test_twig_tokens_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("init tables");

        let user_id = uuid::Uuid::new_v4().to_string();
        let user = User {
            id: user_id.clone(),
            username: "tokenuser".to_string(),
            email: Some("token@example.com".to_string()),
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        db.create_user(&user).await.expect("create user");

        (db, db_path, user_id)
    }

    #[tokio::test]
    async fn test_create_and_get_token_user() {
        let (db, db_path, user_id) = setup_db_with_user().await;
        let token = "test-token-123";

        db.create_token(token, &user_id)
            .await
            .expect("create token");

        let retrieved = db.get_token_user(token).await.expect("get token user");
        assert_eq!(retrieved, Some(user_id));

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_delete_token() {
        let (db, db_path, user_id) = setup_db_with_user().await;
        let token = "test-token-789";

        db.create_token(token, &user_id)
            .await
            .expect("create token");
        db.delete_token(token).await.expect("delete token");

        let retrieved = db.get_token_user(token).await.expect("get token user");
        assert!(retrieved.is_none());

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_cleanup_expired_tokens_removes_old_tokens() {
        let (db, db_path, user_id) = setup_db_with_user().await;
        let token = "test-token-old";

        db.create_token(token, &user_id)
            .await
            .expect("create token");

        // Manually set the token to be 60 days old
        db.conn()
            .await
            .unwrap()
            .execute(
                "UPDATE tokens SET created_at = datetime('now', '-60 days') WHERE token = ?1",
                turso::params![token],
            )
            .await
            .unwrap();

        db.cleanup_expired_tokens().await.expect("cleanup tokens");

        let retrieved = db.get_token_user(token).await.expect("get token user");
        assert!(retrieved.is_none());

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }
}
