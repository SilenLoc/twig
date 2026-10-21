use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Duration, Utc};

use crate::auth::NamespacePermission;
use crate::db::Database;

pub struct NewDeviceAuthorization<'a> {
    pub id: &'a str,
    pub device_code_hash: &'a str,
    pub user_code_hash: &'a str,
    pub created_at: &'a str,
    pub expires_at: &'a str,
    pub poll_interval_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceAuthorizationStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

impl DeviceAuthorizationStatus {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "denied" => Ok(Self::Denied),
            _ => Err(format!("Unknown device authorization status: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceAuthorizationScope {
    pub namespace_id: String,
    pub permission: NamespacePermission,
}

#[derive(Debug, Clone)]
pub struct DeviceAuthorization {
    pub status: DeviceAuthorizationStatus,
}

#[derive(Debug, Clone)]
pub struct ApprovedDeviceAuthorization {
    pub authorization_id: String,
    pub user_id: String,
    pub scopes: Vec<DeviceAuthorizationScope>,
}

pub struct NewDeviceAccessToken<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub token_hash: &'a str,
    pub token_hint: &'a str,
    pub expires_at: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub enum DeviceAuthorizationPollState {
    Pending { interval_seconds: u64 },
    SlowDown { interval_seconds: u64 },
    Denied,
    Expired,
    Approved(ApprovedDeviceAuthorization),
}

impl Database {
    pub async fn create_device_authorization(
        &self,
        challenge: NewDeviceAuthorization<'_>,
    ) -> Result<(), String> {
        let interval = i64::try_from(challenge.poll_interval_seconds)
            .map_err(|_| "Device poll interval is too large".to_string())?;
        self.conn()
            .await?
            .execute(
                "INSERT INTO device_authorizations
                    (id, device_code_hash, user_code_hash, status, created_at, expires_at,
                     poll_interval_seconds)
                 VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6)",
                crate::db::sqlite::params![
                    challenge.id,
                    challenge.device_code_hash,
                    challenge.user_code_hash,
                    challenge.created_at,
                    challenge.expires_at,
                    interval
                ],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn find_device_authorization_by_user_code_hash(
        &self,
        user_code_hash: &str,
    ) -> Result<Option<DeviceAuthorization>, String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT status, expires_at
                 FROM device_authorizations WHERE user_code_hash = ?1",
                crate::db::sqlite::params![user_code_hash],
            )
            .await
            .map_err(|error| error.to_string())?;
        let Some(row) = rows.next().await.map_err(|error| error.to_string())? else {
            return Ok(None);
        };
        let status_name: String = row.get(0).map_err(|error| error.to_string())?;
        let expires_at: String = row.get(1).map_err(|error| error.to_string())?;
        drop(rows);

        let status = parse_status_and_expiry(&status_name, &expires_at)?;
        Ok(Some(DeviceAuthorization { status }))
    }

    pub async fn approve_device_authorization(
        &self,
        user_code_hash: &str,
        user_id: &str,
        scopes: &[DeviceAuthorizationScope],
    ) -> Result<bool, String> {
        if scopes.is_empty() {
            return Err("Choose at least one namespace and permission.".to_string());
        }
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|error| error.to_string())?;

        let result: Result<bool, String> = async {
            let Some((authorization_id, status, expires_at)) =
                find_challenge_header_by_user_code_hash(&conn, user_code_hash).await?
            else {
                return Ok(false);
            };
            if status != "pending" || is_expired(&expires_at)? {
                return Ok(false);
            }

            let updated = conn
                .execute(
                    "UPDATE device_authorizations
                     SET status = 'approved', user_id = ?2
                     WHERE id = ?1 AND status = 'pending'",
                    crate::db::sqlite::params![authorization_id.clone(), user_id],
                )
                .await
                .map_err(|error| error.to_string())?;
            if updated == 0 {
                return Ok(false);
            }

            for scope in scopes {
                conn.execute(
                    "INSERT INTO device_authorization_permissions
                        (authorization_id, namespace_id, permission)
                     VALUES (?1, ?2, ?3)",
                    crate::db::sqlite::params![
                        authorization_id.clone(),
                        scope.namespace_id.clone(),
                        scope.permission.as_str()
                    ],
                )
                .await
                .map_err(|error| error.to_string())?;
            }
            Ok(true)
        }
        .await;

        finish_transaction(&conn, result, "Failed to commit device approval").await
    }

