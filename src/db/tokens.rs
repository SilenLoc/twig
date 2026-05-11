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

    pub async fn get_username_by_token(&self, token: &str) -> Result<Option<String>, String> {
        let mut rows = self
            .conn().await?
            .query(
                "SELECT u.username FROM tokens t JOIN users u ON t.user_id = u.id WHERE t.token = ?1 AND t.created_at > datetime('now', '-30 days')",
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
