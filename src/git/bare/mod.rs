//! Bare-repository access: opening repositories, reading the HEAD tree, the
//! `.twig.toml` configuration, commit history, and namespace listings.
//!
//! Split into focused submodules; everything public is re-exported here so
//! callers keep using the flat `crate::git::bare::…` paths.

pub mod config;
mod handle;
mod history;
mod listing;
mod path;

pub use config::{PresentConfig, ScriptEntry, ScriptGroupNode, TwigConfig, TwigConfigWithRaw};
pub use handle::{RepoHandle, TreeEntry};
pub use history::{Commit, MAX_COMMITS};
pub use listing::{
    RepoInfo, get_repos_with_info, is_repo_private, search_repo_names, search_repos_with_info,
};
pub use path::{is_safe_component, is_safe_repo_path};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
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
            "twig_test_{}_{}",
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

    /// Commits the given `(filename, content)` entries as blobs into `repo_path`'s
    /// `main` branch, using git plumbing so no working tree is required. Nested
    /// paths build their subtrees bottom-up, since `mktree` rejects slashes.
    fn commit_files(repo_path: &Path, files: &[(&str, &str)]) {
        use std::fmt::Write as _;
        use std::io::Write;
        use std::process::Stdio;

        // Writes the tree holding `files`, whose paths all live under one
        // directory: blobs straight into `mktree`, deeper paths into a
        // recursive subtree named by their first path segment.
        fn build_tree(
            git_pipe: &impl Fn(&[&str], &str) -> String,
            files: &[(&str, &str)],
        ) -> String {
            let mut entries = String::new();
            let mut nested: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();
            for (name, content) in files {
                match name.split_once('/') {
                    None => {
                        let blob = git_pipe(&["hash-object", "-w", "--stdin"], content);
                        let _ = writeln!(entries, "100644 blob {blob}\t{name}");
                    }
                    Some((dir, rest)) => {
                        match nested.iter_mut().find(|(prefix, _)| *prefix == dir) {
                            Some((_, group)) => group.push((rest, *content)),
                            None => nested.push((dir, vec![(rest, *content)])),
                        }
                    }
                }
            }
            for (dir, group) in &nested {
                let subtree = build_tree(git_pipe, group);
                let _ = writeln!(entries, "040000 tree {subtree}\t{dir}");
            }
            git_pipe(&["mktree"], &entries)
        }

        let git_pipe = |args: &[&str], input: &str| -> String {
            let mut child = Command::new("git")
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .current_dir(repo_path)
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

        let root_hash = build_tree(&git_pipe, files);

        let commit = Command::new("git")
            .args(["commit-tree", &root_hash, "-m", "config"])
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .current_dir(repo_path)
            .output()
            .unwrap();
        assert!(commit.status.success());
        let commit_hash = String::from_utf8_lossy(&commit.stdout).trim().to_string();

        let update_ref = Command::new("git")
            .args(["update-ref", "refs/heads/main", &commit_hash])
            .current_dir(repo_path)
            .output()
            .unwrap();
        assert!(update_ref.status.success());
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
    fn test_config_fallback_reads_fig_toml() {
        let temp = create_temp_dir("config_fig_fallback");
        let _cleanup = TempDir { path: &temp };

        let repo_path = temp.join("ns").join("old-repo");
        init_bare_repo(&repo_path, "main");
        commit_files(&repo_path, &[("fig.toml", "private = true\n")]);

        let handle = RepoHandle::open(temp.to_str().unwrap(), "ns", "old-repo").unwrap();
        let loaded = handle.load_config_with_raw();
        assert_eq!(loaded.filename.as_deref(), Some("fig.toml"));
        assert!(loaded.error.is_none());
        assert!(loaded.config.private);
    }

    #[test]
    fn test_config_fallback_reads_dot_fig_toml() {
        let temp = create_temp_dir("config_dot_fig_fallback");
        let _cleanup = TempDir { path: &temp };

        let repo_path = temp.join("ns").join("old-repo");
        init_bare_repo(&repo_path, "main");
        commit_files(&repo_path, &[(".fig.toml", "private = true\n")]);

        let handle = RepoHandle::open(temp.to_str().unwrap(), "ns", "old-repo").unwrap();
        let loaded = handle.load_config_with_raw();
        assert_eq!(loaded.filename.as_deref(), Some(".fig.toml"));
        assert!(loaded.config.private);
    }

    #[test]
    fn test_config_prefers_current_name_over_fig() {
        let temp = create_temp_dir("config_prefers_current");
        let _cleanup = TempDir { path: &temp };

        let repo_path = temp.join("ns").join("both");
        init_bare_repo(&repo_path, "main");
        commit_files(
            &repo_path,
            &[
                (".twig.toml", "deleteable = true\n"),
                (".fig.toml", "private = true\n"),
                ("fig.toml", "private = true\n"),
            ],
        );

        let handle = RepoHandle::open(temp.to_str().unwrap(), "ns", "both").unwrap();
        let loaded = handle.load_config_with_raw();
        assert_eq!(loaded.filename.as_deref(), Some(".twig.toml"));
        assert!(loaded.config.deleteable);
        assert!(!loaded.config.private, "the .twig.toml file must win");
    }

    #[test]
    fn test_config_prefers_twig_short_name_over_fig() {
        let temp = create_temp_dir("config_prefers_short");
        let _cleanup = TempDir { path: &temp };

        let repo_path = temp.join("ns").join("short");
        init_bare_repo(&repo_path, "main");
        commit_files(
            &repo_path,
            &[
                (".twig", "deleteable = true\n"),
                (".fig.toml", "private = true\n"),
            ],
        );

        let handle = RepoHandle::open(temp.to_str().unwrap(), "ns", "short").unwrap();
        let loaded = handle.load_config_with_raw();
        assert_eq!(loaded.filename.as_deref(), Some(".twig"));
        assert!(loaded.config.deleteable);
        assert!(!loaded.config.private, "the .twig file must win");
    }

    #[test]
    fn test_is_repo_private() {
        let temp = create_temp_dir("is_private");
        let _cleanup = TempDir { path: &temp };

        let repo_path = temp.join("ns").join("secret");
        std::fs::create_dir_all(&repo_path).unwrap();
        init_bare_repo(&repo_path, "main");
        commit_files(&repo_path, &[(".twig.toml", "private = true\n")]);

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
        let temp = create_temp_dir("list_dir");
        let _cleanup = TempDir { path: &temp };

        let repo_path = temp.join("pub").join("repo");
        std::fs::create_dir_all(&repo_path).unwrap();
        init_bare_repo(&repo_path, "main");
        commit_files(
            &repo_path,
            &[("README.md", "# Readme\n"), ("src/inner.txt", "inner\n")],
        );

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
        let temp = create_temp_dir("paper_pages");
        let _cleanup = TempDir { path: &temp };

        let repo_path = temp.join("pub").join("book");
        std::fs::create_dir_all(&repo_path).unwrap();
        init_bare_repo(&repo_path, "main");
        commit_files(
            &repo_path,
            &[
                ("README.md", "# One\n"),
                ("paper/01.md", "# One\n"),
                ("paper/02.md", "# Two\n"),
                ("paper/extra/03.md", "# Three\n"),
            ],
        );

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
}
