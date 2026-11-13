use super::pull_requests_api::{push_branch, seed_owner_repo, token_for, token_header};
use super::support::*;
use super::*;
use crate::auth::NamespacePermission;

/// Creates an issue through the compatibility API and returns the response body.
async fn create_issue(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    token: &str,
    namespace: &str,
    repo: &str,
    title: &str,
    body: &str,
) -> serde_json::Value {
    let (name, value) = token_header(token);
    let response = test::call_service(
        app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"title": title, "body": body}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    test::read_body_json(response).await
}

#[actix_web::test]
async fn issue_templates_answer_404_and_config_allows_blank_issues() {
    let root = format!("/tmp/test_twig_issues_templates_{}", uuid::Uuid::new_v4());
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

    // Forgejo answers 404 for a repository without templates; `fj` reads that as "blank allowed".
    let templates = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issue_templates"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(templates.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = test::read_body_json(templates).await;
    assert!(
        body["message"].is_string(),
        "clients decode Forgejo's error body"
    );

    let config = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issue_config"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(config.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(config).await;
    assert_eq!(body["blank_issues_enabled"], true);
    assert_eq!(body["contact_links"], serde_json::json!([]));
}

#[actix_web::test]
async fn issue_routes_require_credentials() {
    let root = format!("/tmp/test_twig_issues_auth_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (_user, namespace, repo) = seed_owner_repo(&db, &root).await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues?type=issues"
            ))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn create_issue_returns_201_in_the_forgejo_issue_shape() {
    let root = format!("/tmp/test_twig_issues_create_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(
        &db,
        &user,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;

    let issue = create_issue(
        &app,
        &token,
        &namespace,
        &repo,
        "Crash on start",
        "Segfaults on launch.",
    )
    .await;

    assert_eq!(issue["number"], 1);
    assert_eq!(issue["title"], "Crash on start");
    assert_eq!(issue["body"], "Segfaults on launch.");
    assert_eq!(issue["state"], "open");
    assert_eq!(issue["user"]["login"], user.username);
    assert_eq!(issue["assignees"], serde_json::json!([]));
    assert!(
        issue["pull_request"].is_null(),
        "issues carry no pull request marker"
    );
    assert!(issue["id"].is_i64(), "Forgejo clients expect an integer id");
    assert!(issue["html_url"].as_str().unwrap().ends_with("/issues/1"));
}

#[actix_web::test]
async fn create_issue_requires_a_title() {
    let root = format!("/tmp/test_twig_issues_title_{}", uuid::Uuid::new_v4());
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

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"title": "   ", "body": ""}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = test::read_body_json(response).await;
    assert_eq!(body["message"], "Title is required");
}

#[actix_web::test]
async fn list_issues_pages_newest_first_with_forgejo_headers() {
    let root = format!("/tmp/test_twig_issues_list_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(
        &db,
        &user,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    for title in ["One", "Two", "Three"] {
        create_issue(&app, &token, &namespace, &repo, title, "").await;
    }
    let (name, value) = token_header(&token);

    let first = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues?type=issues&labels=&page=1&limit=2"
            ))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(first.headers().get("x-total-count").unwrap(), "3");
    let link = first
        .headers()
        .get("link")
        .expect("a next page exists")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        link.contains("page=2") && link.contains("rel=\"next\""),
        "{link}"
    );
    let items: serde_json::Value = test::read_body_json(first).await;
    assert_eq!(
        items
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["number"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![3, 2]
    );

    let second = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues?type=issues&page=2&limit=2"
            ))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert!(
        second.headers().get("link").is_none(),
        "the last page has no next link"
    );
    let items: serde_json::Value = test::read_body_json(second).await;
    assert_eq!(items[0]["number"], 1);
}

