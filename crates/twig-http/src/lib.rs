//! Framework-independent Git smart/dumb HTTP handling for Twig and other Rust
//! applications. Call [`classify_request`] before authorization, then pass the
//! normalized request and authorization context to [`handle`].
//!
//! Smart upload-pack and receive-pack handling is implemented in-process with
//! gitoxide; receive hooks are Rust callbacks rather than child processes.

use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

mod receive_pack;
mod upload_pack;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    UploadPack,
    ReceivePack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    AdvertiseRefs(Service),
    UploadPack,
    ReceivePack,
    Dumb,
}

pub struct Config {
    pub project_root: PathBuf,
}

impl Config {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }
}

pub struct Request<'a> {
    pub method: &'a str,
    pub path_info: &'a str,
    pub query_string: &'a str,
    pub content_type: Option<&'a str>,
    pub body: &'a [u8],
    pub authenticated_user: Option<&'a str>,
    pub may_rewrite_protected_branches: bool,
    pub receive_hooks: &'a [Arc<dyn ReceiveHook>],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefUpdate {
    pub refname: String,
    pub old_oid: Option<String>,
    pub new_oid: Option<String>,
}

pub struct HookContext<'a> {
    pub authenticated_user: Option<&'a str>,
    pub may_rewrite_protected_branches: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookError {
    pub message: String,
}

