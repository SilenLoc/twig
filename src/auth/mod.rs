use std::sync::atomic::{AtomicBool, Ordering};

use actix_identity::IdentityExt;
use actix_web::HttpRequest;
use argon2::password_hash::{SaltString, rand_core::RngCore};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use rand::rngs::OsRng;
use uuid::Uuid;

use crate::db::Database;

pub mod handlers;
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

pub struct FigContext {
    db: Database,
    api_key: String,
    initialized: AtomicBool,
}

impl FigContext {
    pub fn new(db: Database, api_key: String) -> Self {
        Self {
            db,
            api_key,
            initialized: AtomicBool::new(false),
        }
    }

    pub fn db(&self) -> &Database {
        &self.db
    }

    pub fn set_initialized(&self) {
        self.initialized.store(true, Ordering::SeqCst);
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
pub async fn seed_dev_admin(ctx: &FigContext, project_root: &str) -> Result<String, String> {
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
mod tests {
    use super::*;

    impl FigContext {
        // only for testing
        pub fn is_initialized(&self) -> bool {
            self.initialized.load(Ordering::SeqCst)
        }
    }

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
    fn test_generate_invite_is_uuid() {
        let invite = generate_invite();
        assert!(uuid::Uuid::parse_str(&invite).is_ok());
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
            "password123",
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
            "password123",
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
        assert_eq!(result, Some(String::new()));
    }

    #[tokio::test]
    async fn test_fig_context_validate_api_key() {
        let db = Database::new("/tmp/test_fig_ctx_validate.db");
        let ctx = FigContext::new(db, "my-api-key".to_string());
        assert!(ctx.validate_api_key("my-api-key"));
        assert!(!ctx.validate_api_key("wrong-key"));
    }

    #[tokio::test]
    async fn test_fig_context_test_pin() {
        let db_path = format!("/tmp/test_fig_ctx_pin_{}.db", Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("init tables");
        let ctx = FigContext::new(db, "key".to_string());
        assert_eq!(ctx.get_test_pin().await, None);
        assert!(!ctx.validate_test_pin("123456").await);

        ctx.set_test_pin("123456").await.expect("set test pin");
        assert_eq!(ctx.get_test_pin().await.as_deref(), Some("123456"));
        assert!(ctx.validate_test_pin("123456").await);
        assert!(!ctx.validate_test_pin("654321").await);
        assert!(!ctx.validate_test_pin("").await);

        ctx.clear_test_pin().await.expect("clear test pin");
        assert_eq!(ctx.get_test_pin().await, None);
        assert!(!ctx.validate_test_pin("123456").await);

        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_fig_context_initialized_flag() {
        let db = Database::new("/tmp/test_fig_ctx_flag.db");
        let ctx = FigContext::new(db, "key".to_string());
        assert!(!ctx.is_initialized());
        ctx.set_initialized();
        assert!(ctx.is_initialized());
    }

    #[tokio::test]
    async fn test_seed_dev_admin_creates_user_namespace_and_session() {
        let db_path = format!("/tmp/test_fig_seed_dev_admin_{}.db", Uuid::new_v4());
        let project_root = format!("/tmp/test_fig_seed_dev_admin_root_{}", Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("init tables");
        let ctx = FigContext::new(db.clone(), "key".to_string());

        let token = seed_dev_admin(&ctx, &project_root)
            .await
            .expect("seed dev admin");

        let user_id = ctx
            .validate_token(&token)
            .await
            .expect("session token should be valid");
        let user = db
            .get_user_by_id(&user_id)
            .await
            .expect("get user")
            .expect("user should exist");
        assert_eq!(user.username, "admin");
        assert!(verify_password("admin", &user.password_hash).unwrap());

        let namespace = db
            .get_namespace_by_name("admin")
            .await
            .expect("get namespace")
            .expect("namespace should exist");
        assert_eq!(namespace.owner_id, user_id);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&project_root);
    }

    #[tokio::test]
    async fn test_seed_dev_admin_is_idempotent() {
        let db_path = format!("/tmp/test_fig_seed_dev_admin_idem_{}.db", Uuid::new_v4());
        let project_root = format!("/tmp/test_fig_seed_dev_admin_idem_root_{}", Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("init tables");
        let ctx = FigContext::new(db.clone(), "key".to_string());

        let token1 = seed_dev_admin(&ctx, &project_root).await.expect("seed 1");
        let token2 = seed_dev_admin(&ctx, &project_root).await.expect("seed 2");

        // Both tokens should be valid sessions for the same, single admin user.
        let user_id1 = ctx.validate_token(&token1).await.expect("token1 valid");
        let user_id2 = ctx.validate_token(&token2).await.expect("token2 valid");
        assert_eq!(user_id1, user_id2);

        let all_namespaces = db
            .get_all_namespaces_with_owners()
            .await
            .expect("list namespaces");
        assert_eq!(all_namespaces.len(), 1);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&project_root);
    }
}
