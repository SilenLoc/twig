use super::support::*;
use super::*;
use crate::auth::{NamespacePermission, PermissionSet};

pub(super) fn token_header(token: &str) -> (&'static str, String) {
    ("Authorization", format!("token {token}"))
}

/// Seeds a user, a namespace they own, and a bare repository on disk whose `main`
/// holds the `.twig.toml` commit. Returns the user, namespace, and repository names.
pub(super) async fn seed_owner_repo(
    db: &crate::db::Database,
    root: &str,
) -> (crate::auth::User, String, String) {
    let user = auth::create_user(
        "pr-owner".to_string(),
        "pr-owner@example.com".to_string(),
        "password123",
    )
    .expect("create owner");
    db.create_user(&user).await.expect("persist owner");
    let namespace = auth::create_namespace("pr-team".to_string(), user.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("persist namespace");
    let repo_path = std::path::Path::new(root).join("pr-team").join("tooling");
    std::fs::create_dir_all(&repo_path).expect("repository dir");
    twig_git::repo::bare_init(&repo_path, "main", "Test", "test@example.com")
        .expect("init bare repo");
    (user, "pr-team".to_string(), "tooling".to_string())
}

/// Creates a branch one commit ahead of `main`, adding `file` with `content`.
pub(super) fn push_branch(root: &str, branch: &str, file: &str, content: &str) {
    let repo_path = std::path::Path::new(root).join("pr-team").join("tooling");
    let repo = git2::Repository::open_bare(&repo_path).expect("open bare repo");
    let main = repo
        .find_reference("refs/heads/main")
        .expect("main")
        .peel_to_commit()
        .expect("main commit");
    let mut builder = repo
        .treebuilder(Some(&main.tree().expect("main tree")))
        .expect("tree builder");
    let blob = repo.blob(content.as_bytes()).expect("blob");
    builder.insert(file, blob, 0o100_644).expect("insert file");
    let tree = repo
        .find_tree(builder.write().expect("write tree"))
        .expect("tree");
    let signature = git2::Signature::now("Twig Test", "twig@example.com").expect("signature");
    repo.commit(
        Some(&format!("refs/heads/{branch}")),
        &signature,
        &signature,
        &format!("Add {file}"),
        &tree,
        &[&main],
    )
    .expect("commit branch");
}

pub(super) async fn token_for(
    db: &crate::db::Database,
    user: &auth::User,
    namespace: &str,
    permissions: &[NamespacePermission],
) -> String {
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let ns = db
        .get_namespace_by_name(namespace)
        .await
        .expect("load namespace")
        .expect("namespace exists");
    let (_, token) = auth_state
        .issue_personal_access_token(
            &user.id,
            "fj",
            std::slice::from_ref(&ns.id),
            &PermissionSet::new(permissions.iter().copied()),
            None,
        )
        .await
        .expect("issue token");
    token
}

#[actix_web::test]
async fn compat_user_and_version_accept_token_scheme() {
    let root = format!("/tmp/test_twig_compat_user_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, _namespace, _repo) = seed_owner_repo(&db, &root).await;
    let token = token_for(&db, &user, "pr-team", &[NamespacePermission::ViewNamespace]).await;

    let (name, value) = token_header(&token);
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/user")
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(response).await;
    assert_eq!(body["login"], user.username);
    assert_eq!(body["username"], user.username);
    assert!(body["id"].is_i64(), "Forgejo clients expect an integer id");
    assert!(body.get("password_hash").is_none());

    let version = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/version")
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(version.status(), StatusCode::OK);
    let version_body: serde_json::Value = test::read_body_json(version).await;
    assert_eq!(version_body["version"], env!("CARGO_PKG_VERSION"));

    let anonymous = test::call_service(
        &app,
        test::TestRequest::get().uri("/api/v1/user").to_request(),
    )
    .await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let anonymous_body: serde_json::Value = test::read_body_json(anonymous).await;
    assert!(anonymous_body["message"].is_string(), "Forgejo error shape");
    assert_eq!(anonymous_body["url"], "");

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn compat_pull_request_create_list_and_get_follow_forgejo_shapes() {
    let root = format!("/tmp/test_twig_compat_pr_{}", uuid::Uuid::new_v4());
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
    let reader = token_for(
        &db,
        &user,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    let pulls_uri = format!("/api/v1/repos/{namespace}/{repo}/pulls");

    let (name, value) = token_header(&writer);
    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&pulls_uri)
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({
                "title": "Add retry",
                "body": "Explains the policy.",
                "head": "feature/retry",
                "base": "main"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let pull: serde_json::Value = test::read_body_json(created).await;
    assert_eq!(pull["number"], 1);
    assert_eq!(pull["state"], "open");
    assert_eq!(pull["title"], "Add retry");
    assert_eq!(pull["head"]["ref"], "feature/retry");
    assert_eq!(pull["base"]["ref"], "main");
    assert_eq!(pull["user"]["login"], user.username);
    assert_eq!(pull["draft"], false);
    assert_eq!(pull["merged"], false);
    assert!(
        pull["html_url"]
            .as_str()
            .unwrap()
            .ends_with("/pr-team/tooling/pulls/1")
    );
    assert_eq!(pull["head"]["sha"].as_str().unwrap().len(), 40);
    assert_eq!(
        pull["head"]["repo"]["full_name"], "pr-team/tooling",
        "same-repository request"
    );

    let duplicate = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&pulls_uri)
            .insert_header((name, value.as_str()))
            .set_json(
                serde_json::json!({"title": "Again", "head": "feature/retry", "base": "main"}),
            )
            .to_request(),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let duplicate_body: serde_json::Value = test::read_body_json(duplicate).await;
    assert!(
        duplicate_body["message"]
            .as_str()
            .unwrap()
            .contains("already exists")
    );

    let same = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&pulls_uri)
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"title": "Same", "head": "main", "base": "main"}))
            .to_request(),
    )
    .await;
    assert_eq!(same.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let (reader_name, reader_value) = token_header(&reader);
    let denied = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&pulls_uri)
            .insert_header((reader_name, reader_value.as_str()))
            .set_json(serde_json::json!({"title": "Nope", "head": "feature/retry", "base": "main"}))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let listed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&pulls_uri)
            .insert_header((reader_name, reader_value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(listed.status(), StatusCode::OK);
    let rows: serde_json::Value = test::read_body_json(listed).await;
    assert_eq!(rows.as_array().map(Vec::len), Some(1));
    assert_eq!(rows[0]["number"], 1);

    let fetched = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{pulls_uri}/1"))
            .insert_header((reader_name, reader_value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_body: serde_json::Value = test::read_body_json(fetched).await;
    assert_eq!(fetched_body["title"], "Add retry");
    assert!(fetched_body["merge_base"].is_string());

    let missing = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{pulls_uri}/99"))
            .insert_header((reader_name, reader_value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let missing_body: serde_json::Value = test::read_body_json(missing).await;
    assert_eq!(missing_body["message"], "The target couldn't be found");

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn compat_pull_request_list_pages_with_link_header_and_state_filter() {
    let root = format!("/tmp/test_twig_compat_page_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    for branch in ["one", "two", "three"] {
        push_branch(&root, branch, &format!("{branch}.txt"), branch);
    }
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
    let pulls_uri = format!("/api/v1/repos/{namespace}/{repo}/pulls");
    for branch in ["one", "two", "three"] {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&pulls_uri)
                .insert_header((name, value.as_str()))
                .set_json(serde_json::json!({"title": branch, "head": branch, "base": "main"}))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED, "create {branch}");
    }

    let first = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{pulls_uri}?limit=2"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let link = first
        .headers()
        .get("Link")
        .expect("a next page exists")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        link.contains("page=2") && link.contains("limit=2") && link.contains("rel=\"next\""),
        "{link}"
    );
    let first_rows: serde_json::Value = test::read_body_json(first).await;
    assert_eq!(first_rows.as_array().map(Vec::len), Some(2));
    assert_eq!(first_rows[0]["number"], 3, "newest request first");

    let second = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{pulls_uri}?limit=2&page=2"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert!(
        second.headers().get("Link").is_none(),
        "last page has no next link"
    );
    let second_rows: serde_json::Value = test::read_body_json(second).await;
    assert_eq!(second_rows.as_array().map(Vec::len), Some(1));

    let closed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{pulls_uri}?state=closed"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let closed_rows: serde_json::Value = test::read_body_json(closed).await;
    assert_eq!(closed_rows.as_array().map(Vec::len), Some(0));

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn compat_pull_requests_are_refused_to_tokens_without_namespace_scope() {
    let root = format!("/tmp/test_twig_compat_scope_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let (_, unscoped) = auth_state
        .issue_personal_access_token(
            &user.id,
            "elsewhere",
            &[],
            &PermissionSet::new([NamespacePermission::ViewNamespace]),
            None,
        )
        .await
        .expect("issue unscoped token");

    let (name, value) = token_header(&unscoped);
    let denied = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/pulls"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let body: serde_json::Value = test::read_body_json(denied).await;
    assert!(body["message"].is_string());

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn compat_repository_branch_and_metadata_routes_match_what_clients_read() {
    let root = format!("/tmp/test_twig_compat_meta_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");
    let reader = token_for(
        &db,
        &user,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
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

    let (name, value) = token_header(&reader);
    let repository = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(repository.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(repository).await;
    assert_eq!(body["full_name"], "pr-team/tooling");
    assert_eq!(body["owner"]["login"], "pr-team");
    assert_eq!(body["default_branch"], "main");
    assert!(
        body["clone_url"]
            .as_str()
            .unwrap()
            .ends_with("/pr-team/tooling.git")
    );
    assert_eq!(body["ssh_url"], "");
    assert_eq!(body["permissions"]["pull"], true);
    assert_eq!(
        body["permissions"]["push"], false,
        "a read-only token cannot push"
    );

    let (name, value) = token_header(&writer);
    let writable = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let body: serde_json::Value = test::read_body_json(writable).await;
    assert_eq!(body["permissions"]["push"], true);

    let branch = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/v1/repos/{namespace}/{repo}/branches/feature/retry"
            ))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(
        branch.status(),
        StatusCode::OK,
        "branch names with slashes are routed"
    );
    let branch_body: serde_json::Value = test::read_body_json(branch).await;
    assert_eq!(branch_body["name"], "feature/retry");
    assert_eq!(branch_body["commit"]["id"].as_str().unwrap().len(), 40);
    assert_eq!(branch_body["enable_status_check"], false);
    assert_eq!(branch_body["status_check_contexts"], serde_json::json!([]));

    let missing = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/branches/nope"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    for suffix in ["statuses/abc123", "labels"] {
        let empty = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/repos/{namespace}/{repo}/{suffix}"))
                .insert_header((name, value.as_str()))
                .to_request(),
        )
        .await;
        assert_eq!(empty.status(), StatusCode::OK, "{suffix}");
        let list: serde_json::Value = test::read_body_json(empty).await;
        assert_eq!(list, serde_json::json!([]), "{suffix}");
    }

    let assignees = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/{namespace}/{repo}/assignees"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let members: serde_json::Value = test::read_body_json(assignees).await;
    let logins: Vec<&str> = members
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member["login"].as_str().unwrap())
        .collect();
    assert_eq!(logins, vec![user.username.as_str()]);
    assert!(
        members[0].get("email").is_some_and(|email| email == ""),
        "member emails stay private"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// Creates a second user who is a contributor of `namespace`.
pub(super) async fn seed_contributor(
    db: &crate::db::Database,
    namespace: &str,
    username: &str,
) -> crate::auth::User {
    let user = auth::create_user(
        username.to_string(),
        format!("{username}@example.com"),
        "password123",
    )
    .expect("create contributor");
    db.create_user(&user).await.expect("persist contributor");
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
             VALUES (?1, ?2, 'contributor', '2026-10-30T00:00:00Z')",
            crate::db::sqlite::params![ns.id, user.id],
        )
        .await
        .expect("add contributor membership");
    user
}

#[actix_web::test]
async fn compat_conversation_comments_reactions_and_state_follow_the_permission_rules() {
    let root = format!("/tmp/test_twig_compat_conv_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    let contributor = seed_contributor(&db, &namespace, "pr-contributor").await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");
    let owner_token = token_for(
        &db,
        &owner,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let contributor_token = token_for(
        &db,
        &contributor,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let viewer_token = token_for(
        &db,
        &contributor,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    let (owner_name, owner_value) = token_header(&owner_token);
    let (contributor_name, contributor_value) = token_header(&contributor_token);
    let (viewer_name, viewer_value) = token_header(&viewer_token);
    let base = format!("/api/v1/repos/{namespace}/{repo}");

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(
                serde_json::json!({"title": "Retry", "head": "feature/retry", "base": "main"}),
            )
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    // Any viewer may comment; the author of a comment may edit it.
    let comment = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/issues/1/comments"))
            .insert_header((contributor_name, contributor_value.as_str()))
            .set_json(serde_json::json!({"body": "Looks **good**."}))
            .to_request(),
    )
    .await;
    assert_eq!(comment.status(), StatusCode::CREATED);
    let comment: serde_json::Value = test::read_body_json(comment).await;
    let comment_id = comment["id"].as_i64().unwrap();
    assert_eq!(comment["user"]["login"], contributor.username);
    assert!(
        comment["html_url"]
            .as_str()
            .unwrap()
            .ends_with(&format!("#issuecomment-{comment_id}"))
    );

    let edited = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("{base}/issues/comments/{comment_id}"))
            .insert_header((contributor_name, contributor_value.as_str()))
            .set_json(serde_json::json!({"body": "Looks good now."}))
            .to_request(),
    )
    .await;
    assert_eq!(edited.status(), StatusCode::OK);

    let not_author = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("{base}/issues/comments/{comment_id}"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"body": "Owner rewrite"}))
            .to_request(),
    )
    .await;
    assert_eq!(
        not_author.status(),
        StatusCode::FORBIDDEN,
        "only the author edits a comment"
    );

    let listed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/issues/1/comments"))
            .insert_header((viewer_name, viewer_value.as_str()))
            .to_request(),
    )
    .await;
    let comments: serde_json::Value = test::read_body_json(listed).await;
    assert_eq!(comments[0]["body"], "Looks good now.");

    // A reaction is added once, listed with its user, and removed again.
    let reacted = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/issues/1/reactions"))
            .insert_header((contributor_name, contributor_value.as_str()))
            .set_json(serde_json::json!({"content": "+1"}))
            .to_request(),
    )
    .await;
    assert_eq!(reacted.status(), StatusCode::CREATED);
    let unknown = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/issues/1/reactions"))
            .insert_header((contributor_name, contributor_value.as_str()))
            .set_json(serde_json::json!({"content": "thumbs"}))
            .to_request(),
    )
    .await;
    assert_eq!(unknown.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let reactions = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/issues/1/reactions"))
            .insert_header((viewer_name, viewer_value.as_str()))
            .to_request(),
    )
    .await;
    let reactions: serde_json::Value = test::read_body_json(reactions).await;
    assert_eq!(reactions[0]["content"], "+1");
    assert_eq!(reactions[0]["user"]["login"], contributor.username);
    let removed = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("{base}/issues/1/reactions"))
            .insert_header((contributor_name, contributor_value.as_str()))
            .set_json(serde_json::json!({"content": "+1"}))
            .to_request(),
    )
    .await;
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);

    // The author's owner-side delete of someone else's comment is allowed because
    // the owner can push.
    let moderated = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("{base}/issues/comments/{comment_id}"))
            .insert_header((owner_name, owner_value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(moderated.status(), StatusCode::NO_CONTENT);

    // Closing and reopening belong to the author or to anyone who can push.
    let viewer_close = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("{base}/pulls/1"))
            .insert_header((viewer_name, viewer_value.as_str()))
            .set_json(serde_json::json!({"state": "closed"}))
            .to_request(),
    )
    .await;
    assert_eq!(viewer_close.status(), StatusCode::FORBIDDEN);
    let closed = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("{base}/pulls/1"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"state": "closed"}))
            .to_request(),
    )
    .await;
    assert_eq!(closed.status(), StatusCode::OK);
    let closed_body: serde_json::Value = test::read_body_json(closed).await;
    assert_eq!(closed_body["state"], "closed");
    assert!(closed_body["closed_at"].is_string());
    let reopened = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("{base}/pulls/1"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"state": "open", "title": "Retry, revised"}))
            .to_request(),
    )
    .await;
    let reopened_body: serde_json::Value = test::read_body_json(reopened).await;
    assert_eq!(reopened_body["state"], "open");
    assert_eq!(reopened_body["closed_at"], serde_json::Value::Null);
    assert_eq!(reopened_body["title"], "Retry, revised");

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn compat_reviews_inline_comments_and_requested_reviewers_follow_forgejo_shapes() {
    let root = format!("/tmp/test_twig_compat_reviews_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    let contributor = seed_contributor(&db, &namespace, "pr-reviewer").await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");
    let owner_token = token_for(
        &db,
        &owner,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let reviewer_token = token_for(
        &db,
        &contributor,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let viewer_token = token_for(
        &db,
        &contributor,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    let (owner_name, owner_value) = token_header(&owner_token);
    let (reviewer_name, reviewer_value) = token_header(&reviewer_token);
    let (viewer_name, viewer_value) = token_header(&viewer_token);
    let base = format!("/api/v1/repos/{namespace}/{repo}");

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(
                serde_json::json!({"title": "Retry", "head": "feature/retry", "base": "main"}),
            )
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    // Authors cannot approve their own request.
    let self_approve = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/reviews"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"event": "APPROVED", "body": "Ship it"}))
            .to_request(),
    )
    .await;
    assert_eq!(self_approve.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // A viewer without push rights cannot review.
    let viewer_review = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/reviews"))
            .insert_header((viewer_name, viewer_value.as_str()))
            .set_json(serde_json::json!({"event": "COMMENT", "body": "Hi"}))
            .to_request(),
    )
    .await;
    assert_eq!(viewer_review.status(), StatusCode::FORBIDDEN);

    // A comment review with nothing in it is refused.
    let empty = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/reviews"))
            .insert_header((reviewer_name, reviewer_value.as_str()))
            .set_json(serde_json::json!({"event": "COMMENT", "body": "  "}))
            .to_request(),
    )
    .await;
    assert_eq!(empty.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Requesting changes with an inline comment stores both.
    let changes = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/reviews"))
            .insert_header((reviewer_name, reviewer_value.as_str()))
            .set_json(serde_json::json!({
                "event": "REQUEST_CHANGES",
                "body": "Please cap the total wait.",
                "comments": [{"path": "retry.txt", "body": "Cap this", "old_position": 0, "new_position": 1}]
            }))
            .to_request(),
    )
    .await;
    assert_eq!(changes.status(), StatusCode::OK);
    let review: serde_json::Value = test::read_body_json(changes).await;
    assert_eq!(review["state"], "REQUEST_CHANGES");
    assert_eq!(review["user"]["login"], contributor.username);
    assert_eq!(review["comments_count"], 1);
    assert_eq!(review["commit_id"].as_str().unwrap().len(), 40);
    let review_id = review["id"].as_i64().unwrap();

    let reviews = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1/reviews"))
            .insert_header((viewer_name, viewer_value.as_str()))
            .to_request(),
    )
    .await;
    let reviews: serde_json::Value = test::read_body_json(reviews).await;
    assert_eq!(reviews.as_array().map(Vec::len), Some(1));

    let inline = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1/reviews/{review_id}/comments"))
            .insert_header((viewer_name, viewer_value.as_str()))
            .to_request(),
    )
    .await;
    let inline: serde_json::Value = test::read_body_json(inline).await;
    assert_eq!(inline[0]["path"], "retry.txt");
    assert_eq!(inline[0]["position"], 1);
    assert_eq!(inline[0]["original_position"], 0);
    assert_eq!(inline[0]["body"], "Cap this");

    // Inline review comments are not conversation comments.
    let conversation = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/issues/1/comments"))
            .insert_header((viewer_name, viewer_value.as_str()))
            .to_request(),
    )
    .await;
    let conversation: serde_json::Value = test::read_body_json(conversation).await;
    assert_eq!(conversation, serde_json::json!([]));

    // Requested reviewers must be namespace members, and appear on the request.
    let stranger = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/requested_reviewers"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"reviewers": ["nobody-here"]}))
            .to_request(),
    )
    .await;
    assert_eq!(stranger.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let requested = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/requested_reviewers"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"reviewers": [contributor.username]}))
            .to_request(),
    )
    .await;
    assert_eq!(requested.status(), StatusCode::CREATED);
    let pull = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1"))
            .insert_header((owner_name, owner_value.as_str()))
            .to_request(),
    )
    .await;
    let pull: serde_json::Value = test::read_body_json(pull).await;
    assert_eq!(
        pull["requested_reviewers"][0]["login"],
        contributor.username
    );
    assert_eq!(
        pull["comments"], 0,
        "conversation comment count excludes review comments"
    );

    let withdrawn = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri(&format!("{base}/pulls/1/requested_reviewers"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"reviewers": [contributor.username]}))
            .to_request(),
    )
    .await;
    assert_eq!(withdrawn.status(), StatusCode::NO_CONTENT);

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn compat_code_review_routes_serve_diffs_commits_and_contents() {
    use base64::Engine;
    let root = format!("/tmp/test_twig_compat_code_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    push_branch(&root, "feature/retry", "retry.txt", "retry policy\n");
    let token = token_for(
        &db,
        &user,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    let (name, value) = token_header(&token);
    let base = format!("/api/v1/repos/{namespace}/{repo}");
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
    let (write_name, write_value) = token_header(&writer);
    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls"))
            .insert_header((write_name, write_value.as_str()))
            .set_json(
                serde_json::json!({"title": "Retry", "head": "feature/retry", "base": "main"}),
            )
            .to_request(),
    )
    .await;
    let pull: serde_json::Value = test::read_body_json(created).await;
    let head_sha = pull["head"]["sha"].as_str().unwrap().to_string();

    let patch = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1.diff"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(patch.status(), StatusCode::OK);
    assert_eq!(
        patch.headers().get("content-type").unwrap(),
        "text/plain; charset=utf-8"
    );
    let text = String::from_utf8(test::read_body(patch).await.to_vec()).unwrap();
    assert!(
        text.contains("diff --git a/retry.txt b/retry.txt"),
        "{text}"
    );
    assert!(text.contains("+retry policy"), "{text}");

    let commits = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1/commits"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let commits: serde_json::Value = test::read_body_json(commits).await;
    assert_eq!(commits.as_array().map(Vec::len), Some(1));
    assert_eq!(commits[0]["sha"], head_sha);
    assert_eq!(commits[0]["commit"]["message"], "Add retry.txt");
    assert!(commits[0]["commit"]["committer"]["date"].is_string());

    let commit = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/git/commits/{head_sha}"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let commit: serde_json::Value = test::read_body_json(commit).await;
    assert_eq!(commit["parents"].as_array().map(Vec::len), Some(1));

    let commit_diff = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/git/commits/{head_sha}.diff"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let commit_diff = String::from_utf8(test::read_body(commit_diff).await.to_vec()).unwrap();
    assert!(commit_diff.contains("+++ b/retry.txt"));

    let contents = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/contents/retry.txt?ref=feature/retry"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(contents.status(), StatusCode::OK);
    let contents: serde_json::Value = test::read_body_json(contents).await;
    assert_eq!(contents["encoding"], "base64");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(contents["content"].as_str().unwrap())
        .unwrap();
    assert_eq!(decoded, b"retry policy\n");

    let missing = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/contents/nope.txt?ref=feature/retry"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn compat_merge_styles_conflicts_and_status_follow_the_rules() {
    let root = format!("/tmp/test_twig_compat_merge_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    let contributor = seed_contributor(&db, &namespace, "pr-merger").await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");
    push_branch(&root, "feature/clash", "shared.txt", "from feature");
    push_branch(&root, "feature/squash", "squash.txt", "squash");
    // Cut before any merge, so it adds retry.txt the way feature/retry does: a true conflict later.
    push_branch(
        &root,
        "feature/clash-two",
        "retry.txt",
        "conflicting content",
    );
    let owner_token = token_for(
        &db,
        &owner,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let viewer_token = token_for(
        &db,
        &contributor,
        &namespace,
        &[NamespacePermission::ViewNamespace],
    )
    .await;
    let (owner_name, owner_value) = token_header(&owner_token);
    let (viewer_name, viewer_value) = token_header(&viewer_token);
    let base = format!("/api/v1/repos/{namespace}/{repo}");
    for (number, branch) in [
        (1, "feature/retry"),
        (2, "feature/clash"),
        (3, "feature/squash"),
        (4, "feature/clash-two"),
    ] {
        let created = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("{base}/pulls"))
                .insert_header((owner_name, owner_value.as_str()))
                .set_json(serde_json::json!({"title": branch, "head": branch, "base": "main"}))
                .to_request(),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED, "#{number}");
    }

    let clean_pull = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1"))
            .insert_header((owner_name, owner_value.as_str()))
            .to_request(),
    )
    .await;
    let clean_pull: serde_json::Value = test::read_body_json(clean_pull).await;
    assert_eq!(clean_pull["mergeable"], true);

    // Before merging, the status endpoint reports not merged.
    let before = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1/merge"))
            .insert_header((owner_name, owner_value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(before.status(), StatusCode::NOT_FOUND);

    // A viewer cannot merge.
    let viewer = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/merge"))
            .insert_header((viewer_name, viewer_value.as_str()))
            .set_json(serde_json::json!({"Do": "merge"}))
            .to_request(),
    )
    .await;
    assert_eq!(viewer.status(), StatusCode::FORBIDDEN);

    let merged = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/merge"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"Do": "merge"}))
            .to_request(),
    )
    .await;
    assert_eq!(merged.status(), StatusCode::OK);

    let status = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1/merge"))
            .insert_header((owner_name, owner_value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(status.status(), StatusCode::NO_CONTENT);
    let pull = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/pulls/1"))
            .insert_header((owner_name, owner_value.as_str()))
            .to_request(),
    )
    .await;
    let pull: serde_json::Value = test::read_body_json(pull).await;
    assert_eq!(pull["state"], "closed");
    assert_eq!(pull["merged"], true);
    assert_eq!(pull["merged_by"]["login"], owner.username);
    assert_eq!(pull["merge_commit_sha"].as_str().unwrap().len(), 40);
    assert_eq!(
        pull["mergeable"],
        serde_json::Value::Null,
        "merged requests have no mergeability"
    );

    let again = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/merge"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"Do": "merge"}))
            .to_request(),
    )
    .await;
    assert_eq!(again.status(), StatusCode::CONFLICT);

    let reopen = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&format!("{base}/pulls/1"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"state": "open"}))
            .to_request(),
    )
    .await;
    assert_eq!(
        reopen.status(),
        StatusCode::CONFLICT,
        "merged requests cannot be reopened"
    );

    // Request 2 conflicts with the merged base: refused, and main keeps its tip.
    let main_before =
        git2::Repository::open_bare(std::path::Path::new(&root).join("pr-team").join("tooling"))
            .unwrap()
            .find_reference("refs/heads/main")
            .unwrap()
            .target()
            .unwrap();
    let conflicting = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/4/merge"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"Do": "merge"}))
            .to_request(),
    )
    .await;
    assert_eq!(conflicting.status(), StatusCode::CONFLICT);
    let message: serde_json::Value = test::read_body_json(conflicting).await;
    assert_eq!(message["message"], "Merge conflicts in: retry.txt");

    let main_after =
        git2::Repository::open_bare(std::path::Path::new(&root).join("pr-team").join("tooling"))
            .unwrap()
            .find_reference("refs/heads/main")
            .unwrap()
            .target()
            .unwrap();
    assert_eq!(
        main_before, main_after,
        "a refused merge leaves the base untouched"
    );

    // Squash is a valid style and produces a single-parent commit on main.
    let squashed = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/3/merge"))
            .insert_header((owner_name, owner_value.as_str()))
            .set_json(serde_json::json!({"Do": "squash", "MergeTitleField": "Squashed"}))
            .to_request(),
    )
    .await;
    assert_eq!(squashed.status(), StatusCode::OK);
    let repository =
        git2::Repository::open_bare(std::path::Path::new(&root).join("pr-team").join("tooling"))
            .unwrap();
    let tip = repository
        .find_reference("refs/heads/main")
        .unwrap()
        .peel_to_commit()
        .unwrap();
    assert_eq!(tip.parent_count(), 1);
    assert_eq!(
        tip.summary().unwrap(),
        Some("Squashed"),
        "a client's title is used as given"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn t3_provider_requests_page_to_an_end_and_never_repeat_rows() {
    // Mirrors the calls T3 Code's Forgejo provider makes on a repository with a request,
    // including `page=` and `limit=50`. T3 stops only on an empty page or one without
    // rel="next", so every paged list must end and must not repeat rows.
    let root = format!("/tmp/test_twig_t3_replay_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    let contributor = seed_contributor(&db, &namespace, "pr-t3-reviewer").await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");
    let owner_token = token_for(
        &db,
        &owner,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let (name, value) = token_header(&owner_token);
    let base = format!("/api/v1/repos/{namespace}/{repo}");
    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls"))
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({"title": "Retry", "head": format!("{namespace}:feature/retry"), "base": "main"}))
            .to_request(),
    )
    .await;
    assert_eq!(
        created.status(),
        StatusCode::CREATED,
        "T3 sends head as owner:branch"
    );
    let pull: serde_json::Value = test::read_body_json(created).await;
    let sha = pull["head"]["sha"].as_str().unwrap().to_string();

    // Comments and reactions, enough to span pages at limit=1.
    for body in ["first", "second", "third"] {
        test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("{base}/issues/1/comments"))
                .insert_header((name, value.as_str()))
                .set_json(serde_json::json!({"body": body}))
                .to_request(),
        )
        .await;
    }
    let reviewer_token = token_for(
        &db,
        &contributor,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let (reviewer_name, reviewer_value) = token_header(&reviewer_token);
    for event in ["COMMENT", "COMMENT"] {
        let review = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("{base}/pulls/1/reviews"))
                .insert_header((reviewer_name, reviewer_value.as_str()))
                .set_json(
                    serde_json::json!({"event": event, "body": "Looks fine", "commit_id": sha}),
                )
                .to_request(),
        )
        .await;
        assert_eq!(review.status(), StatusCode::OK);
    }

    // Reactions on the request and on its first comment, so the reaction lists have rows.
    for (uri, content) in [
        (format!("{base}/issues/1/reactions"), "heart"),
        (format!("{base}/issues/1/reactions"), "+1"),
    ] {
        let reacted = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&uri)
                .insert_header((reviewer_name, reviewer_value.as_str()))
                .set_json(serde_json::json!({"content": content}))
                .to_request(),
        )
        .await;
        assert_eq!(reacted.status(), StatusCode::CREATED, "{content}");
    }

    // Walk every paged list the way T3 does: pages of 1 until an empty page. Each list
    // must end, yield the expected count, and never repeat a row.
    let walks = [
        (
            format!("{base}/pulls?state=open&head=feature%2Fretry&sort=recentupdate"),
            1,
        ),
        (format!("{base}/pulls/1/reviews"), 2),
        (
            format!("{base}/pulls/1/commits?files=false&verification=false"),
            1,
        ),
        (format!("{base}/issues/1/comments"), 3),
        (format!("{base}/issues/1/reactions"), 2),
        (format!("{base}/assignees"), 2),
        (format!("{base}/statuses/{sha}?sort=recentupdate"), 0),
    ];
    for (uri, expected) in walks {
        let separator = if uri.contains('?') { "&" } else { "?" };
        let mut seen: Vec<serde_json::Value> = Vec::new();
        for page in 1..=10 {
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!("{uri}{separator}limit=1&page={page}"))
                    .insert_header((name, value.as_str()))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK, "{uri} page {page}");
            let rows: Vec<serde_json::Value> = test::read_body_json(response).await;
            if rows.is_empty() {
                break;
            }
            for row in rows {
                assert!(!seen.contains(&row), "{uri} repeated a row on page {page}");
                seen.push(row);
            }
            assert!(page < 10, "{uri} never ended");
        }
        assert_eq!(seen.len(), expected, "{uri}");
    }
    let comments = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{base}/issues/1/comments?limit=2&page=1"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(
        comments
            .headers()
            .get("Link")
            .map(|value| value.to_str().unwrap().contains("rel=\"next\"")),
        Some(true)
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn compat_update_user_repo_and_malformed_bodies_follow_forgejo() {
    let root = format!("/tmp/test_twig_compat_misc_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");
    let token = token_for(
        &db,
        &owner,
        &namespace,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let (name, value) = token_header(&token);
    let base = format!("/api/v1/repos/{namespace}/{repo}");
    test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls"))
            .insert_header((name, value.as_str()))
            .set_json(
                serde_json::json!({"title": "Retry", "head": "feature/retry", "base": "main"}),
            )
            .to_request(),
    )
    .await;

    // Malformed JSON gets Forgejo's error shape on compatibility routes.
    let malformed = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls"))
            .insert_header((name, value.as_str()))
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{")
            .to_request(),
    )
    .await;
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = test::read_body_json(malformed).await;
    assert!(body["message"].is_string(), "{body}");

    // Main moves on, so the request is behind; update merges main into the head.
    let main_commit = {
        let repository = git2::Repository::open_bare(
            std::path::Path::new(&root).join("pr-team").join("tooling"),
        )
        .unwrap();
        let tip = repository
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_commit()
            .unwrap();
        let mut builder = repository.treebuilder(Some(&tip.tree().unwrap())).unwrap();
        let blob = repository.blob(b"main moved").unwrap();
        builder.insert("main.txt", blob, 0o100_644).unwrap();
        let tree = repository.find_tree(builder.write().unwrap()).unwrap();
        let signature = git2::Signature::now("Twig Test", "twig@example.com").unwrap();
        repository
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "Main moves",
                &tree,
                &[&tip],
            )
            .unwrap()
    };
    let updated = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/update?style=merge"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let repository =
        git2::Repository::open_bare(std::path::Path::new(&root).join("pr-team").join("tooling"))
            .unwrap();
    let head_tip = repository
        .find_reference("refs/heads/feature/retry")
        .unwrap()
        .peel_to_commit()
        .unwrap();
    assert_eq!(head_tip.parent_count(), 2, "the head now merges main");
    assert_eq!(head_tip.parent_id(1).unwrap(), main_commit);
    let again = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/update"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(again.status(), StatusCode::CONFLICT);
    let rebase = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{base}/pulls/1/update?style=rebase"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(rebase.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Creating a repository in the caller's namespace, which is named after the login.
    let own_namespace = auth::create_namespace(owner.username.clone(), owner.id.clone());
    db.create_namespace(&own_namespace)
        .await
        .expect("namespace named after the login");
    let login_token = token_for(
        &db,
        &owner,
        &owner.username,
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::CreateRepositories,
        ],
    )
    .await;
    let (login_name, login_value) = token_header(&login_token);
    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/user/repos")
            .insert_header((login_name, login_value.as_str()))
            .set_json(serde_json::json!({"name": "fresh"}))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let fresh: serde_json::Value = test::read_body_json(created).await;
    assert_eq!(fresh["full_name"], format!("{}/fresh", owner.username));
    let duplicate = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/user/repos")
            .insert_header((login_name, login_value.as_str()))
            .set_json(serde_json::json!({"name": "fresh"}))
            .to_request(),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let private = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/user/repos")
            .insert_header((login_name, login_value.as_str()))
            .set_json(serde_json::json!({"name": "secret", "private": true}))
            .to_request(),
    )
    .await;
    assert_eq!(private.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let _ = std::fs::remove_dir_all(&root);
}