    pub async fn deny_device_authorization(
        &self,
        user_code_hash: &str,
        user_id: &str,
    ) -> Result<bool, String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|error| error.to_string())?;

        let result: Result<bool, String> = async {
            let Some((authorization_id, status, expires_at)) =
                find_challenge_header_by_user_code_hash(&conn, user_code_hash).await?
            else {
                return Ok(false);
            };
            if status != "pending" || is_expired(&expires_at)? {
                return Ok(false);
            }

            let updated = conn
                .execute(
                    "UPDATE device_authorizations
                     SET status = 'denied', user_id = ?2
                     WHERE id = ?1 AND status = 'pending'",
                    crate::db::sqlite::params![authorization_id, user_id],
                )
                .await
                .map_err(|error| error.to_string())?;
            Ok(updated > 0)
        }
        .await;

        finish_transaction(&conn, result, "Failed to commit device denial").await
    }

    pub async fn poll_device_authorization(
        &self,
        device_code_hash: &str,
    ) -> Result<DeviceAuthorizationPollState, String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|error| error.to_string())?;

        let result: Result<DeviceAuthorizationPollState, String> = async {
            let mut rows = conn
                .query(
                    "SELECT id, status, user_id, expires_at, poll_interval_seconds, last_polled_at
                     FROM device_authorizations WHERE device_code_hash = ?1",
                    crate::db::sqlite::params![device_code_hash],
                )
                .await
                .map_err(|error| error.to_string())?;
            let Some(row) = rows.next().await.map_err(|error| error.to_string())? else {
                return Ok(DeviceAuthorizationPollState::Expired);
            };
            let authorization_id: String = row.get(0).map_err(|error| error.to_string())?;
            let status: String = row.get(1).map_err(|error| error.to_string())?;
            let user_id: Option<String> = row.get(2).map_err(|error| error.to_string())?;
            let expires_at: String = row.get(3).map_err(|error| error.to_string())?;
            let interval_value: i64 = row.get(4).map_err(|error| error.to_string())?;
            let last_polled_at: Option<String> = row.get(5).map_err(|error| error.to_string())?;
            drop(rows);

            if is_expired(&expires_at)? {
                delete_challenge(&conn, &authorization_id).await?;
                return Ok(DeviceAuthorizationPollState::Expired);
            }
            if status == "denied" {
                return Ok(DeviceAuthorizationPollState::Denied);
            }
            if status != "pending" && status != "approved" {
                return Err(format!("Unknown device authorization status: {status}"));
            }

            let mut interval = positive_interval(interval_value)?;
            let now = Utc::now();
            if let Some(last_polled_at) = last_polled_at {
                let last = parse_timestamp(&last_polled_at)?;
                let interval_seconds = i64::try_from(interval)
                    .map_err(|_| "Poll interval is too large".to_string())?;
                if now.signed_duration_since(last) < Duration::seconds(interval_seconds) {
                    interval = interval.saturating_add(5);
                    conn.execute(
                        "UPDATE device_authorizations
                         SET poll_interval_seconds = ?2, last_polled_at = ?3
                         WHERE id = ?1",
                        crate::db::sqlite::params![
                            authorization_id,
                            i64::try_from(interval)
                                .map_err(|_| "Poll interval is too large".to_string())?,
                            now.to_rfc3339()
                        ],
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                    return Ok(DeviceAuthorizationPollState::SlowDown {
                        interval_seconds: interval,
                    });
                }
            }

            conn.execute(
                "UPDATE device_authorizations SET last_polled_at = ?2 WHERE id = ?1",
                crate::db::sqlite::params![authorization_id.clone(), now.to_rfc3339()],
            )
            .await
            .map_err(|error| error.to_string())?;

            if status == "pending" {
                return Ok(DeviceAuthorizationPollState::Pending {
                    interval_seconds: interval,
                });
            }
            let user_id = user_id
                .ok_or_else(|| "Approved device authorization has no approving user".to_string())?;
            let scopes = load_scopes(&conn, &authorization_id).await?;
            if scopes.is_empty() {
                return Err("Approved device authorization has no permissions".to_string());
            }
            Ok(DeviceAuthorizationPollState::Approved(
                ApprovedDeviceAuthorization {
                    authorization_id,
                    user_id,
                    scopes,
                },
            ))
        }
        .await;

        finish_transaction(&conn, result, "Failed to commit device poll").await
    }

    pub async fn redeem_device_authorization(
        &self,
        device_code_hash: &str,
        token: NewDeviceAccessToken<'_>,
    ) -> Result<bool, String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|error| error.to_string())?;

        let result: Result<bool, String> = async {
            let mut rows = conn
                .query(
                    "SELECT id, user_id, expires_at
                     FROM device_authorizations
                     WHERE device_code_hash = ?1 AND status = 'approved'",
                    crate::db::sqlite::params![device_code_hash],
                )
                .await
                .map_err(|error| error.to_string())?;
            let Some(row) = rows.next().await.map_err(|error| error.to_string())? else {
                return Ok(false);
            };
            let authorization_id: String = row.get(0).map_err(|error| error.to_string())?;
            let user_id: String = row.get(1).map_err(|error| error.to_string())?;
            let expires_at: String = row.get(2).map_err(|error| error.to_string())?;
            drop(rows);
            if is_expired(&expires_at)? {
                delete_challenge(&conn, &authorization_id).await?;
                return Ok(false);
            }

            let scopes = load_scopes(&conn, &authorization_id).await?;
            let (namespace_ids, permissions) = token_permissions_from_scopes(&scopes)?;
            crate::db::personal_access_tokens::insert_personal_access_token(
                &conn,
                crate::db::personal_access_tokens::NewPersonalAccessToken {
                    id: token.id,
                    user_id: &user_id,
                    name: token.name,
                    token_hash: token.token_hash,
                    token_hint: token.token_hint,
                    namespace_ids: &namespace_ids,
                    permissions: &permissions,
                    expires_at: token.expires_at,
                },
            )
            .await?;
            delete_challenge(&conn, &authorization_id).await?;
            Ok(true)
        }
        .await;

        finish_transaction(&conn, result, "Failed to commit device token redemption").await
    }

    pub async fn cleanup_expired_device_authorizations(&self) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|error| error.to_string())?;
        let result: Result<(), String> = async {
            conn.execute(
                "DELETE FROM device_authorization_permissions
                 WHERE authorization_id IN (
                     SELECT id FROM device_authorizations
                     WHERE datetime(expires_at) <= datetime('now')
                 )",
                (),
            )
            .await
            .map_err(|error| error.to_string())?;
            conn.execute(
                "DELETE FROM device_authorizations
                 WHERE datetime(expires_at) <= datetime('now')",
                (),
            )
            .await
            .map_err(|error| error.to_string())?;
            Ok(())
        }
        .await;
        finish_transaction(
            &conn,
            result,
            "Failed to commit device authorization cleanup",
        )
        .await
    }
}

