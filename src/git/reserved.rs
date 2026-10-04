//! Repository-name policy.
//!
//! Names are refused for user-created repositories when they would shadow a
//! namespace-level UI route, because `/{namespace}/{repo}` is registered after
//! those routes in `main.rs` and the repository would simply be unreachable.

use crate::git::bare::is_safe_component;

/// Names refused for user-created repositories.
///
/// Each entry shadows a namespace-level UI route registered in `main.rs`.
const RESERVED_REPO_NAMES: &[&str] = &[
    "create-repo",
    "create-repo-form",
    "settings",
    "assets",
    "health",
    "up",
    "auth",
    "init",
];

/// Whether `name` is reserved. Compared case-insensitively so the check still
/// holds on case-insensitive filesystems.
pub fn is_reserved_repo_name(name: &str) -> bool {
    RESERVED_REPO_NAMES
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(name.trim()))
}

/// Validates a user-supplied repository name, returning a message suitable for
/// display when the name is refused.
pub fn validate_repo_name(name: &str) -> Result<(), String> {
    let name = name.trim();

    if name.is_empty() {
        return Err("Repository name must be at least 1 character".to_string());
    }
    if !is_safe_component(name) {
        return Err("Repository name cannot contain '/', '\\' or path references".to_string());
    }
    if name.starts_with('.') {
        return Err("Repository name cannot start with '.'".to_string());
    }
    if is_reserved_repo_name(name) {
        return Err(format!(
            "'{name}' is a reserved name and cannot be used for a repository"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reserved_check_is_case_insensitive() {
        assert!(is_reserved_repo_name("auth"));
        assert!(is_reserved_repo_name("AUTH"));
        assert!(validate_repo_name("SeTtInGs").is_err());
    }

    #[test]
    fn test_namespace_level_routes_are_reserved() {
        for name in ["create-repo", "create-repo-form", "settings", "assets"] {
            assert!(
                validate_repo_name(name).is_err(),
                "{name} should be reserved"
            );
        }
    }

    #[test]
    fn test_ordinary_names_are_allowed() {
        for name in ["twig", "my-repo", "repo.git", "a", "Ticketing", "ticketz"] {
            assert!(
                validate_repo_name(name).is_ok(),
                "{name} should be allowed: {:?}",
                validate_repo_name(name)
            );
        }
    }

    #[test]
    fn test_empty_name_is_refused() {
        assert!(validate_repo_name("").is_err());
        assert!(validate_repo_name("   ").is_err());
    }

    #[test]
    fn test_path_traversal_is_refused() {
        for name in ["..", ".", "a/b", "a\\b", "../escape"] {
            assert!(
                validate_repo_name(name).is_err(),
                "{name} should be refused"
            );
        }
    }

    #[test]
    fn test_dotfile_names_are_refused() {
        assert!(validate_repo_name(".git").is_err());
        assert!(validate_repo_name(".hidden").is_err());
    }
}
