use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use log::info;
use xshell::cmd;

pub fn default_branch() -> String {
    "main".to_string()
}

pub fn create_repo(
    root: impl AsRef<Path>,
    namespace: impl Into<String>,
    repo: impl Into<String>,
    branch: impl Into<String>,
) -> Result<(), String> {
    create_repo_with_author(root, namespace, repo, branch, "Twig", "twig@localhost")
}

/// Creates a repository using the supplied identity for its initial commit.
pub fn create_repo_with_author(
    root: impl AsRef<Path>,
    namespace: impl Into<String>,
    repo: impl Into<String>,
    branch: impl Into<String>,
    author_name: &str,
    author_email: &str,
) -> Result<(), String> {
    let root = root.as_ref();
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
    crate::reserved::validate_repo_name(&repo)?;
    let repo = ns.join(repo);

    if repo.exists() {
        install_pre_receive_hook(&repo)?;
    } else {
        std::fs::create_dir_all(&repo)
            .map_err(|e| format!("Failed to create repo directory: {e}"))?;
        match bare_init(&repo, &branch, author_name, author_email) {
            Ok(output) => info!("{output}"),
            Err(error) => {
                let _ = fs::remove_dir_all(&repo);
                return Err(error);
            }
        }
    }

    Ok(())
}

/// Whether `path` can be opened as a Git repository.
pub fn is_repository(path: &Path) -> bool {
    git2::Repository::open(path).is_ok()
}

/// Whether `path` is a bare Git repository.
pub fn is_bare_repository(path: &Path) -> bool {
    git2::Repository::open(path).is_ok_and(|repository| repository.is_bare())
}

/// Deletes a valid repository directory and reports expected path failures
/// separately so the application can preserve its HTTP status mapping.
pub fn delete_repository(path: &Path) -> Result<(), RepositoryError> {
    if !path.exists() {
        return Err(RepositoryError::NotFound);
    }
    if !is_repository(path) {
        return Err(RepositoryError::NotRepository);
    }
    fs::remove_dir_all(path).map_err(RepositoryError::Io)
}

/// Moves a valid repository to an unused destination, creating its parent
/// directory when needed.
pub fn move_repository(source: &Path, destination: &Path) -> Result<(), RepositoryError> {
    if !source.exists() {
        return Err(RepositoryError::NotFound);
    }
    if !is_repository(source) {
        return Err(RepositoryError::NotRepository);
    }
    if destination.exists() {
        return Err(RepositoryError::DestinationExists);
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(RepositoryError::Io)?;
    }
    fs::rename(source, destination).map_err(RepositoryError::Io)
}