async fn find_challenge_header_by_user_code_hash(
    conn: &crate::db::sqlite::Connection,
    user_code_hash: &str,
) -> Result<Option<(String, String, String)>, String> {
    let mut rows = conn
        .query(
            "SELECT id, status, expires_at FROM device_authorizations WHERE user_code_hash = ?1",
            crate::db::sqlite::params![user_code_hash],
        )
        .await
        .map_err(|error| error.to_string())?;
    let Some(row) = rows.next().await.map_err(|error| error.to_string())? else {
        return Ok(None);
    };
    Ok(Some((
        row.get(0).map_err(|error| error.to_string())?,
        row.get(1).map_err(|error| error.to_string())?,
        row.get(2).map_err(|error| error.to_string())?,
    )))
}

async fn load_scopes(
    conn: &crate::db::sqlite::Connection,
    authorization_id: &str,
) -> Result<Vec<DeviceAuthorizationScope>, String> {
    let mut rows = conn
        .query(
            "SELECT namespace_id, permission
             FROM device_authorization_permissions
             WHERE authorization_id = ?1
             ORDER BY namespace_id, permission",
            crate::db::sqlite::params![authorization_id],
        )
        .await
        .map_err(|error| error.to_string())?;
    let mut scopes = Vec::new();
    while let Some(row) = rows.next().await.map_err(|error| error.to_string())? {
        let namespace_id: String = row.get(0).map_err(|error| error.to_string())?;
        let permission_name: String = row.get(1).map_err(|error| error.to_string())?;
        let permission = NamespacePermission::parse(&permission_name)
            .ok_or_else(|| format!("Unknown device authorization permission: {permission_name}"))?;
        scopes.push(DeviceAuthorizationScope {
            namespace_id,
            permission,
        });
    }
    Ok(scopes)
}

