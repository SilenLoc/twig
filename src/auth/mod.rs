use std::collections::BTreeMap;

use actix_identity::IdentityExt;
use actix_web::HttpRequest;
use argon2::password_hash::phc::PasswordHash;
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use rand::TryRng;
use rand::rngs::SysRng;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::db::Database;
use crate::db::personal_access_tokens::NewPersonalAccessToken;

pub mod permissions;
pub mod session_store;
pub use permissions::{NamespacePermission, PermissionSet};

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub password_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct Namespace {
    pub id: String,
    pub name: String,
    pub owner_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct Invite {
    pub id: String,
    pub user_id: Option<String>,
    pub used: bool,
    pub created_at: String,
    pub used_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamespaceRole {
    Owner,
    Contributor,
}

impl NamespaceRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Contributor => "contributor",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Owner => "Owner",
            Self::Contributor => "Contributor",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "owner" => Some(Self::Owner),
            "contributor" | "member" => Some(Self::Contributor),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NamespaceInvitation {
    pub token: String,
    pub email: String,
    pub namespace_id: String,
    pub namespace_name: String,
    pub role: NamespaceRole,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub accepted_at: Option<String>,
    pub accepted_user_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TokenNamespacePermissions {
    pub namespace_id: String,
    pub namespace_name: String,
    pub permissions: PermissionSet,
}

#[derive(Debug, Clone)]
pub struct PersonalAccessToken {
    pub id: String,
    pub name: String,
    pub token_hint: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub expires_at: Option<String>,
    pub scopes: Vec<TokenNamespacePermissions>,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedPrincipal {
    pub user: User,
    // None means ordinary password authentication; Some is the token's namespace scope.
    pub token_permissions: Option<BTreeMap<String, PermissionSet>>,
}

#[derive(Debug, Clone)]
pub enum CredentialAuth {
    Missing,
    Invalid,
    Authenticated(AuthenticatedPrincipal),
}

#[derive(Debug, Clone)]
pub struct NewNamespaceInvitation {
    pub email: String,
    pub namespace_id: String,
    pub role: NamespaceRole,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UserManagementEntry {
    pub username: String,
    pub email: Option<String>,
    pub memberships: Vec<(String, NamespaceRole)>,
    pub is_site_admin: bool,
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes())
        .map_err(|e| e.to_string())?;

    Ok(password_hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
    let argon2 = Argon2::default();
    let parsed_hash = PasswordHash::new(hash).map_err(|e| e.to_string())?;

    Ok(argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    SysRng
        .try_fill_bytes(&mut bytes)
        .expect("operating system RNG should be available");
    hex::encode(bytes)
}

pub const PERSONAL_ACCESS_TOKEN_PREFIX: &str = "twigt_";

pub fn generate_personal_access_token() -> String {
    format!("{PERSONAL_ACCESS_TOKEN_PREFIX}{}", generate_token())
}

pub fn hash_personal_access_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn create_user(username: String, email: String, password: &str) -> Result<User, String> {
    let password_hash = hash_password(password)?;
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let user = User {
        id,
        username,
        email: Some(email),
        password_hash,
        created_at: now,
    };

    Ok(user)
}

#[cfg(test)]
pub fn generate_invite() -> String {
    Uuid::new_v4().to_string()
}

pub fn create_namespace(name: String, owner_id: String) -> Namespace {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    Namespace {
        id,
        name,
        owner_id,
        created_at: now,
    }
}

pub fn extract_basic_auth(req: &HttpRequest) -> Option<(String, String)> {
    let auth_header = req.headers().get("Authorization")?;
    let auth_str = auth_header.to_str().ok()?;

    let (scheme, encoded) = auth_str.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Basic") {
        return None;
    }

    let decoded = base64_decode(encoded.trim())?;

    let parts: Vec<&str> = decoded.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }

    Some((parts[0].to_string(), parts[1].to_string()))
}

fn base64_decode(input: &str) -> Option<String> {
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(input)
        .ok()?;
    String::from_utf8(decoded).ok()
}

pub struct TwigContext {
    db: Database,
    api_key: String,
}

impl TwigContext {
    pub fn new(db: Database, api_key: String) -> Self {
        Self { db, api_key }
    }

    pub fn db(&self) -> &Database {
        &self.db
    }

    pub fn validate_api_key(&self, key: &str) -> bool {
        self.api_key == key
    }

    pub async fn create_session(&self, user_id: String) -> Result<String, String> {
        let db = self.db();
        let token = generate_token();
        db.create_token(&token, &user_id).await?;
        if let Err(e) = db.cleanup_expired_tokens().await {
            log::warn!("Failed to cleanup expired tokens: {e}");
        }
        Ok(token)
    }

    pub async fn validate_token(&self, token: &str) -> Option<String> {
        let db = self.db();
        db.get_token_user(token).await.ok().flatten()
    }

    pub async fn invalidate_token(&self, token: &str) -> Result<(), String> {
        let db = self.db();
        db.delete_token(token).await
    }

    pub async fn issue_personal_access_token(
        &self,
        user_id: &str,
        name: &str,
        namespace_ids: &[String],
        permissions: &PermissionSet,
        expires_at: Option<&str>,
    ) -> Result<(PersonalAccessToken, String), String> {
        let secret = generate_personal_access_token();
        let token_hash = hash_personal_access_token(&secret);
        let token_hint = secret.chars().rev().take(4).collect::<Vec<_>>();
        let token_hint = token_hint.into_iter().rev().collect::<String>();
        let token_id = Uuid::new_v4().to_string();
        let permissions = permissions.iter().collect::<Vec<NamespacePermission>>();

        self.db
            .create_personal_access_token(NewPersonalAccessToken {
                id: &token_id,
                user_id,
                name,
                token_hash: &token_hash,
                token_hint: &token_hint,
                namespace_ids,
                permissions: &permissions,
                expires_at,
            })
            .await?;

        let tokens = match self.db.list_personal_access_tokens(user_id).await {
            Ok(tokens) => tokens,
            Err(error) => {
                if let Err(revoke_error) = self
                    .db
                    .revoke_personal_access_token(user_id, &token_id)
                    .await
                {
                    return Err(format!(
                        "Failed to load created token ({error}); cleanup failed ({revoke_error})"
                    ));
                }
                return Err(error);
            }
        };
        let Some(token) = tokens.into_iter().find(|token| token.id == token_id) else {
            self.db
                .revoke_personal_access_token(user_id, &token_id)
                .await?;
            return Err("Created token was not returned from the database".to_string());
        };

        Ok((token, secret))
    }

    pub async fn authenticate_request(&self, req: &HttpRequest) -> Result<CredentialAuth, String> {
        match self.authenticate_bearer(req).await? {
            CredentialAuth::Missing => {}
            credential => return Ok(credential),
        }

        let Some(auth_header) = req.headers().get("Authorization") else {
            return Ok(CredentialAuth::Missing);
        };
        let Ok(auth_value) = auth_header.to_str() else {
            return Ok(CredentialAuth::Missing);
        };
        let Some((scheme, _)) = auth_value.split_once(' ') else {
            return Ok(CredentialAuth::Missing);
        };

        if !scheme.eq_ignore_ascii_case("Basic") {
            return Ok(CredentialAuth::Missing);
        }
        let Some((username, password)) = extract_basic_auth(req) else {
            return Ok(CredentialAuth::Missing);
        };
        if password.starts_with(PERSONAL_ACCESS_TOKEN_PREFIX) {
            return self.authenticate_personal_access_token(&password).await;
        }

        let Some(user) = self.db.get_user_by_username(&username).await? else {
            return Ok(CredentialAuth::Invalid);
        };
        match verify_password(&password, &user.password_hash) {
            Ok(true) => Ok(CredentialAuth::Authenticated(AuthenticatedPrincipal {
                user,
                token_permissions: None,
            })),
            Ok(false) => Ok(CredentialAuth::Invalid),
            Err(error) => Err(error),
        }
    }

    pub async fn authenticate_bearer(&self, req: &HttpRequest) -> Result<CredentialAuth, String> {
        let Some(auth_header) = req.headers().get("Authorization") else {
            return Ok(CredentialAuth::Missing);
        };
        let Ok(auth_value) = auth_header.to_str() else {
            return Ok(CredentialAuth::Missing);
        };
        let Some((scheme, credential)) = auth_value.split_once(' ') else {
            return Ok(CredentialAuth::Missing);
        };
        if !scheme.eq_ignore_ascii_case("Bearer") {
            return Ok(CredentialAuth::Missing);
        }

        let token = credential.trim();
        if !token.starts_with(PERSONAL_ACCESS_TOKEN_PREFIX) {
            return Ok(CredentialAuth::Missing);
        }
        self.authenticate_personal_access_token(token).await
    }

    async fn authenticate_personal_access_token(
        &self,
        token: &str,
    ) -> Result<CredentialAuth, String> {
        let token_hash = hash_personal_access_token(token);
        Ok(
            match self
                .db
                .authenticate_personal_access_token(&token_hash)
                .await?
            {
                Some(principal) => CredentialAuth::Authenticated(principal),
                None => CredentialAuth::Invalid,
            },
        )
    }

    pub async fn user_id_from_request(&self, req: &HttpRequest) -> Option<String> {
        if req.cookie("id").is_some() {
            if let Ok(identity) = req.get_identity()
                && let Ok(user_id) = identity.id()
                && self
                    .db
                    .get_user_by_id(&user_id)
                    .await
                    .ok()
                    .flatten()
                    .is_some()
            {
                return Some(user_id);
            }
            return None;
        }

        let token = req.cookie("session")?;
        self.validate_token(token.value()).await
    }

    pub async fn get_test_pin(&self) -> Option<String> {
        match self.db().get_test_pin().await {
            Ok(pin) => pin,
            Err(e) => {
                log::error!("Failed to get test pin from database: {e}");
                None
            }
        }
    }

    pub async fn set_test_pin(&self, pin: &str) -> Result<(), String> {
        self.db().set_test_pin(pin).await
    }

    pub async fn clear_test_pin(&self) -> Result<(), String> {
        self.db().clear_test_pins().await
    }

    pub async fn validate_test_pin(&self, pin: &str) -> bool {
        if pin.is_empty() {
            return false;
        }
        match self.db().validate_test_pin(pin).await {
            Ok(valid) => valid,
            Err(e) => {
                log::error!("Failed to validate test pin from database: {e}");
                false
            }
        }
    }
}

/// Dev convenience seeded when `RESET_DB` is true: creates a fixed
/// `admin`/`admin` user (and an `admin` namespace) if they do not already
/// exist, then returns a fresh session token for that user so local
/// inspection never has to go through signup/login manually.
pub async fn seed_dev_admin(ctx: &TwigContext, project_root: &str) -> Result<String, String> {
    let db = ctx.db();

    let user = if let Some(user) = db.get_user_by_username("admin").await? {
        user
    } else {
        let user = create_user("admin".to_string(), "admin@localhost".to_string(), "admin")?;
        db.create_user(&user).await?;
        log::warn!("RESET_DB: seeded dev user 'admin' with password 'admin'");
        user
    };

    if db.get_namespace_by_name("admin").await?.is_none() {
        let namespace = create_namespace("admin".to_string(), user.id.clone());
        db.create_namespace(&namespace).await?;

        let namespace_path = std::path::Path::new(project_root).join(&namespace.name);
        if let Err(e) = std::fs::create_dir_all(&namespace_path) {
            log::warn!("RESET_DB: failed to create dev admin namespace dir: {e}");
        }
    }

    ctx.create_session(user.id).await
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