#[derive(Debug)]
pub enum RepositoryError {
    NotFound,
    NotRepository,
    DestinationExists,
    Io(std::io::Error),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("repository not found"),
            Self::NotRepository => formatter.write_str("path is not a valid Git repository"),
            Self::DestinationExists => formatter.write_str("repository destination already exists"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for RepositoryError {}

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

    // Create initial commit with .twig.toml file
    // Use git plumbing commands to create a commit in a bare repo
    let blob_content = r#"#:schema https://twig.silenlocatelli.ch/assets/twig.schema.json
# Created with Twig
# All configuration options are listed below, commented out with their defaults.

# Files or folders to ignore in the file browser view.
# Examples: "docs/temp", "skills/", "notes.txt"
#ignore_for_view = []

# Tabs to display. If empty or not present, all tabs are shown.
#tabs = []

# Whether the repository can be deleted from the UI.
#deleteable = false

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
    let blob_hash = cmd!(sh, "git hash-object -w --stdin")
        .stdin(blob_content)
        .read()
        .map_err(|e| format!("Failed to create blob: {e}"))?;

    let tree_entry = format!("100644 blob {blob_hash}\t.twig.toml\n");
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

    install_pre_receive_hook(repo_path)?;

    Ok(output)
}

const PRE_RECEIVE_HOOK: &str = include_str!("hooks/pre-receive");
const PRE_RECEIVE_HOOK_MARKER: &str = "TWIG_MANAGED_PRE_RECEIVE_V1";

/// Installs Twig's receive policy while preserving any pre-existing user hook.
/// Repeated calls are idempotent; additional custom hooks are kept as uniquely
/// named sidecars and chained by the managed wrapper.
pub fn install_pre_receive_hook(repo_path: &Path) -> Result<(), String> {
    let hooks_dir = repo_path.join("hooks");
    fs::create_dir_all(&hooks_dir)
        .map_err(|error| format!("Failed to create repository hooks directory: {error}"))?;
    let hook_path = hooks_dir.join("pre-receive");

    if fs::symlink_metadata(&hook_path).is_ok() {
        let metadata = fs::symlink_metadata(&hook_path)
            .map_err(|error| format!("Failed to inspect existing receive hook: {error}"))?;
        if metadata.file_type().is_file() {
            let contents = fs::read_to_string(&hook_path)
                .map_err(|error| format!("Failed to read existing receive hook: {error}"))?;
            if contents.contains(PRE_RECEIVE_HOOK_MARKER) {
                set_hook_executable(&hook_path)?;
                return Ok(());
            }
        } else if !metadata.file_type().is_symlink() {
            return Err("Existing pre-receive hook is not a regular file".to_string());
        }
    }

    let temporary_hook = hooks_dir.join(format!(".pre-receive.twig-{}", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_hook)
        .map_err(|error| format!("Failed to create managed receive hook: {error}"))?;
    if let Err(error) = file.write_all(PRE_RECEIVE_HOOK.as_bytes()) {
        let _ = fs::remove_file(&temporary_hook);
        return Err(format!("Failed to write managed receive hook: {error}"));
    }
    drop(file);
    if let Err(error) = set_hook_executable(&temporary_hook) {
        let _ = fs::remove_file(&temporary_hook);
        return Err(error);
    }

    let preserved_hook = if fs::symlink_metadata(&hook_path).is_ok() {
        match next_preserved_hook_path(&hooks_dir) {
            Ok(path) => {
                if let Err(error) = fs::rename(&hook_path, &path) {
                    let _ = fs::remove_file(&temporary_hook);
                    return Err(format!("Failed to preserve existing receive hook: {error}"));
                }
                Some(path)
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary_hook);
                return Err(error);
            }
        }
    } else {
        None
    };

    if let Err(error) = fs::rename(&temporary_hook, &hook_path) {
        if let Some(preserved_hook) = preserved_hook {
            let _ = fs::rename(preserved_hook, &hook_path);
        }
        let _ = fs::remove_file(&temporary_hook);
        return Err(format!("Failed to activate managed receive hook: {error}"));
    }
    Ok(())
}

fn next_preserved_hook_path(hooks_dir: &Path) -> Result<PathBuf, String> {
    let base = hooks_dir.join("pre-receive.twig-user");
    if fs::symlink_metadata(&base).is_err() {
        return Ok(base);
    }
    for suffix in 1..=u32::MAX {
        let candidate = hooks_dir.join(format!("pre-receive.twig-user.{suffix}"));
        if fs::symlink_metadata(&candidate).is_err() {
            return Ok(candidate);
        }
    }
    Err("No free sidecar name is available for the existing receive hook".to_string())
}

fn set_hook_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|error| format!("Failed to inspect receive hook permissions: {error}"))?
            .permissions();
        permissions.set_mode(permissions.mode() | 0o111);
        fs::set_permissions(path, permissions)
            .map_err(|error| format!("Failed to make receive hook executable: {error}"))?;
    }
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(path)
            .map_err(|error| format!("Failed to inspect receive hook permissions: {error}"))?
            .permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)
            .map_err(|error| format!("Failed to make receive hook executable: {error}"))?;
    }
    Ok(())
}

fn sh() -> Result<xshell::Shell, String> {
    xshell::Shell::new().map_err(|e| format!("Failed to create shell: {e}"))
}

#[cfg(test)]
#[path = "repo/tests.rs"]
mod tests;
