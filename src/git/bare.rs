use std::path::Path;

use chrono::Utc;
use serde::Deserialize;

/// Configuration from `.fig.toml` file in repository
#[derive(Debug, Deserialize, Default)]
pub struct FigConfig {
    #[serde(default)]
    pub ignore_for_view: Vec<String>,
}

impl FigConfig {
    /// Load config from `.fig.toml` file in the repository
    pub fn load(root: &str, namespace: &str, repo: &str) -> Self {
        match read_file(root, namespace, repo, ".fig.toml") {
            Ok(Some(content)) => Self::parse(&content),
            _ => Self::default(),
        }
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

pub fn get_commits(
    root: &str,
    namespace: &str,
    repo: &str,
    depth: Depth,
) -> Result<Vec<Commit>, git2::Error> {
    git_commits(root, namespace, repo, depth)
}

fn git_commits(
    root: &str,
    namespace: &str,
    repo: &str,
    depth: Depth,
) -> Result<Vec<Commit>, git2::Error> {
    let path = Path::new(root).join(namespace).join(repo);

    let repo = git2::Repository::open(&path)?;

    // Get HEAD commit (handle unborn branch - no commits yet)
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => return Ok(Vec::new()), // No HEAD yet (empty repo)
    };

    // Check if this is an unborn branch (HEAD exists but points to non-existent ref)
    let obj = match head.resolve() {
        Ok(resolved) => resolved.peel(git2::ObjectType::Commit)?,
        Err(_) => return Ok(Vec::new()), // Unborn branch - no commits yet
    };

    let commit = obj
        .into_commit()
        .map_err(|_| git2::Error::from_str("Couldn't find commit"))?;

    // Create a revision walker
    let mut revwalk = repo.revwalk()?;
    revwalk.push(commit.id())?;

    let mut commits = Vec::new();

    revwalk.take(depth.depth).for_each(|oid_result| {
        let Ok(oid) = oid_result else {
            return;
        };

        let Ok(commit) = repo.find_commit(oid) else {
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

fn chrono(git_time: git2::Time) -> chrono::DateTime<Utc> {
    chrono::DateTime::from_timestamp(git_time.seconds(), 0).unwrap()
}

pub struct RepoInfo {
    pub name: String,
    pub last_commit_date: Option<chrono::DateTime<Utc>>,
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
    let all_repos = get_repos_with_info(root, namespace)?;
    let query_lower = query.to_lowercase();

    Ok(all_repos
        .into_iter()
        .filter(|repo| repo.name.to_lowercase().contains(&query_lower))
        .collect())
}

/// Reads a file from the repository at the given path
pub fn read_file(
    root: &str,
    namespace: &str,
    repo: &str,
    file_path: &str,
) -> Result<Option<String>, git2::Error> {
    let path = Path::new(root).join(namespace).join(repo);
    let repo = git2::Repository::open(&path)?;

    // Get HEAD commit (handle unborn branch - no commits yet)
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => return Ok(None), // No HEAD yet (empty repo)
    };

    // Check if this is an unborn branch (HEAD exists but points to non-existent ref)
    let obj = match head.resolve() {
        Ok(resolved) => match resolved.peel(git2::ObjectType::Commit) {
            Ok(obj) => obj,
            Err(_) => return Ok(None), // Can't peel to commit
        },
        Err(_) => return Ok(None), // Unborn branch - no commits yet
    };

    let commit = obj
        .into_commit()
        .map_err(|_| git2::Error::from_str("Couldn't find commit"))?;

    let tree = commit.tree()?;

    // Try to find the file
    let entry = match tree.get_path(Path::new(file_path)) {
        Ok(entry) => entry,
        Err(_) => return Ok(None),
    };

    let object = entry.to_object(&repo)?;
    let blob = object
        .as_blob()
        .ok_or_else(|| git2::Error::from_str("Not a blob"))?;

    let content = String::from_utf8_lossy(blob.content());
    Ok(Some(content.to_string()))
}

/// Lists all markdown files in the repository, optionally filtering based on config
pub fn list_markdown_files(
    root: &str,
    namespace: &str,
    repo: &str,
    config: Option<&FigConfig>,
) -> Result<Vec<String>, git2::Error> {
    let path = Path::new(root).join(namespace).join(repo);
    let repo = git2::Repository::open(&path)?;

    // Get HEAD commit (handle unborn branch - no commits yet)
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => return Ok(Vec::new()), // No HEAD yet (empty repo)
    };

    // Check if this is an unborn branch (HEAD exists but points to non-existent ref)
    let obj = match head.resolve() {
        Ok(resolved) => match resolved.peel(git2::ObjectType::Commit) {
            Ok(obj) => obj,
            Err(_) => return Ok(Vec::new()), // Can't peel to commit
        },
        Err(_) => return Ok(Vec::new()), // Unborn branch - no commits yet
    };

    let commit = obj
        .into_commit()
        .map_err(|_| git2::Error::from_str("Couldn't find commit"))?;

    let tree = commit.tree()?;
    let mut markdown_files = Vec::new();

    // Walk the tree recursively to find all .md files
    fn walk_tree(
        repo: &git2::Repository,
        tree: &git2::Tree,
        prefix: &str,
        files: &mut Vec<String>,
        config: Option<&FigConfig>,
    ) -> Result<(), git2::Error> {
        for entry in tree {
            let name = entry.name().unwrap_or("");
            let path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", prefix, name)
            };

            // Skip this entry if it matches ignore patterns
            if let Some(config) = config
                && config.should_ignore(&path)
            {
                continue;
            }

            match entry.kind() {
                Some(git2::ObjectType::Tree) => {
                    let obj = entry.to_object(repo)?;
                    if let Ok(subtree) = obj.into_tree() {
                        walk_tree(repo, &subtree, &path, files, config)?;
                    }
                }
                Some(git2::ObjectType::Blob) => {
                    if name.ends_with(".md") || name.ends_with(".markdown") {
                        files.push(path);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    walk_tree(&repo, &tree, "", &mut markdown_files, config)?;
    markdown_files.sort();

    Ok(markdown_files)
}

/// Commits a file to a non-bare repository (used for testing)
#[allow(dead_code)]
pub fn commit_file(
    repo_path: &Path,
    file_path: &str,
    content: &str,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> Result<String, git2::Error> {
    let repo = git2::Repository::open(repo_path)?;

    // Write the file
    let full_path = repo_path.join(file_path);
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| git2::Error::from_str(&format!("Failed to create directory: {}", e)))?;
    }
    std::fs::write(&full_path, content)
        .map_err(|e| git2::Error::from_str(&format!("Failed to write file: {}", e)))?;

    // Add the file to the index
    let mut index = repo.index()?;
    index.add_path(Path::new(file_path))?;
    index.write()?;

    // Create signature
    let sig = git2::Signature::now(author_name, author_email)?;

    // Get the tree
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    // Get parent commit if exists
    let parents = match repo.head() {
        Ok(head) => {
            let parent_commit = head.resolve()?.peel_to_commit()?;
            vec![parent_commit]
        }
        Err(_) => vec![],
    };

    // Create the commit
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    let commit_id = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)?;

    Ok(commit_id.to_string())
}

/// Push a local repository to a bare repository
#[allow(dead_code)]
pub fn push_to_bare(
    local_repo_path: &Path,
    bare_repo_path: &Path,
    branch: &str,
) -> Result<(), String> {
    use xshell::cmd;

    let sh = xshell::Shell::new().map_err(|e| e.to_string())?;
    sh.change_dir(local_repo_path);

    // Add the bare repo as remote
    let bare_path_str = bare_repo_path.to_str().ok_or("Invalid path")?;
    let _ = cmd!(sh, "git remote add origin {bare_path_str}").run();

    // Push to the bare repo
    cmd!(sh, "git push -u origin {branch}")
        .run()
        .map_err(|e| e.to_string())?;

    Ok(())
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

    fn init_repo(path: &Path, branch: &str) {
        std::fs::create_dir_all(path).unwrap();
        let output = Command::new("git")
            .args(["init", "--initial-branch", branch])
            .current_dir(path)
            .output()
            .expect("Failed to init repo");
        assert!(output.status.success(), "{:?}", output);

        // Configure git user
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    #[test]
    fn test_readme_is_displayed() {
        let temp = create_temp_dir("readme");
        let _cleanup = TempDir { path: &temp };

        let bare_path = temp.join("bare.git");
        let local_path = temp.join("local");

        init_bare_repo(&bare_path, "main");
        init_repo(&local_path, "main");

        // Create and commit README
        let readme_content = "# Test Repository\n\nThis is a **test** README file.\n\n> Generated by Fig test suite for validating README display functionality.";
        commit_file(
            &local_path,
            "README.md",
            readme_content,
            "Initial commit with README",
            "Test User",
            "test@example.com",
        )
        .expect("Failed to commit README");

        // Push to bare repo
        push_to_bare(&local_path, &bare_path, "main").expect("Failed to push");

        // Read the README from bare repo (bare.git is at root of temp, no namespace)
        let result = read_file(temp.to_str().unwrap(), "", "bare.git", "README.md");

        assert!(result.is_ok(), "Failed to read README: {:?}", result.err());
        let content = result.unwrap();
        assert!(content.is_some(), "README should be found");

        let content = content.unwrap();
        assert!(
            content.contains("# Test Repository"),
            "Content should contain '# Test Repository', but got: {}",
            content
        );
        assert!(
            content.contains("test") && content.contains("README"),
            "Content should contain 'test' and 'README', but got: {}",
            content
        );
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
    fn test_read_readme_variants() {
        let temp = create_temp_dir("readme_variants");
        let _cleanup = TempDir { path: &temp };

        let local_path = temp.join("local");
        init_repo(&local_path, "main");

        // Test with lowercase readme.md
        commit_file(
            &local_path,
            "readme.md",
            "# lowercase readme\n\n> Generated by Fig test suite for validating case-insensitive README detection.",
            "Add lowercase readme",
            "Test",
            "test@test.com",
        )
        .unwrap();

        // Verify we can read it back
        let result = read_file(temp.to_str().unwrap(), "", "local", "readme.md");
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.is_some());
        let content = content.unwrap();
        assert!(content.contains("# lowercase readme"));
    }

    #[test]
    fn test_fig_config_parse() {
        let toml_content = r#"
ignore_for_view = ["skills", "temp", "drafts/"]
"#;
        let config = FigConfig::parse(toml_content);
        assert_eq!(config.ignore_for_view.len(), 3);
        assert!(config.ignore_for_view.contains(&"skills".to_string()));
        assert!(config.ignore_for_view.contains(&"temp".to_string()));
        assert!(config.ignore_for_view.contains(&"drafts/".to_string()));
    }

    #[test]
    fn test_fig_config_should_ignore_folder() {
        let config = FigConfig {
            ignore_for_view: vec!["skills".to_string()],
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
        };

        assert!(!config.should_ignore("any/file.md"));
        assert!(!config.should_ignore("README.md"));
    }

    #[test]
    fn test_list_markdown_files_with_config_filter() {
        let temp = create_temp_dir("config_filter");
        let _cleanup = TempDir { path: &temp };

        let bare_path = temp.join("bare.git");
        let local_path = temp.join("local");

        init_bare_repo(&bare_path, "main");
        init_repo(&local_path, "main");

        // Create markdown files in different folders
        commit_file(
            &local_path,
            "README.md",
            "# Main README",
            "Add README",
            "Test",
            "test@test.com",
        )
        .unwrap();

        commit_file(
            &local_path,
            "skills/rust.md",
            "# Rust Skills",
            "Add rust skills",
            "Test",
            "test@test.com",
        )
        .unwrap();

        commit_file(
            &local_path,
            "skills/python.md",
            "# Python Skills",
            "Add python skills",
            "Test",
            "test@test.com",
        )
        .unwrap();

        commit_file(
            &local_path,
            "docs/guide.md",
            "# Guide",
            "Add guide",
            "Test",
            "test@test.com",
        )
        .unwrap();

        // Push to bare repo
        push_to_bare(&local_path, &bare_path, "main").expect("Failed to push");

        // Test without config - should get all files
        let config = FigConfig {
            ignore_for_view: vec![],
        };
        let files = list_markdown_files(temp.to_str().unwrap(), "", "bare.git", Some(&config))
            .expect("Failed to list files");
        assert_eq!(files.len(), 4);
        assert!(files.contains(&"README.md".to_string()));
        assert!(files.contains(&"skills/rust.md".to_string()));
        assert!(files.contains(&"skills/python.md".to_string()));
        assert!(files.contains(&"docs/guide.md".to_string()));

        // Test with config filtering "skills"
        let config = FigConfig {
            ignore_for_view: vec!["skills".to_string()],
        };
        let files = list_markdown_files(temp.to_str().unwrap(), "", "bare.git", Some(&config))
            .expect("Failed to list files");
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"README.md".to_string()));
        assert!(files.contains(&"docs/guide.md".to_string()));
        assert!(!files.contains(&"skills/rust.md".to_string()));
        assert!(!files.contains(&"skills/python.md".to_string()));
    }
}
