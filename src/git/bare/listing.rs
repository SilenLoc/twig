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

/// Get the non-commercial license fallback text. It covers everything in the
/// repository the author created — code and prose alike — and explicitly
/// leaves third-party content under its own license.
pub(super) fn get_non_commercial_license() -> String {
    r#"Non-Commercial License

This license covers everything in this repository that its author created, not just the software: source code, documentation, papers and notes, slides and presentations, data, images, and other original material (together, the "Works"). Third-party content is not covered — see clause 2.

1. **Non-Commercial Use Only**: The Works may only be used for non-commercial purposes. Any commercial use, including but not limited to sale, licensing, or use in commercial products or services, is strictly prohibited.

2. **Third-Party Content**: Vendored dependencies, submodules, datasets, fonts, and any other third-party material included in the repository remain under their own licenses and terms. This license grants no rights to that content, and using it is subject to its own license regardless of what this one allows.

3. **No AI/ML Training**: The Works may not be used for training large language models (LLMs), machine learning models, or for any other form of AI/ML training or fine-tuning, whether commercial or non-commercial.

4. **No Redistribution for Training**: You may not distribute, share, or make the Works available to others for the purpose of AI/ML training.

5. **Attribution**: When you share or adapt the Works in a way this license permits, keep a notice of this license and credit the author as the copyright holder.

6. **Permitted Uses**: You may use and adapt the Works for personal, educational, research (non-AI/ML), and other non-commercial purposes, provided you comply with all other restrictions.

7. **No Warranty**: The Works are provided "AS IS" without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose, and non-infringement.

8. **Termination**: Violation of any of these terms will result in immediate termination of your rights to use the Works."#.to_string()
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

fn list_repos_with_info(root: &str, namespace: &str, query: Option<&str>) -> Vec<RepoInfo> {
    let Ok(namespace_path) = crate::git::repo::checked_namespace_path(Path::new(root), namespace)
    else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&namespace_path) else {
        return Vec::new();
    };

    let mut repos = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        if entry
            .file_type()
            .is_ok_and(|file_type| file_type.is_symlink())
        {
            continue;
        }

        let Ok(repo_name) = entry.file_name().into_string() else {
            continue;
        };

        if let Some(query) = query
            && !repo_name
                .to_lowercase()
                .contains(query.to_lowercase().as_str())
        {
            continue;
        }

        let repo_path = namespace_path.join(&repo_name);
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

pub fn get_repos_with_info(root: &str, namespace: &str) -> Vec<RepoInfo> {
    list_repos_with_info(root, namespace, None)
}

pub fn search_repos_with_info(root: &str, namespace: &str, query: &str) -> Vec<RepoInfo> {
    list_repos_with_info(root, namespace, Some(query))
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
