use std::collections::BTreeSet;

use chrono::{Duration, Utc};
use rand::{TryRng, rngs::SysRng};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    auth::{
        NamespacePermission, PermissionSet, TwigContext, generate_personal_access_token,
        hash_personal_access_token, permissions,
    },
    config,
    db::device_authorizations::{
        ApprovedDeviceAuthorization, DeviceAuthorizationPollState, DeviceAuthorizationScope,
        DeviceAuthorizationStatus, NewDeviceAccessToken, NewDeviceAuthorization,
    },
};

const DEVICE_AUTHORIZATION_LIFETIME: Duration = Duration::minutes(10);
const INITIAL_POLL_INTERVAL_SECONDS: u64 = 5;
pub const TREE_ACCESS_TOKEN_NAME: &str = "Tree CLI";
const USER_CODE_ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

#[derive(Serialize)]
pub struct DeviceAuthorizationChallenge {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

pub enum DeviceAuthorizationDecision {
    Approve {
        namespace_ids: Vec<String>,
        permissions: PermissionSet,
    },
    Deny,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DeviceTokenPoll {
    Pending { interval: u64 },
    SlowDown { interval: u64 },
    Denied,
    Expired,
    Authorized { access_token: String },
}

impl TwigContext {
    pub async fn start_device_authorization(&self) -> Result<DeviceAuthorizationChallenge, String> {
        self.db().cleanup_expired_device_authorizations().await?;

        let device_code = super::generate_token();
        let user_code = generate_user_code();
        let now = Utc::now();
        let created_at = now.to_rfc3339();
        let expires_at = (now + DEVICE_AUTHORIZATION_LIFETIME).to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let device_code_hash = hash_device_code(&device_code);
        let user_code_hash = hash_device_code(&user_code);
        self.db()
            .create_device_authorization(NewDeviceAuthorization {
                id: &id,
                device_code_hash: &device_code_hash,
                user_code_hash: &user_code_hash,
                created_at: &created_at,
                expires_at: &expires_at,
                poll_interval_seconds: INITIAL_POLL_INTERVAL_SECONDS,
            })
            .await?;

        Ok(DeviceAuthorizationChallenge {
            device_code,
            user_code: format_user_code(&user_code),
            verification_uri: "/auth/device".to_string(),
            expires_in: DEVICE_AUTHORIZATION_LIFETIME.num_seconds().cast_unsigned(),
            interval: INITIAL_POLL_INTERVAL_SECONDS,
        })
    }

    pub async fn decide_device_authorization(
        &self,
        server: &config::Server,
        user_id: &str,
        user_code: &str,
        decision: DeviceAuthorizationDecision,
    ) -> Result<(), String> {
        let normalized_code = normalize_user_code(user_code)?;
        let user_code_hash = hash_device_code(&normalized_code);
        let challenge = self
            .db()
            .find_device_authorization_by_user_code_hash(&user_code_hash)
            .await?
            .ok_or_else(|| "Device authorization was not found or has expired.".to_string())?;
        if challenge.status != DeviceAuthorizationStatus::Pending {
            return Err("Device authorization was already decided or has expired.".to_string());
        }

        match decision {
            DeviceAuthorizationDecision::Deny => {
                if self
                    .db()
                    .deny_device_authorization(&user_code_hash, user_id)
                    .await?
                {
                    Ok(())
                } else {
                    Err("Device authorization was already decided or has expired.".to_string())
                }
            }
            DeviceAuthorizationDecision::Approve {
                namespace_ids,
                permissions: selected_permissions,
            } => {
                let user = self
                    .db()
                    .get_user_by_id(user_id)
                    .await?
                    .ok_or_else(|| "Signed-in user was not found.".to_string())?;
                let is_site_admin = server.is_configured_admin(&user.username);
                let accessible = permissions::namespaces_for_user(
                    self.db(),
                    user_id,
                    is_site_admin,
                    NamespacePermission::ViewNamespace,
                )
                .await?;
                let selected_ids = namespace_ids.into_iter().collect::<BTreeSet<_>>();
                let selected_namespaces =
                    permissions::select_namespaces(&accessible, false, &selected_ids)?;
                permissions::validate_selected_permissions(
                    self.db(),
                    user_id,
                    is_site_admin,
                    &selected_namespaces,
                    &selected_permissions,
                )
                .await?;

                let scopes = selected_namespaces
                    .iter()
                    .flat_map(|namespace| {
                        selected_permissions
                            .iter()
                            .map(|permission| DeviceAuthorizationScope {
                                namespace_id: namespace.id.clone(),
                                permission,
                            })
                    })
                    .collect::<Vec<_>>();
                if self
                    .db()
                    .approve_device_authorization(&user_code_hash, user_id, &scopes)
                    .await?
                {
                    Ok(())
                } else {
                    Err("Device authorization was already decided or has expired.".to_string())
                }
            }
        }
    }