async fn delete_challenge(
    conn: &crate::db::sqlite::Connection,
    authorization_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM device_authorization_permissions WHERE authorization_id = ?1",
        crate::db::sqlite::params![authorization_id],
    )
    .await
    .map_err(|error| error.to_string())?;
    conn.execute(
        "DELETE FROM device_authorizations WHERE id = ?1",
        crate::db::sqlite::params![authorization_id],
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn token_permissions_from_scopes(
    scopes: &[DeviceAuthorizationScope],
) -> Result<(Vec<String>, Vec<NamespacePermission>), String> {
    if scopes.is_empty() {
        return Err("Device authorization has no permissions".to_string());
    }
    let mut by_namespace = BTreeMap::<String, BTreeSet<NamespacePermission>>::new();
    for scope in scopes {
        by_namespace
            .entry(scope.namespace_id.clone())
            .or_default()
            .insert(scope.permission);
    }
    let Some(permissions) = by_namespace.values().next().cloned() else {
        return Err("Device authorization has no namespaces".to_string());
    };
    if by_namespace
        .values()
        .any(|selected| selected != &permissions)
    {
        return Err("Device token permissions must match across selected namespaces".to_string());
    }
    Ok((
        by_namespace.into_keys().collect(),
        permissions.into_iter().collect(),
    ))
}

fn positive_interval(value: i64) -> Result<u64, String> {
    u64::try_from(value)
        .ok()
        .filter(|interval| *interval > 0)
        .ok_or_else(|| "Device poll interval must be positive".to_string())
}

fn parse_status_and_expiry(
    status: &str,
    expires_at: &str,
) -> Result<DeviceAuthorizationStatus, String> {
    if is_expired(expires_at)? {
        Ok(DeviceAuthorizationStatus::Expired)
    } else {
        DeviceAuthorizationStatus::parse(status)
    }
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|error| format!("Invalid device authorization timestamp: {error}"))
}

fn is_expired(expires_at: &str) -> Result<bool, String> {
    Ok(parse_timestamp(expires_at)? <= Utc::now())
}