#[actix_web::test]
async fn get_issue_returns_the_issue_or_a_forgejo_404() {
    let root = format!("/tmp/test_twig_issues_get_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(
        &db,
        &user,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    create_issue(&app, &token, &namespace, &repo, "Find me", "body text").await;
    let (name, value) = token_header(&token);

    let found = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues/1"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(found.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(found).await;
    assert_eq!(body["title"], "Find me");

    let missing = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues/99"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let body: serde_json::Value = test::read_body_json(missing).await;
    assert_eq!(body["message"], "The target couldn't be found");
}

#[actix_web::test]
async fn pull_requests_and_issues_share_one_number_sequence() {
    let root = format!("/tmp/test_twig_issues_shared_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(
        &db,
        &user,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    create_issue(&app, &token, &namespace, &repo, "Issue first", "").await;
    push_branch(&root, "feature/shared", "shared.txt", "shared");
    let (name, value) = token_header(&token);

    let pull = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/pulls"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({
                "base": "main",
                "head": "feature/shared",
                "title": "Change after the issue",
                "body": ""
            }))
            .to_request(),
    )
    .await;
    assert_eq!(pull.status(), StatusCode::CREATED);
    let pull: serde_json::Value = test::read_body_json(pull).await;
    assert_eq!(
        pull["number"], 2,
        "the pull request takes the next shared number"
    );

    // The issue's number answers as an issue; the pull request's number answers in issue shape.
    let as_pull = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues/2"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let as_pull: serde_json::Value = test::read_body_json(as_pull).await;
    assert_eq!(as_pull["title"], "Change after the issue");
    assert!(
        as_pull["pull_request"].is_object(),
        "pull requests keep their marker"
    );

    let as_issue = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues/1"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let as_issue: serde_json::Value = test::read_body_json(as_issue).await;
    assert_eq!(as_issue["title"], "Issue first");
    assert!(as_issue["pull_request"].is_null());
}

/// Adds `user_id` to `namespace` as a contributor, as an accepted invitation does.
pub(super) async fn add_contributor(db: &crate::db::Database, namespace: &str, user_id: &str) {
    let ns = db
        .get_namespace_by_name(namespace)
        .await
        .expect("load namespace")
        .expect("namespace exists");
    db.conn()
        .await
        .expect("connection")
        .execute(
            "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
             VALUES (?1, ?2, 'contributor', ?3)",
            crate::db::sqlite::params![
                ns.id.clone(),
                user_id.to_string(),
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .await
        .expect("add contributor membership");
}

#[actix_web::test]
async fn issue_comments_round_trip_with_offset_ids() {
    let root = format!("/tmp/test_twig_issues_comments_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(
        &db,
        &user,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    create_issue(&app, &token, &namespace, &repo, "Talk", "").await;
    let (name, value) = token_header(&token);
    let comments_uri = format!("/api/v1/repos/{namespace}/{repo}/issues/1/comments");

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&comments_uri)
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"body": "First note"}))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let comment: serde_json::Value = test::read_body_json(created).await;
    let id = comment["id"].as_i64().unwrap();
    assert!(id >= 1 << 40, "issue comments use the offset ID range");
    assert_eq!(comment["body"], "First note");
    assert_eq!(comment["user"]["login"], user.username);
    assert!(
        comment["issue_url"]
            .as_str()
            .unwrap()
            .ends_with("/issues/1")
    );

    let listed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&comments_uri)
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let listed: serde_json::Value = test::read_body_json(listed).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);

    let issue = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues/1"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let issue: serde_json::Value = test::read_body_json(issue).await;
    assert_eq!(issue["comments"], 1, "the issue counts its comments");

    let edited = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues/comments/{id}"
            ))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"body": "Edited note"}))
            .to_request(),
    )
    .await;
    assert_eq!(edited.status(), StatusCode::OK);
    let edited: serde_json::Value = test::read_body_json(edited).await;
    assert_eq!(edited["body"], "Edited note");

    let deleted = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues/comments/{id}"
            ))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    let listed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&comments_uri)
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let listed: serde_json::Value = test::read_body_json(listed).await;
    assert_eq!(listed, serde_json::json!([]));
}

