use xshell::Shell;

#[derive(Debug)]
pub enum GitRequestKind {
    AdvertiseRefs(GitService), // initial handshake
    FetchClone,                // fetch / clone
    Push,                      // push
    DumbGet,                   // fallback static file
}

#[derive(Debug)]
pub enum GitService {
    ReadPack,
    WritePack,
}

pub struct GitRequest<'a> {
    method: &'a str,
    path_info: &'a str,
    query_string: &'a str,
    content_type: &'a str,
}

impl<'a> GitRequest<'a> {
    fn kind(&self) -> GitRequestKind {
        match (self.method, self.path_info) {
            // Smart HTTP handshake — GET /repo.git/info/refs?service=...
            ("GET", p) if p.ends_with("/info/refs") => match self.query_string {
                "service=git-upload-pack" => GitRequestKind::AdvertiseRefs(GitService::ReadPack),
                "service=git-receive-pack" => GitRequestKind::AdvertiseRefs(GitService::WritePack),
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
}

fn prepare_cgi_env(sh: Shell, req: GitRequest) -> (Shell, GitRequest) {
    sh.set_var("REQUEST_METHOD", req.method);
    sh.set_var("PATH_INFO", req.path_info);
    sh.set_var("QUERY_STRING", req.query_string);
    sh.set_var("GIT_PROJECT_ROOT", "/srv/git");
    sh.set_var("GIT_HTTP_EXPORT_ALL", "1");

    match req.kind() {
        GitRequestKind::Push => {
            // Pushes need write access — enforce auth here before proceeding
            sh.set_var("CONTENT_TYPE", req.content_type);
            sh.set_var("REMOTE_USER", "authenticated_user"); // must be set to allow push
        }
        GitRequestKind::FetchClone => {
            // needs read access — enforce auth here before proceeding
            sh.set_var("CONTENT_TYPE", req.content_type);
        }
        GitRequestKind::AdvertiseRefs(service) => {
            match service {
                
            }
            // No body, just env vars needed
        }
        GitRequestKind::DumbGet => {
            sh.set_var("GIT_HTTP_GET_ANY_FILE", "1"); // if you want to allow dumb access
        }
    }
    (sh, req)
}

fn sh() -> Shell {
    xshell::Shell::new().unwrap()
}
