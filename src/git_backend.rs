use std::fmt::Display;

use log::debug;
use xshell::Shell;

#[derive(Default)]
pub struct Config {
    pub project_root: Option<String>,
}

impl Config {
    pub fn new(project_root: impl Into<String>) -> Self {
        Self {
            project_root: Some(project_root.into()),
        }
    }

    pub fn project_root(&self) -> String {
        self.project_root.clone().unwrap_or("/srv/git".to_string())
    }
}

#[derive(Debug, Clone)]
pub enum GitRequestKind {
    AdvertiseRefs(GitService),
    FetchClone,
    Push,
    DumbGet,
}

#[derive(Debug, Clone)]
pub enum GitService {
    ReadRef,
    WriteRef,
}

impl Display for GitService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
pub struct GitRequest {
    method: String,
    path_info: String,
    query_string: String,
    content_type: String,
}

impl GitRequest {
    pub fn kind(&self) -> GitRequestKind {
        match (self.method.as_str(), self.path_info.as_str()) {
            // Smart HTTP handshake — GET /repo.git/info/refs?service=...
            ("GET", p) if p.ends_with("/info/refs") => match self.query_string.as_str() {
                "service=git-upload-pack" => GitRequestKind::AdvertiseRefs(GitService::ReadRef),
                "service=git-receive-pack" => GitRequestKind::AdvertiseRefs(GitService::WriteRef),
                _ => GitRequestKind::DumbGet,
            },

            // Pull / clone data transfer
            ("POST", p) if p.ends_with("/git-upload-pack") => GitRequestKind::FetchClone,

            // Push data transfer
            ("POST", p) if p.ends_with("/git-receive-pack") => GitRequestKind::Push,

            // Dumb HTTP fallback (static objects, HEAD, etc.)
            ("GET", _) | ("HEAD", _) => GitRequestKind::DumbGet,

            _ => GitRequestKind::DumbGet, // or return a 405
        }
    }

    pub fn new(
        method: impl Into<String>,
        path_info: impl Into<String>,
        query_string: impl Into<String>,
        content_type: impl Into<String>,
    ) -> Self {
        Self {
            method: method.into(),
            path_info: path_info.into(),
            query_string: query_string.into(),
            content_type: content_type.into(),
        }
    }
}

pub fn run_with_config(
    config: &Config,
    namespace: &str,
    req: &GitRequest,
    body: Vec<u8>,
) -> Result<(String, Vec<u8>), String> {
    let sh = sh();

    // todo make this configurable
    let actual_root = format!("{}/{}", config.project_root(), namespace);
    let sh = prepare_cgi_env(&actual_root, sh, req.clone());

    if !req.content_type.is_empty() {
        sh.set_var("CONTENT_TYPE", req.content_type.clone());
    }
    if !body.is_empty() {
        sh.set_var("CONTENT_LENGTH", body.len().to_string());
    }

    // Run with or without stdin body
    let output = if body.is_empty() {
        xshell::cmd!(sh, "git http-backend").output()
    } else {
        xshell::cmd!(sh, "git http-backend").stdin(body).output()
    };

    if let Ok(out) = output {
        let stdout = out.stdout;
        let parsed = parse_cgi_response(&stdout);
        Ok(parsed)
    } else {
        Err(output.unwrap_err().to_string())
    }
}

fn parse_cgi_response(output: &[u8]) -> (String, Vec<u8>) {
    // Find the header/body separator (\r\n\r\n or \n\n)
    let separator = b"\r\n\r\n";
    if let Some(pos) = output.windows(4).position(|w| w == separator) {
        let headers = String::from_utf8_lossy(&output[..pos]).to_string();
        let body = output[pos + 4..].to_vec();
        (headers, body)
    } else if let Some(pos) = output.windows(2).position(|w| w == b"\n\n") {
        let headers = String::from_utf8_lossy(&output[..pos]).to_string();
        let body = output[pos + 2..].to_vec();
        (headers, body)
    } else {
        (String::new(), output.to_vec())
    }
}

pub fn prepare_cgi_env(project_root: &str, sh: Shell, req: GitRequest) -> Shell {
    sh.set_var("REQUEST_METHOD", req.method.clone());
    sh.set_var("PATH_INFO", req.path_info.clone());
    sh.set_var("QUERY_STRING", req.query_string.clone());
    sh.set_var("GIT_PROJECT_ROOT", project_root);
    sh.set_var("GIT_HTTP_EXPORT_ALL", "1");

    debug!("{req:?}");

    match req.kind() {
        GitRequestKind::Push => {
            // Pushes need write access — enforce auth here before proceeding
            sh.set_var("CONTENT_TYPE", req.content_type.clone());
            sh.set_var("REMOTE_USER", "authenticated_user"); // must be set to allow push
        }
        GitRequestKind::FetchClone => {
            // needs read access — enforce auth here before proceeding
            sh.set_var("CONTENT_TYPE", req.content_type.clone());
        }
        GitRequestKind::AdvertiseRefs(service) => {
            match service {
                GitService::ReadRef => {
                    // needs read access — enforce auth here before proceeding
                }

                GitService::WriteRef => {
                    // needs write access — enforce auth here before proceeding
                }
            }
            // No body, just env vars needed
        }
        GitRequestKind::DumbGet => {
            sh.set_var("GIT_HTTP_GET_ANY_FILE", "1");
        }
    }
    sh
}

fn sh() -> Shell {
    xshell::Shell::new().unwrap()
}
