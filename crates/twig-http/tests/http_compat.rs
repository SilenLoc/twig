use std::time::{SystemTime, UNIX_EPOCH};

use twig_http::{Config, Request, handle};

struct TestDir(std::path::PathBuf);

impl TestDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("twig_http_compat_{nonce}"));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn serves_head_from_repository_through_public_api() {
    let root = TestDir::new();
    let repo = root.0.join("public/example");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("HEAD"), "ref: refs/heads/main\n").unwrap();

    let response = handle(
        &Config::new(&root.0),
        "public",
        Request {
            method: "GET",
            path_info: "/example/HEAD",
            query_string: "",
            content_type: None,
            body: &[],
            authenticated_user: None,
            may_rewrite_protected_branches: false,
            receive_hooks: &[],
        },
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"ref: refs/heads/main\n");
}
