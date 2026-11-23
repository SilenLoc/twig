use super::pull_requests_api::{seed_owner_repo, token_for, token_header};
use super::support::*;
use super::*;
use crate::auth::NamespacePermission;

#[actix_web::test]
async fn github_issue_create_get_list_and_close_use_github_shapes() {
    let root = format!("/tmp/test_twig_github_issues_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(
        &db,
        &user,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    let (name, value) = token_header(&token);
    let base = format!("/api/v3/repos/{namespace}/{repo}");

    // Create: 201 with GitHub's issue fields.
    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/issues"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"title": "Crash on start", "body": "Steps below"}))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let issue: serde_json::Value = test::read_body_json(created).await;
    assert_eq!(issue["number"], 1);
    assert_eq!(issue["title"], "Crash on start");
    assert_eq!(issue["body"], "Steps below");
    assert_eq!(issue["state"], "open");
    assert!(issue["state_reason"].is_null());
    assert_eq!(issue["user"]["login"], user.username.as_str());
    assert_eq!(issue["comments"], 0);
    assert!(issue["id"].is_i64(), "GitHub clients expect an integer id");
    assert!(
        issue["url"]
            .as_str()
            .unwrap()
            .ends_with(&format!("{base}/issues/1")),
        "url names the GitHub API path"
    );
    assert!(issue.get("pull_request").is_none());

    // Get by number.
    let got = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/issues/1"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(got.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(got).await;
    assert_eq!(body["title"], "Crash on start");

    // Missing number: GitHub's error body.
    let missing = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/issues/99"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = test::read_body_json(missing).await;
    assert_eq!(body["message"], "Not Found");
    assert_eq!(body["documentation_url"], "https://docs.github.com/rest");

    // Close with PATCH: state closed, state_reason completed.
    let closed = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("{base}/issues/1"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"state": "closed"}))
            .to_request(),
    )
    .await;
    assert_eq!(closed.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(closed).await;
    assert_eq!(body["state"], "closed");
    assert_eq!(body["state_reason"], "completed");
    assert!(body["closed_at"].is_string());

    // Default list is open issues only; state=closed finds it.
    let open = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/issues"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(open.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(open).await;
    assert_eq!(body, serde_json::json!([]));

    let closed_list = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/issues?state=closed"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(closed_list).await;
    assert_eq!(body[0]["number"], 1);

    // An unknown state is refused in GitHub's shape.
    let bad_state = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/issues?state=nope"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(bad_state.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Labels are refused, since Twig has none.
    let labelled = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/issues"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"title": "x", "labels": ["bug"]}))
            .to_request(),
    )
    .await;
    assert_eq!(labelled.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[actix_web::test]
async fn github_issue_routes_require_credentials_in_github_shape() {
    let root = format!("/tmp/test_twig_github_auth_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (_user, namespace, repo) = seed_owner_repo(&db, &root).await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v3/repos/{namespace}/{repo}/issues"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body: serde_json::Value = test::read_body_json(response).await;
    assert!(body["message"].is_string());
    assert_eq!(body["documentation_url"], "https://docs.github.com/rest");
}
