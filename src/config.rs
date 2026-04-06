pub struct Server {
    address: (String, u16),
    log_level: String,
    project_root: String,
    db_path: String,
    api_key: String,
    reset_db: bool,
}

impl Server {
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

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn reset_db(&self) -> bool {
        self.reset_db
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

    Server::new(
        ("0.0.0.0".to_string(), port),
        log_level,
        project_root,
        db_path,
        api_key,
        reset_db,
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
        }
    }
}
