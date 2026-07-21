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

pub struct RepoHandle {
    repo: git2::Repository,
}

impl RepoHandle {
    pub fn open(root: &str, namespace: &str, repo: &str) -> Result<Self, git2::Error> {
        let path = Path::new(root).join(namespace).join(repo);
        let repo = git2::Repository::open(&path)?;
        Ok(Self { repo })
    }

    fn head_commit(&self) -> Result<Option<git2::Commit<'_>>, git2::Error> {
        let head = match self.repo.head() {
            Ok(head) => head,
            Err(_) => return Ok(None),
        };
        let obj = match head.resolve() {
            Ok(resolved) => resolved.peel(git2::ObjectType::Commit)?,
            Err(_) => return Ok(None),
        };
        match obj.into_commit() {
            Ok(commit) => Ok(Some(commit)),
            Err(_) => Err(git2::Error::from_str("Couldn't find commit")),
        }
    }

    pub fn get_commits(&self, depth: Depth) -> Result<Vec<Commit>, git2::Error> {
        let commit = match self.head_commit()? {
            Some(c) => c,
            None => return Ok(Vec::new()),
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

    pub fn read_file(&self, file_path: &str) -> Result<Option<String>, git2::Error> {
        let commit = match self.head_commit()? {
            Some(c) => c,
            None => return Ok(None),
        };

        let tree = commit.tree()?;
        let entry = match tree.get_path(Path::new(file_path)) {
            Ok(entry) => entry,
            Err(_) => return Ok(None),
        };

        let object = entry.to_object(&self.repo)?;
        let blob = object
            .as_blob()
            .ok_or_else(|| git2::Error::from_str("Not a blob"))?;

        let content = String::from_utf8_lossy(blob.content());
        Ok(Some(content.to_string()))
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

    /// Check if the repository has a license file
    pub fn has_license(&self) -> bool {
        self.read_file("LICENSE.md").ok().flatten().is_some()
            || self.read_file("LICENSE").ok().flatten().is_some()
            || self.read_file("Cargo.toml").ok().flatten().is_some()
    }

    pub fn load_config_with_raw(&self) -> FigConfigWithRaw {
        if let Ok(Some(content)) = self.read_file(".fig.toml") {
            return FigConfigWithRaw {
                config: FigConfig::parse(&content),
                raw: Some(content),
                filename: Some(".fig.toml".to_string()),
            };
        }
        if let Ok(Some(content)) = self.read_file(".fig") {
            return FigConfigWithRaw {
                config: FigConfig::parse(&content),
                raw: Some(content),
                filename: Some(".fig".to_string()),
            };
        }
        FigConfigWithRaw {
            config: FigConfig::default(),
            raw: None,
            filename: None,
        }
    }

    pub fn list_files(&self, config: Option<&FigConfig>) -> Result<RepoFiles, git2::Error> {
        let commit = match self.head_commit()? {
            Some(c) => c,
            None => {
                return Ok(RepoFiles {
                    markdown_files: Vec::new(),
                });
            }
        };

        let tree = commit.tree()?;
        let mut markdown_files = Vec::new();

        fn walk_tree(
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
                            walk_tree(repo, &subtree, &path, markdown_files, config)?;
                        }
                    }
                    Some(git2::ObjectType::Blob)
                        if name.ends_with(".md") || name.ends_with(".markdown") =>
                    {
                        markdown_files.push(path);
                    }
                    _ => {}
                }
            }
            Ok(())
        }

        walk_tree(&self.repo, &tree, "", &mut markdown_files, config)?;
        markdown_files.sort();

        Ok(RepoFiles { markdown_files })
    }
}

/// Presentation configuration from `.fig.toml`
#[derive(Debug, Deserialize, Default, Clone)]
pub struct PresentConfig {
    pub files: Vec<String>,
    #[serde(flatten)]
    pub template_vars: HashMap<String, String>,
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
    /// Presentation configuration.
    #[serde(default)]
    pub present: PresentConfig,
}

pub struct FigConfigWithRaw {
    pub config: FigConfig,
    pub raw: Option<String>,
    pub filename: Option<String>,
}

