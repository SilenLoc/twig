use std::path::Path;

use super::super::config::{CONFIG_FILENAMES, TwigConfig, TwigConfigWithRaw};
use super::super::history::{Commit, chrono};
use super::super::listing::{extract_license_from_cargo_toml, get_non_commercial_license};
use super::{BranchInfo, RepoFiles, RepoHandle, TreeEntry};

impl RepoHandle {
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

    /// Return the branch name selected by symbolic HEAD, if HEAD points to a
    /// local branch.
    pub fn default_branch_name(&self) -> Result<Option<String>, git2::Error> {
        let head = match self.repo.find_reference("HEAD") {
            Ok(head) => head,
            Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let target = head.symbolic_target()?;
        Ok(target
            .and_then(|target| target.strip_prefix("refs/heads/"))
            .map(str::to_string))
    }

    /// List exact local branch names. Remote branches and tags are excluded.
    pub fn list_branches(&self) -> Result<Vec<BranchInfo>, git2::Error> {
        use git2::BranchType;

        let default = self.default_branch_name()?;
        let mut branches = Vec::new();
        for branch in self.repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            let Some(name) = branch.name()? else {
                continue;
            };
            let tip = branch.get().target();
            branches.push(BranchInfo {
                name: name.to_string(),
                tip,
                is_default: default.as_deref() == Some(name),
            });
        }
        branches.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(branches)
    }

    /// Resolve a local branch name exactly; revision expressions and tags are
    /// deliberately not accepted by API branch reads.
    pub fn branch_tip(&self, branch: &str) -> Result<Option<git2::Oid>, git2::Error> {
        use git2::BranchType;

        let reference = format!("refs/heads/{branch}");
        if !git2::Reference::is_valid_name(&reference) {
            return Err(git2::Error::from_str("Invalid branch name"));
        }
        match self.repo.find_branch(branch, BranchType::Local) {
            Ok(branch) => Ok(branch.get().target()),
            Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Return the Git mode of a path in an exact local branch.
    pub fn file_mode_at(&self, branch: &str, file_path: &str) -> Result<Option<i32>, git2::Error> {
        let Some(commit) = self.branch_commit(branch)? else {
            return Ok(None);
        };
        match commit.tree()?.get_path(Path::new(file_path)) {
            Ok(entry) => Ok(Some(entry.filemode())),
            Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn branch_commit(&self, branch: &str) -> Result<Option<git2::Commit<'_>>, git2::Error> {
        use git2::BranchType;

        let reference = format!("refs/heads/{branch}");
        if !git2::Reference::is_valid_name(&reference) {
            return Err(git2::Error::from_str("Invalid branch name"));
        }
        let branch = match self.repo.find_branch(branch, BranchType::Local) {
            Ok(branch) => branch,
            Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        match branch.get().peel_to_commit() {
            Ok(commit) => Ok(Some(commit)),
            Err(error)
                if matches!(
                    error.code(),
                    git2::ErrorCode::NotFound | git2::ErrorCode::UnbornBranch
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub fn get_commits_at(&self, branch: &str, limit: usize) -> Result<Vec<Commit>, git2::Error> {
        let Some(commit) = self.branch_commit(branch)? else {
            return Ok(Vec::new());
        };
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push(commit.id())?;
        let mut commits = Vec::new();
        for oid in revwalk.take(limit).flatten() {
            let commit = self.repo.find_commit(oid)?;
            commits.push(Commit::new(
                oid.to_string(),
                commit.author().name().unwrap_or("").to_string(),
                chrono(commit.author().when()),
                commit.message().unwrap_or("").to_string(),
            ));
        }
        Ok(commits)
    }

    pub fn read_blob_bytes_at(
        &self,
        branch: &str,
        file_path: &str,
    ) -> Result<Option<(git2::Oid, Vec<u8>)>, git2::Error> {
        let Some(commit) = self.branch_commit(branch)? else {
            return Ok(None);
        };
        let tree = commit.tree()?;
        let entry = match tree.get_path(Path::new(file_path)) {
            Ok(entry) => entry,
            Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let object = entry.to_object(&self.repo)?;
        let blob = object
            .as_blob()
            .ok_or_else(|| git2::Error::from_str("Path is not a blob"))?;
        Ok(Some((entry.id(), blob.content().to_vec())))
    }

    /// Load Twig's display configuration from the same branch used for a
    /// branch-specific API read, falling back through historic filenames.
    pub fn load_config_at(&self, branch: &str) -> Result<TwigConfigWithRaw, git2::Error> {
        for filename in CONFIG_FILENAMES {
            if let Some((_, bytes)) = self.read_blob_bytes_at(branch, filename)? {
                let content = String::from_utf8_lossy(&bytes);
                return Ok(TwigConfigWithRaw::from_source(&content, filename));
            }
        }
        Ok(TwigConfigWithRaw::default())
    }

    pub fn list_dir_at(
        &self,
        branch: &str,
        dir_path: &str,
        config: Option<&TwigConfig>,
    ) -> Result<Vec<TreeEntry>, git2::Error> {
        let Some(commit) = self.branch_commit(branch)? else {
            return Ok(Vec::new());
        };
        let mut tree = commit.tree()?;
        if !dir_path.is_empty() {
            let entry = match tree.get_path(Path::new(dir_path)) {
                Ok(entry) => entry,
                Err(error) if error.code() == git2::ErrorCode::NotFound => {
                    return Ok(Vec::new());
                }
                Err(error) => return Err(error),
            };
            let object = entry.to_object(&self.repo)?;
            let Ok(subtree) = object.into_tree() else {
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
            if config.is_some_and(|config| config.should_ignore(&path)) {
                continue;
            }
            entries.push(TreeEntry {
                name,
                path,
                is_dir: entry.kind() == Some(git2::ObjectType::Tree),
            });
        }
        entries.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(entries)
    }

    pub fn path_exists_at(&self, branch: &str, file_path: &str) -> Result<bool, git2::Error> {
        let Some(commit) = self.branch_commit(branch)? else {
            return Ok(false);
        };
        match commit.tree()?.get_path(Path::new(file_path)) {
            Ok(_) => Ok(true),
            Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(error) => Err(error),
        }
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
