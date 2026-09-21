use crate::db::Database;

impl Database {
    pub async fn set_test_pin(&self, pin: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn().await?;
        conn.execute("DELETE FROM test_pins", ())
            .await
            .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO test_pins (pin, created_at) VALUES (?1, ?2)",
            turso::params![pin, now],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_test_pin(&self) -> Result<Option<String>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT pin FROM test_pins ORDER BY created_at DESC LIMIT 1",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(row.get(0).map_err(|e| e.to_string())?))
        } else {
            Ok(None)
        }
    }

    pub async fn clear_test_pins(&self) -> Result<(), String> {
        self.conn()
            .await?
            .execute("DELETE FROM test_pins", ())
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn validate_test_pin(&self, pin: &str) -> Result<bool, String> {
        if pin.is_empty() {
            return Ok(false);
        }
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT 1 FROM test_pins WHERE pin = ?1",
                turso::params![pin],
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(rows.next().await.map_err(|e| e.to_string())?.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> (Database, String) {
        let db_path = format!("/tmp/test_fig_pins_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("init tables");
        (db, db_path)
    }

    #[tokio::test]
    async fn test_set_and_get_test_pin() {
        let (db, db_path) = setup_db().await;

        assert_eq!(db.get_test_pin().await.unwrap(), None);
        assert!(!db.validate_test_pin("123456").await.unwrap());

        db.set_test_pin("123456").await.unwrap();
        assert_eq!(db.get_test_pin().await.unwrap().as_deref(), Some("123456"));
        assert!(db.validate_test_pin("123456").await.unwrap());
        assert!(!db.validate_test_pin("654321").await.unwrap());
        assert!(!db.validate_test_pin("").await.unwrap());

        // Overwrite
        db.set_test_pin("654321").await.unwrap();
        assert_eq!(db.get_test_pin().await.unwrap().as_deref(), Some("654321"));
        assert!(!db.validate_test_pin("123456").await.unwrap());
        assert!(db.validate_test_pin("654321").await.unwrap());

        // Clear
        db.clear_test_pins().await.unwrap();
        assert_eq!(db.get_test_pin().await.unwrap(), None);
        assert!(!db.validate_test_pin("654321").await.unwrap());

        let _ = std::fs::remove_file(&db_path);
    }
}
