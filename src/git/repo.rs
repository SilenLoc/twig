use std::path::{Path, PathBuf};

use actix_web::{HttpRequest, HttpResponse, Responder, post, web};
use git2::{Repository, RepositoryInitOptions, Signature};
use log::info;
use serde::Deserialize;

use crate::auth::{
    CredentialAuth, TwigContext,
    permissions::{self, NamespacePermission},
};
use crate::config;

/// Resolves a namespace beneath the configured root, refusing symlinks and
/// existing paths whose canonical destination is outside that root.
pub fn checked_namespace_path(project_root: &Path, namespace: &str) -> Result<PathBuf, String> {
    crate::git::reserved::validate_namespace_name(namespace)?;

    if !project_root.exists() {
        return Ok(project_root.join(namespace));
    }

    let canonical_root = project_root
        .canonicalize()
        .map_err(|error| format!("Failed to resolve project root: {error}"))?;
    let namespace_path = canonical_root.join(namespace);
    match std::fs::symlink_metadata(&namespace_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err("Namespace directory must not be a symlink".to_string());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(namespace_path);
        }
        Err(error) => return Err(format!("Failed to inspect namespace directory: {error}")),
    }

    let canonical_namespace = namespace_path
        .canonicalize()
        .map_err(|error| format!("Failed to resolve namespace directory: {error}"))?;
    if !canonical_namespace.starts_with(&canonical_root) || canonical_namespace == canonical_root {
        return Err("Namespace path is outside the project root".to_string());
    }
    if !canonical_namespace.is_dir() {
        return Err("Namespace path is not a directory".to_string());
    }

    Ok(canonical_namespace)
}

/// Creates the namespace directory only after validating its name and checking
/// that the resulting directory remains under the canonical project root.
pub fn ensure_namespace_directory(project_root: &Path, namespace: &str) -> Result<PathBuf, String> {
    crate::git::reserved::validate_namespace_name(namespace)?;
    std::fs::create_dir_all(project_root)
        .map_err(|error| format!("Failed to create project root: {error}"))?;

    let namespace_path = checked_namespace_path(project_root, namespace)?;
    if !namespace_path.exists() {
        std::fs::create_dir(&namespace_path)
            .map_err(|error| format!("Failed to create namespace directory: {error}"))?;
    }

    checked_namespace_path(project_root, namespace)
}

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

#[post("/init")]
pub async fn init(
    req: HttpRequest,
    init_repo: web::Form<InitRepo>,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> impl Responder {
    if let Err(message) = crate::git::reserved::validate_namespace_name(&init_repo.namespace) {
        return HttpResponse::BadRequest().body(message);
    }

    let db = auth_state.db();

    let principal = match auth_state.authenticate_request(&req).await {
        Ok(CredentialAuth::Authenticated(principal)) => principal,
        Ok(CredentialAuth::Missing) => {
            return HttpResponse::Unauthorized()
                .insert_header(("WWW-Authenticate", "Basic realm=\"twig\""))
                .body("Missing credentials");
        }
        Ok(CredentialAuth::Invalid) => {
            return HttpResponse::Unauthorized().body("Invalid credentials");
        }
        Err(error) => {
            log::error!("Authentication failed for repository initialization: {error}");
            return HttpResponse::InternalServerError().body("Authentication error");
        }
    };

    match permissions::for_principal(
        db,
        &principal,
        &init_repo.namespace,
        server.is_configured_admin(&principal.user.username),
    )
    .await
    {
        Ok(Some(permissions)) if permissions.contains(NamespacePermission::CreateRepositories) => {}
        Ok(Some(_) | None) => {
            return HttpResponse::Forbidden().body("Only namespace owners can create repositories");
        }
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError().body("Database error");
        }
    }

    match create_repo(
        server.project_root(),
        init_repo.namespace.clone(),
        init_repo.repo.clone(),
        init_repo.branch.clone(),
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
    let ns: String = namespace.into();
    let repo: String = repo.into();

    crate::git::reserved::validate_namespace_name(&ns)?;
    crate::git::reserved::validate_repo_name(&repo)?;

    info!("root path:{}", root.display());

    let ns = ensure_namespace_directory(root, &ns)?;
    let repo = ns.join(repo);

    if !repo.exists() {
        std::fs::create_dir_all(&repo)
            .map_err(|e| format!("Failed to create repo directory: {e}"))?;
        let output = bare_init(&repo, &branch, "Twig", "twig@localhost")?;
        info!("{output}");
    }

    Ok(())
}

pub fn bare_init(
    repo_path: &Path,
    branch: &str,
    author_name: &str,
    author_email: &str,
) -> Result<String, String> {
    let branch_ref = format!("refs/heads/{branch}");
    if !git2::Reference::is_valid_name(&branch_ref) {
        return Err(format!("Invalid initial branch name: {branch}"));
    }
    let mut options = RepositoryInitOptions::new();
    options.bare(true).initial_head(branch);
    let repo = Repository::init_opts(repo_path, &options)
        .map_err(|e| format!("Failed to initialize bare repository: {e}"))?;
    repo.config()
        .and_then(|mut config| config.set_bool("http.receivepack", true))
        .map_err(|e| format!("Failed to enable http.receivepack: {e}"))?;

    // Create initial commit with .twig.toml file
    // Create the initial configuration commit directly through libgit2.
    let blob_content = r#"#:schema https://twig.silenlocatelli.ch/assets/twig.schema.json
# Created with Twig
# All configuration options are listed below, commented out with their defaults.

# Files or folders to ignore in the file browser view.
# Examples: "docs/temp", "skills/", "notes.txt"
#ignore_for_view = []

# Tabs to display. If empty or not present, all tabs are shown.
#tabs = []

# Whether the repository can be deleted from the UI.
#deletable = false

# Whether the repository is private (read operations require authentication).
#private = false

#[present]
# Markdown files to include in the presentation view.
#files = []

#[paper]
# Directory whose Markdown pages form the long-form Paper view.
#dir = "paper"

# Runnable scripts, shown in the Scripts tab. Each key inside [scripts]
# names a group; nesting groups ([scripts.linux.maintenance]) builds the
# hierarchy. The tab appears only when a group lists a script.
#[scripts.linux]
#name = "Linux"
#scripts = [
#    { name = "Install", path = "scripts/install.sh" },
#]
"#;
    let blob_id = repo
        .blob(blob_content.as_bytes())
        .map_err(|e| format!("Failed to create .twig.toml blob: {e}"))?;
    let mut tree_builder = repo
        .treebuilder(None)
        .map_err(|e| format!("Failed to create initial tree: {e}"))?;
    tree_builder
        .insert(".twig.toml", blob_id, 0o100_644)
        .map_err(|e| format!("Failed to add .twig.toml to initial tree: {e}"))?;
    let tree_id = tree_builder
        .write()
        .map_err(|e| format!("Failed to write initial tree: {e}"))?;
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| format!("Failed to load initial tree: {e}"))?;
    let signature = Signature::now(author_name, author_email)
        .map_err(|e| format!("Failed to create initial commit signature: {e}"))?;
    let commit_id = repo
        .commit(
            Some(&branch_ref),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )
        .map_err(|e| format!("Failed to create initial commit: {e}"))?;

    Ok(commit_id.to_string())
}

#[cfg(test)]
#[path = "repo/tests.rs"]
mod tests;
