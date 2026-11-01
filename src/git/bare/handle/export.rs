//! Commit-addressed reads for CI: one file at a commit, and a commit's tree
//! written out as plain files for an isolated checkout.

use std::io::Write;
use std::path::Path;

use git2::{ObjectType, Oid, TreeWalkMode, TreeWalkResult};

use super::RepoHandle;
use crate::git::bare::is_safe_component;

const MODE_EXECUTABLE: i32 = 0o100_755;
/// Git stores a symbolic link as a blob whose mode is this.
const MODE_SYMLINK: i32 = 0o120_000;

impl RepoHandle {
    /// The bytes of a regular file at `commit`, or `None` when the path is
    /// absent or names something other than a file.
    pub fn read_file_at_commit(
        &self,
        commit: Oid,
        file_path: &str,
    ) -> Result<Option<Vec<u8>>, git2::Error> {
        let tree = self.repo.find_commit(commit)?.tree()?;
        let Ok(entry) = tree.get_path(Path::new(file_path)) else {
            return Ok(None);
        };
        if entry.kind() != Some(ObjectType::Blob) || entry.filemode() == MODE_SYMLINK {
            return Ok(None);
        }
        Ok(Some(self.repo.find_blob(entry.id())?.content().to_vec()))
    }

    /// Writes the regular files of `commit` beneath `destination`. Symbolic
    /// links and submodules are skipped, and so is any entry whose name is not
    /// a plain path component, so the export cannot be steered outside
    /// `destination`.
    pub fn export_commit(&self, commit: Oid, destination: &Path) -> Result<(), String> {
        let tree = self
            .repo
            .find_commit(commit)
            .and_then(|commit| commit.tree())
            .map_err(|error| format!("Failed to read commit tree: {error}"))?;
        let mut failure = None;
        tree.walk(TreeWalkMode::PreOrder, |root, entry| {
            if entry.kind() != Some(ObjectType::Blob) || entry.filemode() == MODE_SYMLINK {
                return TreeWalkResult::Ok;
            }
            let Ok(name) = entry.name() else {
                return TreeWalkResult::Ok;
            };
            let directories: Vec<&str> = root.split('/').filter(|part| !part.is_empty()).collect();
            if !is_safe_component(name) || !directories.iter().all(|part| is_safe_component(part)) {
                return TreeWalkResult::Ok;
            }
            let result = write_blob(
                self,
                entry.id(),
                &destination.join(directories.join("/")).join(name),
                entry.filemode(),
            );
            match result {
                Ok(()) => TreeWalkResult::Ok,
                Err(error) => {
                    failure = Some(error);
                    TreeWalkResult::Abort
                }
            }
        })
        .map_err(|error| format!("Failed to walk commit tree: {error}"))?;
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn write_blob(handle: &RepoHandle, blob_id: Oid, target: &Path, mode: i32) -> Result<(), String> {
    let blob = handle
        .repo
        .find_blob(blob_id)
        .map_err(|error| format!("Failed to read blob: {error}"))?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create checkout directory: {error}"))?;
    }
    let mut file = std::fs::File::create(target)
        .map_err(|error| format!("Failed to write checkout file: {error}"))?;
    file.write_all(blob.content())
        .map_err(|error| format!("Failed to write checkout file: {error}"))?;
    #[cfg(unix)]
    if mode == MODE_EXECUTABLE {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("Failed to mark checkout file executable: {error}"))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(())
}
