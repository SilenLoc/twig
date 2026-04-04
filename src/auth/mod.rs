use std::sync::Arc;

use actix_web::HttpRequest;
use argon2::password_hash::{SaltString, rand_core::RngCore};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::Database;

pub mod handlers;

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
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
pub struct Ticket {
    pub id: String,
    pub user_id: Option<String>,
    pub used: bool,
    pub created_at: String,
    pub used_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthToken {
    pub token: String,
    pub user_id: String,
    pub username: String,
}

// Request/Response types
#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub ticket: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct SignupResponse {
    pub user_id: String,
    pub username: String,
    pub ticket: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateNamespaceRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CreateNamespaceResponse {
    pub namespace_id: String,
    pub name: String,
}

// Password hashing
pub fn hash_password(password: &str) -> Result<String, String> {
    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);

    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
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

// Generate random token
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

// Create new user
pub fn create_user(username: String, password: String) -> Result<(User, String), String> {
    let password_hash = hash_password(&password)?;
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let user = User {
        id,
        username,
        password_hash,
        created_at: now,
    };

    let ticket = create_ticket(&user.id);

    Ok((user, ticket))
}

// Create ticket for namespace creation
pub fn create_ticket(_user_id: &str) -> String {
    // Return just the UUID part for easier use
    Uuid::new_v4().to_string()
}

// Create namespace
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

// Extract basic auth credentials from request
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

// Extract bearer token from request
pub fn extract_bearer_token(req: &HttpRequest) -> Option<String> {
    let auth_header = req.headers().get("Authorization")?;
    let auth_str = auth_header.to_str().ok()?;

    if !auth_str.starts_with("Bearer ") {
        return None;
    }

    Some(auth_str[7..].to_string())
}

// Extract API key from request
pub fn extract_api_key(req: &HttpRequest) -> Option<String> {
    // Check X-API-Key header first
    if let Some(key) = req.headers().get("X-API-Key")
        && let Ok(key_str) = key.to_str()
    {
        return Some(key_str.to_string());
    }

    // Fallback to query parameter
    if let Some(query) = req.uri().query() {
        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=')
                && key == "api_key"
            {
                return Some(value.to_string());
            }
        }
    }

    None
}

fn base64_decode(input: &str) -> Option<String> {
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(input)
        .ok()?;
    String::from_utf8(decoded).ok()
}

pub struct AuthState {
    pub db: Arc<Database>,
    pub api_key: String,
}

impl AuthState {
    pub async fn new(db_path: &str, api_key: String) -> Result<Self, String> {
        let db = Arc::new(Database::new(db_path).await?);

        Ok(Self { db, api_key })
    }

    pub fn validate_api_key(&self, key: &str) -> bool {
        self.api_key == key
    }

    pub async fn create_session(&self, user_id: String) -> Result<String, String> {
        let token = generate_token();
        self.db.create_token(&token, &user_id).await?;
        Ok(token)
    }

    pub async fn validate_token(&self, token: &str) -> Option<String> {
        self.db.get_token_user(token).await.ok().flatten()
    }

    pub async fn invalidate_token(&self, token: &str) -> Result<(), String> {
        self.db.delete_token(token).await
    }
}