impl HookError {
    pub fn reject(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait ReceiveHook: Send + Sync {
    fn pre_receive(
        &self,
        _repository: &Path,
        _updates: &[RefUpdate],
        _context: &HookContext<'_>,
    ) -> Result<(), HookError> {
        Ok(())
    }

    fn update(
        &self,
        _repository: &Path,
        _update: &RefUpdate,
        _context: &HookContext<'_>,
    ) -> Result<(), HookError> {
        Ok(())
    }

    fn post_receive(&self, _repository: &Path, _updates: &[RefUpdate], _context: &HookContext<'_>) {
    }

    fn post_update(&self, _repository: &Path, _refnames: &[String], _context: &HookContext<'_>) {}
}

pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub enum Error {
    UnsupportedRequest,
    Repository(String),
    Protocol(String),
    Io(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedRequest => write!(formatter, "unsupported Git HTTP request"),
            Self::Repository(error) => write!(formatter, "Git repository access failed: {error}"),
            Self::Protocol(error) => write!(formatter, "Git protocol error: {error}"),
            Self::Io(error) => write!(formatter, "Git HTTP file read failed: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UnsupportedRequest | Self::Repository(_) | Self::Protocol(_) => None,
            Self::Io(error) => Some(error),
        }
    }
}

pub fn classify_request(method: &str, path_info: &str, query_string: &str) -> RequestKind {
    match (method, path_info) {
        ("GET", path) if path.ends_with("/info/refs") => match query_string {
            "service=git-upload-pack" => RequestKind::AdvertiseRefs(Service::UploadPack),
            "service=git-receive-pack" => RequestKind::AdvertiseRefs(Service::ReceivePack),
            _ => RequestKind::Dumb,
        },
        ("POST", path) if path.ends_with("/git-upload-pack") => RequestKind::UploadPack,
        ("POST", path) if path.ends_with("/git-receive-pack") => RequestKind::ReceivePack,
        _ => RequestKind::Dumb,
    }
}

/// Handles Git smart HTTP requests and dumb/static repository reads.
pub fn handle(config: &Config, namespace: &str, request: Request<'_>) -> Result<Response, Error> {
    if request
        .path_info
        .split('/')
        .any(|component| component == "..")
    {
        return Ok(not_found());
    }

    match classify_request(request.method, request.path_info, request.query_string) {
        RequestKind::AdvertiseRefs(Service::UploadPack) => {
            if !path_has_suffix(request.path_info, &["info", "refs"]) {
                return Err(Error::UnsupportedRequest);
            }
            let Some(repo_path) = resolve_repo(config, namespace, request.path_info)? else {
                return Ok(not_found());
            };
            let body = advertise_upload_pack(&repo_path)?;
            Ok(git_response(
                "application/x-git-upload-pack-advertisement",
                body,
            ))
        }
        RequestKind::UploadPack => {
            if !path_has_suffix(request.path_info, &["git-upload-pack"]) {
                return Ok(not_found());
            }
            if request.content_type != Some("application/x-git-upload-pack-request") {
                return Ok(unsupported_media_type());
            }
            let Some(repo_path) = resolve_repo(config, namespace, request.path_info)? else {
                return Ok(not_found());
            };
            let body = upload_pack::handle(&repo_path, request.body)?;
            Ok(git_response("application/x-git-upload-pack-result", body))
        }
        RequestKind::AdvertiseRefs(Service::ReceivePack) => {
            if !path_has_suffix(request.path_info, &["info", "refs"]) {
                return Ok(not_found());
            }
            let Some(repo_path) = resolve_repo(config, namespace, request.path_info)? else {
                return Ok(not_found());
            };
            let body = advertise_receive_pack(&repo_path)?;
            Ok(git_response(
                "application/x-git-receive-pack-advertisement",
                body,
            ))
        }
        RequestKind::ReceivePack => {
            if !path_has_suffix(request.path_info, &["git-receive-pack"]) {
                return Ok(not_found());
            }
            if request.content_type != Some("application/x-git-receive-pack-request") {
                return Ok(unsupported_media_type());
            }
            let Some(repo_path) = resolve_repo(config, namespace, request.path_info)? else {
                return Ok(not_found());
            };
            let body = receive_pack::handle(&repo_path, &request)?;
            Ok(git_response("application/x-git-receive-pack-result", body))
        }
        RequestKind::Dumb if request.method == "GET" => {
            serve_repository_file(config, namespace, request.path_info)
        }
        RequestKind::Dumb => Ok(not_found()),
    }
}

fn path_has_suffix(path_info: &str, suffix: &[&str]) -> bool {
    let Some(relative_path) = path_info.strip_prefix('/') else {
        return false;
    };
    let components: Vec<_> = relative_path.split('/').collect();
    components.len() == suffix.len() + 1
        && components[1..] == *suffix
        && components
            .iter()
            .all(|component| is_safe_component(component))
}

fn advertise_upload_pack(repo_path: &Path) -> Result<Vec<u8>, Error> {
    advertise_refs(
        repo_path,
        "git-upload-pack",
        "side-band-64k no-progress object-format=sha1 agent=twig-http/0.1",
    )
}

fn advertise_receive_pack(repo_path: &Path) -> Result<Vec<u8>, Error> {
    advertise_refs(
        repo_path,
        "git-receive-pack",
        "report-status side-band-64k delete-refs object-format=sha1 agent=twig-http/0.1",
    )
}

fn advertise_refs(repo_path: &Path, service: &str, capabilities: &str) -> Result<Vec<u8>, Error> {
    const ZERO_ID: &str = "0000000000000000000000000000000000000000";
    let repo = gix::open(repo_path).map_err(|error| Error::Repository(error.to_string()))?;
    let head_name = repo
        .head_name()
        .ok()
        .flatten()
        .map(|name| name.as_bstr().to_string());
    let head_id = repo.head_id().ok().map(|id| id.to_string());

    let mut refs = Vec::new();
    if let Some(id) = head_id {
        refs.push(("HEAD".to_string(), id));
    }
    let references = repo
        .references()
        .map_err(|error| Error::Repository(error.to_string()))?;
    let all_references = references
        .all()
        .map_err(|error| Error::Repository(error.to_string()))?;
    for reference in all_references {
        let reference = reference.map_err(|error| Error::Repository(error.to_string()))?;
        let name = reference.name().as_bstr().to_string();
        if name == "HEAD" {
            continue;
        }
        if let Some(id) = reference.try_id() {
            refs.push((name, id.detach().to_string()));
        }
    }
    refs.sort_by(|left, right| left.0.cmp(&right.0));
    refs.dedup_by(|left, right| left.0 == right.0);

    let mut response = service_preamble(service);
    if refs.is_empty() {
        let line = format!("{ZERO_ID} capabilities^{{}}\0{capabilities}\n");
        response.extend(encode_pkt_line(line.as_bytes())?);
    } else {
        let mut capabilities = capabilities.to_string();
        if let Some(head_name) = head_name {
            capabilities.push_str(&format!(" symref=HEAD:{head_name}"));
        }
        for (index, (name, id)) in refs.iter().enumerate() {
            let line = if index == 0 {
                format!("{id} {name}\0{capabilities}\n")
            } else {
                format!("{id} {name}\n")
            };
            response.extend(encode_pkt_line(line.as_bytes())?);
        }
    }
    response.extend_from_slice(b"0000");
    Ok(response)
}

fn encode_pkt_line(payload: &[u8]) -> Result<Vec<u8>, Error> {
    let length = payload.len() + 4;
    if length > 0xffff {
        return Err(Error::Protocol(
            "pkt-line payload exceeds maximum size".to_string(),
        ));
    }
    let mut packet = format!("{length:04x}").into_bytes();
    packet.extend_from_slice(payload);
    Ok(packet)
}

fn resolve_repo(
    config: &Config,
    namespace: &str,
    path_info: &str,
) -> Result<Option<PathBuf>, Error> {
    let Some(relative_path) = path_info.strip_prefix('/') else {
        return Ok(None);
    };
    let Some(repo) = relative_path.split('/').next() else {
        return Ok(None);
    };
    if !is_safe_component(namespace) || !is_safe_component(repo) {
        return Ok(None);
    }

    let root = match canonicalize(&config.project_root) {
        Ok(path) => path,
        Err(None) => return Ok(None),
        Err(Some(error)) => return Err(Error::Io(error)),
    };
    let namespace_path = match canonicalize(&root.join(namespace)) {
        Ok(path) if path.starts_with(&root) => path,
        Ok(_) | Err(None) => return Ok(None),
        Err(Some(error)) => return Err(Error::Io(error)),
    };
    let repo_path = match canonicalize(&namespace_path.join(repo)) {
        Ok(path) if path.starts_with(&namespace_path) => path,
        Ok(_) | Err(None) => return Ok(None),
        Err(Some(error)) => return Err(Error::Io(error)),
    };
    Ok(Some(repo_path))
}

fn serve_repository_file(
    config: &Config,
    namespace: &str,
    path_info: &str,
) -> Result<Response, Error> {
    let Some(relative_path) = path_info.strip_prefix('/') else {
        return Ok(not_found());
    };
    let components: Vec<_> = relative_path.split('/').collect();
    if components.len() < 2
        || !components
            .iter()
            .all(|component| is_safe_component(component))
    {
        return Ok(not_found());
    }

    let Some(repo_path) = resolve_repo(config, namespace, path_info)? else {
        return Ok(not_found());
    };
    let mut file_path = repo_path.clone();
    for component in &components[1..] {
        file_path.push(component);
    }
    let file_path = match canonicalize(&file_path) {
        Ok(path) if path.starts_with(&repo_path) && path.is_file() => path,
        Ok(_) | Err(None) => return Ok(not_found()),
        Err(Some(error)) => return Err(Error::Io(error)),
    };
    let metadata = std::fs::metadata(&file_path).map_err(Error::Io)?;
    let body = std::fs::read(&file_path).map_err(Error::Io)?;
    let relative_file = components[1..].join("/");
    let mut response = git_response(content_type_for(&relative_file), body);
    response.headers.push((
        "Expires".to_string(),
        "Fri, 01 Jan 1980 00:00:00 GMT".to_string(),
    ));
    response
        .headers
        .push(("Pragma".to_string(), "no-cache".to_string()));
    if let Ok(modified) = metadata.modified() {
        response.headers.push((
            "Last-Modified".to_string(),
            httpdate::fmt_http_date(modified),
        ));
    }
    Ok(response)
}

fn content_type_for(path: &str) -> &'static str {
    if path.ends_with(".pack") {
        "application/x-git-packed-objects"
    } else if path.ends_with(".idx") {
        "application/x-git-packed-objects-toc"
    } else if path.starts_with("objects/") {
        "application/x-git-loose-object"
    } else {
        "text/plain"
    }
}

fn service_preamble(service: &str) -> Vec<u8> {
    let line = format!("# service={service}\n");
    let mut preamble = format!("{:04x}", line.len() + 4).into_bytes();
    preamble.extend_from_slice(line.as_bytes());
    preamble.extend_from_slice(b"0000");
    preamble
}

fn git_response(content_type: &str, body: Vec<u8>) -> Response {
    Response {
        status: 200,
        headers: vec![
            ("Content-Type".to_string(), content_type.to_string()),
            ("Cache-Control".to_string(), "no-cache".to_string()),
        ],
        body,
    }
}

fn canonicalize(path: &Path) -> Result<PathBuf, Option<std::io::Error>> {
    path.canonicalize().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            None
        } else {
            Some(error)
        }
    })
}

