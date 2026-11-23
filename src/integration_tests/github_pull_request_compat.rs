//! GitHub compatibility for the pull request pages and the GraphQL fields clients read
//! from them: the `/pull/{number}` URLs GitHub uses, and the fields and mutations a pull
//! request view sends to `/api/graphql`.

use super::fixtures::{push_branch, seed_owner_repo, token_for};
use super::support::*;
use super::*;
use crate::auth::NamespacePermission;

const ALL: &[NamespacePermission] = &[
    NamespacePermission::ViewNamespace,
    NamespacePermission::PushChanges,
];

/// Posts a GraphQL document with the token and returns the decoded response.
async fn graphql(
    app: &impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    auth: &str,
    query: &str,
    variables: serde_json::Value,
) -> serde_json::Value {
    let response = test::call_service(
        app,
        test::TestRequest::post()
            .uri("/api/graphql")
            .insert_header(("Authorization", auth))
            .set_json(serde_json::json!({"query": query, "variables": variables}))
            .to_request(),
    )
    .await;
    test::read_body_json(response).await
}

/// Seeds a repository with a feature branch one commit ahead of `main`, and opens a
/// request for it. Returns the app, the token header, the repository ID, the request's
/// node ID, its number, and the owner and repository names.
async fn seeded_request(
    root: &str,
) -> (
    impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    String,
    String,
    String,
    i64,
    String,
    String,
    crate::db::Database,
    crate::auth::User,
) {
    let (app, db) = create_test_service_with_db_in(root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, root).await;
    let token = token_for(&db, &user, &namespace, ALL).await;
    push_branch(root, "feature", "feature.txt", "new feature\n");
    let auth = format!("token {token}");
    let repository_id = format!("R_{}", hex::encode(format!("{namespace}/{repo}")));
    let created = graphql(
        &app,
        &auth,
        r"mutation($input: CreatePullRequestInput!) {
            createPullRequest(input: $input) { pullRequest { id number } }
        }",
        serde_json::json!({"input": {
            "repositoryId": repository_id,
            "baseRefName": "main",
            "headRefName": "feature",
            "title": "Add feature",
            "body": "Adds the feature.",
        }}),
    )
    .await;
    assert!(created["errors"].is_null(), "create: {created}");
    let pull = &created["data"]["createPullRequest"]["pullRequest"];
    let id = pull["id"].as_str().expect("pull id").to_string();
    let number = pull["number"].as_i64().expect("pull number");
    (
        app,
        auth,
        repository_id,
        id,
        number,
        namespace,
        repo,
        db,
        user,
    )
}

