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
