use std::sync::Arc;
use tokio::sync::OnceCell;

pub mod binaries;
pub mod data;
pub mod device_authorizations;
pub mod invitations;
pub mod invites;
pub mod migration;
pub mod namespaces;
pub mod personal_access_tokens;
pub mod sessions;
pub mod sqlite;
pub mod test_pins;
pub mod tokens;
pub mod users;

#[derive(Clone)]
pub struct Database {
    db: Arc<OnceCell<()>>,
    db_path: String,
}

impl Database {
    pub fn new(db_path: &str) -> Self {
        Self {
            db: Arc::new(OnceCell::new()),
            db_path: db_path.to_owned(),
        }
    }

    /// Opens a rusqlite connection on demand, creating the database directory
    /// only when the first connection is requested.
    pub async fn conn(&self) -> Result<sqlite::Connection, String> {
        self.db
            .get_or_try_init(|| async {
                let db_path = std::path::PathBuf::from(&self.db_path);
                tokio::task::spawn_blocking(move || {
                    let parent = db_path
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .unwrap_or(std::path::Path::new("."));
                    if !parent.exists() {
                        log::info!("Creating parent directory: {}", parent.display());
                        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                    }
                    Ok::<(), String>(())
                })
                .await
                .map_err(|error| error.to_string())??;
                Ok::<(), String>(())
            })
            .await?;

        sqlite::Connection::open(std::path::PathBuf::from(&self.db_path))
            .await
            .map_err(|error| error.to_string())
    }

    pub async fn init_tables(&self) -> Result<(), String> {
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
        let db = Database::new("/tmp/test_twig_new.db");
        assert_eq!(db.db_path, "/tmp/test_twig_new.db");
    }

    #[tokio::test]
    async fn test_database_conn_creates_file() {
        let db_path = format!("/tmp/test_twig_conn_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);

        let conn = db.conn().await;
        assert!(conn.is_ok(), "conn should succeed");
        assert!(
            std::path::Path::new(&db_path).exists(),
            "db file should be created"
        );

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_database_new_does_not_create_parent_until_first_connection() {
        let directory = std::env::temp_dir().join(format!("twig-lazy-db-{}", uuid::Uuid::new_v4()));
        let db_path = directory.join("twig.db");
        let db = Database::new(db_path.to_str().expect("UTF-8 temp path"));

        assert!(!directory.exists(), "constructing Database is lazy");
        db.conn().await.expect("open database on demand");
        assert!(
            db_path.exists(),
            "first connection creates the database file"
        );

        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_dir(directory);
    }

    #[tokio::test]
    async fn test_database_conn_opens_a_relative_path() {
        let db_path = format!(".twig-relative-{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);

        db.conn().await.expect("open relative database path");
        assert!(std::path::Path::new(&db_path).exists());

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn test_database_init_tables_succeeds() {
        let db_path = format!("/tmp/test_twig_init_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);

        let result = db.init_tables().await;
        assert!(result.is_ok(), "init_tables should succeed: {result:?}");

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }
}
