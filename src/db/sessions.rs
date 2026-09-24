use crate::db::Database;

impl Database {
    pub async fn load_actix_session(&self, session_key: &str) -> Result<Option<String>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT state FROM actix_sessions WHERE session_key = ?1 AND expires_at > unixepoch()",
                turso::params![session_key],
            )
            .await
            .map_err(|e| e.to_string())?;

        rows.next()
            .await
            .map_err(|e| e.to_string())?
            .map(|row| row.get(0).map_err(|e| e.to_string()))
            .transpose()
    }

    pub async fn save_actix_session(
        &self,
        session_key: &str,
        state: &str,
        ttl_seconds: i64,
    ) -> Result<(), String> {
        self.conn()
            .await?
            .execute(
                "INSERT INTO actix_sessions (session_key, state, expires_at) VALUES (?1, ?2, unixepoch() + ?3)",
                turso::params![session_key, state, ttl_seconds],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn update_actix_session(
        &self,
        session_key: &str,
        state: &str,
        ttl_seconds: i64,
    ) -> Result<bool, String> {
        let updated = self
            .conn()
            .await?
            .execute(
                "UPDATE actix_sessions SET state = ?2, expires_at = unixepoch() + ?3 WHERE session_key = ?1 AND expires_at > unixepoch()",
                turso::params![session_key, state, ttl_seconds],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(updated > 0)
    }

    pub async fn update_actix_session_ttl(
        &self,
        session_key: &str,
        ttl_seconds: i64,
    ) -> Result<bool, String> {
        let updated = self
            .conn()
            .await?
            .execute(
                "UPDATE actix_sessions SET expires_at = unixepoch() + ?2 WHERE session_key = ?1 AND expires_at > unixepoch()",
                turso::params![session_key, ttl_seconds],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(updated > 0)
    }

    pub async fn delete_actix_session(&self, session_key: &str) -> Result<(), String> {
        self.conn()
            .await?
            .execute(
                "DELETE FROM actix_sessions WHERE session_key = ?1",
                turso::params![session_key],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
