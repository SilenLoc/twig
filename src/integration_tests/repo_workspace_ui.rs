use super::*;

use super::support::*;

use super::auth_and_invitations::{create_namespace, signup_and_login};

pub(super) fn init_text_repo(repo_path: &std::path::Path, path: &str, content: &[u8]) -> git2::Oid {
    init_repo_files(repo_path, &[(path, content, git2::FileMode::Blob.into())])
}
pub(super) fn init_repo_files(
    repo_path: &std::path::Path,
    files: &[(&str, &[u8], i32)],
) -> git2::Oid {
    std::fs::create_dir_all(repo_path).unwrap();
    let repo = git2::Repository::init_bare(repo_path).unwrap();
    let empty_builder = repo.treebuilder(None).unwrap();
    let empty_tree_id = empty_builder.write().unwrap();
    let empty_tree = repo.find_tree(empty_tree_id).unwrap();
    let mut update = git2::build::TreeUpdateBuilder::new();
    for (path, content, mode) in files {
        let blob = repo.blob(content).unwrap();
        let mode = match *mode {
            0o100_644 => git2::FileMode::Blob,
            0o100_755 => git2::FileMode::BlobExecutable,
            0o120_000 => git2::FileMode::Link,
            _ => panic!("unsupported test file mode: {mode:o}"),
        };
        update.upsert(path, blob, mode);
    }
    let tree_oid = update.create_updated(&repo, &empty_tree).unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let signature = git2::Signature::now("Seed", "seed@example.com").unwrap();
    let commit = repo
        .commit(
            Some("refs/heads/main"),
            &signature,
            &signature,
            "seed file",
            &tree,
            &[],
        )
        .unwrap();
    repo.set_head("refs/heads/main").unwrap();
    commit
}
#[actix_web::test]
async fn repo_edit_workspace_opens_from_overview_and_navigates_tree_through_htmx() {
    let root = format!("/tmp/test_twig_repo_edit_tracer_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "repoworkspaceowner").await;
    create_namespace(&app, session.clone(), "repoworkspace").await;
    let repo_path = std::path::Path::new(&root).join("repoworkspace/repo");
    init_repo_files(
        &repo_path,
        &[
            ("README.md", b"# Not rendered by the tracer\n", 0o100_644),
            ("src/main.rs", b"fn main() {}\n", 0o100_644),
        ],
    );

    let request = test::TestRequest::get()
        .uri("/repoworkspace/repo")
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let overview = String::from_utf8_lossy(&body);
    assert!(
        overview.contains(r#"href="/repoworkspace/repo/edit""#),
        "the repository overview exposes the root workspace: {overview}"
    );
    assert!(overview.contains(">Open workspace</a>"), "{overview}");

    let request = test::TestRequest::get()
        .uri("/repoworkspace/repo/edit")
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Repository workspace"), "{html}");
    assert!(
        html.contains("twig-workspace-shell"),
        "the editor uses its standalone viewport layout: {html}"
    );
    assert!(
        !html.contains("twig-masthead") && !html.contains("class=\"twig-page\""),
        "the site masthead and centered page width do not constrain the editor: {html}"
    );
    assert!(
        html.contains("Repository overview"),
        "the left navigation keeps a route back to the repository: {html}"
    );
    assert!(
        html.contains("Select a file to edit."),
        "opening from the overview starts at the repository root with no file selected: {html}"
    );
    assert!(
        html.contains("hx-get=\"/repoworkspace/repo/edit/tree/\""),
        "{html}"
    );
    assert!(
        !html.contains("Not rendered by the tracer"),
        "the stub does not expose repository contents: {html}"
    );

    let request = test::TestRequest::get()
        .uri("/repoworkspace/repo/edit/tree/")
        .insert_header(("HX-Request", "true"))
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("README.md"), "{html}");
    assert!(
        html.contains("hx-get=\"/repoworkspace/repo/edit/file/README.md\""),
        "{html}"
    );
    assert!(!html.contains("<html"), "HTMX receives a fragment: {html}");

    let request = test::TestRequest::get()
        .uri("/repoworkspace/repo/edit/file/README.md")
        .insert_header(("HX-Request", "true"))
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Rich text"), "{html}");
    assert!(html.contains("Markdown source"), "{html}");
    assert!(html.contains("Not rendered by the tracer"), "{html}");
    assert!(html.contains("data-editor-rich"), "{html}");
    assert!(html.contains("data-editor-source"), "{html}");
    assert!(
        !html.contains("readonly"),
        "workspace text is editable: {html}"
    );
    assert!(
        !html.contains("<script>"),
        "repository text is not executable markup: {html}"
    );

    let request = test::TestRequest::get()
        .uri("/repoworkspace/repo/edit/file/src/main.rs")
        .insert_header(("HX-Request", "true"))
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Source text"), "{html}");
    assert!(html.contains("fn main() {}"), "{html}");
    assert!(
        !html.contains("readonly"),
        "workspace source is editable: {html}"
    );
    for execution_control in ["Run code", "Run file", "Execute", "Preview"] {
        assert!(
            !html.contains(execution_control),
            "the editor must not offer {execution_control}: {html}"
        );
    }

    let request = test::TestRequest::get()
        .uri("/repoworkspace/repo/edit/README.md")
        .cookie(session)
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(
        response.headers().get("location").unwrap(),
        "/repoworkspace/repo/edit?path=README.md"
    );

    let _ = std::fs::remove_dir_all(&root);
}
#[actix_web::test]
async fn repo_edit_operations_stage_multiple_file_changes_without_writing_git() {
    let root = format!("/tmp/test_twig_repo_edit_draft_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "repoeditdraftowner").await;
    create_namespace(&app, session.clone(), "repoeditdraft").await;
    let repo_path = std::path::Path::new(&root).join("repoeditdraft/repo");
    let base_head = init_repo_files(
        &repo_path,
        &[
            ("README.md", b"# Before\n", 0o100_644),
            ("src/main.rs", b"fn main() {}\n", 0o100_644),
        ],
    );
    let handle = crate::git::bare::RepoHandle::open(&root, "repoeditdraft", "repo").unwrap();

    let empty_draft = serde_json::json!({
        "base_head": base_head.to_string(),
        "active_path": "README.md",
        "tree_path": "",
        "changes": []
    })
    .to_string();
    let create = test::TestRequest::post()
        .uri("/repoeditdraft/repo/edit/operation")
        .cookie(session.clone())
        .set_form([
            ("draft", empty_draft.as_str()),
            ("kind", "create"),
            ("path", "docs/new.md"),
        ])
        .to_request();
    let response = test::call_service(&app, create).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("data-operation-success=\"true\""), "{html}");
    assert!(html.contains("Created docs/new.md"), "{html}");
    assert_eq!(handle.head_oid().unwrap(), Some(base_head));
    assert_eq!(handle.read_blob_bytes("docs/new.md").unwrap(), None);

    let renamed = test::TestRequest::post()
        .uri("/repoeditdraft/repo/edit/operation")
        .cookie(session.clone())
        .set_form([
            ("draft", empty_draft.as_str()),
            ("kind", "rename"),
            ("from", "README.md"),
            ("to", "docs/intro.md"),
        ])
        .to_request();
    let response = test::call_service(&app, renamed).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(
        html.contains("Renamed README.md to docs/intro.md"),
        "{html}"
    );
    assert!(
        html.contains("# Before"),
        "the renamed draft preserves its content: {html}"
    );
    assert_eq!(handle.head_oid().unwrap(), Some(base_head));

    let rename_state = serde_json::json!({
        "base_head": base_head.to_string(),
        "active_path": "docs/intro.md",
        "tree_path": "docs",
        "changes": [
            { "path": "README.md", "state": { "kind": "deleted" } },
            { "path": "docs/intro.md", "state": { "kind": "content", "text": "# Before\n" } }
        ]
    })
    .to_string();
    let deleted = test::TestRequest::post()
        .uri("/repoeditdraft/repo/edit/operation")
        .cookie(session.clone())
        .set_form([
            ("draft", rename_state.as_str()),
            ("kind", "delete"),
            ("path", "src/main.rs"),
        ])
        .to_request();
    let response = test::call_service(&app, deleted).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Marked src/main.rs for removal"), "{html}");
    assert!(
        html.contains("docs/intro.md"),
        "the entire change set is retained: {html}"
    );
    assert_eq!(handle.head_oid().unwrap(), Some(base_head));

    let invalid = test::TestRequest::post()
        .uri("/repoeditdraft/repo/edit/operation")
        .cookie(session)
        .set_form([
            ("draft", rename_state.as_str()),
            ("kind", "create"),
            ("path", "../escape.txt"),
        ])
        .to_request();
    let response = test::call_service(&app, invalid).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("data-operation-success=\"false\""), "{html}");
    assert!(html.contains("safe path"), "{html}");
    assert_eq!(handle.head_oid().unwrap(), Some(base_head));

    let _ = std::fs::remove_dir_all(&root);
}
#[actix_web::test]
async fn repo_edit_legacy_file_entry_denies_authenticated_users_without_namespace_access() {
    let root = format!("/tmp/test_twig_editor_access_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let owner_session = signup_and_login(&app, "editaccessowner").await;
    create_namespace(&app, owner_session, "ownededitspace").await;
    let repo_path = std::path::Path::new(&root).join("ownededitspace/repo");
    init_text_repo(&repo_path, "README.md", b"# Private to namespace\n");
    let other_session = signup_and_login(&app, "editaccessother").await;

    let request = test::TestRequest::get()
        .uri("/ownededitspace/repo/edit/README.md")
        .cookie(other_session)
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let _ = std::fs::remove_dir_all(&root);
}
#[actix_web::test]
async fn repo_edit_refuses_ignored_binary_invalid_and_non_regular_files() {
    let root = format!("/tmp/test_twig_editor_files_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "editorfileowner").await;
    create_namespace(&app, session.clone(), "editfilespace").await;
    let large = vec![b'x'; 1024 * 1024 + 1];
    let repo_path = std::path::Path::new(&root).join("editfilespace/repo");
    init_repo_files(
        &repo_path,
        &[
            (
                ".twig.toml",
                b"ignore_for_view = [\"hidden.txt\"]\n",
                0o100_644,
            ),
            ("README.md", b"# Allowed\n", 0o100_644),
            ("hidden.txt", b"secret\n", 0o100_644),
            ("binary.bin", b"\0\x01\x02", 0o100_644),
            ("invalid.txt", b"\xff", 0o100_644),
            ("large.txt", &large, 0o100_644),
            ("link.txt", b"README.md", 0o120_000),
        ],
    );

    for (path, expected_message) in [
        ("hidden.txt", "cannot be opened"),
        ("missing.txt", "cannot be opened"),
        ("binary.bin", "Binary files cannot"),
        ("invalid.txt", "Only UTF-8 text files"),
        ("large.txt", "larger than 1 MiB"),
        ("link.txt", "Only regular text files"),
    ] {
        let request = test::TestRequest::get()
            .uri(&format!("/editfilespace/repo/edit/file/{path}"))
            .insert_header(("HX-Request", "true"))
            .cookie(session.clone())
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let body = test::read_body(response).await;
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains(expected_message), "{path}: {html}");
        if path == "hidden.txt" {
            assert!(!html.contains("secret"), "ignored content leaked: {html}");
        }
    }

    let _ = std::fs::remove_dir_all(&root);
}
