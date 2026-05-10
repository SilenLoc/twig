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
    let (username, password) = match extract_basic_auth(&req) {
        Some(creds) => creds,
        None => {
            return HttpResponse::Unauthorized()
                .insert_header(("WWW-Authenticate", "Basic realm=\"fig\""))
                .body("Missing credentials");
        }
    };

    // Get user from database
    let user = match db.get_user_by_username(&username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return HttpResponse::Unauthorized().body("Invalid credentials");
        }
        Err(e) => {
            log::error!("Database error: {}", e);
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
            log::error!("Password verification error: {}", e);
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
            log::error!("Database error: {}", e);
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
            log::error!("Failed to create repository: {}", e);
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

    info!("root path:{root:?}");

    if !root.exists() {
        std::fs::create_dir_all(root)
            .map_err(|e| format!("Failed to create root directory: {}", e))?;
    }

    let ns: String = namespace.into();
    let ns = root.join(ns);

    if !ns.exists() {
        std::fs::create_dir_all(&ns)
            .map_err(|e| format!("Failed to create namespace directory: {}", e))?;
    }

    let repo: String = repo.into();
    let repo = ns.join(repo);

    if !repo.exists() {
        std::fs::create_dir_all(&repo)
            .map_err(|e| format!("Failed to create repo directory: {}", e))?;
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
        .map_err(|e| format!("Failed to enable http.receivepack: {}", e))?;

    // Create initial commit with .fig.toml file
    // Use git plumbing commands to create a commit in a bare repo
    let blob_content = "# Created with Fig\n\nignore_for_view = []\n";
    let blob_hash = cmd!(sh, "git hash-object -w --stdin")
        .stdin(blob_content)
        .read()
        .map_err(|e| format!("Failed to create blob: {}", e))?;

    let tree_entry = format!("100644 blob {}\t.fig.toml\n", blob_hash);
    let tree_hash = cmd!(sh, "git mktree")
        .stdin(tree_entry)
        .read()
        .map_err(|e| format!("Failed to create tree: {}", e))?;

    // Set author and committer info from user to avoid "Author unknown" error
    let commit_hash = cmd!(sh, "git commit-tree {tree_hash} -m 'Initial commit'")
        .env("GIT_AUTHOR_NAME", author_name)
        .env("GIT_AUTHOR_EMAIL", author_email)
        .env("GIT_COMMITTER_NAME", author_name)
        .env("GIT_COMMITTER_EMAIL", author_email)
        .read()
        .map_err(|e| format!("Failed to create commit: {}", e))?;

    cmd!(sh, "git update-ref refs/heads/{branch} {commit_hash}")
        .run()
        .map_err(|e| format!("Failed to update ref: {}", e))?;

    Ok(output)
}

fn sh() -> Result<xshell::Shell, String> {
    xshell::Shell::new().map_err(|e| format!("Failed to create shell: {}", e))
}
