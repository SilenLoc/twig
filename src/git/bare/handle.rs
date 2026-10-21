//! Opening a repository and reading from its HEAD tree: file contents,
//! directory listings, Markdown discovery, and configuration loading.

use std::path::Path;

use super::path::is_safe_component;

/// Result of listing repository files
#[derive(Default)]
pub struct RepoFiles {
    pub markdown_files: Vec<String>,
}

/// A single entry in a repository directory listing
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

pub struct RepoHandle {
    repo: git2::Repository,
}

impl RepoHandle {
    /// The on-disk Git directory used by administrative repository views.
    pub fn path(&self) -> &Path {
        self.repo.path()
    }

    /// Takes ownership of an already opened repository, so namespace listings
    /// can reuse the handle they needed for the HEAD peek.
    pub(super) fn from_repository(repo: git2::Repository) -> Self {
        Self { repo }
    }

    pub fn open(root: &str, namespace: &str, repo: &str) -> Result<Self, git2::Error> {
        if !is_safe_component(namespace) || !is_safe_component(repo) {
            return Err(git2::Error::from_str(
                "Invalid namespace or repository name",
            ));
        }
        let namespace_path = crate::git::repo::checked_namespace_path(Path::new(root), namespace)
            .map_err(|error| git2::Error::from_str(&error))?;
        let path = namespace_path.join(repo);
        if std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(git2::Error::from_str(
                "Repository path must not be a symlink",
            ));
        }
        let repo = git2::Repository::open(&path)?;
        Ok(Self { repo })
    }
}

/// One final-tree operation from a repository edit draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoEditChange {
    Upsert { path: String, content: Vec<u8> },
    Delete { path: String },
}

/// Result of the compare-and-swap branch update for a repository edit draft.
pub enum CommitChangesOutcome {
    Committed(git2::Oid),
    Conflict,
}

/// Result of the compare-and-swap branch update for a test-only single-file edit.
#[cfg(test)]
pub enum CommitFileOutcome {
    Committed(git2::Oid),
    Conflict,
}

mod commit;
mod read;

#[cfg(test)]
#[path = "handle/tests.rs"]
mod edit_tests;
