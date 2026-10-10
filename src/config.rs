use actix_web::http::header::HeaderValue;

pub const DEFAULT_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
pub const DEFAULT_RESEND_FROM: &str = "onboarding@resend.dev";

#[derive(Clone)]
pub struct Server {
    address: (String, u16),
    log_level: String,
    project_root: String,
    db_path: String,
    api_key: String,
    reset_db: bool,
    test_user: Option<String>,
    admin_user: Option<String>,
    resend_api_key: Option<String>,
    resend_from: String,
    public_base_url: Option<String>,
    cache_control: HeaderValue,
    session_key: actix_web::cookie::Key,
}

impl Server {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        address: (String, u16),
        log_level: String,
        project_root: String,
        db_path: String,
        api_key: String,
        reset_db: bool,
    ) -> Self {
        Server {
            address,
            log_level,
            project_root,
            db_path,
            api_key,
            reset_db,
            test_user: None,
            admin_user: None,
            resend_api_key: None,
            resend_from: DEFAULT_RESEND_FROM.to_string(),
            public_base_url: None,
            cache_control: HeaderValue::from_static(DEFAULT_CACHE_CONTROL),
            session_key: actix_web::cookie::Key::generate(),
        }
    }

    pub fn with_cache_control(mut self, cache_control: HeaderValue) -> Self {
        self.cache_control = cache_control;
        self
    }

    pub fn with_test_user(mut self, test_user: Option<String>) -> Self {
        self.test_user = test_user;
        self
    }

    pub fn with_admin_user(mut self, admin_user: Option<String>) -> Self {
        self.admin_user = admin_user;
        self
    }

    pub fn with_resend_settings(
        mut self,
        api_key: Option<String>,
        from: String,
        public_base_url: Option<String>,
    ) -> Self {
        self.resend_api_key = api_key.filter(|key| !key.trim().is_empty());
        self.resend_from = from;
        self.public_base_url = public_base_url;
        self
    }

    pub fn resend_api_key(&self) -> Option<&str> {
        self.resend_api_key.as_deref()
    }

    pub fn resend_from(&self) -> &str {
        &self.resend_from
    }

    pub fn public_base_url(&self) -> Option<&str> {
        self.public_base_url.as_deref()
    }

    pub fn admin_user(&self) -> Option<&str> {
        self.admin_user.as_deref()
    }

    pub fn is_configured_admin(&self, username: &str) -> bool {
        self.admin_user.as_deref() == Some(username)
    }

    pub fn address(&self) -> (String, u16) {
        self.address.clone()
    }

    pub fn log_level(&self) -> &str {
        &self.log_level
    }

    pub fn project_root(&self) -> &str {
        &self.project_root
    }

    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    /// Returns whether `RESET_DB` is set to true.
    pub fn reset_db(&self) -> bool {
        self.reset_db
    }

    /// Returns the configured test admin user, if set.
    pub fn test_user(&self) -> Option<&str> {
        self.test_user.as_deref()
    }

    /// Returns whether a test user is configured.
    pub fn is_test_user_enabled(&self) -> bool {
        self.test_user.is_some()
    }

    /// Returns whether the provided username matches the configured test-page user.
    pub fn is_test_user(&self, username: &str) -> bool {
        self.test_user.as_deref() == Some(username)
    }

    /// Deletes the database file if `RESET_DB` is set to true.
    /// Logs warnings and results appropriately.
    pub fn maybe_reset_database(&self) {
        if !self.reset_db {
            return;
        }

        log::warn!(
            "RESET_DB is set to true, deleting database file: {}",
            self.db_path
        );

        if std::path::Path::new(&self.db_path).exists() {
            if let Err(e) = std::fs::remove_file(&self.db_path) {
                log::error!("Failed to delete database file: {e}");
            } else {
                log::info!("Database file deleted successfully");
            }
        }
    }

    pub fn cache_control(&self) -> &HeaderValue {
        &self.cache_control
    }

    pub fn session_key(&self) -> actix_web::cookie::Key {
        self.session_key.clone()
    }

    fn with_session_key(mut self, session_key: actix_web::cookie::Key) -> Self {
        self.session_key = session_key;
        self
    }

    /// Returns the API key, generating a random one if not provided.
    /// Logs a warning when generating a random key.
    pub fn effective_api_key(&self) -> String {
        if self.api_key.is_empty() {
            let key = crate::auth::generate_token();
            log::warn!("No API_KEY set, using generated key: {key}");
            key
        } else {
            self.api_key.clone()
        }
    }
}