#[actix_web::test]
async fn github_pull_urls_serve_the_request_pages_and_its_diff() {
    let root = format!("/tmp/test_twig_pull_urls_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    push_branch(&root, "feature/retry", "retry.txt", "retry\n");
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let session = auth_state
        .create_session(user.id.clone())
        .await
        .expect("session");
    let cookie = || actix_web::cookie::Cookie::new("session", session.clone());

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/{namespace}/{repo}/pulls/new"))
            .cookie(cookie())
            .set_form(&[
                ("base", "main"),
                ("head", "feature/retry"),
                ("title", "Add retry"),
                ("body", "Explains the policy."),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::SEE_OTHER);

    // GitHub's `/pull/{number}` paths serve the same pages as Twig's `/pulls/{number}`.
    for (suffix, shown) in [
        ("", "Add retry"),
        ("/files", "retry.txt"),
        ("/commits", "retry.txt"),
    ] {
        let (status, body) = body_of(&app, &format!("/{namespace}/{repo}/pull/1{suffix}")).await;
        assert_eq!(status, StatusCode::OK, "pull/1{suffix} answers");
        assert!(body.contains(shown), "pull/1{suffix} shows {shown}");
    }
    let (status, _) = body_of(&app, &format!("/{namespace}/{repo}/pull/99")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an unknown request is not found"
    );

    // A signed-in visitor gets the compare form at GitHub's `pull/new` path too.
    let new_form = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/{namespace}/{repo}/pull/new?base=main&head=feature/retry"
            ))
            .cookie(cookie())
            .to_request(),
    )
    .await;
    assert_eq!(new_form.status(), StatusCode::OK);

    // The author's edit form, at GitHub's path too.
    let edit_form = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/{namespace}/{repo}/pull/1/edit"))
            .cookie(cookie())
            .to_request(),
    )
    .await;
    assert_eq!(edit_form.status(), StatusCode::OK);
    let edit_html = String::from_utf8(test::read_body(edit_form).await.to_vec()).unwrap();
    assert!(
        edit_html.contains("name=\"title\""),
        "edit form has a title field"
    );

    // `pull/{number}.diff` is the diff of the request, as GitHub serves it. The repository
    // is private, so the request carries a token.
    let token = token_for(&db, &user, &namespace, ALL).await;
    let diff = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/{namespace}/{repo}/pull/1.diff"))
            .insert_header(("Authorization", format!("token {token}")))
            .to_request(),
    )
    .await;
    assert_eq!(diff.status(), StatusCode::OK);
    let diff_text = String::from_utf8(test::read_body(diff).await.to_vec()).unwrap();
    assert!(
        diff_text.contains("+retry"),
        "diff adds the file: {diff_text}"
    );

    // The API links to the same GitHub-style paths.
    let api = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v3/repos/{namespace}/{repo}/pulls/1"))
            .insert_header(("Authorization", format!("token {token}")))
            .to_request(),
    )
    .await;
    assert_eq!(api.status(), StatusCode::OK);
    let item: serde_json::Value = test::read_body_json(api).await;
    assert!(
        item["html_url"]
            .as_str()
            .unwrap()
            .ends_with(&format!("/{namespace}/{repo}/pull/1")),
        "html_url: {}",
        item["html_url"]
    );
    assert!(item["diff_url"].as_str().unwrap().ends_with("/pull/1.diff"));

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn pull_request_view_fields_resolve_as_github_answers_them() {
    let root = format!("/tmp/test_twig_pr_view_fields_{}", uuid::Uuid::new_v4());
    let (app, auth, _repository_id, _id, number, namespace, repo, _db, user) =
        seeded_request(&root).await;

    // The fields a request view names, including the ones that used to be unknown.
    let view = graphql(
        &app,
        &auth,
        r#"query($owner: String!, $name: String!, $number: Int!) {
            viewer { login }
            repository(owner: $owner, name: $name) {
                autoMergeAllowed
                pullRequest(number: $number) {
                    viewerCanUpdate
                    viewerDidAuthor
                    viewerCanUpdateBranch
                    isMergeQueueEnabled
                    timelineItems(first: 10, itemTypes: [REVIEW_DISMISSED_EVENT]) {
                        totalCount
                        nodes {
                            __typename
                            ... on ReviewDismissedEvent { dismissalMessage review { id } }
                            ... on RemovedFromMergeQueueEvent { id }
                        }
                    }
                    baseRef {
                        compare(headRef: "feature") { aheadBy behindBy status }
                    }
                    commits(last: 1) {
                        nodes {
                            commit {
                                statusCheckRollup {
                                    contexts(first: 5) {
                                        nodes {
                                            __typename
                                            ... on StatusContext { isRequired(pullRequestNumber: $number) }
                                            ... on CheckRun { isRequired(pullRequestNumber: $number) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }"#,
        serde_json::json!({"owner": namespace, "name": repo, "number": number}),
    )
    .await;
    assert!(view["errors"].is_null(), "view: {view}");
    let data = &view["data"];
    assert_eq!(data["viewer"]["login"], user.username.as_str());
    assert_eq!(data["repository"]["autoMergeAllowed"], false);
    let pull = &data["repository"]["pullRequest"];
    assert_eq!(pull["viewerDidAuthor"], true, "the caller opened it");
    assert_eq!(pull["viewerCanUpdate"], true, "its author may change it");
    assert_eq!(
        pull["viewerCanUpdateBranch"], false,
        "Twig cannot update a branch"
    );
    assert_eq!(pull["isMergeQueueEnabled"], false);
    assert_eq!(pull["timelineItems"]["totalCount"], 0);
    assert_eq!(
        pull["baseRef"]["compare"],
        serde_json::json!({"aheadBy": 1, "behindBy": 0, "status": "AHEAD"}),
        "the feature branch is one commit ahead"
    );
}

#[actix_web::test]
async fn ref_compare_reports_ahead_behind_and_diverged() {
    let root = format!("/tmp/test_twig_ref_compare_{}", uuid::Uuid::new_v4());
    let (app, auth, _repository_id, _id, _number, namespace, repo, _db, _user) =
        seeded_request(&root).await;

    let compared = graphql(
        &app,
        &auth,
        r#"query($owner: String!, $name: String!) {
            repository(owner: $owner, name: $name) {
                ahead: ref(qualifiedName: "refs/heads/main") {
                    compare(headRef: "feature") { aheadBy behindBy status }
                }
                same: ref(qualifiedName: "refs/heads/feature") {
                    compare(headRef: "feature") { status }
                }
                missing: ref(qualifiedName: "refs/heads/nope") { name }
                target: ref(qualifiedName: "refs/heads/feature") { target { oid } }
            }
        }"#,
        serde_json::json!({"owner": namespace, "name": repo}),
    )
    .await;
    assert!(compared["errors"].is_null(), "compare: {compared}");
    let repository = &compared["data"]["repository"];
    assert_eq!(
        repository["ahead"]["compare"],
        serde_json::json!({"aheadBy": 1, "behindBy": 0, "status": "AHEAD"})
    );
    assert_eq!(repository["same"]["compare"]["status"], "IDENTICAL");
    assert_eq!(
        repository["missing"],
        serde_json::Value::Null,
        "a missing branch is null"
    );
    assert_eq!(
        repository["target"]["target"]["oid"]
            .as_str()
            .unwrap()
            .len(),
        40
    );
}

#[actix_web::test]
async fn reactions_and_comment_edits_round_trip() {
    let root = format!("/tmp/test_twig_pr_reactions_{}", uuid::Uuid::new_v4());
    let (app, auth, _repository_id, id, number, namespace, repo, _db, _user) =
        seeded_request(&root).await;

    let comment = graphql(
        &app,
        &auth,
        r#"mutation($subject: ID!) {
            addComment(input: {subjectId: $subject, body: "First."}) {
                commentEdge { node { id body } }
            }
        }"#,
        serde_json::json!({"subject": id}),
    )
    .await;
    assert!(comment["errors"].is_null(), "comment: {comment}");
    let comment_id = comment["data"]["addComment"]["commentEdge"]["node"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // React to the request and to its comment, then take the request's reaction back.
    for subject in [&id, &comment_id] {
        let added = graphql(
            &app,
            &auth,
            r"mutation($subject: ID!) {
                addReaction(input: {subjectId: $subject, content: HEART}) { reaction { content } }
            }",
            serde_json::json!({"subject": subject}),
        )
        .await;
        assert!(added["errors"].is_null(), "add: {added}");
        assert_eq!(added["data"]["addReaction"]["reaction"]["content"], "HEART");
    }
    let reacted = graphql(
        &app,
        &auth,
        r"query($owner: String!, $name: String!, $number: Int!) {
            repository(owner: $owner, name: $name) {
                pullRequest(number: $number) {
                    reactionGroups { content viewerHasReacted users { totalCount } }
                    comments(first: 5) { nodes { reactionGroups { content viewerHasReacted } } }
                }
            }
        }",
        serde_json::json!({"owner": namespace, "name": repo, "number": number}),
    )
    .await;
    assert!(reacted["errors"].is_null(), "read: {reacted}");
    let pull = &reacted["data"]["repository"]["pullRequest"];
    let heart = pull["reactionGroups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["content"] == "HEART")
        .expect("heart group");
    assert_eq!(heart["viewerHasReacted"], true);
    assert_eq!(heart["users"]["totalCount"], 1);
    assert_eq!(
        pull["reactionGroups"].as_array().unwrap().len(),
        8,
        "every reaction is listed, as GitHub lists them"
    );
    let comment_heart = pull["comments"]["nodes"][0]["reactionGroups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["content"] == "HEART")
        .unwrap();
    assert_eq!(comment_heart["viewerHasReacted"], true);

    let removed = graphql(
        &app,
        &auth,
        r"mutation($subject: ID!) {
            removeReaction(input: {subjectId: $subject, content: HEART}) { reaction { content } }
        }",
        serde_json::json!({"subject": id}),
    )
    .await;
    assert!(removed["errors"].is_null(), "remove: {removed}");

    // Editing the comment through its node ID changes its text.
    let edited = graphql(
        &app,
        &auth,
        r#"mutation($id: ID!) {
            updateIssueComment(input: {id: $id, body: "Edited."}) {
                issueComment { id body lastEditedAt }
            }
        }"#,
        serde_json::json!({"id": comment_id}),
    )
    .await;
    assert!(edited["errors"].is_null(), "edit: {edited}");
    assert_eq!(
        edited["data"]["updateIssueComment"]["issueComment"]["body"],
        "Edited."
    );
}

#[actix_web::test]
async fn draft_title_and_request_edits_are_made_through_the_api() {
    let root = format!("/tmp/test_twig_pr_draft_{}", uuid::Uuid::new_v4());
    let (app, auth, _repository_id, id, _number, _namespace, _repo, _db, _user) =
        seeded_request(&root).await;

    let drafted = graphql(
        &app,
        &auth,
        r"mutation($id: ID!) {
            convertPullRequestToDraft(input: {pullRequestId: $id}) {
                pullRequest { title isDraft }
            }
        }",
        serde_json::json!({"id": id}),
    )
    .await;
    assert!(drafted["errors"].is_null(), "draft: {drafted}");
    assert_eq!(
        drafted["data"]["convertPullRequestToDraft"]["pullRequest"],
        serde_json::json!({"title": "WIP: Add feature", "isDraft": true})
    );

    let ready = graphql(
        &app,
        &auth,
        r"mutation($id: ID!) {
            markPullRequestReadyForReview(input: {pullRequestId: $id}) {
                pullRequest { title isDraft }
            }
        }",
        serde_json::json!({"id": id}),
    )
    .await;
    assert!(ready["errors"].is_null(), "ready: {ready}");
    assert_eq!(
        ready["data"]["markPullRequestReadyForReview"]["pullRequest"],
        serde_json::json!({"title": "Add feature", "isDraft": false})
    );

    let updated = graphql(
        &app,
        &auth,
        r#"mutation($id: ID!) {
            updatePullRequest(input: {pullRequestId: $id, title: "Renamed", body: "New body"}) {
                pullRequest { title body }
            }
        }"#,
        serde_json::json!({"id": id}),
    )
    .await;
    assert!(updated["errors"].is_null(), "update: {updated}");
    assert_eq!(
        updated["data"]["updatePullRequest"]["pullRequest"],
        serde_json::json!({"title": "Renamed", "body": "New body"})
    );
}

#[actix_web::test]
async fn operations_twig_cannot_perform_fail_as_unsupported() {
    let root = format!("/tmp/test_twig_pr_unsupported_{}", uuid::Uuid::new_v4());
    let (app, auth, _repository_id, id, _number, _namespace, _repo, _db, _user) =
        seeded_request(&root).await;

    let auto_merge = graphql(
        &app,
        &auth,
        r"mutation($id: ID!) {
            enablePullRequestAutoMerge(input: {pullRequestId: $id}) { pullRequest { number } }
        }",
        serde_json::json!({"id": id}),
    )
    .await;
    assert_eq!(auto_merge["errors"][0]["extensions"]["type"], "UNSUPPORTED");
    assert_eq!(
        auto_merge["errors"][0]["message"],
        "Auto-merge is not supported on Twig."
    );

    let revert = graphql(
        &app,
        &auth,
        r"mutation($id: ID!) {
            revertPullRequest(input: {pullRequestId: $id}) { revertPullRequest { id } }
        }",
        serde_json::json!({"id": id}),
    )
    .await;
    assert_eq!(revert["errors"][0]["extensions"]["type"], "UNSUPPORTED");
}

