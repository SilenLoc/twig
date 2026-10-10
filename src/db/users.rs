use crate::auth::{NamespaceRole, User, UserManagementEntry};
use crate::db::Database;
use std::collections::BTreeMap;

impl Database {
    pub async fn list_users_for_management(
        &self,
        namespace_ids: Option<&[String]>,
    ) -> Result<Vec<UserManagementEntry>, String> {
        let mut users = BTreeMap::new();
        match namespace_ids {
            None => {
                for entry in self.list_users_for_management_in(None).await? {
                    merge_user_entry(&mut users, entry);
                }
            }
            Some(namespace_ids) => {
                for namespace_id in namespace_ids {
                    for entry in self
                        .list_users_for_management_in(Some(namespace_id))
                        .await?
                    {
                        merge_user_entry(&mut users, entry);
                    }
                }
            }
        }
        let mut users: Vec<_> = users.into_values().collect();
        users.sort_by(|left, right| left.username.cmp(&right.username));
        Ok(users)
    }

    async fn list_users_for_management_in(
        &self,
        namespace_id: Option<&String>,
    ) -> Result<Vec<(String, UserManagementEntry)>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT u.id, u.username, u.email, n.name, nm.role
                 FROM users u
                 LEFT JOIN namespace_members nm ON nm.user_id = u.id
                 LEFT JOIN namespaces n ON n.id = nm.namespace_id
                 WHERE (?1 IS NULL OR n.id = ?1)
                 ORDER BY u.username, n.name",
                turso::params![namespace_id.cloned()],
            )
            .await
            .map_err(|error| error.to_string())?;
        let mut users = Vec::new();
        while let Some(row) = rows.next().await.map_err(|error| error.to_string())? {
            let role_name: Option<String> = row.get(4).map_err(|error| error.to_string())?;
            let role = role_name
                .as_deref()
                .map(|role| {
                    NamespaceRole::parse(role)
                        .ok_or_else(|| format!("Unknown namespace role: {role}"))
                })
                .transpose()?;
            let namespace_name: Option<String> = row.get(3).map_err(|error| error.to_string())?;
            let memberships = namespace_name
                .zip(role)
                .map_or_else(Vec::new, |(namespace, role)| vec![(namespace, role)]);
            users.push((
                row.get(0).map_err(|error| error.to_string())?,
                UserManagementEntry {
                    username: row.get(1).map_err(|error| error.to_string())?,
                    email: row.get(2).map_err(|error| error.to_string())?,
                    memberships,
                },
            ));
        }
        Ok(users)
    }

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

fn merge_user_entry(
    users: &mut BTreeMap<String, UserManagementEntry>,
    (user_id, mut entry): (String, UserManagementEntry),
) {
    if let Some(existing) = users.get_mut(&user_id) {
        existing.memberships.append(&mut entry.memberships);
    } else {
        users.insert(user_id, entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{NamespaceRole, User};

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

    #[tokio::test]
    async fn user_management_lists_global_users_and_scopes_members_by_namespace() {
        let (db, db_path) = setup_db().await;
        let owner = test_user();
        db.create_user(&owner).await.expect("create owner");
        let namespace = crate::auth::create_namespace("users-scope".to_string(), owner.id.clone());
        db.create_namespace(&namespace)
            .await
            .expect("create namespace");

        let contributor = User {
            id: uuid::Uuid::new_v4().to_string(),
            username: "scope-contributor".to_string(),
            email: Some("contributor@example.com".to_string()),
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        db.create_user(&contributor)
            .await
            .expect("create contributor");
        db.conn()
            .await
            .unwrap()
            .execute(
                "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
                 VALUES (?1, ?2, ?3, ?4)",
                turso::params![
                    namespace.id.clone(),
                    contributor.id.clone(),
                    NamespaceRole::Contributor.as_str(),
                    chrono::Utc::now().to_rfc3339()
                ],
            )
            .await
            .expect("add contributor membership");

        let orphan = User {
            id: uuid::Uuid::new_v4().to_string(),
            username: "orphan".to_string(),
            email: None,
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        db.create_user(&orphan).await.expect("create unscoped user");

        let global = db
            .list_users_for_management(None)
            .await
            .expect("list all users");
        assert_eq!(global.len(), 3);
        assert!(
            global
                .iter()
                .any(|entry| entry.username == "orphan" && entry.memberships.is_empty())
        );
        let scoped = db
            .list_users_for_management(Some(std::slice::from_ref(&namespace.id)))
            .await
            .expect("list namespace users");
        assert_eq!(scoped.len(), 2);
        assert!(scoped.iter().all(|entry| entry.memberships.len() == 1));
        assert!(scoped.iter().any(|entry| {
            entry.username == "testuser"
                && entry.memberships == vec![("users-scope".to_string(), NamespaceRole::Owner)]
        }));
        assert!(scoped.iter().any(|entry| {
            entry.username == "scope-contributor"
                && entry.memberships
                    == vec![("users-scope".to_string(), NamespaceRole::Contributor)]
        }));

        let _ = std::fs::remove_file(db_path);
    }
}