impl std::fmt::Display for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", ascii(self))
    }
}

pub fn from_env() -> Server {
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "80".to_string())
        .parse()
        .unwrap_or(80);
    let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let project_root = std::env::var("PROJECT_ROOT").unwrap_or_else(|_| "/srv/git".to_string());
    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "twig.db".to_string());
    let api_key = std::env::var("API_KEY").unwrap_or_default();
    let session_key = std::env::var("SESSION_KEY")
        .ok()
        .filter(|key| !key.is_empty())
        .map_or_else(
            || {
                log::warn!("SESSION_KEY is unset; identity sessions will not survive a restart");
                actix_web::cookie::Key::generate()
            },
            |key| actix_web::cookie::Key::derive_from(key.as_bytes()),
        );
    let reset_db = std::env::var("RESET_DB").unwrap_or_default() == "true";
    let test_user = std::env::var("TEST_USER")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| if s == "true" { "admin".to_string() } else { s });
    let admin_user = std::env::var("ADMIN_USER")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let resend_api_key = std::env::var("RESEND_API_KEY")
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty());
    let resend_from = std::env::var("RESEND_FROM")
        .ok()
        .map(|from| from.trim().to_string())
        .filter(|from| !from.is_empty())
        .unwrap_or_else(|| DEFAULT_RESEND_FROM.to_string());
    let public_base_url = std::env::var("PUBLIC_BASE_URL")
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty());
    let cache_control = std::env::var("CACHE_CONTROL")
        .unwrap_or_else(|_| DEFAULT_CACHE_CONTROL.to_string())
        .parse::<HeaderValue>()
        .unwrap_or_else(|error| {
            log::error!("Invalid CACHE_CONTROL header value: {error}; using default");
            HeaderValue::from_static(DEFAULT_CACHE_CONTROL)
        });

    Server::new(
        ("0.0.0.0".to_string(), port),
        log_level,
        project_root,
        db_path,
        api_key,
        reset_db,
    )
    .with_cache_control(cache_control)
    .with_session_key(session_key)
    .with_test_user(test_user)
    .with_admin_user(admin_user)
    .with_resend_settings(resend_api_key, resend_from, public_base_url)
}

