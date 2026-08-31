use std::path::Path;

use actix_web::{HttpRequest, HttpResponse, Responder, post, web};
use log::info;
use serde::Deserialize;
use xshell::cmd;

use crate::auth::{FigContext, extract_basic_auth, verify_password};
use crate::config;

#[derive(Deserialize)]
struct InitRepo {
    namespace: String,
    repo: String,
    #[serde(default = "default_branch")]
    branch: String,
}

pub fn default_branch() -> String {
    "main".to_string()
}

impl InitRepo {
    pub fn namespace(&self) -> String {
        self.namespace.clone()
    }

    pub fn repo(&self) -> String {
        self.repo.clone()
    }

    pub fn branch(&self) -> String {
        self.branch.clone()
    }
}

#[post("/init")]
pub async fn init(
    req: HttpRequest,
    init_repo: web::Form<InitRepo>,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
) -> impl Responder {
    let db = auth_state.db();

    // Authenticate the request
    let Some((username, password)) = extract_basic_auth(&req) else {
        return HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", "Basic realm=\"fig\""))
            .body("Missing credentials");
    };

    // Get user from database
    let user = match db.get_user_by_username(&username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return HttpResponse::Unauthorized().body("Invalid credentials");
        }
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError().body("Database error");
        }
    };

    // Verify password
    match verify_password(&password, &user.password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Unauthorized().body("Invalid credentials");
        }
        Err(e) => {
            log::error!("Password verification error: {e}");
            return HttpResponse::InternalServerError().body("Authentication error");
        }
    }

    // Check if user has access to namespace
    match db
        .user_has_namespace_access(&user.id, &init_repo.namespace)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Forbidden().body("Access denied to namespace");
        }
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError().body("Database error");
        }
    }

    let init = init_repo;

    match create_repo(
        server.project_root(),
        init.namespace(),
        init.repo(),
        init.branch(),
    ) {
        Ok(()) => HttpResponse::Ok().body("Repository created"),
        Err(e) => {
            log::error!("Failed to create repository: {e}");
            HttpResponse::InternalServerError().body("Failed to create repository")
        }
    }
}

fn create_repo(
    root: impl Into<String>,
    namespace: impl Into<String>,
    repo: impl Into<String>,
    branch: impl Into<String>,
) -> Result<(), String> {
    let root: String = root.into();
    let root: &Path = Path::new(&root);
    let branch: String = branch.into();

    info!("root path:{}", root.display());

    if !root.exists() {
        std::fs::create_dir_all(root)
            .map_err(|e| format!("Failed to create root directory: {e}"))?;
    }

    let ns: String = namespace.into();
    let ns = root.join(ns);

    if !ns.exists() {
        std::fs::create_dir_all(&ns)
            .map_err(|e| format!("Failed to create namespace directory: {e}"))?;
    }

    let repo: String = repo.into();
    crate::git::reserved::validate_repo_name(&repo)?;
    let repo = ns.join(repo);

    if !repo.exists() {
        std::fs::create_dir_all(&repo)
            .map_err(|e| format!("Failed to create repo directory: {e}"))?;
        let res = bare_init(&repo, &branch, "Fig", "fig@localhost");

        match res {
            Ok(std) => info!("{std}"),
            Err(e) => log::error!("{e}"),
        }
    }

    Ok(())
}

pub fn bare_init(
    repo_path: &Path,
    branch: &str,
    author_name: &str,
    author_email: &str,
) -> Result<String, String> {
    let sh = sh()?;
    sh.change_dir(repo_path);

    // Initialize bare repo
    let output = cmd!(sh, "git init --bare --initial-branch={branch}")
        .read()
        .map_err(|e| e.to_string())?;

    // Enable http.receivepack to allow pushes via HTTP
    cmd!(sh, "git config http.receivepack true")
        .run()
        .map_err(|e| format!("Failed to enable http.receivepack: {e}"))?;

    // Create initial commit with .fig.toml file
    // Use git plumbing commands to create a commit in a bare repo
    let blob_content = r#"# Created with Fig
# All configuration options are listed below, commented out with their defaults.

# Files or folders to ignore in the file browser view.
# Examples: "docs/temp", "skills/", "notes.txt"
#ignore_for_view = []

# Tabs to display. If empty or not present, all tabs are shown.
#tabs = []

# Whether the repository can be deleted from the UI.
#deleteable = false

[present]
# Markdown files to include in the presentation view.
#files = []
"#;
    let blob_hash = cmd!(sh, "git hash-object -w --stdin")
        .stdin(blob_content)
        .read()
        .map_err(|e| format!("Failed to create blob: {e}"))?;

    let tree_entry = format!("100644 blob {blob_hash}\t.fig.toml\n");
    let tree_hash = cmd!(sh, "git mktree")
        .stdin(tree_entry)
        .read()
        .map_err(|e| format!("Failed to create tree: {e}"))?;

    // Set author and committer info from user to avoid "Author unknown" error
    let commit_hash = cmd!(sh, "git commit-tree {tree_hash} -m 'Initial commit'")
        .env("GIT_AUTHOR_NAME", author_name)
        .env("GIT_AUTHOR_EMAIL", author_email)
        .env("GIT_COMMITTER_NAME", author_name)
        .env("GIT_COMMITTER_EMAIL", author_email)
        .read()
        .map_err(|e| format!("Failed to create commit: {e}"))?;

    cmd!(sh, "git update-ref refs/heads/{branch} {commit_hash}")
        .run()
        .map_err(|e| format!("Failed to update ref: {e}"))?;

    Ok(output)
}

fn sh() -> Result<xshell::Shell, String> {
    xshell::Shell::new().map_err(|e| format!("Failed to create shell: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_branch_is_main() {
        assert_eq!(default_branch(), "main");
    }

    #[test]
    fn test_init_repo_getters() {
        let init_repo = InitRepo {
            namespace: "ns".to_string(),
            repo: "repo".to_string(),
            branch: "trunk".to_string(),
        };
        assert_eq!(init_repo.namespace(), "ns");
        assert_eq!(init_repo.repo(), "repo");
        assert_eq!(init_repo.branch(), "trunk");
    }

    #[test]
    fn test_create_repo_creates_directory_structure() {
        let temp_root = format!("/tmp/test_fig_repo_{}", uuid::Uuid::new_v4());
        let result = create_repo(&temp_root, "myns", "myrepo", "main");
        assert!(result.is_ok(), "create_repo failed: {result:?}");

        let repo_path = Path::new(&temp_root).join("myns").join("myrepo");
        assert!(repo_path.exists(), "repo directory should exist");
        assert!(
            repo_path.join("HEAD").exists(),
            "bare repo HEAD should exist"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn test_bare_init_creates_valid_repository() {
        let temp_dir = format!("/tmp/test_fig_bare_init_{}", uuid::Uuid::new_v4());
        std::fs::create_dir_all(&temp_dir).unwrap();

        let repo_path = Path::new(&temp_dir);
        let result = bare_init(repo_path, "main", "Test Author", "test@example.com");
        assert!(result.is_ok(), "bare_init failed: {result:?}");

        let git_dir = repo_path.join("HEAD");
        assert!(git_dir.exists(), "HEAD should exist after bare_init");

        // Verify git2 can open it
        let repo = git2::Repository::open(repo_path);
        assert!(repo.is_ok(), "repo should be openable by git2");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