#[actix_web::test]
async fn pull_request_comments_still_answer_through_issue_paths() {
    let root = format!("/tmp/test_twig_issues_pr_comments_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(
        &db,
        &user,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    create_issue(&app, &token, &namespace, &repo, "Issue first", "").await;
    push_branch(&root, "feature/talk", "talk.txt", "talk");
    let (name, value) = token_header(&token);
    let pull = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/pulls"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({
                "base": "main",
                "head": "feature/talk",
                "title": "Talk about it",
                "body": ""
            }))
            .to_request(),
    )
    .await;
    assert_eq!(pull.status(), StatusCode::CREATED);
    let pull: serde_json::Value = test::read_body_json(pull).await;
    assert_eq!(pull["number"], 2);

    let comment = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues/2/comments"
            ))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"body": "On the change"}))
            .to_request(),
    )
    .await;
    assert_eq!(comment.status(), StatusCode::CREATED);
    let comment: serde_json::Value = test::read_body_json(comment).await;
    let pr_comment_id = comment["id"].as_i64().unwrap();
    assert!(
        pr_comment_id < 1 << 40,
        "pull request comments keep their own IDs"
    );
    assert!(
        comment["pull_request_url"]
            .as_str()
            .unwrap()
            .ends_with("/pulls/2")
    );

    let edited = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues/comments/{pr_comment_id}"
            ))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"body": "Still on the change"}))
            .to_request(),
    )
    .await;
    assert_eq!(edited.status(), StatusCode::OK);
    let edited: serde_json::Value = test::read_body_json(edited).await;
    assert_eq!(edited["body"], "Still on the change");
}

#[actix_web::test]
async fn close_and_reopen_patch_the_issue_and_answer_201() {
    let root = format!("/tmp/test_twig_issues_state_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(
        &db,
        &user,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    create_issue(&app, &token, &namespace, &repo, "Flaky test", "").await;
    push_branch(&root, "feature/state", "state.txt", "state");
    let (name, value) = token_header(&token);
    let issue_uri = format!("/api/v1/repos/{namespace}/{repo}/issues/1");

    // The shape `fj issue close` sends: every optional field present as null.
    let closed = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&issue_uri)
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({
                "assignee": null, "assignees": null, "body": null, "due_date": null,
                "milestone": null, "ref": null, "state": "closed", "title": null,
                "unset_due_date": null, "updated_at": null
            }))
            .to_request(),
    )
    .await;
    assert_eq!(closed.status(), StatusCode::CREATED);
    let closed: serde_json::Value = test::read_body_json(closed).await;
    assert_eq!(closed["state"], "closed");
    assert!(closed["closed_at"].is_string());

    let reopened = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&issue_uri)
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"state": "open"}))
            .to_request(),
    )
    .await;
    assert_eq!(reopened.status(), StatusCode::CREATED);
    let reopened: serde_json::Value = test::read_body_json(reopened).await;
    assert_eq!(reopened["state"], "open");
    assert!(reopened["closed_at"].is_null());

    let bogus = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&issue_uri)
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"state": "archived"}))
            .to_request(),
    )
    .await;
    assert_eq!(bogus.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = test::read_body_json(bogus).await;
    assert_eq!(body["message"], "Invalid state");

    // Create a pull request so number 2 names one; patching it through the issue path is refused.
    let pull = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/pulls"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({
                "base": "main",
                "head": "feature/state",
                "title": "State change",
                "body": ""
            }))
            .to_request(),
    )
    .await;
    assert_eq!(pull.status(), StatusCode::CREATED);
    let refused = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues/2"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"state": "closed"}))
            .to_request(),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = test::read_body_json(refused).await;
    assert_eq!(body["message"], "Use pulls/2 to change a pull request");
}

