//! Namespace listings: the repositories inside one namespace, with the
//! presentation flags the UI shows next to each row.

use std::path::Path;

use chrono::Utc;

use super::handle::RepoHandle;
use super::history::chrono;

/// One repository row in a namespace listing.
pub struct RepoInfo {
    pub name: String,
    pub last_commit_date: Option<chrono::DateTime<Utc>>,
    pub is_private: bool,
}

/// Extract license from Cargo.toml content
pub(super) fn extract_license_from_cargo_toml(cargo_content: &str) -> Result<String, ()> {
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
pub(super) fn get_non_commercial_license() -> String {
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
        let handle = RepoHandle::from_repository(repo);
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
        let handle = RepoHandle::from_repository(repo);
        let is_private = handle.load_config_with_raw().config.private;

        repos.push(RepoInfo {
            name: repo_name,
            last_commit_date: last_commit,
            is_private,
        });
    }

    repos
}

/// Check if a repository is configured as private in its `.twig.toml`, `.twig`,
/// `.fig.toml` or `fig.toml`.
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
    use crate::git::bare::config::CONFIG_FILENAMES;

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

    /// Keeps the config filename list honest: dropping a historic name from
    /// `CONFIG_FILENAMES` would silently un-private older repositories.
    #[test]
    fn test_config_filenames_still_cover_the_historic_names() {
        assert_eq!(
            CONFIG_FILENAMES,
            [".twig.toml", ".twig", ".fig.toml", "fig.toml"]
        );
    }
}
