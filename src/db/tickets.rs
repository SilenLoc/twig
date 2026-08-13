use crate::auth::Ticket;
use crate::db::Database;

impl Database {
    pub async fn create_ticket(&self, ticket: &Ticket) -> Result<(), String> {
        self.conn().await?
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
            .conn()
            .await?
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
        self.conn()
            .await?
            .execute(
                "UPDATE tickets SET used = TRUE, used_at = ?1 WHERE id = ?2",
                turso::params![now, ticket_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Ticket;

    async fn setup_db() -> (Database, String) {
        let db_path = format!("/tmp/test_fig_tickets_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("init tables");
        (db, db_path)
    }

    fn test_ticket() -> Ticket {
        Ticket {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: None,
            used: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            used_at: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_ticket() {
        let (db, db_path) = setup_db().await;
        let ticket = test_ticket();

        db.create_ticket(&ticket).await.expect("create ticket");

        let retrieved = db.get_ticket(&ticket.id).await.expect("get ticket");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, ticket.id);
        assert!(retrieved.user_id.is_none());
        assert!(!retrieved.used);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_mark_ticket_used() {
        let (db, db_path) = setup_db().await;
        let ticket = test_ticket();

        db.create_ticket(&ticket).await.expect("create ticket");
        db.mark_ticket_used(&ticket.id).await.expect("mark used");

        let retrieved = db
            .get_ticket(&ticket.id)
            .await
            .expect("get ticket")
            .unwrap();
        assert!(retrieved.used);
        assert!(retrieved.used_at.is_some());

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_get_ticket_not_found() {
        let (db, db_path) = setup_db().await;

        let retrieved = db
            .get_ticket("nonexistent-ticket")
            .await
            .expect("query should not fail");
        assert!(retrieved.is_none());

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }
}