#[actix_web::test]
async fn only_the_comment_author_may_edit_or_delete_it() {
    let root = format!("/tmp/test_twig_issues_author_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    let owner_token = token_for(
        &db,
        &owner,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    create_issue(&app, &owner_token, &namespace, &repo, "Discuss", "").await;

    let other = auth::create_user(
        "issue-reader".to_string(),
        "reader@example.com".to_string(),
        "password123",
    )
    .expect("create reader");
    db.create_user(&other).await.expect("persist reader");
    add_contributor(&db, &namespace, &other.id).await;
    let other_token = token_for(
        &db,
        &other,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;

    let (owner_name, owner_value) = token_header(&owner_token);
    let posted = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues/1/comments"
            ))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"body": "Owner's note"}))
            .to_request(),
    )
    .await;
    let posted: serde_json::Value = test::read_body_json(posted).await;
    let id = posted["id"].as_i64().unwrap();
    let uri = format!("/api/v1/repos/{namespace}/{repo}/issues/comments/{id}");

    let (other_name, other_value) = token_header(&other_token);
    let edit = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&uri)
            .insert_header((other_name, other_value.as_str()))
            .set_json(serde_json::json!({"body": "Hijacked"}))
            .to_request(),
    )
    .await;
    assert_eq!(edit.status(), StatusCode::FORBIDDEN);

    let delete = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&uri)
            .insert_header((other_name, other_value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(delete.status(), StatusCode::FORBIDDEN);

    let other_comment = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues/1/comments"
            ))
            .insert_header((other_name, other_value.as_str()))
            .set_json(serde_json::json!({"body": "A viewer may comment"}))
            .to_request(),
    )
    .await;
    assert_eq!(other_comment.status(), StatusCode::CREATED);
}

#[actix_web::test]
async fn single_issue_comment_is_readable_by_its_id() {
    let root = format!("/tmp/test_twig_issues_one_comment_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(
        &db,
        &user,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    create_issue(&app, &token, &namespace, &repo, "Read a comment", "").await;
    let (name, value) = token_header(&token);
    let posted = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues/1/comments"
            ))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"body": "Look at this"}))
            .to_request(),
    )
    .await;
    let posted: serde_json::Value = test::read_body_json(posted).await;
    let id = posted["id"].as_i64().unwrap();

    let read = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues/comments/{id}"
            ))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(read.status(), StatusCode::OK);
    let read: serde_json::Value = test::read_body_json(read).await;
    assert_eq!(read["body"], "Look at this");
    assert_eq!(
        read["assets"],
        serde_json::json!([]),
        "Forgejo clients expect assets"
    );

    let missing = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues/comments/{}",
                id + 1000
            ))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn author_edits_title_and_body_and_assignees_follow_fj_shapes() {
    let root = format!("/tmp/test_twig_issues_edit_api_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    let author_token = token_for(
        &db,
        &owner,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    create_issue(
        &app,
        &author_token,
        &namespace,
        &repo,
        "First title",
        "First body",
    )
    .await;

    let reviewer = auth::create_user(
        "reviewer".to_string(),
        "reviewer@example.com".to_string(),
        "password123",
    )
    .expect("create reviewer");
    db.create_user(&reviewer).await.expect("persist reviewer");
    add_contributor(&db, &namespace, &reviewer.id).await;

    let pusher = token_for(
        &db,
        &owner,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let uri = format!("/api/v1/repos/{namespace}/{repo}/issues/1");

    // The author may change the title and body without push rights.
    let (name, value) = token_header(&author_token);
    let edited = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&uri)
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({
                "title": "Second title", "body": "Second body", "state": null,
                "assignee": null, "assignees": null
            }))
            .to_request(),
    )
    .await;
    assert_eq!(edited.status(), StatusCode::CREATED);
    let edited: serde_json::Value = test::read_body_json(edited).await;
    assert_eq!(edited["title"], "Second title");
    assert_eq!(edited["body"], "Second body");

    // Assigning needs push rights, so the author's view-only token is refused.
    let refused = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&uri)
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"assignees": ["reviewer"]}))
            .to_request(),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);

    // With push rights, `fj issue assign` sends the whole set and gets it back.
    let (push_name, push_value) = token_header(&pusher);
    let assigned = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&uri)
            .insert_header((push_name, push_value.as_str()))
            .set_json(serde_json::json!({"assignees": ["reviewer"]}))
            .to_request(),
    )
    .await;
    assert_eq!(assigned.status(), StatusCode::CREATED);
    let assigned: serde_json::Value = test::read_body_json(assigned).await;
    assert_eq!(assigned["assignees"][0]["login"], "reviewer");

    // `fj issue unassign` sends an empty list, which clears the set.
    let cleared = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&uri)
            .insert_header((push_name, push_value.as_str()))
            .set_json(serde_json::json!({"assignees": []}))
            .to_request(),
    )
    .await;
    let cleared: serde_json::Value = test::read_body_json(cleared).await;
    assert_eq!(cleared["assignees"], serde_json::json!([]));
}

