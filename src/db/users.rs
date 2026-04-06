use crate::auth::User;
use crate::db::Database;

impl Database {
    pub async fn create_user(&self, user: &User) -> Result<(), String> {
        self.conn()
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
            .conn()
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
            .execute(
                "UPDATE users SET email = ?1 WHERE id = ?2",
                turso::params![email, user_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
