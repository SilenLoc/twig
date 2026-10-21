use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

const MANAGED_PRE_RECEIVE_MARKER: &str = "TWIG_MANAGED_PRE_RECEIVE_V1";
const CUSTOM_HOOK_PREFIX: &str = "pre-receive.twig-user";
const MAX_SOURCE_BYTES: usize = 64 * 1024;
const MAX_MANAGED_HOOK_BYTES: usize = 16 * 1024;
const MAX_RECENT_RUNS: usize = 50;
const MAX_RECORDED_OUTPUT_BYTES: usize = 16 * 1024;
const MAX_RECORDED_REFS_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedHookState {
    Healthy,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookRunStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomHookState {
    Enabled,
    Disabled,
    Unsafe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomHookInfo {
    pub name: String,
    pub state: CustomHookState,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRun {
    pub id: String,
    pub hook_name: String,
    pub started_at: String,
    pub actor: Option<String>,
    pub ref_updates: Vec<String>,
    pub status: HookRunStatus,
    pub stdout: String,
    pub stderr: String,
    pub output_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookInventory {
    pub managed_state: ManagedHookState,
    pub custom_hooks: Vec<CustomHookInfo>,
    pub recent_runs: Vec<HookRun>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum HookError {
    InvalidName,
    NotFound,
    UnsafePath,
    Unsupported,
    Io(io::Error),
}

impl std::fmt::Display for HookError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName => formatter.write_str("custom hook name is invalid"),
            Self::NotFound => formatter.write_str("custom hook was not found"),
            Self::UnsafePath => {
                formatter.write_str("repository hook path is not a regular directory")
            }
            Self::Unsupported => {
                formatter.write_str("hook operation is unsupported on this platform")
            }
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for HookError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidName | Self::NotFound | Self::UnsafePath | Self::Unsupported => None,
        }
    }
}

/// Inspects the managed pre-receive hook and Twig's custom receive-hook chain.
/// File symlinks and unexpected custom-hook names are reported as unsafe and
/// are never followed. This function only reads hook files; it never executes
/// them.
pub fn inspect_repository_hooks(repo_path: &Path) -> Result<HookInventory, HookError> {
    let repo_metadata = fs::symlink_metadata(repo_path).map_err(HookError::Io)?;
    if repo_metadata.file_type().is_symlink() || !repo_metadata.is_dir() {
        return Err(HookError::UnsafePath);
    }

    let hooks_path = repo_path.join("hooks");
    let hooks_metadata = match fs::symlink_metadata(&hooks_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(HookInventory {
                managed_state: ManagedHookState::Missing,
                custom_hooks: Vec::new(),
                recent_runs: Vec::new(),
                warnings: Vec::new(),
            });
        }
        Err(error) => return Err(HookError::Io(error)),
    };
    if hooks_metadata.file_type().is_symlink() || !hooks_metadata.is_dir() {
        return Ok(HookInventory {
            managed_state: ManagedHookState::Invalid,
            custom_hooks: Vec::new(),
            recent_runs: Vec::new(),
            warnings: vec!["The hooks path is not a regular directory.".to_string()],
        });
    }

    let managed_state = inspect_managed_hook(&hooks_path.join("pre-receive"));
    let mut warnings = Vec::new();
    if managed_state == ManagedHookState::Invalid {
        warnings.push("The managed pre-receive hook is missing its marker, is not executable, or is not a regular file.".to_string());
    } else if managed_state == ManagedHookState::Missing {
        warnings.push("The managed pre-receive hook is not installed.".to_string());
    }

    let mut custom_hooks = Vec::new();
    for entry in fs::read_dir(&hooks_path).map_err(HookError::Io)? {
        let entry = entry.map_err(HookError::Io)?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !name.starts_with(CUSTOM_HOOK_PREFIX) {
            continue;
        }

        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                warnings.push(format!("Could not inspect custom hook {name}: {error}"));
                custom_hooks.push(CustomHookInfo {
                    name,
                    state: CustomHookState::Unsafe,
                    source: None,
                });
                continue;
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            warnings.push(format!("Custom hook {name} is not a regular file."));
            custom_hooks.push(CustomHookInfo {
                name,
                state: CustomHookState::Unsafe,
                source: None,
            });
            continue;
        }

        let supported_name = validate_custom_hook_name(&name).is_ok();
        let source = match read_capped(&path, MAX_SOURCE_BYTES) {
            Ok((bytes, truncated)) => {
                if truncated {
                    warnings.push(format!(
                        "Source for {name} is longer than the {} KiB display limit and was truncated.",
                        MAX_SOURCE_BYTES / 1024
                    ));
                }
                let mut source = String::from_utf8_lossy(&bytes).into_owned();
                if truncated {
                    source.push_str("\n[Source truncated for display.]\n");
                }
                Some(source)
            }
            Err(error) => {
                warnings.push(format!("Could not read custom hook {name}: {error}"));
                None
            }
        };

        let state = if !supported_name || source.is_none() {
            CustomHookState::Unsafe
        } else if is_executable(&metadata) {
            CustomHookState::Enabled
        } else {
            CustomHookState::Disabled
        };
        if !supported_name {
            warnings.push(format!(
                "Custom hook {name} has a name outside Twig's supported hook pattern."
            ));
        }
        custom_hooks.push(CustomHookInfo {
            name,
            state,
            source,
        });
    }
    custom_hooks.sort_by(|left, right| left.name.cmp(&right.name));

    let recent_runs = inspect_recent_runs(&hooks_path, &mut warnings);
    Ok(HookInventory {
        managed_state,
        custom_hooks,
        recent_runs,
        warnings,
    })
}

