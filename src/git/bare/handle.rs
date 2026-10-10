//! Opening a repository and reading from its HEAD tree: file contents,
//! directory listings, Markdown discovery, and configuration loading.

use std::path::Path;

use super::config::{CONFIG_FILENAMES, TwigConfig, TwigConfigWithRaw};
use super::history::{Commit, chrono};
use super::listing::{extract_license_from_cargo_toml, get_non_commercial_license};
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
        let path = Path::new(root).join(namespace).join(repo);
        let repo = git2::Repository::open(&path)?;
        Ok(Self { repo })
    }

    fn head_commit(&self) -> Result<Option<git2::Commit<'_>>, git2::Error> {
        let Ok(head) = self.repo.head() else {
            return Ok(None);
        };
        let Ok(resolved) = head.resolve() else {
            return Ok(None);
        };
        let obj = resolved.peel(git2::ObjectType::Commit)?;
        match obj.into_commit() {
            Ok(commit) => Ok(Some(commit)),
            Err(_) => Err(git2::Error::from_str("Couldn't find commit")),
        }
    }

    /// The resolved commit at HEAD, if this repository has one.
    pub fn head_oid(&self) -> Result<Option<git2::Oid>, git2::Error> {
        Ok(self.head_commit()?.map(|commit| commit.id()))
    }

    /// The Git mode of a path in HEAD, if it exists.
    pub fn file_mode(&self, file_path: &str) -> Result<Option<i32>, git2::Error> {
        let Some(commit) = self.head_commit()? else {
            return Ok(None);
        };
        let tree = commit.tree()?;
        match tree.get_path(Path::new(file_path)) {
            Ok(entry) => Ok(Some(entry.filemode())),
            Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Create a commit replacing one regular file and move the current branch
    /// only if it still points at `expected_head`. `reference_matching` is a
    /// compare-and-swap, so a concurrent push/edit cannot be silently lost.
    pub fn commit_file(
        &self,
        file_path: &str,
        content: &[u8],
        expected_head: git2::Oid,
        author_name: &str,
        author_email: &str,
        message: &str,
    ) -> Result<CommitFileOutcome, git2::Error> {
        use git2::{FileMode, ObjectType, Signature, build::TreeUpdateBuilder};

        if !super::path::is_safe_repo_path(file_path) || file_path.is_empty() {
            return Err(git2::Error::from_str("Invalid repository file path"));
        }

        let head = self.repo.head()?;
        let reference_name = head
            .name()
            .ok()
            .filter(|name| name.starts_with("refs/heads/"))
            .ok_or_else(|| git2::Error::from_str("Repository HEAD is not a branch"))?;
        let parent = self.repo.find_commit(expected_head)?;
        let parent_tree = parent.tree()?;
        let entry = parent_tree.get_path(Path::new(file_path))?;
        if entry.kind() != Some(ObjectType::Blob)
            || !matches!(entry.filemode(), 0o100_644 | 0o100_755)
        {
            return Err(git2::Error::from_str(
                "Only regular repository files can be edited",
            ));
        }

        let blob_oid = self.repo.blob(content)?;
        let mut update = TreeUpdateBuilder::new();
        let mode = if entry.filemode() == 0o100_755 {
            FileMode::BlobExecutable
        } else {
            FileMode::Blob
        };
        update.upsert(file_path, blob_oid, mode);
        let tree_oid = update.create_updated(&self.repo, &parent_tree)?;
        let tree = self.repo.find_tree(tree_oid)?;
        let author = Signature::now(author_name, author_email)?;
        let commit_oid = self
            .repo
            .commit(None, &author, &author, message, &tree, &[&parent])?;

        match self.repo.reference_matching(
            reference_name,
            commit_oid,
            true,
            expected_head,
            "Edit file in Twig",
        ) {
            Ok(_) => Ok(CommitFileOutcome::Committed(commit_oid)),
            Err(error) if error.code() == git2::ErrorCode::Modified => {
                Ok(CommitFileOutcome::Conflict)
            }
            Err(error) => Err(error),
        }
    }

    /// Create a new regular file and advance the current branch only if it
    /// still points at `expected_head`. Existing paths are never replaced.
    pub fn commit_new_file(
        &self,
        file_path: &str,
        content: &[u8],
        expected_head: git2::Oid,
        author_name: &str,
        author_email: &str,
        message: &str,
    ) -> Result<CommitFileOutcome, git2::Error> {
        use git2::{FileMode, Signature, build::TreeUpdateBuilder};

        if !super::path::is_safe_repo_path(file_path) || file_path.is_empty() {
            return Err(git2::Error::from_str("Invalid repository file path"));
        }

        let head = self.repo.head()?;
        let reference_name = head
            .name()
            .ok()
            .filter(|name| name.starts_with("refs/heads/"))
            .ok_or_else(|| git2::Error::from_str("Repository HEAD is not a branch"))?;
        let parent = self.repo.find_commit(expected_head)?;
        let parent_tree = parent.tree()?;
        match parent_tree.get_path(Path::new(file_path)) {
            Ok(_) => return Ok(CommitFileOutcome::Conflict),
            Err(error) if error.code() == git2::ErrorCode::NotFound => {}
            Err(error) => return Err(error),
        }

        let blob_oid = self.repo.blob(content)?;
        let mut update = TreeUpdateBuilder::new();
        update.upsert(file_path, blob_oid, FileMode::Blob);
        let tree_oid = update.create_updated(&self.repo, &parent_tree)?;
        let tree = self.repo.find_tree(tree_oid)?;
        let author = Signature::now(author_name, author_email)?;
        let commit_oid = self
            .repo
            .commit(None, &author, &author, message, &tree, &[&parent])?;

        match self.repo.reference_matching(
            reference_name,
            commit_oid,
            true,
            expected_head,
            "Save conflict copy in Twig",
        ) {
            Ok(_) => Ok(CommitFileOutcome::Committed(commit_oid)),
            Err(error) if error.code() == git2::ErrorCode::Modified => {
                Ok(CommitFileOutcome::Conflict)
            }
            Err(error) => Err(error),
        }
    }

    pub fn get_commits(&self, limit: usize) -> Result<Vec<Commit>, git2::Error> {
        let Some(commit) = self.head_commit()? else {
            return Ok(Vec::new());
        };

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push(commit.id())?;

        let mut commits = Vec::new();
        revwalk.take(limit).for_each(|oid_result| {
            let Ok(oid) = oid_result else {
                return;
            };
            let Ok(commit) = self.repo.find_commit(oid) else {
                return;
            };
            commits.push(Commit::new(
                oid.to_string(),
                commit.author().name().unwrap_or("").to_string(),
                chrono(commit.author().when()),
                commit.message().unwrap_or("").to_string(),
            ));
        });

        Ok(commits)
    }

    /// Read the raw bytes of a blob at the given path in HEAD
    pub fn read_blob_bytes(&self, file_path: &str) -> Result<Option<Vec<u8>>, git2::Error> {
        let Some(commit) = self.head_commit()? else {
            return Ok(None);
        };

        let tree = commit.tree()?;
        let Ok(entry) = tree.get_path(Path::new(file_path)) else {
            return Ok(None);
        };

        let object = entry.to_object(&self.repo)?;
        let blob = object
            .as_blob()
            .ok_or_else(|| git2::Error::from_str("Not a blob"))?;

        Ok(Some(blob.content().to_vec()))
    }

    pub fn read_file(&self, file_path: &str) -> Result<Option<String>, git2::Error> {
        match self.read_blob_bytes(file_path)? {
            Some(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).to_string())),
            None => Ok(None),
        }
    }

    /// Whether a path exists in the HEAD tree, without reading its contents.
    pub fn path_exists(&self, file_path: &str) -> bool {
        self.head_commit().ok().flatten().is_some_and(|commit| {
            commit
                .tree()
                .and_then(|tree| tree.get_path(Path::new(file_path)).map(|_| ()))
                .is_ok()
        })
    }

    /// Get license content with fallback logic:
    /// 1. Try LICENSE.md
    /// 2. Try LICENSE
    /// 3. Try to extract from Cargo.toml
    /// 4. Fall back to the non-commercial license, which covers the author's
    ///    own content (code and prose) but no third-party material
    pub fn get_license_content(&self) -> String {
        // Try LICENSE.md first
        if let Ok(Some(content)) = self.read_file("LICENSE.md") {
            return content;
        }

        // Try LICENSE
        if let Ok(Some(content)) = self.read_file("LICENSE") {
            return content;
        }

        // Try to extract from Cargo.toml
        if let Ok(Some(cargo_content)) = self.read_file("Cargo.toml")
            && let Ok(license) = extract_license_from_cargo_toml(&cargo_content)
        {
            return license;
        }

        // Fall back to the non-commercial license
        get_non_commercial_license()
    }

    pub fn load_config_with_raw(&self) -> TwigConfigWithRaw {
        for filename in CONFIG_FILENAMES {
            if let Ok(Some(content)) = self.read_file(filename) {
                return TwigConfigWithRaw::from_source(&content, filename);
            }
        }
        TwigConfigWithRaw::default()
    }

    pub fn list_files(&self, config: Option<&TwigConfig>) -> Result<RepoFiles, git2::Error> {
        let Some(commit) = self.head_commit()? else {
            return Ok(RepoFiles {
                markdown_files: Vec::new(),
            });
        };

        let tree = commit.tree()?;
        let mut markdown_files = Vec::new();

        collect_markdown_files(&self.repo, &tree, "", &mut markdown_files, config)?;
        markdown_files.sort();

        Ok(RepoFiles { markdown_files })
    }

    /// List the entries of a directory inside the HEAD tree.
    /// An empty path lists the repository root. Directories sort first,
    /// then files, each alphabetically. Ignored paths are skipped.
    pub fn list_dir(
        &self,
        dir_path: &str,
        config: Option<&TwigConfig>,
    ) -> Result<Vec<TreeEntry>, git2::Error> {
        let Some(commit) = self.head_commit()? else {
            return Ok(Vec::new());
        };

        let mut tree = commit.tree()?;
        if !dir_path.is_empty() {
            let Ok(entry) = tree.get_path(Path::new(dir_path)) else {
                return Ok(Vec::new());
            };
            let obj = entry.to_object(&self.repo)?;
            let Ok(subtree) = obj.into_tree() else {
                return Ok(Vec::new());
            };
            tree = subtree;
        }

        let mut entries = Vec::new();
        for entry in &tree {
            let name = entry.name().unwrap_or("").to_string();
            let path = if dir_path.is_empty() {
                name.clone()
            } else {
                format!("{dir_path}/{name}")
            };

            if let Some(config) = config
                && config.should_ignore(&path)
            {
                continue;
            }

            let is_dir = entry.kind() == Some(git2::ObjectType::Tree);
            entries.push(TreeEntry { name, path, is_dir });
        }

        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
        Ok(entries)
    }

    /// List the Markdown pages under `dir` in HEAD, sorted for reading order.
    /// An empty or missing directory yields no pages.
    pub fn list_paper_pages(&self, dir: &str) -> Result<Vec<String>, git2::Error> {
        let dir = dir.trim_matches('/');
        let Some(commit) = self.head_commit()? else {
            return Ok(Vec::new());
        };

        let mut tree = commit.tree()?;
        if !dir.is_empty() {
            let Ok(entry) = tree.get_path(Path::new(dir)) else {
                return Ok(Vec::new());
            };
            let obj = entry.to_object(&self.repo)?;
            let Ok(subtree) = obj.into_tree() else {
                return Ok(Vec::new());
            };
            tree = subtree;
        }

        let mut pages = Vec::new();
        collect_markdown_files(&self.repo, &tree, dir, &mut pages, None)?;
        pages.sort();
        Ok(pages)
    }
}