    pub async fn poll_device_authorization(
        &self,
        server: &config::Server,
        device_code: &str,
    ) -> Result<DeviceTokenPoll, String> {
        if device_code.is_empty() {
            return Ok(DeviceTokenPoll::Expired);
        }
        let device_code_hash = hash_device_code(device_code);
        match self
            .db()
            .poll_device_authorization(&device_code_hash)
            .await?
        {
            DeviceAuthorizationPollState::Pending { interval_seconds } => {
                Ok(DeviceTokenPoll::Pending {
                    interval: interval_seconds,
                })
            }
            DeviceAuthorizationPollState::SlowDown { interval_seconds } => {
                Ok(DeviceTokenPoll::SlowDown {
                    interval: interval_seconds,
                })
            }
            DeviceAuthorizationPollState::Denied => Ok(DeviceTokenPoll::Denied),
            DeviceAuthorizationPollState::Expired => Ok(DeviceTokenPoll::Expired),
            DeviceAuthorizationPollState::Approved(authorization) => {
                if !self.device_grant_is_current(server, &authorization).await? {
                    log::warn!(
                        "Tree device authorization {} no longer matches the user's permissions",
                        authorization.authorization_id
                    );
                    return Ok(DeviceTokenPoll::Denied);
                }

                let access_token = generate_personal_access_token();
                let token_hash = hash_personal_access_token(&access_token);
                let token_hint = access_token
                    .chars()
                    .rev()
                    .take(4)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<String>();
                let token_id = Uuid::new_v4().to_string();
                let redeemed = self
                    .db()
                    .redeem_device_authorization(
                        &device_code_hash,
                        NewDeviceAccessToken {
                            id: &token_id,
                            name: TREE_ACCESS_TOKEN_NAME,
                            token_hash: &token_hash,
                            token_hint: &token_hint,
                            expires_at: None,
                        },
                    )
                    .await?;
                if redeemed {
                    Ok(DeviceTokenPoll::Authorized { access_token })
                } else {
                    Ok(DeviceTokenPoll::Expired)
                }
            }
        }
    }

    async fn device_grant_is_current(
        &self,
        server: &config::Server,
        authorization: &ApprovedDeviceAuthorization,
    ) -> Result<bool, String> {
        let Some(user) = self.db().get_user_by_id(&authorization.user_id).await? else {
            return Ok(false);
        };
        let is_site_admin = server.is_configured_admin(&user.username);
        let accessible = permissions::namespaces_for_user(
            self.db(),
            &authorization.user_id,
            is_site_admin,
            NamespacePermission::ViewNamespace,
        )
        .await?;
        let selected_ids = authorization
            .scopes
            .iter()
            .map(|scope| scope.namespace_id.clone())
            .collect::<BTreeSet<_>>();
        let Ok(selected_namespaces) =
            permissions::select_namespaces(&accessible, false, &selected_ids)
        else {
            return Ok(false);
        };
        let Ok(selected_permissions) = permission_set_from_scopes(&authorization.scopes) else {
            return Ok(false);
        };
        Ok(permissions::validate_selected_permissions(
            self.db(),
            &authorization.user_id,
            is_site_admin,
            &selected_namespaces,
            &selected_permissions,
        )
        .await
        .is_ok())
    }
}

pub fn normalize_user_code(value: &str) -> Result<String, String> {
    let normalized = value
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && *character != '-')
        .map(|character| character.to_ascii_uppercase())
        .collect::<String>();
    if normalized.len() != 8
        || !normalized
            .bytes()
            .all(|character| USER_CODE_ALPHABET.contains(&character))
    {
        return Err("Enter the eight-character code shown in Tree.".to_string());
    }
    Ok(normalized)
}

pub(crate) fn hash_device_code(code: &str) -> String {
    hex::encode(Sha256::digest(code.as_bytes()))
}

fn generate_user_code() -> String {
    let mut random = [0_u8; 5];
    SysRng
        .try_fill_bytes(&mut random)
        .expect("operating system RNG should be available");

    let mut code = String::with_capacity(8);
    let mut buffer = 0_u64;
    let mut bits = 0_u32;
    for byte in random {
        buffer = (buffer << 8) | u64::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let index = ((buffer >> bits) & 0b1_1111) as usize;
            code.push(char::from(USER_CODE_ALPHABET[index]));
        }
    }
    code
}

pub fn format_user_code(code: &str) -> String {
    format!("{}-{}", &code[..4], &code[4..])
}

fn permission_set_from_scopes(
    scopes: &[DeviceAuthorizationScope],
) -> Result<PermissionSet, String> {
    let mut by_namespace =
        std::collections::BTreeMap::<String, BTreeSet<NamespacePermission>>::new();
    for scope in scopes {
        by_namespace
            .entry(scope.namespace_id.clone())
            .or_default()
            .insert(scope.permission);
    }
    let Some(permissions) = by_namespace.values().next().cloned() else {
        return Err("Device authorization has no scopes".to_string());
    };
    if by_namespace
        .values()
        .any(|selected| selected != &permissions)
    {
        return Err("Device authorization scopes do not match across namespaces".to_string());
    }
    Ok(PermissionSet::new(permissions))
}
