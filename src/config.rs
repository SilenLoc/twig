#[derive(Clone)]
pub struct Server {
    address: (String, u16),
    log_level: String,
    project_root: String,
    db_path: String,
    api_key: String,
    reset_db: bool,
    traces_sample_rate: f32,
}

impl Server {
    pub fn new(
        address: (String, u16),
        log_level: String,
        project_root: String,
        db_path: String,
        api_key: String,
        reset_db: bool,
        traces_sample_rate: f32,
    ) -> Self {
        Server {
            address,
            log_level,
            project_root,
            db_path,
            api_key,
            reset_db,
            traces_sample_rate,
        }
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

    /// Deletes the database file if RESET_DB is set to true.
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

    pub fn traces_sample_rate(&self) -> f32 {
        self.traces_sample_rate
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
    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "fig.db".to_string());
    let api_key = std::env::var("API_KEY").unwrap_or_default();
    let reset_db = std::env::var("RESET_DB").unwrap_or_default() == "true";
    let traces_sample_rate = std::env::var("SENTRY_TRACES_SAMPLE_RATE")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0);

    Server::new(
        ("0.0.0.0".to_string(), port),
        log_level,
        project_root,
        db_path,
        api_key,
        reset_db,
        traces_sample_rate,
    )
}

fn ascii(server: &Server) -> String {
    let (_, port) = server.address();

    let url = format!("http://localhost:{port}");
    let version = env!("CARGO_PKG_VERSION");
    format!(
        "

        ▐▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▌
        ▐  ███████╗██╗ ██████╗   ▌
        ▐  ██╔════╝██║██╔════╝   ▌
        ▐  █████╗  ██║██║  ███╗  ▌
        ▐  ██╔══╝  ██║██║   ██║  ▌
        ▐  ██║     ██║╚██████╔╝  ▌
        ▐  ╚═╝     ╚═╝ ╚═════╝   ▌
        ▐▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▌

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
            "fig.db".to_string(),
            "mykey".to_string(),
            false,
            1.0,
        );
        assert_eq!(server.address(), ("127.0.0.1".to_string(), 8080));
        assert_eq!(server.log_level(), "debug");
        assert_eq!(server.project_root(), "/srv/git");
        assert_eq!(server.db_path(), "fig.db");
    }

    #[test]
    fn test_server_clone() {
        let server = Server::new(
            ("0.0.0.0".to_string(), 80),
            "info".to_string(),
            "/srv/git".to_string(),
            "fig.db".to_string(),
            "apikey".to_string(),
            false,
            1.0,
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
            "fig.db".to_string(),
            "my-secret-key".to_string(),
            false,
            1.0,
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
            1.0,
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
            "/tmp/fig_test_no_reset.db".to_string(),
            "key".to_string(),
            false,
            1.0,
        );
        server.maybe_reset_database();
    }

    #[test]
    fn test_address_returns_clone() {
        let server = Server::new(
            ("0.0.0.0".to_string(), 3000),
            "info".to_string(),
            "/srv/git".to_string(),
            "fig.db".to_string(),
            "key".to_string(),
            false,
            1.0,
        );
        let addr = server.address();
        assert_eq!(addr, ("0.0.0.0".to_string(), 3000));
    }
}
