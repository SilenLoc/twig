use crate::auth::User;
use crate::db::Database;

impl Database {
    pub async fn create_user(&self, user: &User) -> Result<(), String> {
        self.conn().await?
            .execute(
                "INSERT INTO users (id, username, email, password_hash, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                turso::params![user.id.clone(), user.username.clone(), user.email.clone(), user.password_hash.clone(), user.created_at.clone()],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>, String> {
        let mut rows = self
            .conn().await?
            .query(
                "SELECT id, username, email, password_hash, created_at FROM users WHERE username = ?1",
                turso::params![username],
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(User {
                id: row.get(0).map_err(|e| e.to_string())?,
                username: row.get(1).map_err(|e| e.to_string())?,
                email: row.get(2).map_err(|e| e.to_string())?,
                password_hash: row.get(3).map_err(|e| e.to_string())?,
                created_at: row.get(4).map_err(|e| e.to_string())?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_user_by_id(&self, user_id: &str) -> Result<Option<User>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT id, username, email, password_hash, created_at FROM users WHERE id = ?1",
                turso::params![user_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(User {
                id: row.get(0).map_err(|e| e.to_string())?,
                username: row.get(1).map_err(|e| e.to_string())?,
                email: row.get(2).map_err(|e| e.to_string())?,
                password_hash: row.get(3).map_err(|e| e.to_string())?,
                created_at: row.get(4).map_err(|e| e.to_string())?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn update_user_email(&self, user_id: &str, email: &str) -> Result<(), String> {
        self.conn()
            .await?
            .execute(
                "UPDATE users SET email = ?1 WHERE id = ?2",
                turso::params![email, user_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::User;

    async fn setup_db() -> (Database, String) {
        let db_path = format!("/tmp/test_twig_users_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("init tables");
        (db, db_path)
    }

    fn test_user() -> User {
        User {
            id: uuid::Uuid::new_v4().to_string(),
            username: "testuser".to_string(),
            email: Some("test@example.com".to_string()),
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn test_create_and_get_user_by_username() {
        let (db, db_path) = setup_db().await;
        let user = test_user();

        db.create_user(&user).await.expect("create user");

        let retrieved = db
            .get_user_by_username(&user.username)
            .await
            .expect("get user by username");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, user.id);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_get_user_by_id() {
        let (db, db_path) = setup_db().await;
        let user = test_user();

        db.create_user(&user).await.expect("create user");

        let retrieved = db.get_user_by_id(&user.id).await.expect("get user by id");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().username, user.username);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_get_user_by_username_not_found() {
        let (db, db_path) = setup_db().await;

        let retrieved = db
            .get_user_by_username("nonexistent")
            .await
            .expect("query should not fail");
        assert!(retrieved.is_none());

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_update_user_email() {
        let (db, db_path) = setup_db().await;
        let user = test_user();

        db.create_user(&user).await.expect("create user");
        db.update_user_email(&user.id, "new@example.com")
            .await
            .expect("update email");

        let retrieved = db
            .get_user_by_id(&user.id)
            .await
            .expect("get user")
            .unwrap();
        assert_eq!(retrieved.email, Some("new@example.com".to_string()));

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }
}
