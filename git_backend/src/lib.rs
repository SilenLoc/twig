use xshell::Shell;

#[derive(Debug)]
pub enum GitRequestKind {
    AdvertiseRefs(GitService),
    FetchClone,
    Push,
    DumbGet,
}

#[derive(Debug)]
pub enum GitService {
    ReadRef,
    WriteRef,
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

pub fn run_git_backend(req: &GitRequest, body: Vec<u8>) -> Result<(String, Vec<u8>), String> {
    let sh = sh();

    let (sh, req) = prepare_cgi_env(sh, req);

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
        return Ok(parsed);
    } else {
        return Err(output.unwrap_err().to_string());
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

pub fn prepare_cgi_env(sh: Shell, req: &GitRequest) -> (Shell, &GitRequest) {
    sh.set_var("REQUEST_METHOD", req.method.clone());
    sh.set_var("PATH_INFO", req.path_info.clone());
    sh.set_var("QUERY_STRING", req.query_string.clone());
    sh.set_var("GIT_PROJECT_ROOT", "/srv/git");
    sh.set_var("GIT_HTTP_EXPORT_ALL", "1");

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
    (sh, req)
}

fn sh() -> Shell {
    xshell::Shell::new().unwrap()
}