async fn finish_transaction<T>(
    conn: &crate::db::sqlite::Connection,
    result: Result<T, String>,
    commit_error: &str,
) -> Result<T, String> {
    match result {
        Ok(value) => {
            conn.execute("COMMIT", ())
                .await
                .map_err(|error| format!("{commit_error}: {error}"))?;
            Ok(value)
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{User, create_namespace};

    async fn setup_db() -> (Database, String, String, String) {
        let db_path = format!("/tmp/test_twig_device_auth_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("initialize tables");
        let user_id = uuid::Uuid::new_v4().to_string();
        let user = User {
            id: user_id.clone(),
            username: "device-owner".to_string(),
            email: Some("device-owner@example.com".to_string()),
            password_hash: "hash".to_string(),
            created_at: Utc::now().to_rfc3339(),
        };
        db.create_user(&user).await.expect("create user");
        let namespace = create_namespace("device-ns".to_string(), user_id.clone());
        db.create_namespace(&namespace)
            .await
            .expect("create namespace");
        (db, db_path, user_id, namespace.id)
    }

    fn challenge<'a>(
        id: &'a str,
        device_hash: &'a str,
        user_hash: &'a str,
        expires_at: &'a str,
    ) -> NewDeviceAuthorization<'a> {
        NewDeviceAuthorization {
            id,
            device_code_hash: device_hash,
            user_code_hash: user_hash,
            created_at: "2026-10-20T12:00:00+00:00",
            expires_at,
            poll_interval_seconds: 5,
        }
    }

    #[tokio::test]
    async fn device_challenges_store_only_hashes_and_redeem_once() {
        let (db, db_path, user_id, namespace_id) = setup_db().await;
        let expires_at = (Utc::now() + Duration::minutes(10)).to_rfc3339();
        db.create_device_authorization(challenge(
            "authorization-1",
            "device-hash",
            "user-hash",
            &expires_at,
        ))
        .await
        .expect("create challenge");
        let scopes = vec![DeviceAuthorizationScope {
            namespace_id: namespace_id.clone(),
            permission: NamespacePermission::ViewNamespace,
        }];
        assert!(
            db.approve_device_authorization("user-hash", &user_id, &scopes)
                .await
                .expect("approve challenge")
        );

        let first = db
            .redeem_device_authorization(
                "device-hash",
                NewDeviceAccessToken {
                    id: "token-1",
                    name: "Tree CLI",
                    token_hash: "token-hash",
                    token_hint: "hint",
                    expires_at: None,
                },
            )
            .await
            .expect("redeem challenge");
        let second = db
            .redeem_device_authorization(
                "device-hash",
                NewDeviceAccessToken {
                    id: "token-2",
                    name: "Tree CLI",
                    token_hash: "another-hash",
                    token_hint: "hint",
                    expires_at: None,
                },
            )
            .await
            .expect("try duplicate redemption");

        assert!(first);
        assert!(!second);
        let tokens = db
            .list_personal_access_tokens(&user_id)
            .await
            .expect("list created token");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].name, "Tree CLI");
        assert_eq!(tokens[0].scopes.len(), 1);
        assert_eq!(tokens[0].scopes[0].namespace_id, namespace_id);
        assert!(
            db.find_device_authorization_by_user_code_hash("user-hash")
                .await
                .expect("look up consumed challenge")
                .is_none()
        );

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn expired_device_challenges_are_rejected_and_cleaned() {
        let (db, db_path, _user_id, _namespace_id) = setup_db().await;
        let expired = (Utc::now() - Duration::minutes(1)).to_rfc3339();
        db.create_device_authorization(challenge(
            "authorization-expired",
            "device-expired",
            "user-expired",
            &expired,
        ))
        .await
        .expect("create expired challenge");

        assert!(matches!(
            db.poll_device_authorization("device-expired")
                .await
                .expect("poll expired challenge"),
            DeviceAuthorizationPollState::Expired
        ));
        db.cleanup_expired_device_authorizations()
            .await
            .expect("clean expired challenges");
        assert!(
            db.find_device_authorization_by_user_code_hash("user-expired")
                .await
                .expect("look up expired challenge")
                .is_none()
        );

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn polling_too_soon_increases_the_required_interval() {
        let (db, db_path, _user_id, _namespace_id) = setup_db().await;
        let expires_at = (Utc::now() + Duration::minutes(10)).to_rfc3339();
        db.create_device_authorization(challenge(
            "authorization-slowdown",
            "device-slowdown",
            "user-slowdown",
            &expires_at,
        ))
        .await
        .expect("create challenge");
        db.conn()
            .await
            .expect("connect to test database")
            .execute(
                "UPDATE device_authorizations SET last_polled_at = ?2 WHERE id = ?1",
                crate::db::sqlite::params!["authorization-slowdown", Utc::now().to_rfc3339()],
            )
            .await
            .expect("set prior poll time");

        assert!(matches!(
            db.poll_device_authorization("device-slowdown")
                .await
                .expect("poll too soon"),
            DeviceAuthorizationPollState::SlowDown {
                interval_seconds: 10
            }
        ));

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn concurrent_redemption_creates_only_one_personal_token() {
        let (db, db_path, user_id, namespace_id) = setup_db().await;
        let expires_at = (Utc::now() + Duration::minutes(10)).to_rfc3339();
        db.create_device_authorization(challenge(
            "authorization-concurrent",
            "device-concurrent",
            "user-concurrent",
            &expires_at,
        ))
        .await
        .expect("create challenge");
        let scopes = vec![DeviceAuthorizationScope {
            namespace_id,
            permission: NamespacePermission::ViewNamespace,
        }];
        db.approve_device_authorization("user-concurrent", &user_id, &scopes)
            .await
            .expect("approve challenge");

        let first_db = db.clone();
        let second_db = db.clone();
        let (first, second) = tokio::join!(
            first_db.redeem_device_authorization(
                "device-concurrent",
                NewDeviceAccessToken {
                    id: "concurrent-token-one",
                    name: "Tree CLI",
                    token_hash: "concurrent-hash-one",
                    token_hint: "one1",
                    expires_at: None,
                },
            ),
            second_db.redeem_device_authorization(
                "device-concurrent",
                NewDeviceAccessToken {
                    id: "concurrent-token-two",
                    name: "Tree CLI",
                    token_hash: "concurrent-hash-two",
                    token_hint: "two2",
                    expires_at: None,
                },
            )
        );
        let outcomes = [
            first.expect("first redemption"),
            second.expect("second redemption"),
        ];
        assert_eq!(outcomes.into_iter().filter(|redeemed| *redeemed).count(), 1);
        assert_eq!(
            db.list_personal_access_tokens(&user_id)
                .await
                .expect("list concurrent token")
                .len(),
            1
        );

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn token_scopes_do_not_union_different_namespace_grants() {
        let scopes = vec![
            DeviceAuthorizationScope {
                namespace_id: "namespace-one".to_string(),
                permission: NamespacePermission::ViewNamespace,
            },
            DeviceAuthorizationScope {
                namespace_id: "namespace-two".to_string(),
                permission: NamespacePermission::ViewNamespace,
            },
            DeviceAuthorizationScope {
                namespace_id: "namespace-two".to_string(),
                permission: NamespacePermission::PushChanges,
            },
        ];

        assert!(token_permissions_from_scopes(&scopes).is_err());
    }
}
