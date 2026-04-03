use std::path::Path;

use chrono::Utc;

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

pub fn get_namespaces(root: &str) -> Result<Vec<String>, git2::Error> {
    let path = Path::new(root);
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return Ok(Vec::new()), // Directory doesn't exist or can't be read
    };
    let mut namespaces = Vec::new();

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };

        let Ok(namespace) = entry.file_name().into_string() else {
            continue;
        };
        namespaces.push(namespace);
    }

    Ok(namespaces)
}

pub fn get_repos(root: &str, namespace: &str) -> Result<Vec<String>, git2::Error> {
    let path = Path::new(root).join(namespace);
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return Ok(Vec::new()), // Directory doesn't exist or can't be read
    };
    let mut repos = Vec::new();

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };

        let Ok(repo) = entry.file_name().into_string() else {
            continue;
        };

        // check if they are repos
        let repo_path = Path::new(root).join(namespace).join(&repo);
        let is_repo = git2::Repository::open(&repo_path).is_ok();

        if !is_repo {
            continue;
        }

        repos.push(repo);
    }

    Ok(repos)
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

/// Reads README file from the repository (tries common README filenames)
pub fn read_readme(
    root: &str,
    namespace: &str,
    repo: &str,
) -> Result<Option<(String, String)>, git2::Error> {
    let readme_names = [
        "README.md",
        "Readme.md",
        "readme.md",
        "README.markdown",
        "README",
        "Readme",
        "readme",
    ];

    for name in &readme_names {
        if let Some(content) = read_file(root, namespace, repo, name)? {
            return Ok(Some((name.to_string(), content)));
        }
    }

    Ok(None)
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
        let result = read_readme(temp.to_str().unwrap(), "", "bare.git");

        assert!(result.is_ok(), "Failed to read README: {:?}", result.err());
        let readme = result.unwrap();
        assert!(readme.is_some(), "README should be found");

        let (filename, content) = readme.unwrap();
        assert_eq!(filename, "README.md");
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
        let result = read_readme(temp.to_str().unwrap(), "", "local");
        assert!(result.is_ok());
        let readme = result.unwrap();
        assert!(readme.is_some());
        assert_eq!(readme.unwrap().0, "readme.md");
    }
}
