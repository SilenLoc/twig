use super::*;

#[actix_web::test]
async fn repository_editor_requires_access_and_commits_with_compare_and_swap() {
    let root = format!("/tmp/test_twig_editor_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "editorowner").await;
    create_namespace(&app, session.clone(), "editspace").await;
    let repo_path = std::path::Path::new(&root).join("editspace/repo");
    let original_head = init_text_repo(&repo_path, "README.md", b"# Before\n");

    let unauthenticated = test::TestRequest::get()
        .uri("/editspace/repo/edit/README.md")
        .to_request();
    let response = test::call_service(&app, unauthenticated).await;
    assert_eq!(response.status(), StatusCode::FOUND);

    let page = test::TestRequest::get()
        .uri("/editspace/repo/edit/README.md")
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, page).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("# Before"));
    assert!(html.contains(&original_head.to_string()));
    assert!(html.contains("Commit changes"));

    let unauthorized_save = test::TestRequest::post()
        .uri("/editspace/repo/edit/README.md")
        .set_json(serde_json::json!({
            "content": "# Unauthorized\n",
            "message": "Should not write",
            "expected_head": original_head.to_string()
        }))
        .to_request();
    let response = test::call_service(&app, unauthorized_save).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let save = test::TestRequest::post()
        .uri("/editspace/repo/edit/README.md")
        .cookie(session.clone())
        .set_json(serde_json::json!({
            "content": "# After\n",
            "message": "Update README",
            "expected_head": original_head.to_string()
        }))
        .to_request();
    let response = test::call_service(&app, save).await;
    assert_eq!(response.status(), StatusCode::OK);
    let result: serde_json::Value = test::read_body_json(response).await;
    let location = result["location"].as_str().unwrap();
    assert!(location.contains("committed="));

    let saved_page = test::TestRequest::get()
        .uri(location)
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, saved_page).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    assert!(String::from_utf8_lossy(&body).contains("Changes committed successfully."));

    let handle = twig_git::bare::RepoHandle::open(&root, "editspace", "repo").unwrap();
    assert_eq!(
        handle.read_blob_bytes("README.md").unwrap().as_deref(),
        Some(&b"# After\n"[..])
    );
    let commits = handle.get_commits(2).unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].author(), "editorowner");

    let stale_save = test::TestRequest::post()
        .uri("/editspace/repo/edit/README.md")
        .cookie(session)
        .set_json(serde_json::json!({
            "content": "# Stale\n",
            "message": "Stale README",
            "expected_head": original_head.to_string()
        }))
        .to_request();
    let response = test::call_service(&app, stale_save).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        handle.read_blob_bytes("README.md").unwrap().as_deref(),
        Some(&b"# After\n"[..])
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn repository_editor_denies_authenticated_users_without_namespace_access() {
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
async fn repository_editor_edits_non_markdown_files_as_exact_source_text() {
    let root = format!("/tmp/test_twig_editor_source_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "editorsourceowner").await;
    create_namespace(&app, session.clone(), "editsourcespace").await;
    let repo_path = std::path::Path::new(&root).join("editsourcespace/repo");
    let head = init_text_repo(&repo_path, "src/main.rs", b"fn main() {}\n");

    let page = test::TestRequest::get()
        .uri("/editsourcespace/repo/edit/src/main.rs")
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, page).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Source editor"));
    assert!(html.contains("fn main() {}"));
    assert!(!html.contains("quill-"), "source files must not load Quill");

    let save = test::TestRequest::post()
        .uri("/editsourcespace/repo/edit/src/main.rs")
        .cookie(session)
        .set_json(serde_json::json!({
            "content": "fn main() { println!(\"edited\"); }\n",
            "message": "Edit Rust source",
            "expected_head": head.to_string()
        }))
        .to_request();
    let response = test::call_service(&app, save).await;
    assert_eq!(response.status(), StatusCode::OK);
    let handle = twig_git::bare::RepoHandle::open(&root, "editsourcespace", "repo").unwrap();
    assert_eq!(
        handle.read_blob_bytes("src/main.rs").unwrap().as_deref(),
        Some(&b"fn main() { println!(\"edited\"); }\n"[..])
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn repository_editor_saves_conflicted_draft_as_prefixed_file_on_latest_head() {
    let root = format!(
        "/tmp/test_twig_editor_conflict_copy_{}",
        uuid::Uuid::new_v4()
    );
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "editorcopyowner").await;
    create_namespace(&app, session.clone(), "editcopyspace").await;
    let repo_path = std::path::Path::new(&root).join("editcopyspace/repo");
    let original_head = init_text_repo(&repo_path, "README.md", b"# Original\n");
    let handle = twig_git::bare::RepoHandle::open(&root, "editcopyspace", "repo").unwrap();

    let concurrent = handle
        .commit_file(
            "README.md",
            b"# Concurrent edit\n",
            original_head,
            "Other editor",
            "other@example.com",
            "Concurrent update",
        )
        .unwrap();
    let twig_git::bare::CommitFileOutcome::Committed(concurrent_head) = concurrent else {
        panic!("the concurrent update should commit");
    };

    let stale_save = test::TestRequest::post()
        .uri("/editcopyspace/repo/edit/README.md")
        .cookie(session.clone())
        .set_json(serde_json::json!({
            "content": "# My preserved draft\n",
            "message": "My draft",
            "expected_head": original_head.to_string()
        }))
        .to_request();
    let response = test::call_service(&app, stale_save).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = test::read_body(response).await;
    assert!(String::from_utf8_lossy(&body).contains("save it as a new file"));

    let save_copy = test::TestRequest::post()
        .uri("/editcopyspace/repo/edit/README.md")
        .cookie(session.clone())
        .set_json(serde_json::json!({
            "content": "# My preserved draft\n",
            "message": "My draft",
            "expected_head": original_head.to_string(),
            "save_conflict_copy": true
        }))
        .to_request();
    let response = test::call_service(&app, save_copy).await;
    assert_eq!(response.status(), StatusCode::OK);
    let result: serde_json::Value = test::read_body_json(response).await;
    let location = result["location"].as_str().unwrap();
    assert!(location.contains("?committed="));
    assert!(location.contains("&conflict_copy=true"));
    let (copy_path, _) = location
        .strip_prefix("/editcopyspace/repo/content/")
        .unwrap()
        .split_once('?')
        .unwrap();
    let mut name_parts = copy_path.splitn(4, '-');
    assert_eq!(name_parts.next(), Some("editorcopyowner"));
    let date = name_parts.next().unwrap();
    let time = name_parts.next().unwrap();
    assert_eq!(date.len(), 8);
    assert!(date.chars().all(|character| character.is_ascii_digit()));
    assert_eq!(time.len(), 6);
    assert!(time.chars().all(|character| character.is_ascii_digit()));
    assert_eq!(name_parts.next(), Some("README.md"));

    assert_eq!(
        handle.read_blob_bytes("README.md").unwrap().as_deref(),
        Some(&b"# Concurrent edit\n"[..]),
        "the other user's update must remain untouched"
    );
    assert_eq!(
        handle.read_blob_bytes(copy_path).unwrap().as_deref(),
        Some(&b"# My preserved draft\n"[..])
    );
    let parents = git_repo_output(&repo_path, &["show", "-s", "--format=%P", "HEAD"]);
    assert_eq!(parents, concurrent_head.to_string());
    let author = git_repo_output(&repo_path, &["show", "-s", "--format=%an", "HEAD"]);
    assert_eq!(author, "editorcopyowner");

    let saved_page = test::TestRequest::get()
        .uri(location)
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, saved_page).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    assert!(String::from_utf8_lossy(&body).contains("Draft saved as a conflict copy."));

    // A later change may delete the file being edited entirely. The
    // already-open editor must still be able to persist its draft copy.
    commit_deleting_file(
        &repo_path,
        "README.md",
        "Other editor",
        "other@example.com",
        "Delete README",
    );

    let after_delete = test::TestRequest::post()
        .uri("/editcopyspace/repo/edit/README.md")
        .cookie(session.clone())
        .set_json(serde_json::json!({
            "content": "# Draft after deletion\n",
            "message": "Keep my deleted-file draft",
            "expected_head": concurrent_head.to_string(),
            "save_conflict_copy": true
        }))
        .to_request();
    let response = test::call_service(&app, after_delete).await;
    assert_eq!(response.status(), StatusCode::OK);
    let result: serde_json::Value = test::read_body_json(response).await;
    let location = result["location"].as_str().unwrap();
    let (second_copy_path, _) = location
        .strip_prefix("/editcopyspace/repo/content/")
        .unwrap()
        .split_once('?')
        .unwrap();
    assert_eq!(handle.read_blob_bytes("README.md").unwrap(), None);
    assert_eq!(
        handle.read_blob_bytes(second_copy_path).unwrap().as_deref(),
        Some(&b"# Draft after deletion\n"[..])
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn repository_editor_refuses_ignored_binary_invalid_and_non_regular_files() {
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

    for (path, expected) in [
        ("hidden.txt", StatusCode::NOT_FOUND),
        ("missing.txt", StatusCode::NOT_FOUND),
        ("binary.bin", StatusCode::UNSUPPORTED_MEDIA_TYPE),
        ("invalid.txt", StatusCode::UNSUPPORTED_MEDIA_TYPE),
        ("large.txt", StatusCode::PAYLOAD_TOO_LARGE),
        ("link.txt", StatusCode::UNSUPPORTED_MEDIA_TYPE),
    ] {
        let request = test::TestRequest::get()
            .uri(&format!("/editfilespace/repo/edit/{path}"))
            .cookie(session.clone())
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), expected, "{path}");
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn repository_editor_rejects_empty_or_unchanged_commits_without_history_changes() {
    let root = format!("/tmp/test_twig_editor_validation_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "editorvalidator").await;
    create_namespace(&app, session.clone(), "editvalidspace").await;
    let repo_path = std::path::Path::new(&root).join("editvalidspace/repo");
    let head = init_text_repo(&repo_path, "README.md", b"# Stable\n");

    for (message, expected_status) in [
        ("", StatusCode::BAD_REQUEST),
        (&"x".repeat(201), StatusCode::BAD_REQUEST),
    ] {
        let request = test::TestRequest::post()
            .uri("/editvalidspace/repo/edit/README.md")
            .cookie(session.clone())
            .set_json(serde_json::json!({
                "content": "# Changed\n",
                "message": message,
                "expected_head": head.to_string()
            }))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), expected_status);
    }

    let unchanged = test::TestRequest::post()
        .uri("/editvalidspace/repo/edit/README.md")
        .cookie(session.clone())
        .set_json(serde_json::json!({
            "content": "# Stable\n",
            "message": "No change",
            "expected_head": head.to_string()
        }))
        .to_request();
    let response = test::call_service(&app, unchanged).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let oversized = test::TestRequest::post()
        .uri("/editvalidspace/repo/edit/README.md")
        .cookie(session)
        .set_json(serde_json::json!({
            "content": "x".repeat(1024 * 1024 + 1),
            "message": "Too large",
            "expected_head": head.to_string()
        }))
        .to_request();
    let response = test::call_service(&app, oversized).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let handle = twig_git::bare::RepoHandle::open(&root, "editvalidspace", "repo").unwrap();
    assert_eq!(handle.head_oid().unwrap(), Some(head));
    assert_eq!(handle.get_commits(5).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}
