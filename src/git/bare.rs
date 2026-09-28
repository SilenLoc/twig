use std::collections::HashMap;
use std::path::Path;

use chrono::Utc;
use serde::Deserialize;

pub mod namespace {
    use std::path::Path;

    pub fn has_any_repository(root: &str, namespace: &str) -> bool {
        let namespace_dir = Path::new(root).join(namespace);
        if !namespace_dir.is_dir() {
            return false;
        }
        namespace_dir
            .read_dir()
            .is_ok_and(|mut entries| entries.next().is_some())
    }
}

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

/// A path component is safe when it is non-empty and does not reference a
/// parent directory or contain separators or NUL bytes.
pub fn is_safe_component(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

/// Checks whether a repository-relative path is safe to resolve inside a
/// repository. Rejects absolute paths, ".", "..", empty components, NUL
/// bytes and Windows-style separators so callers cannot escape the
/// repository tree through crafted URLs.
pub fn is_safe_repo_path(path: &str) -> bool {
    if path.starts_with('/') || path.contains('\0') {
        return false;
    }
    if path.is_empty() {
        return true;
    }
    path.split('/').all(is_safe_component)
}

pub struct RepoHandle {
    repo: git2::Repository,
}

impl RepoHandle {
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

    pub fn get_commits(&self, depth: &Depth) -> Result<Vec<Commit>, git2::Error> {
        let Some(commit) = self.head_commit()? else {
            return Ok(Vec::new());
        };

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push(commit.id())?;

        let mut commits = Vec::new();
        revwalk.take(depth.depth).for_each(|oid_result| {
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

    /// Get license content with fallback logic:
    /// 1. Try LICENSE.md
    /// 2. Try LICENSE
    /// 3. Try to extract from Cargo.toml
    /// 4. Fall back to non-commercial license
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

        // Fall back to non-commercial license
        get_non_commercial_license()
    }

    pub fn load_config_with_raw(&self) -> FigConfigWithRaw {
        if let Ok(Some(content)) = self.read_file(".fig.toml") {
            return FigConfigWithRaw::from_source(&content, ".fig.toml");
        }
        if let Ok(Some(content)) = self.read_file(".fig") {
            return FigConfigWithRaw::from_source(&content, ".fig");
        }
        FigConfigWithRaw::default()
    }

    pub fn list_files(&self, config: Option<&FigConfig>) -> Result<RepoFiles, git2::Error> {
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
        config: Option<&FigConfig>,
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

/// Presentation configuration from `.fig.toml`
#[derive(Debug, Deserialize, Default, Clone)]
pub struct PresentConfig {
    pub files: Vec<String>,
    #[serde(flatten)]
    pub template_vars: HashMap<String, String>,
}

/// Paper (long-form reading) configuration from `.fig.toml`.
///
/// The section is optional; a repository only offers the Paper tab when
/// `[paper]` is present and its directory holds at least one Markdown page.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct PaperConfig {
    /// Directory holding the paper's Markdown pages, read in sorted order.
    #[serde(default = "default_paper_dir")]
    pub dir: String,
}

impl PaperConfig {
    /// Whether the configured directory is usable as a page source.
    pub fn is_configured(&self) -> bool {
        !self.dir.trim().trim_matches('/').is_empty()
    }
}

fn default_paper_dir() -> String {
    "paper".to_string()
}

/// Configuration from `.fig.toml` file in repository
#[derive(Debug, Deserialize, Default)]
pub struct FigConfig {
    #[serde(default)]
    pub ignore_for_view: Vec<String>,
    /// Tabs to display. If empty or not present, all tabs are shown.
    #[serde(default)]
    pub tabs: Vec<String>,
    /// Whether the repository can be deleted from the UI.
    #[serde(default)]
    pub deleteable: bool,
    /// Whether the repository is private (read operations require authentication).
    #[serde(default)]
    pub private: bool,
    /// Presentation configuration.
    #[serde(default)]
    pub present: PresentConfig,
    /// Paper (long-form reading) configuration. Present only when `[paper]`
    /// appears in the configuration file.
    #[serde(default)]
    pub paper: Option<PaperConfig>,
}

/// A repository's parsed configuration alongside its source.
#[derive(Default)]
pub struct FigConfigWithRaw {
    pub config: FigConfig,
    pub raw: Option<String>,
    pub filename: Option<String>,
    /// The parse error, when `raw` is present but invalid. `config` then holds
    /// defaults so the rest of the UI keeps working around the bad file.
    pub error: Option<String>,
}

impl FigConfigWithRaw {
    /// Parses `content` and records a human-readable error instead of silently
    /// discarding it, so the UI can point at the offending line.
    fn from_source(content: &str, filename: &str) -> Self {
        match FigConfig::parse(content) {
            Ok(config) => Self {
                config,
                raw: Some(content.to_string()),
                filename: Some(filename.to_string()),
                error: None,
            },
            Err(error) => Self {
                config: FigConfig::default(),
                raw: Some(content.to_string()),
                filename: Some(filename.to_string()),
                error: Some(error.to_string()),
            },
        }
    }
}

impl FigConfig {
    /// Load config from `.fig.toml` file in the repository
    /// Falls back to `.fig` for backwards compatibility
    /// Opens and closes the repo each time — prefer `RepoHandle::load_config_with_raw` when possible
    pub fn load(root: &str, namespace: &str, repo: &str) -> Self {
        let Ok(handle) = RepoHandle::open(root, namespace, repo) else {
            return Self::default();
        };
        handle.load_config_with_raw().config
    }

    /// Parse config from TOML content. The error is returned rather than
    /// swallowed so callers can surface it.
    fn parse(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    /// Check if a file path matches any of the ignore patterns
    pub fn should_ignore(&self, file_path: &str) -> bool {
        let file_path = file_path.trim_start_matches("./");

        for pattern in &self.ignore_for_view {
            // Check if the file path starts with the pattern (for folder patterns)
            // or matches exactly (for file patterns)
            if pattern.ends_with('/') {
                // Folder pattern (e.g., "skills/")
                let pattern_prefix = pattern.trim_end_matches('/');
                if file_path.starts_with(pattern_prefix)
                    && (file_path.len() == pattern_prefix.len()
                        || file_path[pattern_prefix.len()..].starts_with('/'))
                {
                    return true;
                }
            } else if pattern.contains('/') {
                // Path pattern with subdirectories (e.g., "docs/temp")
                if file_path.starts_with(pattern)
                    && (file_path.len() == pattern.len()
                        || file_path[pattern.len()..].starts_with('/'))
                {
                    return true;
                }
            } else {
                // Simple file or folder name pattern
                // Check if any path component matches
                if file_path.split('/').any(|component| component == pattern) {
                    return true;
                }
            }
        }
        false
    }
}

/// Recursively collects the markdown files of `tree` into `markdown_files`,
/// skipping anything `config` ignores.
fn collect_markdown_files(
    repo: &git2::Repository,
    tree: &git2::Tree,
    prefix: &str,
    markdown_files: &mut Vec<String>,
    config: Option<&FigConfig>,
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

pub struct Commit {
    hash: String,
    author: String,
    date: chrono::DateTime<Utc>,
    message: String,
}

impl Commit {
    pub fn new(hash: String, author: String, date: chrono::DateTime<Utc>, message: String) -> Self {
        Self {
            hash,
            author,
            date,
            message,
        }
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn date(&self) -> &chrono::DateTime<Utc> {
        &self.date
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub struct Depth {
    pub depth: usize,
}

impl Depth {
    pub fn new(depth: usize) -> Self {
        Self { depth }
    }
}

impl Default for Depth {
    fn default() -> Self {
        Self::new(1000)
    }
}

fn chrono(git_time: git2::Time) -> chrono::DateTime<Utc> {
    chrono::DateTime::from_timestamp(git_time.seconds(), 0).unwrap_or(chrono::DateTime::UNIX_EPOCH)
}

pub struct RepoInfo {
    pub name: String,
    pub last_commit_date: Option<chrono::DateTime<Utc>>,
    pub is_private: bool,
}

/// Extract license from Cargo.toml content
fn extract_license_from_cargo_toml(cargo_content: &str) -> Result<String, ()> {
    // Parse the Cargo.toml to extract the license field
    // We'll do a simple parsing since we only need the license field
    let mut in_package = false;
    let mut found_license = false;
    let mut license_value = String::new();

    for line in cargo_content.lines() {
        let trimmed = line.trim();

        // Check if we're entering the [package] section
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }

        // Check if we're leaving the [package] section
        if trimmed.starts_with('[') && trimmed != "[package]" {
            in_package = false;
        }

        // If we're in the package section, look for license
        if in_package && trimmed.starts_with("license") {
            // Handle both "license = " and 'license = ' formats
            if let Some(eq_pos) = trimmed.find('=') {
                let value_part = trimmed[eq_pos + 1..].trim();
                // Remove quotes if present
                let clean_value = value_part.trim_matches('"').trim_matches('\'');
                if !clean_value.is_empty() {
                    license_value = clean_value.to_string();
                    found_license = true;
                    break;
                }
            }
        }
    }

    if found_license && !license_value.is_empty() {
        Ok(format!("License: {license_value}"))
    } else {
        Err(())
    }
}

/// Get the non-commercial license fallback text
fn get_non_commercial_license() -> String {
    r#"Non-Commercial License

This software is provided under a non-commercial license with the following restrictions:

1. **Non-Commercial Use Only**: This software and its source code may only be used for non-commercial purposes. Any commercial use, including but not limited to sale, licensing, or use in commercial products or services, is strictly prohibited.

2. **No LLM Training**: This software and its source code may not be used for training large language models (LLMs), machine learning models, or any other form of AI/ML training, whether commercial or non-commercial.

3. **No Redistribution for Training**: You may not distribute, share, or make this software available to others for the purpose of AI/ML training.

4. **Permitted Uses**: You may use this software for personal, educational, research (non-AI/ML), and other non-commercial purposes, provided you comply with all other restrictions.

5. **No Warranty**: This software is provided "AS IS" without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose, and non-infringement.

Violation of any of these terms will result in immediate termination of your rights to use this software."#.to_string()
}

/// Date of the commit `HEAD` points at, or `None` when the repository has no
/// resolvable HEAD commit (a freshly initialised repository, for instance).
fn last_commit_date(repo: &git2::Repository) -> Option<chrono::DateTime<Utc>> {
    let head = repo.head().ok()?;
    let resolved = head.resolve().ok()?;
    let obj = resolved.peel(git2::ObjectType::Commit).ok()?;
    let commit = obj.into_commit().ok()?;
    Some(chrono(commit.author().when()))
}

pub fn get_repos_with_info(root: &str, namespace: &str) -> Vec<RepoInfo> {
    let path = Path::new(root).join(namespace);
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };

    let mut repos = Vec::new();

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };

        let Ok(repo_name) = entry.file_name().into_string() else {
            continue;
        };

        let repo_path = Path::new(root).join(namespace).join(&repo_name);
        let Ok(repo) = git2::Repository::open(&repo_path) else {
            continue;
        };

        let last_commit = last_commit_date(&repo);
        let handle = RepoHandle { repo };
        let is_private = handle.load_config_with_raw().config.private;

        repos.push(RepoInfo {
            name: repo_name,
            last_commit_date: last_commit,
            is_private,
        });
    }

    repos
}

pub fn search_repos_with_info(root: &str, namespace: &str, query: &str) -> Vec<RepoInfo> {
    let query_lower = query.to_lowercase();
    let path = Path::new(root).join(namespace);
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };

    let mut repos = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };

        let Ok(repo_name) = entry.file_name().into_string() else {
            continue;
        };

        if !repo_name.to_lowercase().contains(&query_lower) {
            continue;
        }

        let repo_path = Path::new(root).join(namespace).join(&repo_name);
        let Ok(repo) = git2::Repository::open(&repo_path) else {
            continue;
        };

        let last_commit = last_commit_date(&repo);
        let handle = RepoHandle { repo };
        let is_private = handle.load_config_with_raw().config.private;

        repos.push(RepoInfo {
            name: repo_name,
            last_commit_date: last_commit,
            is_private,
        });
    }

    repos
}

/// Check if a repository is configured as private in its `.fig.toml` or `.fig`.
pub fn is_repo_private(root: &str, namespace: &str, repo: &str) -> bool {
    let clean_repo = repo.strip_suffix(".git").unwrap_or(repo);
    if let Ok(handle) = RepoHandle::open(root, namespace, clean_repo)
        && handle.load_config_with_raw().config.private
    {
        return true;
    }
    if clean_repo != repo
        && let Ok(handle) = RepoHandle::open(root, namespace, repo)
        && handle.load_config_with_raw().config.private
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    struct TempDir<'a> {
        path: &'a Path,
    }

    impl Drop for TempDir<'_> {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.path);
        }
    }

    fn create_temp_dir(name: &str) -> PathBuf {
        let temp = std::env::temp_dir().join(format!(
            "fig_test_{}_{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        temp
    }

    fn init_bare_repo(path: &Path, branch: &str) {
        std::fs::create_dir_all(path).unwrap();
        let output = Command::new("git")
            .args(["init", "--bare", "--initial-branch", branch])
            .current_dir(path)
            .output()
            .expect("Failed to init bare repo");
        assert!(output.status.success(), "{output:?}");
    }

    #[test]
    fn test_init_branch_parameter() {
        let temp = create_temp_dir("branch");
        let _cleanup = TempDir { path: &temp };

        let bare_path = temp.join("custom-branch.git");
        init_bare_repo(&bare_path, "develop");

        // Verify the branch name
        let output = Command::new("git")
            .args(["symbolic-ref", "HEAD"])
            .current_dir(&bare_path)
            .output()
            .expect("Failed to get branch name");

        let branch = String::from_utf8_lossy(&output.stdout);
        assert!(
            branch.contains("develop"),
            "Branch should be 'develop' but was: {branch}"
        );
    }

    #[test]
    fn test_fig_config_parse() {
        let toml_content = r#"
ignore_for_view = ["skills/", "temp", "drafts/"]
deleteable = true
"#;
        let config = FigConfig::parse(toml_content).expect("valid config");
        assert_eq!(config.ignore_for_view.len(), 3);
        assert!(config.ignore_for_view.contains(&"skills/".to_string()));
        assert!(config.ignore_for_view.contains(&"temp".to_string()));
        assert!(config.ignore_for_view.contains(&"drafts/".to_string()));
        assert!(config.deleteable);
    }

    #[test]
    fn test_present_config_parse() {
        let toml_content = r#"
[present]
files = ["slides/intro.md", "slides/conclusion.md"]
author = "Jane Doe"
"#;
        let config = FigConfig::parse(toml_content).expect("valid config");
        assert_eq!(config.present.files.len(), 2);
        assert!(
            config
                .present
                .files
                .contains(&"slides/intro.md".to_string())
        );
        assert_eq!(
            config.present.template_vars.get("author").unwrap(),
            "Jane Doe"
        );
    }

    #[test]
    fn test_paper_config_parse() {
        let config = FigConfig::parse("[paper]\ndir = \"manuscript\"\n").expect("valid config");
        let paper = config.paper.expect("paper section should be present");
        assert_eq!(paper.dir, "manuscript");
        assert!(paper.is_configured());

        // An empty section falls back to the default directory.
        let config = FigConfig::parse("[paper]\n").expect("valid config");
        assert_eq!(config.paper.expect("paper section").dir, "paper");

        // Without the section the Paper tab stays off.
        assert!(
            FigConfig::parse("private = true")
                .expect("valid config")
                .paper
                .is_none()
        );
    }

    #[test]
    fn test_paper_config_empty_dir_is_not_configured() {
        let config = FigConfig::parse("[paper]\ndir = \"  \"\n").expect("valid config");
        assert!(!config.paper.expect("paper section").is_configured());
    }

    #[test]
    fn test_config_parse_errors_are_reported_not_swallowed() {
        let parsed = FigConfigWithRaw::from_source("not = = valid\n", ".fig.toml");
        assert!(parsed.error.is_some(), "the parse error must be kept");
        assert!(
            parsed.config.paper.is_none() && !parsed.config.deleteable,
            "a broken file falls back to defaults so the UI keeps rendering"
        );
        assert_eq!(parsed.filename.as_deref(), Some(".fig.toml"));
        assert_eq!(parsed.raw.as_deref(), Some("not = = valid\n"));

        let parsed = FigConfigWithRaw::from_source("private = true\n", ".fig.toml");
        assert!(parsed.error.is_none());
        assert!(parsed.config.private);

        let parsed = FigConfigWithRaw::default();
        assert!(parsed.error.is_none() && parsed.raw.is_none());
    }

    #[test]
    fn test_fig_config_should_ignore_folder() {
        let config = FigConfig {
            ignore_for_view: vec!["skills".to_string()],
            tabs: vec![],
            deleteable: false,
            private: false,
            present: PresentConfig::default(),
            paper: None,
        };

        // Should ignore files in the skills folder
        assert!(config.should_ignore("skills/README.md"));
        assert!(config.should_ignore("skills/guide.md"));
        assert!(config.should_ignore("skills/nested/file.md"));

        // Should not ignore files outside skills folder
        assert!(!config.should_ignore("README.md"));
        assert!(!config.should_ignore("docs/guide.md"));
        assert!(!config.should_ignore("my-skills.md"));
    }

    #[test]
    fn test_fig_config_should_ignore_with_slash_pattern() {
        let config = FigConfig {
            ignore_for_view: vec!["drafts/".to_string()],
            tabs: vec![],
            deleteable: false,
            private: false,
            present: PresentConfig::default(),
            paper: None,
        };

        // Should ignore files in the drafts folder
        assert!(config.should_ignore("drafts/temp.md"));
        assert!(config.should_ignore("drafts/ideas/file.md"));

        // Should not ignore files outside drafts folder
        assert!(!config.should_ignore("README.md"));
        assert!(!config.should_ignore("final/drafts.md"));
    }

    #[test]
    fn test_fig_config_should_ignore_multiple_patterns() {
        let config = FigConfig {
            ignore_for_view: vec!["temp".to_string(), "archive".to_string()],
            tabs: vec![],
            deleteable: false,
            private: false,
            present: PresentConfig::default(),
            paper: None,
        };

        // Should ignore files in temp
        assert!(config.should_ignore("temp/file.md"));

        // Should ignore files in archive
        assert!(config.should_ignore("archive/old.md"));

        // Should not ignore other files
        assert!(!config.should_ignore("README.md"));
    }

    #[test]
    fn test_fig_config_empty() {
        let config = FigConfig {
            ignore_for_view: vec![],
            tabs: vec![],
            deleteable: false,
            private: false,
            present: PresentConfig::default(),
            paper: None,
        };

        assert!(!config.should_ignore("any/file.md"));
        assert!(!config.should_ignore("README.md"));
    }

    #[test]
    fn test_fig_config_should_ignore_folder_with_trailing_slash() {
        // Test that "skills/" pattern correctly ignores the skills folder
        let config = FigConfig {
            ignore_for_view: vec!["skills/".to_string()],
            tabs: vec![],
            deleteable: false,
            private: false,
            present: PresentConfig::default(),
            paper: None,
        };

        // The folder itself should be ignored
        assert!(config.should_ignore("skills"));

        // Files in the folder should be ignored
        assert!(config.should_ignore("skills/README.md"));
        assert!(config.should_ignore("skills/nested/file.md"));

        // Files outside should not be ignored
        assert!(!config.should_ignore("README.md"));
        assert!(!config.should_ignore("my-skills.md"));
        assert!(!config.should_ignore("other/skills/file.md")); // 'skills' is not at root
    }

    #[test]
    fn test_fig_config_should_ignore_exact_file() {
        // Test that "AGENTS.md" pattern correctly ignores any file with that name
        // (simple patterns match any path component)
        let config = FigConfig {
            ignore_for_view: vec!["AGENTS.md".to_string()],
            tabs: vec![],
            deleteable: false,
            private: false,
            present: PresentConfig::default(),
            paper: None,
        };

        // Root level AGENTS.md should be ignored
        assert!(config.should_ignore("AGENTS.md"));

        // AGENTS.md in subfolders SHOULD also be ignored (simple pattern matches any component)
        assert!(config.should_ignore("docs/AGENTS.md"));
        assert!(config.should_ignore("a/b/c/AGENTS.md"));

        // Other files should not be ignored
        assert!(!config.should_ignore("README.md"));
        assert!(!config.should_ignore("OTHER_AGENTS.md"));
    }

    #[test]
    fn test_fig_config_toml_format_parsing() {
        let toml_content = r#"
# Fig Configuration File
ignore_for_view = ["skills/", "AGENTS.md"]
"#;
        let config = FigConfig::parse(toml_content).expect("valid config");
        assert_eq!(config.ignore_for_view.len(), 2);
        assert!(config.ignore_for_view.contains(&"skills/".to_string()));
        assert!(config.ignore_for_view.contains(&"AGENTS.md".to_string()));

        assert!(config.should_ignore("skills"));
        assert!(config.should_ignore("skills/file.md"));
        assert!(config.should_ignore("AGENTS.md"));
        assert!(!config.should_ignore("README.md"));
    }

    #[test]
    fn test_chrono_handles_valid_timestamp() {
        let time = git2::Time::new(0, 0);
        let result = chrono(time);
        assert_eq!(result.timestamp(), 0);
    }

    #[test]
    fn test_chrono_handles_current_time() {
        let now_secs = chrono::Utc::now().timestamp();
        let time = git2::Time::new(now_secs, 0);
        let result = chrono(time);
        assert_eq!(result.timestamp(), now_secs);
    }

    #[test]
    fn test_fig_config_config_filename_order() {
        let config = FigConfig::default();
        assert_eq!(config.ignore_for_view.len(), 0);
    }

    #[test]
    fn test_repo_info_has_name_and_date() {
        let info = RepoInfo {
            name: "test-repo".to_string(),
            last_commit_date: None,
            is_private: false,
        };
        assert_eq!(info.name, "test-repo");
        assert!(info.last_commit_date.is_none());
        assert!(!info.is_private);
    }

    #[test]
    fn test_fig_config_parse_private() {
        let toml_content = r"
private = true
";
        let config = FigConfig::parse(toml_content).expect("valid config");
        assert!(config.private);

        let default_config = FigConfig::parse("").expect("empty config is valid");
        assert!(!default_config.private);
    }

    #[test]
    fn test_is_repo_private() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let temp = create_temp_dir("is_private");
        let _cleanup = TempDir { path: &temp };

        let repo_path = temp.join("ns").join("secret");
        std::fs::create_dir_all(&repo_path).unwrap();
        init_bare_repo(&repo_path, "main");

        let git_pipe = |args: &[&str], input: &str| -> String {
            let mut child = Command::new("git")
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .current_dir(&repo_path)
                .spawn()
                .unwrap();
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(input.as_bytes())
                .unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "{args:?}: {out:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };

        let blob = git_pipe(&["hash-object", "-w", "--stdin"], "private = true\n");
        let root_hash = git_pipe(&["mktree"], &format!("100644 blob {blob}\t.fig.toml\n"));
        let commit = Command::new("git")
            .args(["commit-tree", &root_hash, "-m", "set private"])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .current_dir(&repo_path)
            .output()
            .unwrap();
        assert!(commit.status.success());
        let commit_hash = String::from_utf8_lossy(&commit.stdout).trim().to_string();

        let update_ref = Command::new("git")
            .args(["update-ref", "refs/heads/main", &commit_hash])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        assert!(update_ref.status.success());

        assert!(is_repo_private(temp.to_str().unwrap(), "ns", "secret"));
        assert!(is_repo_private(temp.to_str().unwrap(), "ns", "secret.git"));
        assert!(!is_repo_private(
            temp.to_str().unwrap(),
            "ns",
            "nonexistent"
        ));

        let repos = get_repos_with_info(temp.to_str().unwrap(), "ns");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "secret");
        assert!(repos[0].is_private);
    }

    #[test]
    fn test_list_dir_walks_head_tree() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let temp = create_temp_dir("list_dir");
        let _cleanup = TempDir { path: &temp };

        let repo_path = temp.join("pub").join("repo");
        std::fs::create_dir_all(&repo_path).unwrap();
        init_bare_repo(&repo_path, "main");

        // Helper: pipe stdin through a git command in the bare repo
        let git_pipe = |args: &[&str], input: &str| -> String {
            let mut child = Command::new("git")
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .current_dir(&repo_path)
                .spawn()
                .unwrap();
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(input.as_bytes())
                .unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "{args:?}: {out:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };

        // Create blobs, a subtree and the root tree via plumbing
        let blob1 = git_pipe(&["hash-object", "-w", "--stdin"], "# Readme\n");
        let blob2 = git_pipe(&["hash-object", "-w", "--stdin"], "inner\n");
        let tree_hash = git_pipe(&["mktree"], &format!("100644 blob {blob2}\tinner.txt\n"));
        let root_hash = git_pipe(
            &["mktree"],
            &format!("100644 blob {blob1}\tREADME.md\n040000 tree {tree_hash}\tsrc\n"),
        );

        let commit = Command::new("git")
            .args(["commit-tree", &root_hash, "-m", "test commit"])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .current_dir(&repo_path)
            .output()
            .unwrap();
        assert!(commit.status.success(), "{commit:?}");
        let commit_hash = String::from_utf8_lossy(&commit.stdout).trim().to_string();

        Command::new("git")
            .args(["update-ref", "refs/heads/main", &commit_hash])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let handle = RepoHandle::open(temp.to_str().unwrap(), "pub", "repo").unwrap();

        let root = handle.list_dir("", None).unwrap();
        assert_eq!(root.len(), 2);
        assert_eq!(root[0].name, "src");
        assert!(root[0].is_dir);
        assert!(!root[1].is_dir);
        assert_eq!(root[1].name, "README.md");

        let src = handle.list_dir("src", None).unwrap();
        assert_eq!(src.len(), 1);
        assert!(!src[0].is_dir);
        assert_eq!(src[0].path, "src/inner.txt");

        let missing = handle.list_dir("nope", None).unwrap();
        assert!(missing.is_empty());
    }

    #[test]
    fn test_list_paper_pages_reads_sorted_markdown_under_dir() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let temp = create_temp_dir("paper_pages");
        let _cleanup = TempDir { path: &temp };

        let repo_path = temp.join("pub").join("book");
        std::fs::create_dir_all(&repo_path).unwrap();
        init_bare_repo(&repo_path, "main");

        let git_pipe = |args: &[&str], input: &str| -> String {
            let mut child = Command::new("git")
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .current_dir(&repo_path)
                .spawn()
                .unwrap();
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(input.as_bytes())
                .unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "{args:?}: {out:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };

        let one = git_pipe(&["hash-object", "-w", "--stdin"], "# One\n");
        let two = git_pipe(&["hash-object", "-w", "--stdin"], "# Two\n");
        let three = git_pipe(&["hash-object", "-w", "--stdin"], "# Three\n");
        let extra = git_pipe(&["mktree"], &format!("100644 blob {three}\t03.md\n"));
        let paper = git_pipe(
            &["mktree"],
            &format!(
                "100644 blob {one}\t01.md\n100644 blob {two}\t02.md\n040000 tree {extra}\textra\n"
            ),
        );
        let root = git_pipe(
            &["mktree"],
            &format!("100644 blob {one}\tREADME.md\n040000 tree {paper}\tpaper\n"),
        );

        let commit = Command::new("git")
            .args(["commit-tree", &root, "-m", "paper"])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .current_dir(&repo_path)
            .output()
            .unwrap();
        assert!(commit.status.success(), "{commit:?}");
        let commit_hash = String::from_utf8_lossy(&commit.stdout).trim().to_string();
        Command::new("git")
            .args(["update-ref", "refs/heads/main", &commit_hash])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let handle = RepoHandle::open(temp.to_str().unwrap(), "pub", "book").unwrap();
        assert_eq!(
            handle.list_paper_pages("paper").unwrap(),
            vec![
                "paper/01.md".to_string(),
                "paper/02.md".to_string(),
                "paper/extra/03.md".to_string(),
            ]
        );
        assert!(handle.list_paper_pages("missing").unwrap().is_empty());
        assert!(
            handle
                .list_paper_pages("")
                .unwrap()
                .contains(&"README.md".to_string()),
            "an empty dir lists the whole repository"
        );
    }

    #[test]
    fn test_open_rejects_parent_directory_components() {
        let temp = create_temp_dir("open_guard");
        let _cleanup = TempDir { path: &temp };

        assert!(RepoHandle::open(temp.to_str().unwrap(), "..", "repo").is_err());
        assert!(RepoHandle::open(temp.to_str().unwrap(), "ns", "..").is_err());
        assert!(RepoHandle::open(temp.to_str().unwrap(), "", "repo").is_err());
        assert!(RepoHandle::open(temp.to_str().unwrap(), ".", "repo").is_err());
        assert!(
            RepoHandle::open(temp.to_str().unwrap(), "a/b", "repo").is_err(),
            "namespace must not contain separators"
        );
    }

    #[test]
    fn test_is_safe_repo_path() {
        // Safe paths
        assert!(is_safe_repo_path(""));
        assert!(is_safe_repo_path("README.md"));
        assert!(is_safe_repo_path("src/main.rs"));
        assert!(is_safe_repo_path("docs/guide/intro.md"));

        // Traversal attempts
        assert!(!is_safe_repo_path("../secret"));
        assert!(!is_safe_repo_path("src/../secret"));
        assert!(!is_safe_repo_path(".."));
        assert!(!is_safe_repo_path("."));
        assert!(!is_safe_repo_path("src/.."));
        assert!(!is_safe_repo_path("/etc/passwd"));
        assert!(!is_safe_repo_path("//etc/passwd"));
        assert!(!is_safe_repo_path("a//b"));
        assert!(!is_safe_repo_path("file\0.txt"));
        assert!(!is_safe_repo_path("dir\\..\\secret"));
    }
}