#[actix_web::test]
async fn assignees_must_be_namespace_members() {
    let root = format!("/tmp/test_twig_issues_members_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    let pusher = token_for(
        &db,
        &owner,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    create_issue(&app, &pusher, &namespace, &repo, "Assign me", "").await;
    let (name, value) = token_header(&pusher);

    let stranger = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues/1"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"assignees": ["stranger"]}))
            .to_request(),
    )
    .await;
    assert_eq!(stranger.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = test::read_body_json(stranger).await;
    assert_eq!(
        body["message"],
        format!("stranger is not a member of {namespace}")
    );

    // Creating with an assignee also needs push rights, and the same membership rule.
    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"title": "Owned", "assignees": [owner.username]}))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created: serde_json::Value = test::read_body_json(created).await;
    assert_eq!(created["assignees"][0]["login"], owner.username);
}

/// Lists issues with a raw query string. Returns the status, the numbers, and the total.
async fn list_with(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    token: &str,
    namespace: &str,
    repo: &str,
    query: &str,
) -> (StatusCode, Vec<i64>, String) {
    let (name, value) = token_header(token);
    let response = test::call_service(
        app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/issues?type=issues&{query}"
            ))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let status = response.status();
    let total = response
        .headers()
        .get("x-total-count")
        .map_or_else(String::new, |value| value.to_str().unwrap().to_string());
    let body: serde_json::Value = test::read_body_json(response).await;
    let numbers = body.as_array().map_or_else(Vec::new, |items| {
        items
            .iter()
            .filter_map(|item| item["number"].as_i64())
            .collect()
    });
    (status, numbers, total)
}

#[actix_web::test]
async fn fj_search_filters_by_state_text_and_people() {
    let root = format!("/tmp/test_twig_issues_filters_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    let owner_view = token_for(
        &db,
        &owner,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    let owner_push = token_for(
        &db,
        &owner,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;

    let reviewer = auth::create_user(
        "filter-reviewer".to_string(),
        "filter-reviewer@example.com".to_string(),
        "password123",
    )
    .expect("create reviewer");
    db.create_user(&reviewer).await.expect("persist reviewer");
    add_contributor(&db, &namespace, &reviewer.id).await;
    let reviewer_view = token_for(
        &db,
        &reviewer,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;

    // #1 by the owner, closed later. #2 by the reviewer, then assigned to them.
    create_issue(
        &app,
        &owner_view,
        &namespace,
        &repo,
        "Crash on start",
        "It segfaults on launch.",
    )
    .await;
    create_issue(
        &app,
        &reviewer_view,
        &namespace,
        &repo,
        "Docs",
        "The README is out of date.",
    )
    .await;
    let (push_name, push_value) = token_header(&owner_push);
    let assigned = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues/2"))
            .insert_header((push_name, push_value.as_str()))
            .set_json(serde_json::json!({"assignees": ["filter-reviewer"]}))
            .to_request(),
    )
    .await;
    assert_eq!(assigned.status(), StatusCode::CREATED);
    let closed = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/issues/1"))
            .insert_header((push_name, push_value.as_str()))
            .set_json(serde_json::json!({"state": "closed"}))
            .to_request(),
    )
    .await;
    assert_eq!(closed.status(), StatusCode::CREATED);

    let check = |query: &'static str| {
        let (app, namespace, repo, token) = (&app, &namespace, &repo, &reviewer_view);
        async move { list_with(app, token, namespace, repo, query).await }
    };

    let (status, numbers, total) = check("").await;
    assert_eq!(
        (status, numbers.as_slice(), total.as_str()),
        (StatusCode::OK, &[2][..], "1")
    );
    let (_, numbers, _) = check("state=all").await;
    assert_eq!(numbers, vec![2, 1], "newest first");
    let (_, numbers, _) = check("state=closed").await;
    assert_eq!(numbers, vec![1]);
    let (_, numbers, _) = check("state=all&q=SEGFAULT").await;
    assert_eq!(
        numbers,
        vec![1],
        "text matches the body, case-insensitively"
    );
    let (_, numbers, _) = check("state=all&created_by=filter-reviewer").await;
    assert_eq!(numbers, vec![2]);
    let (_, numbers, total) = check("state=all&created_by=ghost").await;
    assert!(numbers.is_empty(), "an unknown author lists nothing");
    assert_eq!(total, "0");
    let (_, numbers2, _) = check("state=all&assigned_by=filter-reviewer").await;
    assert_eq!(numbers2, vec![2]);

    // Empty parameters, which fj sends by default, filter nothing.
    let (status, numbers, _) = check("state=all&labels=&milestones=").await;
    assert_eq!((status, numbers.len()), (StatusCode::OK, 2));

    // Twig has no labels or milestones, so asking for them is refused, not answered empty.
    let (status, _, _) = check("state=all&labels=bug").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _, _) = check("state=all&milestones=v1").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[actix_web::test]
