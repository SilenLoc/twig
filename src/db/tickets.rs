use crate::auth::Ticket;
use crate::db::Database;

impl Database {
    pub async fn create_ticket(&self, ticket: &Ticket) -> Result<(), String> {
        self.conn()?
            .execute(
                "INSERT INTO tickets (id, user_id, used, created_at, used_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                turso::params![
                    ticket.id.clone(),
                    ticket.user_id.clone(),
                    ticket.used,
                    ticket.created_at.clone(),
                    ticket.used_at.clone()
                ],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_ticket(&self, ticket_id: &str) -> Result<Option<Ticket>, String> {
        let mut rows = self
            .conn()?
            .query(
                "SELECT id, user_id, used, created_at, used_at FROM tickets WHERE id = ?1",
                turso::params![ticket_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(Ticket {
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

    pub async fn mark_ticket_used(&self, ticket_id: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn()?
            .execute(
                "UPDATE tickets SET used = TRUE, used_at = ?1 WHERE id = ?2",
                turso::params![now, ticket_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
