//! Path validation for repository names and repository-relative paths.

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

#[cfg(test)]
mod tests {
    use super::*;

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
