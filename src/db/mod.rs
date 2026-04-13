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

        let db_path_str = db_path.to_str().unwrap();

        // Retry with backoff to allow time for the CLI to kill the old instance
        let mut last_error = None;
        for attempt in 1..=30 {
            match Builder::new_local(db_path_str).build().await {
                Ok(db) => {
                    let database = Self { db };
                    database.init_tables().await?;
                    return Ok(database);
                }
                Err(e) => {
                    let err_str = e.to_string();
                    last_error = Some(err_str.clone());

                    // Check if this is a locking error
                    if err_str.contains("lock") || err_str.contains("Lock") {
                        if attempt == 1 {
                            log::warn!("Database locked, waiting for CLI to kill old instance...");
                        }
                        if attempt % 5 == 0 {
                            log::info!("Still waiting for database lock (attempt {}/30)...", attempt);
                        }

                        // Wait 500ms between attempts (total ~15s max wait)
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    } else {
                        // Non-locking error, fail immediately
                        return Err(err_str);
                    }
                }
            }
        }

        Err(format!(
            "Failed to open database after 30 attempts: {}",
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        ))
    }

    pub fn conn(&self) -> turso::Connection {
        self.db.connect().unwrap()
    }
}
