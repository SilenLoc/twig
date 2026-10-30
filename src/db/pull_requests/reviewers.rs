//! Users asked to review a request. Requests carry the list; membership is
//! checked by the caller against the namespace.

use chrono::Utc;

use crate::db::{Database, sqlite};

impl Database {
    /// Asks `user_id` to review, or withdraws the request. Asking twice is a no-op.
    pub async fn set_requested_reviewer(
        &self,
        pull_request_id: &str,
        user_id: &str,
        requested: bool,
    ) -> Result<(), String> {
        let conn = self.conn().await?;
        if requested {
            conn.execute(
                "INSERT OR IGNORE INTO pull_request_reviewers (pull_request_id, user_id, requested_at)
                 VALUES (?1, ?2, ?3)",
                sqlite::params![pull_request_id, user_id, Utc::now().to_rfc3339()],
            )
            .await
            .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "DELETE FROM pull_request_reviewers WHERE pull_request_id = ?1 AND user_id = ?2",
                sqlite::params![pull_request_id, user_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Usernames of everyone asked to review, in the order they were asked.
    pub async fn list_requested_reviewers(
        &self,
        pull_request_id: &str,
    ) -> Result<Vec<String>, String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT u.username FROM pull_request_reviewers r
                 JOIN users u ON u.id = r.user_id
                 WHERE r.pull_request_id = ?1
                 ORDER BY r.requested_at ASC, u.username ASC",
                sqlite::params![pull_request_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        let mut usernames = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            usernames.push(row.get::<String>(0).map_err(|e| e.to_string())?);
        }
        Ok(usernames)
    }
}