fn ascii(server: &Server) -> String {
    let (_, port) = server.address();

    let url = format!("http://localhost:{port}");
    let version = env!("CARGO_PKG_VERSION");
    format!(
        "

        ▐▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▌
        ▐  ████████╗██╗    ██╗██╗ ██████╗   ▌
        ▐  ╚══██╔══╝██║    ██║██║██╔════╝   ▌
        ▐     ██║   ██║ █╗ ██║██║██║  ███╗  ▌
        ▐     ██║   ██║███╗██║██║██║   ██║  ▌
        ▐     ██║   ╚███╔███╔╝██║╚██████╔╝  ▌
        ▐     ╚═╝    ╚══╝╚══╝ ╚═╝ ╚═════╝   ▌
        ▐▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▌

        Server running at: {url}
        Version: {version}
        "
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_new() {
        let server = Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "mykey".to_string(),
            false,
        );
        assert_eq!(server.address(), ("127.0.0.1".to_string(), 8080));
        assert_eq!(server.log_level(), "debug");
        assert_eq!(server.project_root(), "/srv/git");
        assert_eq!(server.db_path(), "twig.db");
        assert_eq!(
            server.cache_control(),
            &HeaderValue::from_static(DEFAULT_CACHE_CONTROL)
        );
    }

    #[test]
    fn test_server_cache_control_round_trip() {
        let server = Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "mykey".to_string(),
            false,
        )
        .with_cache_control(HeaderValue::from_static("no-cache"));

        assert_eq!(
            server.cache_control(),
            &HeaderValue::from_static("no-cache")
        );
    }

    #[test]
    fn test_server_clone() {
        let server = Server::new(
            ("0.0.0.0".to_string(), 80),
            "info".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "apikey".to_string(),
            false,
        );
        let cloned = server.clone();
        assert_eq!(cloned.project_root(), server.project_root());
        assert_eq!(cloned.log_level(), server.log_level());
    }

    #[test]
    fn test_effective_api_key_with_set_key() {
        let server = Server::new(
            ("0.0.0.0".to_string(), 80),
            "info".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "my-secret-key".to_string(),
            false,
        );
        assert_eq!(server.effective_api_key(), "my-secret-key");
    }

    #[test]
    fn test_effective_api_key_generates_when_empty() {
        let server = Server::new(
            ("0.0.0.0".to_string(), 80),
            "info".to_string(),
            "/srv/git".to_string(),
            String::new(),
            String::new(),
            false,
        );
        let key = server.effective_api_key();
        assert!(!key.is_empty());
        assert_eq!(key.len(), 64);
    }

    #[test]
    fn test_maybe_reset_database_no_flag() {
        let server = Server::new(
            ("0.0.0.0".to_string(), 80),
            "info".to_string(),
            "/srv/git".to_string(),
            "/tmp/twig_test_no_reset.db".to_string(),
            "key".to_string(),
            false,
        );
        server.maybe_reset_database();
    }

    #[test]
    fn test_address_returns_clone() {
        let server = Server::new(
            ("0.0.0.0".to_string(), 3000),
            "info".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "key".to_string(),
            false,
        );
        let addr = server.address();
        assert_eq!(addr, ("0.0.0.0".to_string(), 3000));
    }

    #[test]
    fn test_server_test_user() {
        let server = Server::new(
            ("0.0.0.0".to_string(), 8080),
            "info".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "key".to_string(),
            false,
        );
        assert_eq!(server.test_user(), None);
        assert!(!server.is_test_user_enabled());
        assert!(!server.is_test_user("admin"));

        let server_with_admin = server.with_test_user(Some("admin".to_string()));
        assert_eq!(server_with_admin.test_user(), Some("admin"));
        assert!(server_with_admin.is_test_user_enabled());
        assert!(server_with_admin.is_test_user("admin"));
        assert!(!server_with_admin.is_test_user("alice"));
    }

    #[test]
    fn test_server_admin_user_is_independent_from_test_user() {
        let server = Server::new(
            ("0.0.0.0".to_string(), 8080),
            "info".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "key".to_string(),
            false,
        )
        .with_admin_user(Some("root-user".to_string()));

        assert_eq!(server.admin_user(), Some("root-user"));
        assert!(server.is_configured_admin("root-user"));
        assert!(!server.is_configured_admin("admin"));
        assert!(!server.is_test_user_enabled());
    }

    #[test]
    fn test_blank_admin_user_is_not_configured() {
        let server = Server::new(
            ("0.0.0.0".to_string(), 8080),
            "info".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "key".to_string(),
            false,
        )
        .with_admin_user(None);

        assert_eq!(server.admin_user(), None);
    }

    #[test]
    fn test_resend_settings_are_optional_and_keep_sender_configuration() {
        let server = Server::new(
            ("0.0.0.0".to_string(), 8080),
            "info".to_string(),
            "/srv/git".to_string(),
            "twig.db".to_string(),
            "key".to_string(),
            false,
        );
        assert_eq!(server.resend_api_key(), None);
        assert_eq!(server.resend_from(), DEFAULT_RESEND_FROM);
        assert_eq!(server.public_base_url(), None);

        let configured = server.with_resend_settings(
            Some("re_example_secret".to_string()),
            "Twig <invites@example.com>".to_string(),
            Some("https://twig.example".to_string()),
        );
        assert_eq!(configured.resend_api_key(), Some("re_example_secret"));
        assert_eq!(configured.resend_from(), "Twig <invites@example.com>");
        assert_eq!(configured.public_base_url(), Some("https://twig.example"));

        let blank = configured.with_resend_settings(
            Some("  ".to_string()),
            DEFAULT_RESEND_FROM.to_string(),
            None,
        );
        assert_eq!(blank.resend_api_key(), None);
    }
}
