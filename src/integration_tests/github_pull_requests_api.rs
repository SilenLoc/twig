use super::fixtures::{push_branch, seed_owner_repo, token_for, token_header};
use super::support::*;
use super::*;
use crate::auth::NamespacePermission;

const ALL: &[NamespacePermission] = &[
    NamespacePermission::ViewNamespace,
    NamespacePermission::PushChanges,
];

#[actix_web::test]
async fn token_works_in_both_github_schemes_and_is_refused_without_one() {
    let root = format!("/tmp/test_twig_github_auth_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(&db, &user, &namespace, ALL).await;

    // `gh` sends `Authorization: token <PAT>`; REST and GraphQL must both accept it.
    let rest = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v3/user")
            .insert_header(("Authorization", format!("token {token}")))
            .to_request(),
    )
    .await;
    assert_eq!(rest.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(rest).await;
    assert_eq!(body["login"], user.username.as_str());

    let bearer = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v3/user")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    assert_eq!(bearer.status(), StatusCode::OK);

    let graphql = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/graphql")
            .insert_header(("Authorization", format!("token {token}")))
            .set_json(serde_json::json!({"query": "query { viewer { login } }"}))
            .to_request(),
    )
    .await;
    assert_eq!(graphql.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(graphql).await;
    assert_eq!(body["data"]["viewer"]["login"], user.username.as_str());

    let anonymous = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v3/repos/{namespace}/{repo}/pulls"))
            .to_request(),
    )
    .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let body: serde_json::Value = test::read_body_json(anonymous).await;
    assert_eq!(body["message"], "Requires authentication");

    let anonymous_graphql = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/graphql")
            .set_json(serde_json::json!({"query": "query { viewer { login } }"}))
            .to_request(),
    )
    .await;
    assert_eq!(anonymous_graphql.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn github_pull_request_lifecycle_over_rest() {
    let root = format!("/tmp/test_twig_github_pulls_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(&db, &user, &namespace, ALL).await;
    push_branch(&root, "feature", "feature.txt", "new feature\n");
    let (name, value) = token_header(&token);
    let base = format!("/api/v3/repos/{namespace}/{repo}");

    // Create: 201 with GitHub's pull request shape.
    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({
                "title": "Add feature",
                "body": "Adds the feature file",
                "head": "feature",
                "base": "main",
            }))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let pull: serde_json::Value = test::read_body_json(created).await;
    assert_eq!(pull["number"], 1);
    assert_eq!(pull["state"], "open");
    assert_eq!(pull["head"]["ref"], "feature");
    assert_eq!(pull["base"]["ref"], "main");
    assert_eq!(pull["merged"], false);
    assert_eq!(pull["draft"], false);
    assert!(
        pull["head"]["sha"]
            .as_str()
            .is_some_and(|sha| sha.len() == 40)
    );

    // List: open pull requests, and none once they are closed.
    let listed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(listed).await;
    assert_eq!(body.as_array().map(Vec::len), Some(1));

    // Files and commits since the merge base.
    let files = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1/files"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(files).await;
    assert_eq!(body[0]["filename"], "feature.txt");
    assert_eq!(body[0]["status"], "added");

    let commits = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1/commits"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(commits).await;
    assert_eq!(body.as_array().map(Vec::len), Some(1));

    // The diff, by content negotiation.
    let diff = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1"))
            .insert_header((name, value.as_str()))
            .insert_header(("Accept", "application/vnd.github.v3.diff"))
            .to_request(),
    )
    .await;
    assert_eq!(diff.status(), StatusCode::OK);
    let text = String::from_utf8(test::read_body(diff).await.to_vec()).expect("utf8 diff");
    assert!(text.contains("feature.txt"), "diff names the file: {text}");

    // A comment review: allowed on one's own request. Approving is not.
    let reviewed = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/reviews"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"event": "COMMENT", "body": "Looks fine"}))
            .to_request(),
    )
    .await;
    assert_eq!(reviewed.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(reviewed).await;
    assert_eq!(body["state"], "COMMENTED");

    let self_approve = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/reviews"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"event": "APPROVE"}))
            .to_request(),
    )
    .await;
    assert_eq!(self_approve.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let reviews = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1/reviews"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(reviews).await;
    assert_eq!(body.as_array().map(Vec::len), Some(1));

    // Merge with squash: 200 with the merge commit.
    let merged = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("{base}/pulls/1/merge"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"merge_method": "squash"}))
            .to_request(),
    )
    .await;
    assert_eq!(merged.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(merged).await;
    assert_eq!(body["merged"], true);
    assert!(body["sha"].as_str().is_some_and(|sha| sha.len() == 40));

    let after = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(after).await;
    assert_eq!(body["state"], "closed");
    assert_eq!(body["merged"], true);

    let status = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1/merge"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(status.status(), StatusCode::NO_CONTENT);

    // Merging again is a conflict.
    let again = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(&format!("{base}/pulls/1/merge"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"merge_method": "merge"}))
            .to_request(),
    )
    .await;
    assert_eq!(again.status(), StatusCode::CONFLICT);

    // The merged request still shows its files, because the merge commit names them.
    let files_after = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1/files"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(files_after).await;
    assert_eq!(body[0]["filename"], "feature.txt");
}

