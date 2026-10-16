use super::*;
use std::path::Path;

#[test]
fn test_default_branch_is_main() {
    assert_eq!(default_branch(), "main");
}

#[test]
fn test_create_repo_creates_directory_structure() {
    let temp_root = format!("/tmp/test_twig_repo_{}", uuid::Uuid::new_v4());
    let result = create_repo(&temp_root, "myns", "myrepo", "main");
    assert!(result.is_ok(), "create_repo failed: {result:?}");

    let repo_path = Path::new(&temp_root).join("myns").join("myrepo");
    assert!(
        repo_path.join("HEAD").exists(),
        "bare repository HEAD should exist"
    );
    assert!(git2::Repository::open(repo_path).is_ok());
    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn test_bare_init_creates_valid_repository_and_initial_config_commit() {
    let temp_dir = format!("/tmp/test_twig_bare_init_{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&temp_dir).unwrap();
    let repo_path = Path::new(&temp_dir);

    bare_init(repo_path, "main", "Test Author", "test@example.com")
        .expect("initialize bare repository natively");

    let repo = git2::Repository::open(repo_path).expect("native init creates a valid bare repo");
    assert!(repo.is_bare());
    assert_eq!(repo.head().unwrap().name().unwrap(), "refs/heads/main");
    assert!(repo.config().unwrap().get_bool("http.receivepack").unwrap());

    let commit = repo.head().unwrap().peel_to_commit().unwrap();
    let entry = commit
        .tree()
        .unwrap()
        .get_path(Path::new(".twig.toml"))
        .unwrap();
    let blob = entry.to_object(&repo).unwrap().into_blob().unwrap();
    let content = std::str::from_utf8(blob.content()).unwrap();
    assert!(
        content.starts_with("#:schema https://twig.silenlocatelli.ch/assets/twig.schema.json"),
        "default .twig.toml should reference the schema, got: {content}"
    );
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_bare_init_honors_a_custom_initial_branch() {
    let temp_dir = format!("/tmp/test_twig_bare_init_branch_{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&temp_dir).unwrap();
    let repo_path = Path::new(&temp_dir);

    bare_init(repo_path, "release/next", "Test Author", "test@example.com")
        .expect("initialize with custom branch");

    let repo = git2::Repository::open(repo_path).unwrap();
    assert_eq!(
        repo.head().unwrap().name().unwrap(),
        "refs/heads/release/next"
    );
    let _ = std::fs::remove_dir_all(&temp_dir);
}