#[actix_web::test]
async fn node_lookups_resolve_requests_and_repositories() {
    let root = format!("/tmp/test_twig_pr_nodes_{}", uuid::Uuid::new_v4());
    let (app, auth, repository_id, id, number, namespace, repo, _db, _user) =
        seeded_request(&root).await;

    let found = graphql(
        &app,
        &auth,
        r"query($id: ID!, $repo: ID!, $ids: [ID!]!) {
            node(id: $id) { __typename id ... on PullRequest { number } }
            nodes(ids: $ids) { __typename }
            repo: node(id: $repo) { __typename ... on Repository { nameWithOwner } }
        }",
        serde_json::json!({"id": id, "repo": repository_id, "ids": [repository_id]}),
    )
    .await;
    assert!(found["errors"].is_null(), "nodes: {found}");
    assert_eq!(found["data"]["node"]["__typename"], "PullRequest");
    assert_eq!(found["data"]["node"]["number"], number);
    assert_eq!(found["data"]["nodes"][0]["__typename"], "Repository");
    assert_eq!(
        found["data"]["repo"]["nameWithOwner"],
        format!("{namespace}/{repo}")
    );
}

#[actix_web::test]
async fn schema_names_the_types_clients_use() {
    let root = format!("/tmp/test_twig_schema_names_{}", uuid::Uuid::new_v4());
    let (app, auth, _repository_id, _id, _number, _namespace, _repo, _db, _user) =
        seeded_request(&root).await;

    let types = graphql(
        &app,
        &auth,
        r#"{
            team: __type(name: "Team") { fields { name } }
            reactable: __type(name: "Reactable") { kind }
            timeline: __type(name: "PullRequestTimelineItems") { kind }
        }"#,
        serde_json::json!({}),
    )
    .await;
    assert!(types["errors"].is_null(), "types: {types}");
    let fields: Vec<&str> = types["data"]["team"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["name"].as_str().unwrap())
        .collect();
    assert!(
        fields.contains(&"avatarUrl"),
        "Team has avatarUrl: {fields:?}"
    );
    assert_eq!(types["data"]["reactable"]["kind"], "INTERFACE");
    assert_eq!(types["data"]["timeline"]["kind"], "UNION");
}
