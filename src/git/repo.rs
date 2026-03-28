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
}

impl InitRepo {
    pub fn namespace(&self) -> String {
        self.namespace.clone()
    }

    pub fn repo(&self) -> String {
        self.repo.clone()
    }
}


#[post("/init")]
pub async fn init(
    init_repo: web::Form<InitRepo>,
    server: web::Data<config::Server>,
) -> impl Responder {
    let init = init_repo;

    create_repo(server.project_root(), init.namespace(), init.repo());

    HttpResponse::Ok()
}

fn create_repo(root: impl Into<String>, namespace: impl Into<String>, repo: impl Into<String>) {
    let root: String = root.into();
    let root: &Path = Path::new(&root);

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
        let res = bare_init(&repo);

        match res {
            Ok(std) => info!("{std}"),
            Err(e) => log::error!("{e}"),
        }
    }
}

pub fn bare_init(repo_path: &Path) -> Result<String, String> {
    let sh = sh();
    sh.change_dir(repo_path);
    cmd!(sh, "git init --bare")
        .read()
        .map_err(|e| e.to_string())
}

fn sh() -> xshell::Shell {
    xshell::Shell::new().unwrap()
}
