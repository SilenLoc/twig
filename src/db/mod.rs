use std::path::Path;
use std::time::Duration;

use turso::Builder;

pub mod migration;
pub mod namespaces;
pub mod tickets;
pub mod tokens;
pub mod users;

pub struct Database {
    db: turso::Database,
}

impl Database {
    pub async fn new(db_path: &str) -> Result<Self, String> {
        let db_path = Path::new(db_path);
        let parent = db_path.parent().unwrap_or(Path::new("."));
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let db_path_str = db_path
            .to_str()
            .ok_or_else(|| "Invalid database path: contains non-UTF8 characters".to_string())?;

        let db = Builder::new_local(db_path_str)
            .build()
            .await
            .map_err(|e| e.to_string())?;

        Ok(Self { db })
    }

    /// Initialize tables and configure connection.
    /// This is called after a delay to allow the server to start up first.
    pub async fn init_tables(&self) -> Result<(), String> {
        // Configure connection to use busy timeout - SQLite will wait for locks
        // instead of immediately returning SQLITE_BUSY
        let conn = self.conn()?;
        conn.busy_timeout(Duration::from_secs(30))
            .map_err(|e| format!("Failed to set busy timeout: {}", e))?;

        // Initialize tables
        self.create_tables().await?;

        log::info!("Database initialized successfully");

        Ok(())
    }

    pub fn conn(&self) -> Result<turso::Connection, String> {
        let conn = self
            .db
            .connect()
            .map_err(|e| format!("Failed to connect to database: {}", e))?;
        conn.busy_timeout(std::time::Duration::from_secs(30))
            .map_err(|e| format!("Failed to set busy timeout: {}", e))?;
        Ok(conn)
    }
}
