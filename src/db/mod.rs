use std::path::Path;

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

        let db = Builder::new_local(db_path.to_str().unwrap())
            .build()
            .await
            .map_err(|e| e.to_string())?;

        let database = Self { db };
        database.init_tables().await?;

        Ok(database)
    }

    pub fn conn(&self) -> turso::Connection {
        self.db.connect().unwrap()
    }
}
