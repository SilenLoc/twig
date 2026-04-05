pub struct Server {
    address: (String, u16),
    log_level: String,
    project_root: String,
}

impl Server {
    pub fn new(address: (String, u16), log_level: String, project_root: String) -> Self {
        Server {
            address,
            log_level,
            project_root,
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

    Server::new(("0.0.0.0".to_string(), port), log_level, project_root)
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