/// Accepts only the original custom-hook name and the numbered sidecars Twig
/// creates when preserving an existing pre-receive hook.
pub fn validate_custom_hook_name(name: &str) -> Result<(), HookError> {
    let Some(suffix) = name.strip_prefix(CUSTOM_HOOK_PREFIX) else {
        return Err(HookError::InvalidName);
    };
    let valid = suffix.is_empty()
        || suffix
            .strip_prefix('.')
            .is_some_and(|number| number.parse::<u32>().is_ok_and(|number| number > 0));
    if valid {
        Ok(())
    } else {
        Err(HookError::InvalidName)
    }
}

/// Removes all execute bits from one supported custom hook without changing
/// its source. It refuses symlinks and never accepts the managed policy hook.
pub fn disable_custom_hook(repo_path: &Path, name: &str) -> Result<(), HookError> {
    validate_custom_hook_name(name)?;
    let path = custom_hook_path(repo_path, name)?;
    let metadata = fs::symlink_metadata(&path).map_err(map_missing_hook)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(HookError::UnsafePath);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        if mode & 0o111 == 0 {
            return Ok(());
        }
        let mut permissions = metadata.permissions();
        permissions.set_mode(mode & !0o111);
        fs::set_permissions(path, permissions).map_err(HookError::Io)
    }
    #[cfg(not(unix))]
    {
        Err(HookError::Unsupported)
    }
}

/// Deletes one supported custom hook. The managed policy hook, symlinks, and
/// non-regular files are never removable through this operation.
pub fn delete_custom_hook(repo_path: &Path, name: &str) -> Result<(), HookError> {
    validate_custom_hook_name(name)?;
    let path = custom_hook_path(repo_path, name)?;
    let metadata = fs::symlink_metadata(&path).map_err(map_missing_hook)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(HookError::UnsafePath);
    }
    fs::remove_file(path).map_err(HookError::Io)
}

fn custom_hook_path(repo_path: &Path, name: &str) -> Result<std::path::PathBuf, HookError> {
    let repo_metadata = fs::symlink_metadata(repo_path).map_err(HookError::Io)?;
    if repo_metadata.file_type().is_symlink() || !repo_metadata.is_dir() {
        return Err(HookError::UnsafePath);
    }
    let hooks_path = repo_path.join("hooks");
    let hooks_metadata = fs::symlink_metadata(&hooks_path).map_err(map_missing_hook)?;
    if hooks_metadata.file_type().is_symlink() || !hooks_metadata.is_dir() {
        return Err(HookError::UnsafePath);
    }
    Ok(hooks_path.join(name))
}

fn map_missing_hook(error: io::Error) -> HookError {
    if error.kind() == io::ErrorKind::NotFound {
        HookError::NotFound
    } else {
        HookError::Io(error)
    }
}