#[actix_web::test]
async fn graphql_reads_and_creates_issues_and_pull_requests() {
    let root = format!("/tmp/test_twig_github_graphql_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(&db, &user, &namespace, ALL).await;
    push_branch(&root, "feature", "feature.txt", "new feature\n");
    let auth = format!("token {token}");

    let query = |query: &str| {
        test::TestRequest::post()
            .uri("/api/graphql")
            .insert_header(("Authorization", auth.clone()))
            .set_json(serde_json::json!({"query": query}))
            .to_request()
    };

    let repository_id = format!("R_{}", hex::encode(format!("{namespace}/{repo}")));
    let created = test::call_service(
        &app,
        query(&format!(
            r#"mutation {{ createIssue(input: {{repositoryId: "{repository_id}", title: "Crash", body: "Steps"}}) {{ issue {{ id number title state }} }} }}"#
        )),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(created).await;
    assert!(body["errors"].is_null(), "create issue: {body}");
    assert_eq!(body["data"]["createIssue"]["issue"]["number"], 1);
    assert_eq!(body["data"]["createIssue"]["issue"]["title"], "Crash");
    let issue_id = body["data"]["createIssue"]["issue"]["id"]
        .as_str()
        .expect("issue id")
        .to_string();

    let pr = test::call_service(
        &app,
        query(&format!(
            r#"mutation {{ createPullRequest(input: {{repositoryId: "{repository_id}", baseRefName: "main", headRefName: "feature", title: "Add feature"}}) {{ pullRequest {{ id number state headRefName }} }} }}"#
        )),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(pr).await;
    assert!(body["errors"].is_null(), "create pull request: {body}");
    assert_eq!(
        body["data"]["createPullRequest"]["pullRequest"]["number"],
        2
    );
    assert_eq!(
        body["data"]["createPullRequest"]["pullRequest"]["state"],
        "OPEN"
    );
    let pull_id = body["data"]["createPullRequest"]["pullRequest"]["id"]
        .as_str()
        .expect("pull id")
        .to_string();

    // Reads, the way `gh` asks for them.
    let read = test::call_service(
        &app,
        query(&format!(
            r#"query {{ repository(owner: "{namespace}", name: "{repo}") {{
                issue(number: 1) {{ title state comments(first: 5) {{ totalCount }} }}
                pullRequest(number: 2) {{ title state mergeable files(first: 10) {{ nodes {{ path }} }} commits(first: 10) {{ totalCount }} }}
                pullRequests(first: 10, states: [OPEN]) {{ totalCount nodes {{ number }} }}
            }} }}"#
        )),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(read).await;
    assert!(body["errors"].is_null(), "read: {body}");
    assert_eq!(body["data"]["repository"]["issue"]["title"], "Crash");
    assert_eq!(
        body["data"]["repository"]["pullRequest"]["mergeable"],
        "MERGEABLE"
    );
    assert_eq!(
        body["data"]["repository"]["pullRequest"]["files"]["nodes"][0]["path"],
        "feature.txt"
    );
    assert_eq!(body["data"]["repository"]["pullRequests"]["totalCount"], 1);

    // Close the issue and merge the request, by node ID.
    let closed = test::call_service(
        &app,
        query(&format!(
            r#"mutation {{ closeIssue(input: {{issueId: "{issue_id}"}}) {{ issue {{ state }} }} }}"#
        )),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(closed).await;
    assert!(body["errors"].is_null(), "close: {body}");
    assert_eq!(body["data"]["closeIssue"]["issue"]["state"], "CLOSED");

    let merged = test::call_service(
        &app,
        query(&format!(
            r#"mutation {{ mergePullRequest(input: {{pullRequestId: "{pull_id}", mergeMethod: SQUASH}}) {{ pullRequest {{ state merged }} }} }}"#
        )),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(merged).await;
    assert!(body["errors"].is_null(), "merge: {body}");
    assert_eq!(
        body["data"]["mergePullRequest"]["pullRequest"]["state"],
        "MERGED"
    );
    assert_eq!(
        body["data"]["mergePullRequest"]["pullRequest"]["merged"],
        true
    );
}