fn is_safe_component(value: &str) -> bool {
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && !value.contains('\\')
        && !value.contains('\0')
}

fn not_found() -> Response {
    Response {
        status: 404,
        headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
        body: b"Not Found".to_vec(),
    }
}

fn unsupported_media_type() -> Response {
    Response {
        status: 415,
        headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
        body: b"Unsupported Media Type".to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("twig_http_{nonce}"));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn classifies_smart_http_requests() {
        assert_eq!(
            classify_request("GET", "/repo/info/refs", "service=git-upload-pack"),
            RequestKind::AdvertiseRefs(Service::UploadPack)
        );
        assert_eq!(
            classify_request("GET", "/repo/info/refs", "service=git-receive-pack"),
            RequestKind::AdvertiseRefs(Service::ReceivePack)
        );
        assert_eq!(
            classify_request("POST", "/repo/git-upload-pack", ""),
            RequestKind::UploadPack
        );
        assert_eq!(
            classify_request("POST", "/repo/git-receive-pack", ""),
            RequestKind::ReceivePack
        );
    }

    #[test]
    fn serves_repository_head() {
        let root = TestDir::new();
        let repo = root.0.join("public").join("example");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        let config = Config::new(&root.0);

        let response = handle(
            &config,
            "public",
            Request {
                method: "GET",
                path_info: "/example/HEAD",
                query_string: "",
                content_type: None,
                body: &[],
                authenticated_user: None,
                may_rewrite_protected_branches: false,
                receive_hooks: &[],
            },
        )
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"ref: refs/heads/main\n");
    }

    #[test]
    fn serves_nested_binary_files_with_git_content_types() {
        let root = TestDir::new();
        let pack_path = root.0.join("public/example/objects/pack/test.pack");
        std::fs::create_dir_all(pack_path.parent().unwrap()).unwrap();
        let pack = b"PACK\0\x00\x00\x02binary";
        std::fs::write(&pack_path, pack).unwrap();

        let response = handle(
            &Config::new(&root.0),
            "public",
            Request {
                method: "GET",
                path_info: "/example/objects/pack/test.pack",
                query_string: "",
                content_type: None,
                body: &[],
                authenticated_user: None,
                may_rewrite_protected_branches: false,
                receive_hooks: &[],
            },
        )
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, pack);
        assert_eq!(
            response.headers[0],
            (
                "Content-Type".to_string(),
                "application/x-git-packed-objects".to_string()
            )
        );
        assert!(
            response
                .headers
                .iter()
                .any(|(name, _)| name == "Last-Modified")
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_dumb_http_symlinks_outside_repository() {
        use std::os::unix::fs::symlink;

        let root = TestDir::new();
        let repo = root.0.join("public/example");
        std::fs::create_dir_all(&repo).unwrap();
        let private_file = root.0.join("private.pack");
        std::fs::write(&private_file, b"private bytes").unwrap();
        symlink(&private_file, repo.join("leak.pack")).unwrap();

        let response = handle(
            &Config::new(&root.0),
            "public",
            Request {
                method: "GET",
                path_info: "/example/leak.pack",
                query_string: "",
                content_type: None,
                body: &[],
                authenticated_user: None,
                may_rewrite_protected_branches: false,
                receive_hooks: &[],
            },
        )
        .unwrap();

        assert_eq!(response.status, 404);
    }

    #[test]
    fn upload_pack_advertisement_uses_smart_http_format() {
        let root = TestDir::new();
        let repo = root.0.join("public").join("example");
        let output = Command::new("git")
            .args(["init", "--bare", "--initial-branch=main"])
            .arg(&repo)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");

        let response = handle(
            &Config::new(&root.0),
            "public",
            Request {
                method: "GET",
                path_info: "/example/info/refs",
                query_string: "service=git-upload-pack",
                content_type: None,
                body: &[],
                authenticated_user: None,
                may_rewrite_protected_branches: false,
                receive_hooks: &[],
            },
        )
        .unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(
            response.headers[0],
            (
                "Content-Type".to_string(),
                "application/x-git-upload-pack-advertisement".to_string()
            )
        );
        assert!(
            response
                .body
                .starts_with(b"001e# service=git-upload-pack\n0000")
        );
        assert!(response.body.len() > b"001e# service=git-upload-pack\n0000".len());
    }

    #[test]
    fn does_not_serve_paths_outside_repository() {
        let root = TestDir::new();
        let repo = root.0.join("public").join("example");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(root.0.join("HEAD"), "outside\n").unwrap();
        let config = Config::new(&root.0);

        let response = handle(
            &config,
            "public",
            Request {
                method: "GET",
                path_info: "/../HEAD",
                query_string: "",
                content_type: None,
                body: &[],
                authenticated_user: None,
                may_rewrite_protected_branches: false,
                receive_hooks: &[],
            },
        )
        .unwrap();

        assert_eq!(response.status, 404);
        assert_eq!(response.body, b"Not Found");
    }
}
