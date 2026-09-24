use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::OnceCell;
use turso::Builder;

pub mod data;
pub mod invites;
pub mod migration;
pub mod namespaces;
pub mod sessions;
pub mod test_pins;
pub mod tokens;
pub mod users;

#[derive(Clone)]
pub struct Database {
    db: Arc<OnceCell<turso::Database>>,
    db_path: String,
}

impl Database {
    pub fn new(db_path: &str) -> Self {
        Self {
            db: Arc::new(OnceCell::new()),
            db_path: db_path.to_owned(),
        }
    }

    pub async fn conn(&self) -> Result<turso::Connection, String> {
        let db = self
            .db
            .get_or_try_init(|| async {
                let db_path = Path::new(&self.db_path);
                let parent = db_path.parent().unwrap_or(Path::new("."));
                if !parent.exists() {
                    log::info!("Creating parent directory: {}", parent.display());
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }

                let db_path_str = db_path.to_str().ok_or_else(|| {
                    "Invalid database path: contains non-UTF8 characters".to_string()
                })?;

                let db = Builder::new_local(db_path_str).build().await.map_err(|e| {
                    log::error!("Failed to create database: {e}");
                    e.to_string()
                })?;

                log::info!("Created db");
                Ok::<turso::Database, String>(db)
            })
            .await?;

        let conn = db
            .connect()
            .map_err(|e| format!("Failed to connect to database: {e}"))?;
        conn.busy_timeout(Duration::from_secs(30))
            .map_err(|e| format!("Failed to set busy timeout: {e}"))?;
        Ok(conn)
    }

    pub async fn init_tables(&self) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.busy_timeout(Duration::from_secs(30))
            .map_err(|e| format!("Failed to set busy timeout: {e}"))?;

        self.create_tables().await?;

        log::info!("Database initialized successfully");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_new_stores_path() {
        let db = Database::new("/tmp/test_fig_new.db");
        assert_eq!(db.db_path, "/tmp/test_fig_new.db");
    }

    #[tokio::test]
    async fn test_database_conn_creates_file() {
        let db_path = format!("/tmp/test_fig_conn_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);

        let conn = db.conn().await;
        assert!(conn.is_ok(), "conn should succeed: {conn:?}");
        assert!(
            std::path::Path::new(&db_path).exists(),
            "db file should be created"
        );

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_database_init_tables_succeeds() {
        let db_path = format!("/tmp/test_fig_init_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);

        let result = db.init_tables().await;
        assert!(result.is_ok(), "init_tables should succeed: {result:?}");

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }
}
