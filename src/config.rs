pub struct Server {
    address: (String, u16),
    log_level: String,
    project_root: String,
    db_path: String,
    api_key: String,
    reset_db: bool,
    migrate: bool,
}

impl Server {
    pub fn new(
        address: (String, u16),
        log_level: String,
        project_root: String,
        db_path: String,
        api_key: String,
        reset_db: bool,
        migrate: bool,
    ) -> Self {
        Server {
            address,
            log_level,
            project_root,
            db_path,
            api_key,
            reset_db,
            migrate,
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

    #[allow(unused)]
    pub fn migrate(&self) -> bool {
        self.migrate
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
                log::error!("Failed to delete database file: {}", e);
            } else {
                log::info!("Database file deleted successfully");
            }
        }
    }

    /// Returns the API key, generating a random one if not provided.
    /// Logs a warning when generating a random key.
    pub fn effective_api_key(&self) -> String {
        if self.api_key.is_empty() {
            let key = crate::auth::generate_token();
            log::warn!("No API_KEY set, using generated key: {}", key);
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
    let migrate = std::env::var("MIGRATE").unwrap_or_default() == "true";

    Server::new(
        ("0.0.0.0".to_string(), port),
        log_level,
        project_root,
        db_path,
        api_key,
        reset_db,
        migrate,
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

impl Clone for Server {
    fn clone(&self) -> Self {
        Self {
            address: self.address.clone(),
            log_level: self.log_level.clone(),
            project_root: self.project_root.clone(),
            db_path: self.db_path.clone(),
            api_key: self.api_key.clone(),
            reset_db: self.reset_db,
            migrate: self.migrate,
        }
    }
}
