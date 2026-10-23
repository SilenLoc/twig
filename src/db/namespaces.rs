use crate::auth::Namespace;
use crate::auth::NamespaceRole;
use crate::db::Database;

impl Database {
    pub async fn create_namespace(&self, namespace: &Namespace) -> Result<(), String> {
        let conn = self.conn().await?;

        conn.execute("BEGIN", ()).await.map_err(|e| e.to_string())?;

        let result: Result<(), String> = async {
            conn.execute(
                "INSERT INTO namespaces (id, name, owner_id, created_at) VALUES (?1, ?2, ?3, ?4)",
                crate::db::sqlite::params![
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
                crate::db::sqlite::params![namespace.id.clone(), namespace.owner_id.clone(), "owner", now],
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
                crate::db::sqlite::params![name],
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
        self.namespaces_with_owners(None).await
    }

    pub async fn search_namespaces_with_owners(
        &self,
        query: &str,
    ) -> Result<Vec<(Namespace, String)>, String> {
        self.namespaces_with_owners(Some(query)).await
    }

    async fn namespaces_with_owners(
        &self,
        query: Option<&str>,
    ) -> Result<Vec<(Namespace, String)>, String> {
        let search_pattern = query.map(|q| format!("%{q}%"));
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT n.id, n.name, n.owner_id, n.created_at, u.username
                 FROM namespaces n
                 JOIN users u ON n.owner_id = u.id
                 WHERE (?1 IS NULL OR n.name LIKE ?1)
                 ORDER BY n.name",
                crate::db::sqlite::params![search_pattern],
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

    #[cfg(test)]
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
                crate::db::sqlite::params![namespace_name, user_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(rows.next().await.map_err(|e| e.to_string())?.is_some())
    }

    pub async fn get_namespace_role(
        &self,
        user_id: &str,
        namespace_name: &str,
    ) -> Result<Option<NamespaceRole>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT CASE WHEN n.owner_id = ?2 THEN 'owner' ELSE nm.role END
                 FROM namespaces n
                 LEFT JOIN namespace_members nm ON n.id = nm.namespace_id AND nm.user_id = ?2
                 WHERE n.name = ?1 AND (n.owner_id = ?2 OR nm.user_id = ?2)
                 LIMIT 1",
                crate::db::sqlite::params![namespace_name, user_id],
            )
            .await
            .map_err(|error| error.to_string())?;

        let Some(row) = rows.next().await.map_err(|error| error.to_string())? else {
            return Ok(None);
        };
        let role: String = row.get(0).map_err(|error| error.to_string())?;
        NamespaceRole::parse(&role)
            .map(Some)
            .ok_or_else(|| format!("Unknown namespace role: {role}"))
    }

    #[cfg(test)]
    pub async fn user_owns_namespace(
        &self,
        user_id: &str,
        namespace_name: &str,
    ) -> Result<bool, String> {
        Ok(self.get_namespace_role(user_id, namespace_name).await? == Some(NamespaceRole::Owner))
    }

    #[cfg(test)]
    pub async fn get_owned_namespaces_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<Namespace>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT DISTINCT n.id, n.name, n.owner_id, n.created_at
                 FROM namespaces n
                 LEFT JOIN namespace_members nm ON n.id = nm.namespace_id AND nm.user_id = ?1
                 WHERE n.owner_id = ?1 OR (nm.user_id = ?1 AND nm.role = 'owner')
                 ORDER BY n.name",
                crate::db::sqlite::params![user_id],
            )
            .await
            .map_err(|error| error.to_string())?;

        let mut namespaces = Vec::new();
        while let Some(row) = rows.next().await.map_err(|error| error.to_string())? {
            namespaces.push(Namespace {
                id: row.get(0).map_err(|error| error.to_string())?,
                name: row.get(1).map_err(|error| error.to_string())?,
                owner_id: row.get(2).map_err(|error| error.to_string())?,
                created_at: row.get(3).map_err(|error| error.to_string())?,
            });
        }
        Ok(namespaces)
    }

    pub async fn user_can_create_namespace(&self, user_id: &str) -> Result<bool, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT CASE
                    WHEN NOT EXISTS (
                        SELECT 1 FROM namespace_members WHERE user_id = ?1
                    ) THEN 1
                    WHEN EXISTS (
                        SELECT 1 FROM namespaces n
                        LEFT JOIN namespace_members nm
                            ON n.id = nm.namespace_id AND nm.user_id = ?1
                        WHERE n.owner_id = ?1 OR (nm.user_id = ?1 AND nm.role = 'owner')
                    ) THEN 1
                    ELSE 0
                 END",
                crate::db::sqlite::params![user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        let row = rows
            .next()
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Namespace creation permission query returned no result".to_string())?;
        let allowed: i64 = row.get(0).map_err(|error| error.to_string())?;
        Ok(allowed != 0)
    }

    pub async fn user_has_any_namespaces(&self, user_id: &str) -> Result<bool, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT 1 FROM namespace_members WHERE user_id = ?1 LIMIT 1",
                crate::db::sqlite::params![user_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(rows.next().await.map_err(|e| e.to_string())?.is_some())
    }

    /// Lists all member-visible namespaces; owner-only management panels use
    /// `get_owned_namespaces_for_user` instead.
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
                crate::db::sqlite::params![user_id],
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
            crate::db::sqlite::params![new_name, namespace_id],
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
                "DELETE FROM personal_access_token_permissions WHERE namespace_id = ?1",
                crate::db::sqlite::params![namespace_id],
            )
            .await
            .map_err(|e| e.to_string())?;

            conn.execute(
                "DELETE FROM device_authorization_permissions WHERE namespace_id = ?1",
                crate::db::sqlite::params![namespace_id],
            )
            .await
            .map_err(|e| e.to_string())?;

            conn.execute(
                "DELETE FROM namespace_invitations WHERE namespace_id = ?1",
                crate::db::sqlite::params![namespace_id],
            )
            .await
            .map_err(|e| e.to_string())?;

            conn.execute(
                "DELETE FROM namespace_members WHERE namespace_id = ?1",
                crate::db::sqlite::params![namespace_id],
            )
            .await
            .map_err(|e| e.to_string())?;

            conn.execute(
                "DELETE FROM namespaces WHERE id = ?1",
                crate::db::sqlite::params![namespace_id],
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
#[path = "namespaces/tests.rs"]
mod tests;
