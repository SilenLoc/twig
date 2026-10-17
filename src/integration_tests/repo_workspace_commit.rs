use super::*;

use super::support::*;

use super::auth_and_invitations::{create_namespace, signup_and_login};
use super::repo_workspace_ui::{init_repo_files, init_text_repo};

#[actix_web::test]
async fn repo_edit_commits_the_complete_draft_as_one_commit_and_keeps_the_workspace() {
    let root = format!("/tmp/test_twig_repo_edit_commit_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "repoeditcommitowner").await;
    create_namespace(&app, session.clone(), "repoeditcommit").await;
    let repo_path = std::path::Path::new(&root).join("repoeditcommit/repo");
    let base_head = init_repo_files(
        &repo_path,
        &[
            ("README.md", b"# Before\n", 0o100_644),
            ("src/main.rs", b"fn main() {}\n", 0o100_644),
            ("docs/old.md", b"# Old name\n", 0o100_644),
        ],
    );
    let handle = crate::git::bare::RepoHandle::open(&root, "repoeditcommit", "repo").unwrap();
    let draft = serde_json::json!({
        "base_head": base_head.to_string(),
        "active_path": "docs/new.md",
        "tree_path": "docs",
        "changes": [
            { "path": "README.md", "state": { "kind": "content", "text": "# Updated\n" } },
            { "path": "src/main.rs", "state": { "kind": "deleted" } },
            { "path": "docs/old.md", "state": { "kind": "deleted" } },
            { "path": "docs/renamed.md", "state": { "kind": "content", "text": "# Renamed\n" } },
            { "path": "docs/new.md", "state": { "kind": "content", "text": "# New\n" } }
        ]
    })
    .to_string();

    let request = test::TestRequest::post()
        .uri("/repoeditcommit/repo/edit/commit")
        .insert_header(("HX-Request", "true"))
        .cookie(session)
        .set_form([
            ("draft", draft.as_str()),
            ("message", "Update repository guides"),
        ])
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("data-commit-success=\"true\""), "{html}");
    assert!(html.contains("Changes committed successfully"), "{html}");
    assert!(
        !html.contains("<html"),
        "HTMX receives a result fragment: {html}"
    );

    let new_head = handle.head_oid().unwrap().unwrap();
    assert_ne!(new_head, base_head);
    assert_eq!(
        handle.read_blob_bytes("README.md").unwrap().as_deref(),
        Some(&b"# Updated\n"[..])
    );
    assert_eq!(handle.read_blob_bytes("src/main.rs").unwrap(), None);
    assert_eq!(handle.read_blob_bytes("docs/old.md").unwrap(), None);
    assert_eq!(
        handle
            .read_blob_bytes("docs/renamed.md")
            .unwrap()
            .as_deref(),
        Some(&b"# Renamed\n"[..])
    );
    assert_eq!(
        handle.read_blob_bytes("docs/new.md").unwrap().as_deref(),
        Some(&b"# New\n"[..])
    );

    let repo = git2::Repository::open(&repo_path).unwrap();
    let commit = repo.find_commit(new_head).unwrap();
    assert_eq!(commit.parent_id(0).unwrap(), base_head);
    assert_eq!(commit.author().name().unwrap(), "repoeditcommitowner");
    assert_eq!(commit.message().unwrap(), "Update repository guides");
    assert_eq!(handle.get_commits(3).unwrap().len(), 2);
}
#[actix_web::test]
async fn repo_edit_commit_conflict_preserves_draft_and_concurrent_head() {
    let root = format!("/tmp/test_twig_repo_edit_conflict_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "repoeditconflictowner").await;
    create_namespace(&app, session.clone(), "repoeditconflict").await;
    let repo_path = std::path::Path::new(&root).join("repoeditconflict/repo");
    let base_head = init_text_repo(&repo_path, "README.md", b"# Before\n");
    let handle = crate::git::bare::RepoHandle::open(&root, "repoeditconflict", "repo").unwrap();
    let concurrent = handle
        .commit_file(
            "README.md",
            b"# Concurrent\n",
            base_head,
            "Other editor",
            "other@example.test",
            "Concurrent edit",
        )
        .unwrap();
    let crate::git::bare::CommitFileOutcome::Committed(concurrent_head) = concurrent else {
        panic!("the independent commit should advance the branch");
    };
    let draft = serde_json::json!({
            "base_head": base_head.to_string(),
            "active_path": "README.md",
            "tree_path": "",
            "changes": [
                { "path": "README.md", "state": { "kind": "content", "text": "# My preserved draft\n" } }
            ]
        })
        .to_string();

    let request = test::TestRequest::post()
        .uri("/repoeditconflict/repo/edit/commit")
        .cookie(session)
        .set_form([("draft", draft.as_str()), ("message", "My draft")])
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("data-commit-conflict=\"true\""), "{html}");
    assert!(
        html.contains("# My preserved draft"),
        "draft must be returned: {html}"
    );
    assert_eq!(handle.head_oid().unwrap(), Some(concurrent_head));
    assert_eq!(
        handle.read_blob_bytes("README.md").unwrap().as_deref(),
        Some(&b"# Concurrent\n"[..])
    );
    assert_eq!(handle.get_commits(3).unwrap().len(), 2);
}
#[actix_web::test]
async fn repo_edit_commit_rejects_invalid_message_noop_and_oversized_file() {
    let root = format!(
        "/tmp/test_twig_repo_edit_commit_validation_{}",
        uuid::Uuid::new_v4()
    );
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "repoeditvalidator").await;
    create_namespace(&app, session.clone(), "repoeditvalidation").await;
    let repo_path = std::path::Path::new(&root).join("repoeditvalidation/repo");
    let base_head = init_text_repo(&repo_path, "README.md", b"# Stable\n");
    let handle = crate::git::bare::RepoHandle::open(&root, "repoeditvalidation", "repo").unwrap();
    let changed_draft = serde_json::json!({
        "base_head": base_head.to_string(),
        "active_path": "README.md",
        "tree_path": "",
        "changes": [
            { "path": "README.md", "state": { "kind": "content", "text": "# Changed\n" } }
        ]
    })
    .to_string();

    for message in [String::new(), "x".repeat(201)] {
        let request = test::TestRequest::post()
            .uri("/repoeditvalidation/repo/edit/commit")
            .cookie(session.clone())
            .set_form([
                ("draft", changed_draft.as_str()),
                ("message", message.as_str()),
            ])
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("data-commit-success=\"false\""), "{html}");
    }

    let empty_draft = serde_json::json!({
        "base_head": base_head.to_string(),
        "active_path": "README.md",
        "tree_path": "",
        "changes": []
    })
    .to_string();
    let empty = test::TestRequest::post()
        .uri("/repoeditvalidation/repo/edit/commit")
        .cookie(session.clone())
        .set_form([("draft", empty_draft.as_str()), ("message", "No changes")])
        .to_request();
    let response = test::call_service(&app, empty).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    assert!(String::from_utf8_lossy(&body).contains("no changes to commit"));

    let unchanged_draft = serde_json::json!({
        "base_head": base_head.to_string(),
        "active_path": "README.md",
        "tree_path": "",
        "changes": [
            { "path": "README.md", "state": { "kind": "content", "text": "# Stable\n" } }
        ]
    })
    .to_string();
    let unchanged = test::TestRequest::post()
        .uri("/repoeditvalidation/repo/edit/commit")
        .cookie(session.clone())
        .set_form([("draft", unchanged_draft.as_str()), ("message", "No-op")])
        .to_request();
    let response = test::call_service(&app, unchanged).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    assert!(String::from_utf8_lossy(&body).contains("no changes to commit"));

    let oversized_content = "x".repeat(1024 * 1024 + 1);
    let oversized_draft = serde_json::json!({
        "base_head": base_head.to_string(),
        "active_path": "README.md",
        "tree_path": "",
        "changes": [
            { "path": "README.md", "state": { "kind": "content", "text": oversized_content } }
        ]
    })
    .to_string();
    let oversized = test::TestRequest::post()
        .uri("/repoeditvalidation/repo/edit/commit")
        .cookie(session)
        .set_form([
            ("draft", oversized_draft.as_str()),
            ("message", "Too large"),
        ])
        .to_request();
    let response = test::call_service(&app, oversized).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("larger than 1 MiB"), "{html}");
    assert_eq!(handle.head_oid().unwrap(), Some(base_head));
    assert_eq!(handle.get_commits(3).unwrap().len(), 1);
}
#[actix_web::test]
async fn repo_edit_commit_accepts_drafts_larger_than_the_default_form_limit() {
    let root = format!(
        "/tmp/test_twig_repo_edit_form_size_{}",
        uuid::Uuid::new_v4()
    );
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "repoeditformowner").await;
    create_namespace(&app, session.clone(), "repoeditform").await;
    let repo_path = std::path::Path::new(&root).join("repoeditform/repo");
    let base_head = init_text_repo(&repo_path, "README.md", b"# Before\n");
    let content = "x".repeat(32 * 1024);
    let draft = serde_json::json!({
        "base_head": base_head.to_string(),
        "active_path": "docs/large.txt",
        "tree_path": "docs",
        "changes": [
            { "path": "docs/large.txt", "state": { "kind": "content", "text": content } }
        ]
    })
    .to_string();

    let request = test::TestRequest::post()
        .uri("/repoeditform/repo/edit/commit")
        .cookie(session)
        .set_form([
            ("draft", draft.as_str()),
            ("message", "Add a large text file"),
        ])
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("data-commit-success=\"true\""), "{html}");

    let handle = crate::git::bare::RepoHandle::open(&root, "repoeditform", "repo").unwrap();
    assert_ne!(handle.head_oid().unwrap(), Some(base_head));
    assert_eq!(
        handle
            .read_blob_bytes("docs/large.txt")
            .unwrap()
            .unwrap()
            .len(),
        32 * 1024
    );
    let _ = std::fs::remove_dir_all(&root);
}
#[actix_web::test]
async fn repo_edit_commit_requires_an_author_email() {
    let root = format!("/tmp/test_twig_repo_edit_no_email_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let session = signup_and_login(&app, "repoeditnoemail").await;
    create_namespace(&app, session.clone(), "repoeditnoemail").await;
    db.conn()
        .await
        .unwrap()
        .execute(
            "UPDATE users SET email = NULL WHERE username = ?1",
            turso::params!["repoeditnoemail"],
        )
        .await
        .unwrap();
    let repo_path = std::path::Path::new(&root).join("repoeditnoemail/repo");
    let base_head = init_text_repo(&repo_path, "README.md", b"# Before\n");
    let draft = serde_json::json!({
        "base_head": base_head.to_string(),
        "active_path": "README.md",
        "tree_path": "",
        "changes": [
            { "path": "README.md", "state": { "kind": "content", "text": "# After\n" } }
        ]
    })
    .to_string();

    let request = test::TestRequest::post()
        .uri("/repoeditnoemail/repo/edit/commit")
        .cookie(session)
        .set_form([("draft", draft.as_str()), ("message", "Update README")])
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Set an email address in"), "{html}");
    assert!(html.contains("data-commit-success=\"false\""), "{html}");

    let handle = crate::git::bare::RepoHandle::open(&root, "repoeditnoemail", "repo").unwrap();
    assert_eq!(handle.head_oid().unwrap(), Some(base_head));
    assert_eq!(
        handle.read_blob_bytes("README.md").unwrap().as_deref(),
        Some(&b"# Before\n"[..])
    );
    let _ = std::fs::remove_dir_all(&root);
}