impl FigConfig {
    /// Load config from `.fig.toml` file in the repository
    /// Falls back to `.fig` for backwards compatibility
    /// Opens and closes the repo each time — prefer RepoHandle::load_config_with_raw when possible
    pub fn load(root: &str, namespace: &str, repo: &str) -> Self {
        let Ok(handle) = RepoHandle::open(root, namespace, repo) else {
            return Self::default();
        };
        handle.load_config_with_raw().config
    }

    /// Parse config from TOML content
    fn parse(content: &str) -> Self {
        toml::from_str(content).unwrap_or_default()
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

pub struct Commit {
    hash: String,
    author: String,
    date: chrono::DateTime<Utc>,
    commit_message: String,
}

impl Commit {
    pub fn new(
        hash: String,
        author: String,
        date: chrono::DateTime<Utc>,
        commit_message: String,
    ) -> Self {
        Self {
            hash,
            author,
            date,
            commit_message,
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

    pub fn commit_message(&self) -> &str {
        &self.commit_message
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
        Ok(format!("License: {}", license_value))
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

pub fn get_repos_with_info(root: &str, namespace: &str) -> Result<Vec<RepoInfo>, git2::Error> {
    let path = Path::new(root).join(namespace);
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return Ok(Vec::new()),
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
        let repo = match git2::Repository::open(&repo_path) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Get last commit date
        let last_commit_date = match repo.head() {
            Ok(head) => match head.resolve() {
                Ok(resolved) => match resolved.peel(git2::ObjectType::Commit) {
                    Ok(obj) => match obj.into_commit() {
                        Ok(commit) => Some(chrono(commit.author().when())),
                        Err(_) => None,
                    },
                    Err(_) => None,
                },
                Err(_) => None,
            },
            Err(_) => None,
        };

        repos.push(RepoInfo {
            name: repo_name,
            last_commit_date,
        });
    }

    Ok(repos)
}

pub fn search_repos_with_info(
    root: &str,
    namespace: &str,
    query: &str,
) -> Result<Vec<RepoInfo>, git2::Error> {
    let query_lower = query.to_lowercase();
    let path = Path::new(root).join(namespace);
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return Ok(Vec::new()),
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
        let repo = match git2::Repository::open(&repo_path) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let last_commit_date = match repo.head() {
            Ok(head) => match head.resolve() {
                Ok(resolved) => match resolved.peel(git2::ObjectType::Commit) {
                    Ok(obj) => match obj.into_commit() {
                        Ok(commit) => Some(chrono(commit.author().when())),
                        Err(_) => None,
                    },
                    Err(_) => None,
                },
                Err(_) => None,
            },
            Err(_) => None,
        };

        repos.push(RepoInfo {
            name: repo_name,
            last_commit_date,
        });
    }

    Ok(repos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    struct TempDir<'a> {
        path: &'a Path,
    }

    impl<'a> Drop for TempDir<'a> {
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
        assert!(output.status.success(), "{:?}", output);
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
            "Branch should be 'develop' but was: {}",
            branch
        );
    }

    #[test]
    fn test_fig_config_parse() {
        let toml_content = r#"
ignore_for_view = ["skills/", "temp", "drafts/"]
deleteable = true
"#;
        let config = FigConfig::parse(toml_content);
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
        let config = FigConfig::parse(toml_content);
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
    fn test_fig_config_should_ignore_folder() {
        let config = FigConfig {
            ignore_for_view: vec!["skills".to_string()],
            tabs: vec![],
            deleteable: false,
            present: PresentConfig::default(),
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
            present: PresentConfig::default(),
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
            present: PresentConfig::default(),
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
            present: PresentConfig::default(),
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
            present: PresentConfig::default(),
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
            present: PresentConfig::default(),
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
        let config = FigConfig::parse(toml_content);
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
        let now_secs = chrono::Utc::now().timestamp() as i64;
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
        };
        assert_eq!(info.name, "test-repo");
        assert!(info.last_commit_date.is_none());
    }
}
