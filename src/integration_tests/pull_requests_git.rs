use super::pull_requests_api::{push_branch, seed_owner_repo, token_for, token_header};
use super::support::*;
use super::*;
use crate::auth::NamespacePermission;

/// Adds one more commit to `branch`, so the head moves after the request opened.
fn advance_branch(root: &str, branch: &str) -> git2::Oid {
    let repo_path = std::path::Path::new(root).join("pr-team").join("tooling");
    let repo = git2::Repository::open_bare(&repo_path).expect("open bare repo");
    let tip = repo
        .find_reference(&format!("refs/heads/{branch}"))
        .expect("branch")
        .peel_to_commit()
        .expect("tip");
    let mut builder = repo
        .treebuilder(Some(&tip.tree().expect("tree")))
        .expect("builder");
    let blob = repo.blob(b"second").expect("blob");
    builder
        .insert("second.txt", blob, 0o100_644)
        .expect("insert");
    let tree = repo
        .find_tree(builder.write().expect("write"))
        .expect("tree");
    let signature = git2::Signature::now("Twig Test", "twig@example.com").expect("signature");
    repo.commit(
        Some(&format!("refs/heads/{branch}")),
        &signature,
        &signature,
        "Second change",
        &tree,
        &[&tip],
    )
    .expect("commit")
}

/// Asks the Git HTTP endpoint for upload-pack refs, as `git fetch` does first.
async fn advertised_refs(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    namespace: &str,
    repo: &str,
) -> String {
    let response = test::call_service(
        app,
        test::TestRequest::get()
            .uri(&format!(
                "/{namespace}/{repo}/info/refs?service=git-upload-pack"
            ))
            .insert_header(("User-Agent", "git/2.43.0"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    String::from_utf8_lossy(&test::read_body(response).await).into_owned()
}

#[actix_web::test]
async fn fetch_advertises_pull_request_head_refs_and_follows_pushes() {
    let root = format!("/tmp/test_twig_pr_git_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");
    let writer = token_for(
        &db,
        &user,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let (name, value) = token_header(&writer);
    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/pulls"))
            .insert_header((name, value.as_str()))
            .set_json(
                serde_json::json!({"title": "Retry", "head": "feature/retry", "base": "main"}),
            )
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let pull: serde_json::Value = test::read_body_json(created).await;
    let head_sha = pull["head"]["sha"].as_str().unwrap().to_string();

    let refs = advertised_refs(&app, &namespace, &repo).await;
    assert!(
        refs.contains("refs/pull/1/head"),
        "the request's head ref is advertised"
    );
    assert!(refs.contains(&head_sha), "it names the head commit");

    let moved = advance_branch(&root, "feature/retry").to_string();
    let refs = advertised_refs(&app, &namespace, &repo).await;
    assert!(
        refs.contains(&moved),
        "the next fetch follows the new head commit"
    );
    assert!(
        !refs.contains(&head_sha),
        "the previous head is no longer advertised"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn receive_pack_updates_to_pull_refs_are_refused_over_git_http() {
    let root = format!("/tmp/test_twig_pr_guard_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let _ = auth_state;
    let pkt = |payload: &str| format!("{:04x}{payload}", payload.len() + 4);
    let zero = "0".repeat(40);
    let new = "1".repeat(40);
    let body = format!(
        "{}0000",
        pkt(&format!("{zero} {new} refs/pull/1/head\0report-status\n"))
    );
    let authorization = {
        use base64::Engine;
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD
                .encode(format!("{}:password123", user.username))
        )
    };
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/{namespace}/{repo}/git-receive-pack"))
            .insert_header(("User-Agent", "git/2.43.0"))
            .insert_header(("Content-Type", "application/x-git-receive-pack-request"))
            .insert_header(("Authorization", authorization))
            .set_payload(body)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let text = String::from_utf8_lossy(&test::read_body(response).await).into_owned();
    assert!(text.contains("refs/pull/"), "{text}");

    let _ = std::fs::remove_dir_all(&root);
}
