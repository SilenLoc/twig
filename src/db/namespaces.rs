use crate::auth::Namespace;
use crate::db::Database;

impl Database {
    pub async fn create_namespace(&self, namespace: &Namespace) -> Result<(), String> {
        let conn = self.conn().await?;

        conn.execute("BEGIN", ()).await.map_err(|e| e.to_string())?;

        let result: Result<(), String> = async {
            conn.execute(
                "INSERT INTO namespaces (id, name, owner_id, created_at) VALUES (?1, ?2, ?3, ?4)",
                turso::params![
                    namespace.id.clone(),
                    namespace.name.clone(),
                    namespace.owner_id.clone(),
                    namespace.created_at.clone()
                ],
            )
            .await
            .map_err(|e| e.to_string())?;

            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO namespace_members (namespace_id, user_id, role, added_at) VALUES (?1, ?2, ?3, ?4)",
                turso::params![namespace.id.clone(), namespace.owner_id.clone(), "owner", now],
            )
            .await
            .map_err(|e| e.to_string())?;

            Ok(())
        }
        .await;

        match result {
            Ok(()) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            Err(e) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|re| format!("{e} (rollback also failed: {re})"))?;
                Err(e)
            }
        }
    }

    pub async fn get_namespace_by_name(&self, name: &str) -> Result<Option<Namespace>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT id, name, owner_id, created_at FROM namespaces WHERE name = ?1",
                turso::params![name],
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(Namespace {
                id: row.get(0).map_err(|e| e.to_string())?,
                name: row.get(1).map_err(|e| e.to_string())?,
                owner_id: row.get(2).map_err(|e| e.to_string())?,
                created_at: row.get(3).map_err(|e| e.to_string())?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_namespaces_with_owners(&self) -> Result<Vec<(Namespace, String)>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT n.id, n.name, n.owner_id, n.created_at, u.username
                 FROM namespaces n
                 JOIN users u ON n.owner_id = u.id
                 ORDER BY n.name",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        let mut result = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            let namespace = Namespace {
                id: row.get(0).map_err(|e| e.to_string())?,
                name: row.get(1).map_err(|e| e.to_string())?,
                owner_id: row.get(2).map_err(|e| e.to_string())?,
                created_at: row.get(3).map_err(|e| e.to_string())?,
            };
            let username: String = row.get(4).map_err(|e| e.to_string())?;
            result.push((namespace, username));
        }
        Ok(result)
    }

    pub async fn search_namespaces_with_owners(
        &self,
        query: &str,
    ) -> Result<Vec<(Namespace, String)>, String> {
        let search_pattern = format!("%{query}%");
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT n.id, n.name, n.owner_id, n.created_at, u.username
                 FROM namespaces n
                 JOIN users u ON n.owner_id = u.id
                 WHERE n.name LIKE ?1
                 ORDER BY n.name",
                turso::params![search_pattern],
            )
            .await
            .map_err(|e| e.to_string())?;

        let mut result = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            let namespace = Namespace {
                id: row.get(0).map_err(|e| e.to_string())?,
                name: row.get(1).map_err(|e| e.to_string())?,
                owner_id: row.get(2).map_err(|e| e.to_string())?,
                created_at: row.get(3).map_err(|e| e.to_string())?,
            };
            let username: String = row.get(4).map_err(|e| e.to_string())?;
            result.push((namespace, username));
        }
        Ok(result)
    }

    pub async fn user_has_namespace_access(
        &self,
        user_id: &str,
        namespace_name: &str,
    ) -> Result<bool, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT 1 FROM namespaces n
                 LEFT JOIN namespace_members nm ON n.id = nm.namespace_id
                 WHERE n.name = ?1 AND (n.owner_id = ?2 OR nm.user_id = ?2)
                 LIMIT 1",
                turso::params![namespace_name, user_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(rows.next().await.map_err(|e| e.to_string())?.is_some())
    }

    pub async fn user_has_any_namespaces(&self, user_id: &str) -> Result<bool, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT 1 FROM namespace_members WHERE user_id = ?1 LIMIT 1",
                turso::params![user_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(rows.next().await.map_err(|e| e.to_string())?.is_some())
    }

    pub async fn get_namespaces_for_user(&self, user_id: &str) -> Result<Vec<Namespace>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT DISTINCT n.id, n.name, n.owner_id, n.created_at
                 FROM namespaces n
                 LEFT JOIN namespace_members nm ON n.id = nm.namespace_id
                 WHERE n.owner_id = ?1 OR nm.user_id = ?1
                 ORDER BY n.name",
                turso::params![user_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        let mut result = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            result.push(Namespace {
                id: row.get(0).map_err(|e| e.to_string())?,
                name: row.get(1).map_err(|e| e.to_string())?,
                owner_id: row.get(2).map_err(|e| e.to_string())?,
                created_at: row.get(3).map_err(|e| e.to_string())?,
            });
        }
        Ok(result)
    }

    pub async fn rename_namespace(&self, namespace_id: &str, new_name: &str) -> Result<(), String> {
        let conn = self.conn().await?;

        conn.execute(
            "UPDATE namespaces SET name = ?1 WHERE id = ?2",
            turso::params![new_name, namespace_id],
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn delete_namespace(&self, namespace_id: &str) -> Result<(), String> {
        let conn = self.conn().await?;

        conn.execute("BEGIN", ()).await.map_err(|e| e.to_string())?;

        let result: Result<(), String> = async {
            conn.execute(
                "DELETE FROM namespace_members WHERE namespace_id = ?1",
                turso::params![namespace_id],
            )
            .await
            .map_err(|e| e.to_string())?;

            conn.execute(
                "DELETE FROM namespaces WHERE id = ?1",
                turso::params![namespace_id],
            )
            .await
            .map_err(|e| e.to_string())?;

            Ok(())
        }
        .await;

        match result {
            Ok(()) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            Err(e) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|re| format!("{e} (rollback also failed: {re})"))?;
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{Namespace, User};

    async fn setup_db_with_user() -> (Database, String, String) {
        let db_path = format!("/tmp/test_twig_namespaces_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("init tables");

        let user_id = uuid::Uuid::new_v4().to_string();
        let user = User {
            id: user_id.clone(),
            username: "nsuser".to_string(),
            email: Some("ns@example.com".to_string()),
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        db.create_user(&user).await.expect("create user");

        (db, db_path, user_id)
    }

    fn test_namespace(owner_id: &str) -> Namespace {
        Namespace {
            id: uuid::Uuid::new_v4().to_string(),
            name: "testns".to_string(),
            owner_id: owner_id.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn test_create_and_get_namespace_by_name() {
        let (db, db_path, user_id) = setup_db_with_user().await;
        let namespace = test_namespace(&user_id);

        db.create_namespace(&namespace)
            .await
            .expect("create namespace");

        let retrieved = db
            .get_namespace_by_name(&namespace.name)
            .await
            .expect("get namespace");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, namespace.id);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_get_all_namespaces_with_owners() {
        let (db, db_path, user_id) = setup_db_with_user().await;
        let namespace = test_namespace(&user_id);

        db.create_namespace(&namespace)
            .await
            .expect("create namespace");

        let result = db
            .get_all_namespaces_with_owners()
            .await
            .expect("get all namespaces");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0.name, namespace.name);
        assert_eq!(result[0].1, "nsuser");

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_user_has_namespace_access_owner() {
        let (db, db_path, user_id) = setup_db_with_user().await;
        let namespace = test_namespace(&user_id);

        db.create_namespace(&namespace)
            .await
            .expect("create namespace");

        let has_access = db
            .user_has_namespace_access(&user_id, &namespace.name)
            .await
            .expect("check access");
        assert!(has_access);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_user_has_namespace_access_denied() {
        let (db, db_path, user_id) = setup_db_with_user().await;
        let namespace = test_namespace(&user_id);

        db.create_namespace(&namespace)
            .await
            .expect("create namespace");

        let other_user_id = uuid::Uuid::new_v4().to_string();
        let has_access = db
            .user_has_namespace_access(&other_user_id, &namespace.name)
            .await
            .expect("check access");
        assert!(!has_access);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_search_namespaces_with_owners() {
        let (db, db_path, user_id) = setup_db_with_user().await;
        let namespace = test_namespace(&user_id);

        db.create_namespace(&namespace)
            .await
            .expect("create namespace");

        let result = db
            .search_namespaces_with_owners("test")
            .await
            .expect("search namespaces");
        assert_eq!(result.len(), 1);

        let no_result = db
            .search_namespaces_with_owners("xyz")
            .await
            .expect("search namespaces");
        assert!(no_result.is_empty());

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_rename_namespace() {
        let (db, db_path, user_id) = setup_db_with_user().await;
        let namespace = test_namespace(&user_id);

        db.create_namespace(&namespace)
            .await
            .expect("create namespace");
        db.rename_namespace(&namespace.id, "renamed")
            .await
            .expect("rename namespace");

        assert!(
            db.get_namespace_by_name(&namespace.name)
                .await
                .expect("get old name")
                .is_none()
        );
        let renamed = db
            .get_namespace_by_name("renamed")
            .await
            .expect("get new name")
            .expect("renamed namespace exists");
        assert_eq!(renamed.id, namespace.id);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_delete_namespace() {
        let (db, db_path, user_id) = setup_db_with_user().await;
        let namespace = test_namespace(&user_id);

        db.create_namespace(&namespace)
            .await
            .expect("create namespace");
        db.delete_namespace(&namespace.id)
            .await
            .expect("delete namespace");

        let retrieved = db
            .get_namespace_by_name(&namespace.name)
            .await
            .expect("get namespace");
        assert!(retrieved.is_none());

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }
}