async fn features_twig_lacks_answer_422_with_a_reason() {
    let root = format!("/tmp/test_twig_issues_refusals_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(
        &db,
        &user,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    create_issue(&app, &token, &namespace, &repo, "Refuse me", "").await;
    let (name, value) = token_header(&token);
    let base = format!("/api/v1/repos/{namespace}/{repo}/issues/1");

    let refusals = [
        (
            "POST",
            "/dependencies",
            "Issue dependencies are not supported on Twig",
        ),
        ("POST", "/blocks", "Blocking is not supported on Twig"),
        ("POST", "/labels", "Labels are not supported on Twig"),
        (
            "POST",
            "/reactions",
            "Reactions on issues are not supported yet",
        ),
    ];
    for (method, suffix, message) in refusals {
        let request = if method == "POST" {
            test::TestRequest::post()
        } else {
            test::TestRequest::get()
        };
        let response = test::call_service(
            &app,
            request
                .uri(&format!("{base}{suffix}"))
                .insert_header((name, value.as_str()))
                .set_json(serde_json::json!({"content": "+1", "dependency": 2}))
                .to_request(),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{suffix}"
        );
        let body: serde_json::Value = test::read_body_json(response).await;
        assert_eq!(body["message"], message, "{suffix}");
    }

    // Issue labels are an empty list, which is true; anonymous requests still get 401.
    let labels = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/labels"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(labels.status(), StatusCode::OK);
    let labels: serde_json::Value = test::read_body_json(labels).await;
    assert_eq!(labels, serde_json::json!([]));

    let anonymous = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/dependencies"))
            .set_json(serde_json::json!({}))
            .to_request(),
    )
    .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn deleting_a_repository_removes_its_issues() {
    use crate::db::issues::IssueFilter;
    let root = format!("/tmp/test_twig_issues_delete_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;

    // A repository may be deleted only when its configuration allows it.
    let handle = crate::git::bare::RepoHandle::open(&root, &namespace, &repo).expect("open repo");
    let repo_id = handle.repo_id().expect("repo id");
    let expected_head = handle.head_oid().unwrap().unwrap();
    handle
        .commit_changes(
            &[crate::git::bare::RepoEditChange::Upsert {
                path: ".twig.toml".to_string(),
                content: b"deletable = true\n".to_vec(),
            }],
            expected_head,
            &user.username,
            user.email.as_deref().unwrap(),
            "Allow deletion for the issue test",
        )
        .expect("mark repo deletable");

    let token = token_for(
        &db,
        &user,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::ManageRepositories,
        ],
    )
    .await;
    create_issue(&app, &token, &namespace, &repo, "Goes with the repo", "").await;
    assert_eq!(
        db.count_issues(
            &repo_id,
            &IssueFilter {
                limit: 50,
                ..Default::default()
            }
        )
        .await
        .unwrap(),
        1
    );

    let (name, value) = token_header(&token);
    let deleted = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!(
                "/api/v1/namespaces/{namespace}/repositories/{repo}"
            ))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::OK);
    assert_eq!(
        db.count_issues(
            &repo_id,
            &IssueFilter {
                limit: 50,
                ..Default::default()
            }
        )
        .await
        .unwrap(),
        0,
        "the repository's issues are removed with it"
    );
}
