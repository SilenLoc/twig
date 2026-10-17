use actix_identity::IdentityExt;
use actix_web::HttpRequest;
use argon2::password_hash::phc::PasswordHash;
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use rand::TryRng;
use rand::rngs::SysRng;
use uuid::Uuid;

use crate::db::Database;

pub mod session_store;

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

    if !auth_str.starts_with("Basic ") {
        return None;
    }

    let encoded = &auth_str[6..];
    let decoded = base64_decode(encoded)?;

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
