use std::sync::atomic::{AtomicBool, Ordering};

use actix_web::HttpRequest;
use argon2::password_hash::{SaltString, rand_core::RngCore};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand::rngs::OsRng;
use uuid::Uuid;

use crate::db::Database;

pub mod handlers;

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
pub struct Ticket {
    pub id: String,
    pub user_id: Option<String>,
    pub used: bool,
    pub created_at: String,
    pub used_at: Option<String>,
}

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

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn create_user(username: String, email: String, password: String) -> Result<User, String> {
    let password_hash = hash_password(&password)?;
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
pub fn generate_ticket() -> String {
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

pub struct FigContext {
    db_path: String,
    api_key: String,
    initialized: AtomicBool,
}

impl FigContext {
    pub fn new(db_path: &str, api_key: String) -> Self {
        Self {
            db_path: db_path.to_string(),
            api_key,
            initialized: AtomicBool::new(false),
        }
    }

    pub async fn db(&self) -> Result<Database, String> {
        Database::new(&self.db_path).await
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    pub fn set_initialized(&self) {
        self.initialized.store(true, Ordering::SeqCst);
    }

    pub fn validate_api_key(&self, key: &str) -> bool {
        self.api_key == key
    }

    pub async fn create_session(&self, user_id: String) -> Result<String, String> {
        let db = self.db().await?;
        let token = generate_token();
        db.create_token(&token, &user_id).await?;
        Ok(token)
    }

    pub async fn validate_token(&self, token: &str) -> Option<String> {
        let db = self.db().await.ok()?;
        db.get_token_user(token).await.ok().flatten()
    }

    pub async fn invalidate_token(&self, token: &str) -> Result<(), String> {
        let db = self.db().await?;
        db.delete_token(token).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify_password() {
        let hash = hash_password("testpassword").unwrap();
        assert!(verify_password("testpassword", &hash).unwrap());
        assert!(!verify_password("wrongpassword", &hash).unwrap());
    }

    #[test]
    fn test_verify_password_invalid_hash() {
        assert!(verify_password("test", "not-a-hash").is_err());
    }

    #[test]
    fn test_generate_token_is_hex() {
        let token = generate_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_ticket_is_uuid() {
        let ticket = generate_ticket();
        assert!(uuid::Uuid::parse_str(&ticket).is_ok());
    }

    #[test]
    fn test_generate_tokens_are_unique() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
    }

    #[test]
    fn test_create_user_fields() {
        let user = create_user(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
        )
        .unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, Some("test@example.com".to_string()));
        assert!(!user.password_hash.is_empty());
        assert!(!user.id.is_empty());
    }

    #[test]
    fn test_create_user_password_is_hashed() {
        let user = create_user(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "password123".to_string(),
        )
        .unwrap();
        assert_ne!(user.password_hash, "password123");
        assert!(verify_password("password123", &user.password_hash).unwrap());
    }

    #[test]
    fn test_create_namespace_fields() {
        let ns = create_namespace("myns".to_string(), "user-1".to_string());
        assert_eq!(ns.name, "myns");
        assert_eq!(ns.owner_id, "user-1");
        assert!(!ns.id.is_empty());
    }

    #[test]
    fn test_base64_decode_valid() {
        let result = base64_decode("dXNlcjpwYXNz");
        assert_eq!(result, Some("user:pass".to_string()));
    }

    #[test]
    fn test_base64_decode_invalid() {
        let result = base64_decode("not-valid-base64!!!");
        assert!(result.is_none());
    }

    #[test]
    fn test_base64_decode_empty() {
        let result = base64_decode("");
        assert_eq!(result, Some("".to_string()));
    }

    #[test]
    fn test_fig_context_validate_api_key() {
        let ctx = FigContext::new("/tmp/test.db", "my-api-key".to_string());
        assert!(ctx.validate_api_key("my-api-key"));
        assert!(!ctx.validate_api_key("wrong-key"));
    }

    #[test]
    fn test_fig_context_initialized_flag() {
        let ctx = FigContext::new("/tmp/test.db", "key".to_string());
        assert!(!ctx.is_initialized());
        ctx.set_initialized();
        assert!(ctx.is_initialized());
    }
}