fn inspect_recent_runs(hooks_path: &Path, warnings: &mut Vec<String>) -> Vec<HookRun> {
    let run_root = hooks_path.join("twig-runs");
    let root_metadata = match fs::symlink_metadata(&run_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            warnings.push(format!("Could not inspect hook run history: {error}"));
            return Vec::new();
        }
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        warnings.push("The hook run history path is not a regular directory.".to_string());
        return Vec::new();
    }

    let entries = match fs::read_dir(&run_root) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(format!("Could not read hook run history: {error}"));
            return Vec::new();
        }
    };
    let mut runs = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(format!("Could not read a hook run entry: {error}"));
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("run-") {
            continue;
        }
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(error) => {
                warnings.push(format!("Could not inspect hook run {name}: {error}"));
                continue;
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            warnings.push(format!("Hook run {name} is not a regular directory."));
            continue;
        }
        let modified = match metadata.modified() {
            Ok(modified) => modified,
            Err(error) => {
                warnings.push(format!(
                    "Could not read the time for hook run {name}: {error}"
                ));
                continue;
            }
        };
        runs.push((modified, name, entry.path()));
    }
    runs.sort_by(|left, right| right.0.cmp(&left.0));

    let mut recent = Vec::with_capacity(runs.len().min(MAX_RECENT_RUNS));
    for (_, id, path) in runs.into_iter().take(MAX_RECENT_RUNS) {
        match read_run_record(&path, id.as_str()) {
            Ok(run) => recent.push(run),
            Err(error) => warnings.push(format!("Could not read hook run {id}: {error}")),
        }
    }
    recent
}

fn read_run_record(path: &Path, id: &str) -> io::Result<HookRun> {
    let (hook_name, _) = read_record_text(path, "hook", 512)?;
    let (started_at, _) = read_record_text(path, "started_at", 128)?;
    let (actor, _) = read_record_text(path, "actor", 512)?;
    let (exit_status, _) = read_record_text(path, "exit_status", 16)?;
    let (ref_updates, refs_capped) = read_record_text(path, "refs", MAX_RECORDED_REFS_BYTES + 128)?;
    let (stdout, stdout_truncated) = read_record_text(path, "stdout", MAX_RECORDED_OUTPUT_BYTES)?;
    let (stderr, stderr_truncated) = read_record_text(path, "stderr", MAX_RECORDED_OUTPUT_BYTES)?;
    let (recorded_truncated, _) = read_record_text(path, "truncated", 8)?;

    if hook_name.is_empty() || started_at.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "hook name or timestamp is empty",
        ));
    }
    let status = exit_status
        .trim()
        .parse::<u8>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let marker = "\n[Ref updates truncated.]\n";
    let refs_truncated = ref_updates.ends_with(marker) || refs_capped;
    let ref_updates_text = ref_updates.strip_suffix(marker).unwrap_or(&ref_updates);
    let mut ref_updates = ref_updates_text
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if refs_truncated
        && !ref_updates
            .last()
            .is_some_and(|line| line == "[Ref updates truncated.]")
    {
        ref_updates.push("[Ref updates truncated.]".to_string());
    }
    Ok(HookRun {
        id: id.to_string(),
        hook_name: hook_name.trim().to_owned(),
        started_at: started_at.trim().to_owned(),
        actor: (!actor.trim().is_empty()).then(|| actor.trim().to_owned()),
        ref_updates,
        status: if status == 0 {
            HookRunStatus::Passed
        } else {
            HookRunStatus::Failed
        },
        stdout,
        stderr,
        output_truncated: recorded_truncated.trim() == "1" || stdout_truncated || stderr_truncated,
    })
}

fn read_record_text(path: &Path, name: &str, limit: usize) -> io::Result<(String, bool)> {
    let file_path = path.join(name);
    let metadata = fs::symlink_metadata(&file_path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("record field {name} is not a regular file"),
        ));
    }
    let (bytes, truncated) = read_capped(&file_path, limit)?;
    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

fn inspect_managed_hook(path: &Path) -> ManagedHookState {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return ManagedHookState::Missing,
        Err(_) => return ManagedHookState::Invalid,
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() || !is_executable(&metadata) {
        return ManagedHookState::Invalid;
    }
    match read_capped(path, MAX_MANAGED_HOOK_BYTES) {
        Ok((contents, false))
            if String::from_utf8_lossy(&contents).contains(MANAGED_PRE_RECEIVE_MARKER) =>
        {
            ManagedHookState::Healthy
        }
        _ => ManagedHookState::Invalid,
    }
}

fn read_capped(path: &Path, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let file = File::open(path)?;
    let mut bytes = Vec::with_capacity(limit.min(8192));
    file.take((limit + 1) as u64).read_to_end(&mut bytes)?;
    let truncated = bytes.len() > limit;
    bytes.truncate(limit);
    Ok((bytes, truncated))
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
#[path = "hooks/tests.rs"]
mod tests;
