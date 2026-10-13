use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const PRE_RECEIVE_HOOK: &str = include_str!("../hooks/pre-receive");
pub(super) const PRE_RECEIVE_HOOK_MARKER: &str = "TWIG_MANAGED_PRE_RECEIVE_V2";
pub(super) const PREVIOUS_PRE_RECEIVE_HOOK_MARKER: &str = "TWIG_MANAGED_PRE_RECEIVE_V1";

/// Installs Twig's receive policy while preserving any pre-existing user hook.
/// Repeated calls are idempotent; additional custom hooks are kept as uniquely
/// named sidecars and chained by the managed wrapper.
pub fn install_pre_receive_hook(repo_path: &Path) -> Result<(), String> {
    let hooks_dir = repo_path.join("hooks");
    fs::create_dir_all(&hooks_dir)
        .map_err(|error| format!("Failed to create repository hooks directory: {error}"))?;
    let hook_path = hooks_dir.join("pre-receive");
    let mut upgrading_managed_hook = false;

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
            // V1 was reused across receive-policy changes. Replace any V1
            // managed hook in place: preserving it as a user hook could keep
            // obsolete restrictions active after an upgrade.
            upgrading_managed_hook = contents.contains(PREVIOUS_PRE_RECEIVE_HOOK_MARKER);
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

    let preserved_hook = if !upgrading_managed_hook && fs::symlink_metadata(&hook_path).is_ok() {
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
