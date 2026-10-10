use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use actix_web::{HttpRequest, HttpResponse, Responder, post, web};
use log::info;
use serde::Deserialize;
use xshell::cmd;

use crate::auth::{TwigContext, extract_basic_auth, verify_password};
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

#[post("/init")]
pub async fn init(
    req: HttpRequest,
    init_repo: web::Form<InitRepo>,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> impl Responder {
    let db = auth_state.db();

    // Authenticate the request
    let Some((username, password)) = extract_basic_auth(&req) else {
        return HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", "Basic realm=\"twig\""))
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
    match db.user_owns_namespace(&user.id, &init_repo.namespace).await {
        Ok(true) => {}
        Ok(false) => {
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

    if repo.exists() {
        install_pre_receive_hook(&repo)?;
    } else {
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
mod tests {
    use super::*;
    use std::process::{Command, Output, Stdio};

    fn run_pre_receive(
        repo_path: &Path,
        namespace_role: &str,
        updates: &str,
        extra_env: &[(&str, &str)],
    ) -> Output {
        let mut child = Command::new(repo_path.join("hooks/pre-receive"))
            .current_dir(repo_path)
            .env("GIT_DIR", repo_path)
            .env("TWIG_NAMESPACE_ROLE", namespace_role)
            .envs(extra_env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run managed pre-receive hook");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(updates.as_bytes())
            .unwrap();
        child.wait_with_output().expect("wait for receive hook")
    }

    #[test]
    fn test_default_branch_is_main() {
        assert_eq!(default_branch(), "main");
    }

    #[test]
    fn test_create_repo_creates_directory_structure() {
        let temp_root = format!("/tmp/test_twig_repo_{}", uuid::Uuid::new_v4());
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
        let temp_dir = format!("/tmp/test_twig_bare_init_{}", uuid::Uuid::new_v4());
        std::fs::create_dir_all(&temp_dir).unwrap();

        let repo_path = Path::new(&temp_dir);
        let result = bare_init(repo_path, "main", "Test Author", "test@example.com");
        assert!(result.is_ok(), "bare_init failed: {result:?}");

        let git_dir = repo_path.join("HEAD");
        assert!(git_dir.exists(), "HEAD should exist after bare_init");
        assert!(repo_path.join("hooks/pre-receive").exists());

        // Verify git2 can open it
        let repo = git2::Repository::open(repo_path);
        assert!(repo.is_ok(), "repo should be openable by git2");

        // The initial commit's .twig.toml points editors at the TOML schema.
        let repo = repo.unwrap();
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        let entry = commit
            .tree()
            .unwrap()
            .get_path(Path::new(".twig.toml"))
            .unwrap();
        let blob = entry.to_object(&repo).unwrap().into_blob().unwrap();
        let content = std::str::from_utf8(blob.content()).unwrap();
        assert!(
            content.starts_with("#:schema https://twig.silenlocatelli.ch/assets/twig.schema.json"),
            "default .twig.toml should reference the schema, got: {content}"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn contributor_hook_allows_fast_forwards_and_rejects_main_rewrites_and_deletions() {
        let temp_dir = format!("/tmp/test_twig_receive_policy_{}", uuid::Uuid::new_v4());
        std::fs::create_dir_all(&temp_dir).unwrap();
        let repo_path = Path::new(&temp_dir);
        bare_init(repo_path, "main", "Test Author", "test@example.com").unwrap();
        let repo = git2::Repository::open(repo_path).unwrap();
        let base = repo.head().unwrap().peel_to_commit().unwrap();
        let tree = base.tree().unwrap();
        let signature = git2::Signature::now("Test Author", "test@example.com").unwrap();
        let fast_forward = repo
            .commit(
                None,
                &signature,
                &signature,
                "fast forward",
                &tree,
                &[&base],
            )
            .unwrap();
        let rewrite = repo
            .commit(None, &signature, &signature, "rewrite", &tree, &[])
            .unwrap();
        let zero_oid = "0".repeat(base.id().to_string().len());

        let allowed = run_pre_receive(
            repo_path,
            "contributor",
            &format!("{} {} refs/heads/main\n", base.id(), fast_forward),
            &[],
        );
        assert!(allowed.status.success());

        let force_push = run_pre_receive(
            repo_path,
            "contributor",
            &format!("{} {} refs/heads/main\n", base.id(), rewrite),
            &[],
        );
        assert!(!force_push.status.success());
        assert!(
            String::from_utf8_lossy(&force_push.stderr)
                .contains("Contributors cannot rewrite history on main")
        );

        let deletion = run_pre_receive(
            repo_path,
            "contributor",
            &format!("{} {} refs/heads/main\n", base.id(), zero_oid),
            &[],
        );
        assert!(!deletion.status.success());
        assert!(
            String::from_utf8_lossy(&deletion.stderr).contains("Contributors cannot delete main")
        );

        let new_main = run_pre_receive(
            repo_path,
            "contributor",
            &format!("{zero_oid} {} refs/heads/main\n", base.id()),
            &[],
        );
        assert!(new_main.status.success());

        let owner_rewrite = run_pre_receive(
            repo_path,
            "owner",
            &format!("{} {} refs/heads/main\n", base.id(), rewrite),
            &[],
        );
        assert!(owner_rewrite.status.success());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn receive_hook_installation_preserves_chains_and_is_idempotent() {
        let temp_dir = format!("/tmp/test_twig_hook_chain_{}", uuid::Uuid::new_v4());
        let repo_path = Path::new(&temp_dir);
        std::fs::create_dir_all(repo_path.join("hooks")).unwrap();
        let init_output = Command::new("git")
            .args(["init", "--bare", "--initial-branch=main"])
            .current_dir(repo_path)
            .output()
            .expect("initialize bare repository");
        assert!(init_output.status.success());

        let custom_hook = "#!/bin/sh\nprintf '%s\\n' \"${TWIG_NAMESPACE_ROLE:-}\" > \"$TWIG_HOOK_ROLE_CAPTURE\"\nprintf '%s\\n' \"${REMOTE_USER:-}\" > \"$TWIG_HOOK_REMOTE_USER_CAPTURE\"\ncat > \"$TWIG_HOOK_INPUT_CAPTURE\"\nexit \"${TWIG_CUSTOM_HOOK_EXIT:-0}\"\n";
        let original_hook = repo_path.join("hooks/pre-receive");
        std::fs::write(&original_hook, custom_hook).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&original_hook, std::fs::Permissions::from_mode(0o750))
                .unwrap();
        }

        install_pre_receive_hook(repo_path).expect("install managed receive hook");
        let preserved = repo_path.join("hooks/pre-receive.twig-user");
        assert_eq!(std::fs::read_to_string(&preserved).unwrap(), custom_hook);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&preserved).unwrap().permissions().mode() & 0o777,
                0o750
            );
            assert_eq!(
                std::fs::metadata(&original_hook)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o111,
                0o111
            );
        }

        install_pre_receive_hook(repo_path).expect("reinstall managed receive hook");
        let sidecars: Vec<_> = std::fs::read_dir(repo_path.join("hooks"))
            .unwrap()
            .map(Result::unwrap)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("pre-receive.twig-user")
            })
            .collect();
        assert_eq!(
            sidecars.len(),
            1,
            "repeated install does not duplicate hooks"
        );

        let input = format!("{} {} refs/heads/feature\n", "0".repeat(40), "1".repeat(40));
        let role_capture = repo_path.join("role.capture");
        let remote_user_capture = repo_path.join("remote-user.capture");
        let input_capture = repo_path.join("input.capture");
        let output = run_pre_receive(
            repo_path,
            "contributor",
            &input,
            &[
                ("REMOTE_USER", "hook-user"),
                ("TWIG_HOOK_ROLE_CAPTURE", role_capture.to_str().unwrap()),
                (
                    "TWIG_HOOK_REMOTE_USER_CAPTURE",
                    remote_user_capture.to_str().unwrap(),
                ),
                ("TWIG_HOOK_INPUT_CAPTURE", input_capture.to_str().unwrap()),
            ],
        );
        assert!(output.status.success());
        assert_eq!(
            std::fs::read_to_string(role_capture).unwrap(),
            "contributor\n"
        );
        assert_eq!(
            std::fs::read_to_string(remote_user_capture).unwrap(),
            "hook-user\n"
        );
        assert_eq!(std::fs::read_to_string(input_capture).unwrap(), input);

        let rejection = run_pre_receive(
            repo_path,
            "owner",
            &input,
            &[("TWIG_CUSTOM_HOOK_EXIT", "23")],
        );
        assert_eq!(rejection.status.code(), Some(23));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