/// Result of the compare-and-swap branch update for a web edit.
pub enum CommitFileOutcome {
    Committed(git2::Oid),
    Conflict,
}

/// Recursively collects the markdown files of `tree` into `markdown_files`,
/// skipping anything `config` ignores.
fn collect_markdown_files(
    repo: &git2::Repository,
    tree: &git2::Tree,
    prefix: &str,
    markdown_files: &mut Vec<String>,
    config: Option<&TwigConfig>,
) -> Result<(), git2::Error> {
    for entry in tree {
        let name = entry.name().unwrap_or("");
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };

        if let Some(config) = config
            && config.should_ignore(&path)
        {
            continue;
        }

        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                let obj = entry.to_object(repo)?;
                if let Ok(subtree) = obj.into_tree() {
                    collect_markdown_files(repo, &subtree, &path, markdown_files, config)?;
                }
            }
            Some(git2::ObjectType::Blob) if crate::md::is_markdown(name) => {
                markdown_files.push(path);
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod edit_tests {
    use super::*;
    use std::path::PathBuf;

    struct TempRepo(PathBuf);

    impl TempRepo {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("twig_editor_repo_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(path.join("ns/repo")).unwrap();
            Self(path)
        }

        fn handle(&self) -> RepoHandle {
            let path = self.0.join("ns/repo");
            let repo = git2::Repository::init_bare(path).unwrap();
            let blob = repo.blob(b"before\n").unwrap();
            {
                let mut root = repo.treebuilder(None).unwrap();
                root.insert("README.md", blob, 0o100_644).unwrap();
                let mut nested = repo.treebuilder(None).unwrap();
                let nested_blob = repo.blob(b"nested before\n").unwrap();
                nested.insert("file.txt", nested_blob, 0o100_755).unwrap();
                let nested_tree = nested.write().unwrap();
                root.insert("nested", nested_tree, 0o040_000).unwrap();
                let tree_oid = root.write().unwrap();
                let tree = repo.find_tree(tree_oid).unwrap();
                let signature = git2::Signature::now("Seed", "seed@example.com").unwrap();
                repo.commit(
                    Some("refs/heads/main"),
                    &signature,
                    &signature,
                    "seed",
                    &tree,
                    &[],
                )
                .unwrap();
                repo.set_head("refs/heads/main").unwrap();
            }
            RepoHandle::open(self.0.to_str().unwrap(), "ns", "repo").unwrap()
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn commit_file_writes_nested_content_and_preserves_other_entries_and_mode() {
        let temp = TempRepo::new();
        let handle = temp.handle();
        let expected = handle.head_oid().unwrap().unwrap();

        let outcome = handle
            .commit_file(
                "nested/file.txt",
                b"after\n",
                expected,
                "Editor",
                "editor@example.com",
                "Update nested file",
            )
            .unwrap();
        let CommitFileOutcome::Committed(oid) = outcome else {
            panic!("the unchanged branch should accept the edit");
        };

        assert_eq!(
            handle
                .read_blob_bytes("nested/file.txt")
                .unwrap()
                .as_deref(),
            Some(&b"after\n"[..])
        );
        assert_eq!(
            handle.read_blob_bytes("README.md").unwrap().as_deref(),
            Some(&b"before\n"[..])
        );
        let repo = git2::Repository::open(temp.0.join("ns/repo")).unwrap();
        let commit = repo.find_commit(oid).unwrap();
        assert_eq!(commit.author().name().unwrap(), "Editor");
        assert_eq!(commit.author().email().unwrap(), "editor@example.com");
        let entry = commit
            .tree()
            .unwrap()
            .get_path(Path::new("nested/file.txt"))
            .unwrap();
        assert_eq!(entry.filemode(), 0o100_755);
    }

    #[test]
    fn commit_file_refuses_to_overwrite_a_newer_branch_tip() {
        let temp = TempRepo::new();
        let handle = temp.handle();
        let original = handle.head_oid().unwrap().unwrap();
        let first = handle
            .commit_file(
                "README.md",
                b"first edit\n",
                original,
                "Editor",
                "editor@example.com",
                "First edit",
            )
            .unwrap();
        let CommitFileOutcome::Committed(first_oid) = first else {
            panic!("the first edit should commit");
        };

        let second = handle
            .commit_file(
                "README.md",
                b"stale edit\n",
                original,
                "Editor",
                "editor@example.com",
                "Stale edit",
            )
            .unwrap();
        assert!(matches!(second, CommitFileOutcome::Conflict));
        assert_eq!(handle.head_oid().unwrap(), Some(first_oid));
        assert_eq!(
            handle.read_blob_bytes("README.md").unwrap().as_deref(),
            Some(&b"first edit\n"[..])
        );
    }

    #[test]
    fn commit_new_file_adds_a_nested_path_and_refuses_overwrites_and_stale_heads() {
        let temp = TempRepo::new();
        let handle = temp.handle();
        let original = handle.head_oid().unwrap().unwrap();

        let created = handle
            .commit_new_file(
                "docs/alice-20261010-123456-README.md",
                b"draft copy\n",
                original,
                "Editor",
                "editor@example.com",
                "Save conflict copy",
            )
            .unwrap();
        let CommitFileOutcome::Committed(copy_oid) = created else {
            panic!("the copy should commit against an unchanged branch");
        };
        assert_eq!(
            handle
                .read_blob_bytes("docs/alice-20261010-123456-README.md")
                .unwrap()
                .as_deref(),
            Some(&b"draft copy\n"[..])
        );
        assert_eq!(
            handle.read_blob_bytes("README.md").unwrap().as_deref(),
            Some(&b"before\n"[..])
        );
        assert!(matches!(
            handle
                .commit_new_file(
                    "docs/alice-20261010-123456-README.md",
                    b"must not replace\n",
                    copy_oid,
                    "Editor",
                    "editor@example.com",
                    "Duplicate copy",
                )
                .unwrap(),
            CommitFileOutcome::Conflict
        ));

        let moved = handle
            .commit_file(
                "README.md",
                b"concurrent update\n",
                copy_oid,
                "Other",
                "other@example.com",
                "Concurrent change",
            )
            .unwrap();
        let CommitFileOutcome::Committed(moved_oid) = moved else {
            panic!("the concurrent update should commit");
        };
        assert!(matches!(
            handle
                .commit_new_file(
                    "docs/stale-copy.md",
                    b"stale draft\n",
                    copy_oid,
                    "Editor",
                    "editor@example.com",
                    "Stale copy",
                )
                .unwrap(),
            CommitFileOutcome::Conflict
        ));
        assert_eq!(handle.head_oid().unwrap(), Some(moved_oid));
        assert_eq!(handle.read_blob_bytes("docs/stale-copy.md").unwrap(), None);
        assert_eq!(
            handle
                .read_blob_bytes("docs/alice-20261010-123456-README.md")
                .unwrap()
                .as_deref(),
            Some(&b"draft copy\n"[..])
        );
    }
}
