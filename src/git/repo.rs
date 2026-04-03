use std::path::Path;

use actix_web::{HttpResponse, Responder, post, web};
use log::info;
use serde::Deserialize;
use xshell::cmd;

use crate::config;

#[derive(Deserialize)]
struct InitRepo {
    namespace: String,
    repo: String,
    #[serde(default = "default_branch")]
    branch: String,
}

fn default_branch() -> String {
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
    init_repo: web::Form<InitRepo>,
    server: web::Data<config::Server>,
) -> impl Responder {
    let init = init_repo;

    create_repo(
        server.project_root(),
        init.namespace(),
        init.repo(),
        init.branch(),
    );

    HttpResponse::Ok()
}

fn create_repo(
    root: impl Into<String>,
    namespace: impl Into<String>,
    repo: impl Into<String>,
    branch: impl Into<String>,
) {
    let root: String = root.into();
    let root: &Path = Path::new(&root);
    let branch: String = branch.into();

    info!("root path:{root:?}");

    if !root.exists() {
        std::fs::create_dir_all(root).unwrap();
    }

    let ns: String = namespace.into();
    let ns = root.join(ns);

    if !ns.exists() {
        std::fs::create_dir_all(&ns).unwrap();
    }

    let repo: String = repo.into();
    let repo = ns.join(repo);

    if !repo.exists() {
        std::fs::create_dir_all(&repo).unwrap();
        let res = bare_init(&repo, &branch);

        match res {
            Ok(std) => info!("{std}"),
            Err(e) => log::error!("{e}"),
        }
    }
}

pub fn bare_init(repo_path: &Path, branch: &str) -> Result<String, String> {
    let sh = sh();
    sh.change_dir(repo_path);

    // Initialize bare repo
    let output = cmd!(sh, "git init --bare --initial-branch={branch}")
        .read()
        .map_err(|e| e.to_string())?;

    // Create initial commit with .fig file
    // Use git plumbing commands to create a commit in a bare repo
    let blob_content = "Created with Fig";
    let blob_hash = cmd!(sh, "git hash-object -w --stdin")
        .stdin(blob_content)
        .read()
        .map_err(|e| format!("Failed to create blob: {}", e))?;

    let tree_entry = format!("100644 blob {}\t.fig\n", blob_hash);
    let tree_hash = cmd!(sh, "git mktree")
        .stdin(tree_entry)
        .read()
        .map_err(|e| format!("Failed to create tree: {}", e))?;

    let commit_hash = cmd!(sh, "git commit-tree {tree_hash} -m 'Initial commit'")
        .read()
        .map_err(|e| format!("Failed to create commit: {}", e))?;

    cmd!(sh, "git update-ref refs/heads/{branch} {commit_hash}")
        .run()
        .map_err(|e| format!("Failed to update ref: {}", e))?;

    Ok(output)
}

fn sh() -> xshell::Shell {
    xshell::Shell::new().unwrap()
}
