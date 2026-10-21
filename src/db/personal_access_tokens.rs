use std::collections::BTreeMap;

use crate::auth::{
    AuthenticatedPrincipal, NamespacePermission, PermissionSet, PersonalAccessToken,
    TokenNamespacePermissions, User,
};
use crate::db::Database;

pub struct NewPersonalAccessToken<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub name: &'a str,
    pub token_hash: &'a str,
    pub token_hint: &'a str,
    pub namespace_ids: &'a [String],
    pub permissions: &'a [NamespacePermission],
    pub expires_at: Option<&'a str>,
}

pub(crate) async fn insert_personal_access_token(
    connection: &crate::db::sqlite::Connection,
    token: NewPersonalAccessToken<'_>,
) -> Result<(), String> {
    let NewPersonalAccessToken {
        id,
        user_id,
        name,
        token_hash,
        token_hint,
        namespace_ids,
        permissions,
        expires_at,
    } = token;

    connection
        .execute(
            "INSERT INTO personal_access_tokens
                (id, user_id, name, token_hash, token_hint, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            crate::db::sqlite::params![
                id,
                user_id,
                name,
                token_hash,
                token_hint,
                chrono::Utc::now().to_rfc3339(),
                expires_at
            ],
        )
        .await
        .map_err(|error| error.to_string())?;

    for namespace_id in namespace_ids {
        for permission in permissions {
            connection
                .execute(
                    "INSERT INTO personal_access_token_permissions
                        (token_id, namespace_id, permission)
                     VALUES (?1, ?2, ?3)",
                    crate::db::sqlite::params![id, namespace_id.clone(), permission.as_str()],
                )
                .await
                .map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

impl Database {
    pub async fn create_personal_access_token(
        &self,
        token: NewPersonalAccessToken<'_>,
    ) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|error| error.to_string())?;

        let result = insert_personal_access_token(&conn, token).await;

        match result {
            Ok(()) => conn
                .execute("COMMIT", ())
                .await
                .map(|_| ())
                .map_err(|error| format!("Failed to commit personal access token: {error}")),
            Err(error) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|rollback_error| {
                        format!("{error} (rollback also failed: {rollback_error})")
                    })?;
                Err(error)
            }
        }
    }

    pub async fn list_personal_access_tokens(
        &self,
        user_id: &str,
    ) -> Result<Vec<PersonalAccessToken>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT t.id, t.name, t.token_hint, t.created_at, t.last_used_at, t.expires_at,
                        p.namespace_id, n.name, p.permission
                 FROM personal_access_tokens t
                 LEFT JOIN personal_access_token_permissions p ON p.token_id = t.id
                 LEFT JOIN namespaces n ON n.id = p.namespace_id
                 WHERE t.user_id = ?1
                 ORDER BY t.created_at DESC, t.id, n.name, p.permission",
                crate::db::sqlite::params![user_id],
            )
            .await
            .map_err(|error| error.to_string())?;

        let mut tokens: Vec<PersonalAccessToken> = Vec::new();
        while let Some(row) = rows.next().await.map_err(|error| error.to_string())? {
            let id: String = row.get(0).map_err(|error| error.to_string())?;
            let token_index = if let Some(index) = tokens.iter().position(|token| token.id == id) {
                index
            } else {
                tokens.push(PersonalAccessToken {
                    id: id.clone(),
                    name: row.get(1).map_err(|error| error.to_string())?,
                    token_hint: row.get(2).map_err(|error| error.to_string())?,
                    created_at: row.get(3).map_err(|error| error.to_string())?,
                    last_used_at: row.get(4).map_err(|error| error.to_string())?,
                    expires_at: row.get(5).map_err(|error| error.to_string())?,
                    scopes: Vec::new(),
                });
                tokens.len() - 1
            };

            let namespace_id: Option<String> = row.get(6).map_err(|error| error.to_string())?;
            let Some(namespace_id) = namespace_id else {
                continue;
            };
            let namespace_name: String = row.get(7).map_err(|error| error.to_string())?;
            let permission_name: String = row.get(8).map_err(|error| error.to_string())?;
            let permission = NamespacePermission::parse(&permission_name)
                .ok_or_else(|| format!("Unknown token permission: {permission_name}"))?;
            let scopes = &mut tokens[token_index].scopes;
            let scope_index = if let Some(index) = scopes
                .iter()
                .position(|scope| scope.namespace_id == namespace_id)
            {
                index
            } else {
                scopes.push(TokenNamespacePermissions {
                    namespace_id: namespace_id.clone(),
                    namespace_name,
                    permissions: PermissionSet::default(),
                });
                scopes.len() - 1
            };
            scopes[scope_index].permissions.insert(permission);
        }

        Ok(tokens)
    }

    pub async fn authenticate_personal_access_token(
        &self,
        token_hash: &str,
    ) -> Result<Option<AuthenticatedPrincipal>, String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT u.id, u.username, u.email, u.password_hash, u.created_at,
                        p.namespace_id, n.name, p.permission
                 FROM personal_access_tokens t
                 JOIN users u ON u.id = t.user_id
                 LEFT JOIN personal_access_token_permissions p ON p.token_id = t.id
                 LEFT JOIN namespaces n ON n.id = p.namespace_id
                 WHERE t.token_hash = ?1
                   AND (t.expires_at IS NULL OR datetime(t.expires_at) > datetime('now'))
                 ORDER BY n.name, p.permission",
                crate::db::sqlite::params![token_hash],
            )
            .await
            .map_err(|error| error.to_string())?;

        let mut principal: Option<AuthenticatedPrincipal> = None;
        while let Some(row) = rows.next().await.map_err(|error| error.to_string())? {
            if principal.is_none() {
                principal = Some(AuthenticatedPrincipal {
                    user: User {
                        id: row.get(0).map_err(|error| error.to_string())?,
                        username: row.get(1).map_err(|error| error.to_string())?,
                        email: row.get(2).map_err(|error| error.to_string())?,
                        password_hash: row.get(3).map_err(|error| error.to_string())?,
                        created_at: row.get(4).map_err(|error| error.to_string())?,
                    },
                    token_permissions: Some(BTreeMap::new()),
                });
            }
            let principal = principal.as_mut().expect("principal was initialized");
            let namespace_id: Option<String> = row.get(5).map_err(|error| error.to_string())?;
            let namespace_name: Option<String> = row.get(6).map_err(|error| error.to_string())?;
            let permission_name: Option<String> = row.get(7).map_err(|error| error.to_string())?;
            let (Some(_), Some(namespace_name), Some(permission_name)) =
                (namespace_id, namespace_name, permission_name)
            else {
                continue;
            };
            let permission = NamespacePermission::parse(&permission_name)
                .ok_or_else(|| format!("Unknown token permission: {permission_name}"))?;
            principal
                .token_permissions
                .as_mut()
                .expect("personal access token has scoped permissions")
                .entry(namespace_name)
                .or_default()
                .insert(permission);
        }
        drop(rows);

        let Some(principal) = principal else {
            return Ok(None);
        };
        conn.execute(
            "UPDATE personal_access_tokens SET last_used_at = ?2
             WHERE token_hash = ?1
               AND (expires_at IS NULL OR datetime(expires_at) > datetime('now'))",
            crate::db::sqlite::params![token_hash, chrono::Utc::now().to_rfc3339()],
        )
        .await
        .map_err(|error| error.to_string())?;

        Ok(Some(principal))
    }

    pub async fn revoke_personal_access_token(
        &self,
        user_id: &str,
        token_id: &str,
    ) -> Result<bool, String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|error| error.to_string())?;

        let result: Result<bool, String> = async {
            let mut rows = conn
                .query(
                    "SELECT 1 FROM personal_access_tokens WHERE id = ?1 AND user_id = ?2",
                    crate::db::sqlite::params![token_id, user_id],
                )
                .await
                .map_err(|error| error.to_string())?;
            let exists = rows
                .next()
                .await
                .map_err(|error| error.to_string())?
                .is_some();
            drop(rows);
            if !exists {
                return Ok(false);
            }

            conn.execute(
                "DELETE FROM personal_access_token_permissions WHERE token_id = ?1",
                crate::db::sqlite::params![token_id],
            )
            .await
            .map_err(|error| error.to_string())?;
            conn.execute(
                "DELETE FROM personal_access_tokens WHERE id = ?1 AND user_id = ?2",
                crate::db::sqlite::params![token_id, user_id],
            )
            .await
            .map_err(|error| error.to_string())?;

            Ok(true)
        }
        .await;

        match result {
            Ok(revoked) => conn
                .execute("COMMIT", ())
                .await
                .map(|_| revoked)
                .map_err(|error| format!("Failed to commit token revocation: {error}")),
            Err(error) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|rollback_error| {
                        format!("{error} (rollback also failed: {rollback_error})")
                    })?;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::create_namespace;

    async fn setup_db() -> (Database, String, User, Vec<String>) {
        let db_path = format!("/tmp/test_twig_personal_tokens_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("initialize token database");
        let user = crate::auth::create_user(
            "personal-token-user".to_string(),
            "personal-token@example.com".to_string(),
            "password123",
        )
        .expect("create user");
        db.create_user(&user).await.expect("store user");
        let namespace = create_namespace("personal-token-ns".to_string(), user.id.clone());
        db.create_namespace(&namespace)
            .await
            .expect("store namespace");
        (db, db_path, user, vec![namespace.id])
    }

    #[tokio::test]
    async fn personal_token_metadata_and_namespace_permissions_round_trip() {
        let (db, db_path, user, namespace_ids) = setup_db().await;
        let permissions = [
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ];
        db.create_personal_access_token(NewPersonalAccessToken {
            id: "token-row-1",
            user_id: &user.id,
            name: "Build server",
            token_hash: "secret-hash",
            token_hint: "9a2f",
            namespace_ids: &namespace_ids,
            permissions: &permissions,
            expires_at: None,
        })
        .await
        .expect("store token and grants");

        let tokens = db
            .list_personal_access_tokens(&user.id)
            .await
            .expect("list user's tokens");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].name, "Build server");
        assert_eq!(tokens[0].token_hint, "9a2f");
        assert_eq!(tokens[0].expires_at, None);
        assert_eq!(tokens[0].scopes.len(), 1);
        assert_eq!(tokens[0].scopes[0].namespace_name, "personal-token-ns");
        assert!(
            tokens[0].scopes[0]
                .permissions
                .contains(NamespacePermission::ViewNamespace)
        );
        assert!(
            tokens[0].scopes[0]
                .permissions
                .contains(NamespacePermission::PushChanges)
        );

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn personal_token_authentication_joins_owner_enforces_expiry_and_updates_last_use() {
        let (db, db_path, user, namespace_ids) = setup_db().await;
        let token = "twigt_database_auth_token";
        let hash = crate::auth::hash_personal_access_token(token);
        db.create_personal_access_token(NewPersonalAccessToken {
            id: "token-auth-valid",
            user_id: &user.id,
            name: "Valid token",
            token_hash: &hash,
            token_hint: "oken",
            namespace_ids: &namespace_ids,
            permissions: &[NamespacePermission::PushChanges],
            expires_at: None,
        })
        .await
        .expect("store valid token");

        let expired_at = (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339();
        let expired_hash = crate::auth::hash_personal_access_token("twigt_database_expired");
        db.create_personal_access_token(NewPersonalAccessToken {
            id: "token-auth-expired",
            user_id: &user.id,
            name: "Expired token",
            token_hash: &expired_hash,
            token_hint: "ired",
            namespace_ids: &namespace_ids,
            permissions: &[NamespacePermission::ViewNamespace],
            expires_at: Some(&expired_at),
        })
        .await
        .expect("store expired token");

        let principal = db
            .authenticate_personal_access_token(&hash)
            .await
            .expect("authenticate token")
            .expect("valid token resolves");
        assert_eq!(principal.user.id, user.id);
        let permissions = principal
            .token_permissions
            .expect("token principal has explicit scopes");
        assert!(permissions["personal-token-ns"].contains(NamespacePermission::PushChanges));
        assert!(!permissions["personal-token-ns"].contains(NamespacePermission::ViewNamespace));
        assert!(
            db.authenticate_personal_access_token(&expired_hash)
                .await
                .expect("check expired token")
                .is_none()
        );
        assert!(
            db.authenticate_personal_access_token("unknown-hash")
                .await
                .expect("check unknown token")
                .is_none()
        );

        let tokens = db
            .list_personal_access_tokens(&user.id)
            .await
            .expect("load token metadata");
        let valid = tokens
            .iter()
            .find(|token| token.id == "token-auth-valid")
            .expect("valid token metadata");
        assert!(valid.last_used_at.is_some());
        let expired = tokens
            .iter()
            .find(|token| token.id == "token-auth-expired")
            .expect("expired token metadata");
        assert!(expired.last_used_at.is_none());

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn token_listing_and_revocation_are_owner_scoped() {
        let (db, db_path, user, namespace_ids) = setup_db().await;
        let other_user = crate::auth::create_user(
            "other-personal-token-user".to_string(),
            "other-personal-token@example.com".to_string(),
            "password123",
        )
        .expect("create other user");
        db.create_user(&other_user).await.expect("store other user");
        db.create_personal_access_token(NewPersonalAccessToken {
            id: "token-row-2",
            user_id: &user.id,
            name: "Owner token",
            token_hash: "another-hash",
            token_hint: "ab12",
            namespace_ids: &namespace_ids,
            permissions: &[NamespacePermission::ViewNamespace],
            expires_at: None,
        })
        .await
        .expect("store token");

        assert!(
            db.list_personal_access_tokens(&other_user.id)
                .await
                .expect("list other user's tokens")
                .is_empty()
        );
        assert!(
            !db.revoke_personal_access_token(&other_user.id, "token-row-2")
                .await
                .expect("foreign owner cannot revoke")
        );
        assert_eq!(
            db.list_personal_access_tokens(&user.id)
                .await
                .expect("list owner tokens")
                .len(),
            1
        );
        assert!(
            db.revoke_personal_access_token(&user.id, "token-row-2")
                .await
                .expect("owner revokes token")
        );
        assert!(
            db.list_personal_access_tokens(&user.id)
                .await
                .expect("list after revocation")
                .is_empty()
        );

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn token_permissions_table_rejects_unknown_permission_codes() {
        let (db, db_path, user, namespace_ids) = setup_db().await;
        db.create_personal_access_token(NewPersonalAccessToken {
            id: "token-row-3",
            user_id: &user.id,
            name: "Owner token",
            token_hash: "unknown-code-hash",
            token_hint: "cdef",
            namespace_ids: &namespace_ids,
            permissions: &[NamespacePermission::ViewNamespace],
            expires_at: None,
        })
        .await
        .expect("store token");

        let result = db
            .conn()
            .await
            .expect("open database")
            .execute(
                "INSERT INTO personal_access_token_permissions (token_id, namespace_id, permission)
                 VALUES (?1, ?2, ?3)",
                crate::db::sqlite::params!["token-row-3", namespace_ids[0].clone(), "unknown"],
            )
            .await;
        assert!(result.is_err(), "unknown permissions must not be stored");

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn permission_groups_cover_every_namespace_action_once() {
        let mut permissions = NamespacePermission::READ.to_vec();
        permissions.extend(NamespacePermission::WRITE);
        permissions.sort();
        let mut all = NamespacePermission::ALL.to_vec();
        all.sort();
        assert_eq!(permissions, all);
        for permission in NamespacePermission::ALL {
            assert_eq!(
                NamespacePermission::parse(permission.as_str()),
                Some(permission)
            );
        }
    }
}
