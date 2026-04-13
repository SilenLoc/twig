use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use turso::Builder;

pub mod migration;
pub mod namespaces;
pub mod tickets;
pub mod tokens;
pub mod users;

pub struct Database {
    db: turso::Database,
    initialized: AtomicBool,
}

impl Database {
    pub async fn new(db_path: &str) -> Result<Self, String> {
        let db_path = Path::new(db_path);
        let parent = db_path.parent().unwrap_or(Path::new("."));
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let db = Builder::new_local(db_path.to_str().unwrap())
            .build()
            .await
            .map_err(|e| e.to_string())?;

        Ok(Self {
            db,
            initialized: AtomicBool::new(false),
        })
    }

    /// Initialize tables and configure connection.
    /// This is called after a delay to allow the server to start up first.
    pub async fn init_tables(&self) -> Result<(), String> {
        // Configure connection to use busy timeout - SQLite will wait for locks
        // instead of immediately returning SQLITE_BUSY
        let conn = self.conn();
        conn.busy_timeout(Duration::from_secs(30))
            .map_err(|e| format!("Failed to set busy timeout: {}", e))?;

        // Initialize tables
        self.create_tables().await?;

        self.initialized.store(true, Ordering::SeqCst);
        log::info!("Database initialized successfully");

        Ok(())
    }

    pub fn conn(&self) -> turso::Connection {
        self.db.connect().unwrap()
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }
}
