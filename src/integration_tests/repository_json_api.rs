use super::support::*;
use super::*;
use crate::auth::{NamespacePermission, PermissionSet};
use crate::integration_tests::tests::repo_content::init_repo_with_files;

fn basic_authorization(username: &str, password: &str) -> String {
    use base64::Engine;
    let credentials =
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
    format!("Basic {credentials}")
}

#[actix_web::test]
async fn api_identity_and_version_require_user_or_token_credentials() {
    let root = format!("/tmp/test_twig_api_auth_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let user = auth::create_user(
        "api-auth-user".to_string(),
        "api-auth@example.com".to_string(),
        "password123",
    )
    .expect("create API user");
    db.create_user(&user).await.expect("persist API user");
    let namespace = auth::create_namespace("api-auth-ns".to_string(), user.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create namespace");
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let session = auth_state
        .create_session(user.id.clone())
        .await
        .expect("create browser session");
    let (token_metadata, personal_access_token) = auth_state
        .issue_personal_access_token(
            &user.id,
            "API integration test",
            std::slice::from_ref(&namespace.id),
            &PermissionSet::new([NamespacePermission::ViewNamespace]),
            None,
        )
        .await
        .expect("create scoped API token");
    assert_eq!(token_metadata.name, "API integration test");

    for uri in ["/api/version", "/api/v1/me"] {
        let response =
            test::call_service(&app, test::TestRequest::get().uri(uri).to_request()).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{uri}");
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
        let body: serde_json::Value = test::read_body_json(response).await;
        assert_eq!(body["error"]["code"], "unauthorized");
    }

    let cookie_only = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/me")
            .cookie(actix_web::cookie::Cookie::new("session", session))
            .to_request(),
    )
    .await;
    assert_eq!(cookie_only.status(), StatusCode::UNAUTHORIZED);

    let invalid = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/me")
            .insert_header((
                "Authorization",
                basic_authorization("api-auth-user", "wrong-password"),
            ))
            .to_request(),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);
    let invalid_body: serde_json::Value = test::read_body_json(invalid).await;
    assert_eq!(invalid_body["error"]["code"], "unauthorized");

    let basic = basic_authorization("api-auth-user", "password123");
    let malformed_json = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/namespaces")
            .insert_header(("Authorization", basic.as_str()))
            .insert_header(("Content-Type", "application/json"))
            .set_payload(b"{".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(malformed_json.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        malformed_json.headers().get("content-type").unwrap(),
        "application/json"
    );
    let malformed_body: serde_json::Value = test::read_body_json(malformed_json).await;
    assert_eq!(malformed_body["error"]["code"], "bad_request");

    let version = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/version")
            .insert_header(("Authorization", basic.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(version.status(), StatusCode::OK);
    let version_body: serde_json::Value = test::read_body_json(version).await;
    assert_eq!(version_body["version"], env!("CARGO_PKG_VERSION"));

    for authorization in [
        basic,
        format!("Bearer {personal_access_token}"),
        basic_authorization("ignored-user", &personal_access_token),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/me")
                .insert_header(("Authorization", authorization))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
        let body: serde_json::Value = test::read_body_json(response).await;
        assert_eq!(body["id"], user.id);
        assert_eq!(body["username"], user.username);
        assert_eq!(body["email"], serde_json::json!(user.email));
        assert!(body.get("password_hash").is_none());
        assert!(body.get("token_permissions").is_none());
        assert!(body.to_string().find("twigt_").is_none());
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn api_reads_and_writes_the_requested_branch_and_rejects_stale_tips() {
    use base64::Engine;

    let root = format!("/tmp/test_twig_api_branch_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let user = auth::create_user(
        "api-branch-owner".to_string(),
        "api-branch@example.com".to_string(),
        "password123",
    )
    .expect("create branch owner");
    db.create_user(&user).await.expect("persist branch owner");
    let namespace = auth::create_namespace("api-branch-ns".to_string(), user.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create branch namespace");
    let repo_path = std::path::Path::new(&root).join("api-branch-ns/project");
    init_repo_with_files(
        &repo_path,
        "",
        &[
            ("README.md", "main branch\n"),
            ("hidden/secret.txt", "branch-specific secret\n"),
        ],
    );
    let repo = git2::Repository::open(&repo_path).expect("open test repository");
    let main = repo
        .find_reference("refs/heads/main")
        .expect("main branch")
        .target()
        .expect("main branch target");
    repo.reference(
        "refs/heads/feature/topic",
        main,
        true,
        "create API test branch",
    )
    .expect("create slash-containing branch");
    let bare_handle = crate::git::bare::RepoHandle::open(&root, "api-branch-ns", "project")
        .expect("open bare repository handle");
    bare_handle
        .commit_changes_at(
            "feature/topic",
            &[crate::git::bare::RepoEditChange::Upsert {
                path: ".twig.toml".to_string(),
                content: b"ignore_for_view = [\"hidden/\"]\n".to_vec(),
            }],
            main,
            "api-branch-owner",
            "api-branch@example.com",
            "Hide a branch-specific path",
        )
        .expect("commit feature branch config");
    let feature_tip = bare_handle.branch_tip("feature/topic").unwrap().unwrap();
    let authorization = basic_authorization("api-branch-owner", "password123");
    let branch_uri = "/api/v1/namespaces/api-branch-ns/repositories/project/branches";
    let branches = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(branch_uri)
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(branches.status(), StatusCode::OK);
    let branches: serde_json::Value = test::read_body_json(branches).await;
    assert!(
        branches
            .as_array()
            .unwrap()
            .iter()
            .any(|branch| branch["name"] == "feature/topic" && branch["is_default"] == false)
    );

    let tree = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/tree?ref=feature%2Ftopic")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(tree.status(), StatusCode::OK);
    let tree: serde_json::Value = test::read_body_json(tree).await;
    assert_eq!(tree["reference"], "feature/topic");
    assert!(
        tree["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["path"] == "README.md")
    );
    assert!(
        !tree["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["path"] == "hidden")
    );

    let feature_hidden = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/contents?ref=feature%2Ftopic&path=hidden%2Fsecret.txt")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(feature_hidden.status(), StatusCode::NOT_FOUND);
    let main_hidden = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/contents?ref=main&path=hidden%2Fsecret.txt")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(main_hidden.status(), StatusCode::OK);

    let main_content = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/contents?ref=main&path=README.md")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(main_content.status(), StatusCode::OK);
    let main_content: serde_json::Value = test::read_body_json(main_content).await;
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(main_content["content"].as_str().unwrap())
            .unwrap(),
        b"main branch\n"
    );

    let branch_tip = feature_tip.to_string();
    let binary = [0, 255, 1, 128];
    let write = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/contents?ref=feature%2Ftopic&path=bin%2Fprogram")
            .insert_header(("Authorization", authorization.as_str()))
            .set_json(serde_json::json!({
                "content": base64::engine::general_purpose::STANDARD.encode(binary),
                "encoding": "base64",
                "message": "Add feature binary",
                "expected_head": branch_tip,
            }))
            .to_request(),
    )
    .await;
    assert_eq!(write.status(), StatusCode::CREATED);
    let write_body: serde_json::Value = test::read_body_json(write).await;
    let feature_commit = write_body["commit"].as_str().unwrap().to_owned();

    let feature_content = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/contents?ref=feature%2Ftopic&path=bin%2Fprogram")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(feature_content.status(), StatusCode::OK);
    let feature_content: serde_json::Value = test::read_body_json(feature_content).await;
    let updated_branch = git2::Repository::open(&repo_path)
        .unwrap()
        .find_reference("refs/heads/feature/topic")
        .unwrap()
        .target()
        .unwrap();
    assert_eq!(updated_branch.to_string(), feature_commit);
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(feature_content["content"].as_str().unwrap())
            .unwrap(),
        binary
    );

    let main_missing = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/contents?ref=main&path=bin%2Fprogram")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(main_missing.status(), StatusCode::NOT_FOUND);

    let commits = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/commits?ref=feature%2Ftopic&limit=1")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(commits.status(), StatusCode::OK);
    let commits: serde_json::Value = test::read_body_json(commits).await;
    assert_eq!(commits[0]["id"], feature_commit);

    let stale_write = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/contents?ref=feature%2Ftopic&path=other.txt")
            .insert_header(("Authorization", authorization.as_str()))
            .set_json(serde_json::json!({
                "content": "stale",
                "encoding": "utf8",
                "message": "Stale write",
                "expected_head": branch_tip,
            }))
            .to_request(),
    )
    .await;
    assert_eq!(stale_write.status(), StatusCode::CONFLICT);

    let renamed = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/contents/rename")
            .insert_header(("Authorization", authorization.as_str()))
            .set_json(serde_json::json!({
                "ref": "feature/topic",
                "from": "bin/program",
                "to": "bin/renamed",
                "message": "Rename feature binary",
                "expected_head": feature_commit,
            }))
            .to_request(),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let renamed: serde_json::Value = test::read_body_json(renamed).await;
    let rename_commit = renamed["commit"].as_str().unwrap().to_owned();

    let deleted = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/contents?ref=feature%2Ftopic&path=bin%2Frenamed")
            .insert_header(("Authorization", authorization.as_str()))
            .set_json(serde_json::json!({
                "message": "Delete feature binary",
                "expected_head": rename_commit,
            }))
            .to_request(),
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::OK);

    let traversal = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-branch-ns/repositories/project/contents?ref=main&path=..%2Fescape")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(traversal.status(), StatusCode::BAD_REQUEST);

    drop(repo);
    drop(app);
    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn api_public_repository_access_requires_effective_view_and_push_grants() {
    let root = format!("/tmp/test_twig_api_permissions_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let owner = auth::create_user(
        "api-permission-owner".to_string(),
        "api-permission-owner@example.com".to_string(),
        "password123",
    )
    .expect("create owner");
    db.create_user(&owner).await.expect("persist owner");
    let outsider = auth::create_user(
        "api-permission-outsider".to_string(),
        "api-permission-outsider@example.com".to_string(),
        "password123",
    )
    .expect("create outsider");
    db.create_user(&outsider).await.expect("persist outsider");
    let namespace = auth::create_namespace("api-permission-ns".to_string(), owner.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create namespace");
    let repo_path = std::path::Path::new(&root).join("api-permission-ns/public-repo");
    init_repo_with_files(&repo_path, "", &[("README.md", "public UI content\n")]);

    let outsider_auth = basic_authorization("api-permission-outsider", "password123");
    let url = "/api/v1/namespaces/api-permission-ns/repositories/public-repo/contents?ref=main&path=README.md";
    let anonymous = test::call_service(&app, test::TestRequest::get().uri(url).to_request()).await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
    let outsider_read = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(url)
            .insert_header(("Authorization", outsider_auth.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(outsider_read.status(), StatusCode::FORBIDDEN);

    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let (_, view_token) = auth_state
        .issue_personal_access_token(
            &owner.id,
            "view only",
            std::slice::from_ref(&namespace.id),
            &PermissionSet::new([NamespacePermission::ViewNamespace]),
            None,
        )
        .await
        .expect("create view-only token");
    let other_namespace =
        auth::create_namespace("api-permission-other".to_string(), owner.id.clone());
    db.create_namespace(&other_namespace)
        .await
        .expect("create second namespace");
    let path = std::path::Path::new(&root).join("api-permission-other/repo");
    init_repo_with_files(&path, "", &[("README.md", "out of scope\n")]);
    let scoped = format!("Bearer {view_token}");
    let scoped_read = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(url)
            .insert_header(("Authorization", scoped.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(scoped_read.status(), StatusCode::OK);
    let namespaces = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces")
            .insert_header(("Authorization", scoped.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(namespaces.status(), StatusCode::OK);
    let namespaces: serde_json::Value = test::read_body_json(namespaces).await;
    assert_eq!(namespaces.as_array().unwrap().len(), 1);
    assert_eq!(namespaces[0]["name"], namespace.name);
    let out_of_scope = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-permission-other/repositories/repo/contents?ref=main&path=README.md")
            .insert_header(("Authorization", scoped.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(out_of_scope.status(), StatusCode::FORBIDDEN);

    let upload = test::call_service(
        &app,
        test::TestRequest::put()
            .uri(
                "/api/v1/namespaces/api-permission-ns/repositories/public-repo/binaries/1.0.0/tool",
            )
            .insert_header(("Authorization", scoped.as_str()))
            .set_payload(b"not allowed".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(upload.status(), StatusCode::FORBIDDEN);
    let token_namespace_create = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/namespaces")
            .insert_header(("Authorization", scoped.as_str()))
            .set_json(serde_json::json!({ "name": "api-token-created-ns" }))
            .to_request(),
    )
    .await;
    assert_eq!(token_namespace_create.status(), StatusCode::FORBIDDEN);
    assert_ne!(namespace.id, other_namespace.id);

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn api_binary_registry_round_trips_bytes_and_returns_json_metadata() {
    let root = format!("/tmp/test_twig_api_binaries_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let user = auth::create_user(
        "api-binary-owner".to_string(),
        "api-binary@example.com".to_string(),
        "password123",
    )
    .expect("create binary owner");
    db.create_user(&user).await.expect("persist binary owner");
    let namespace = auth::create_namespace("api-binary-ns".to_string(), user.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create binary namespace");
    let repo_path = std::path::Path::new(&root).join("api-binary-ns/releases");
    init_repo_with_files(&repo_path, "", &[("README.md", "release repository\n")]);
    let authorization = basic_authorization("api-binary-owner", "password123");
    let bytes = b"binary data\0\xff";
    let upload = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/api/v1/namespaces/api-binary-ns/repositories/releases/binaries/v1.2.3/linux-amd64")
            .insert_header(("Authorization", authorization.as_str()))
            .insert_header(("Content-Type", "application/octet-stream"))
            .set_payload(bytes.to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(upload.status(), StatusCode::CREATED);
    let upload_body: serde_json::Value = test::read_body_json(upload).await;
    assert_eq!(upload_body["outcome"], "stored");
    assert_eq!(upload_body["version"], "1.2.3");

    let listing = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-binary-ns/repositories/releases/binaries")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(listing.status(), StatusCode::OK);
    let listing: serde_json::Value = test::read_body_json(listing).await;
    assert_eq!(listing[0]["version"], "1.2.3");
    assert_eq!(listing[0]["assets"][0]["size_bytes"], bytes.len());

    let download = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-binary-ns/repositories/releases/binaries/latest/linux-amd64")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(
        download.headers().get("content-type").unwrap(),
        "application/octet-stream"
    );
    assert_eq!(test::read_body(download).await.as_ref(), bytes);

    let invalid_latest_upload = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/api/v1/namespaces/api-binary-ns/repositories/releases/binaries/latest/tool")
            .insert_header(("Authorization", authorization.as_str()))
            .set_payload(b"invalid".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(invalid_latest_upload.status(), StatusCode::BAD_REQUEST);

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn api_namespace_and_repository_lifecycle_requires_scoped_management_rights() {
    let root = format!("/tmp/test_twig_api_lifecycle_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let user = auth::create_user(
        "api-lifecycle-owner".to_string(),
        "api-lifecycle@example.com".to_string(),
        "password123",
    )
    .expect("create lifecycle owner");
    db.create_user(&user)
        .await
        .expect("persist lifecycle owner");
    let authorization = basic_authorization("api-lifecycle-owner", "password123");

    let created_namespace = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/namespaces")
            .insert_header(("Authorization", authorization.as_str()))
            .set_json(serde_json::json!({ "name": "api-life-source" }))
            .to_request(),
    )
    .await;
    assert_eq!(created_namespace.status(), StatusCode::CREATED);

    let created_repo = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/namespaces/api-life-source/repositories")
            .insert_header(("Authorization", authorization.as_str()))
            .set_json(serde_json::json!({
                "name": "project",
                "default_branch": "develop",
            }))
            .to_request(),
    )
    .await;
    assert_eq!(created_repo.status(), StatusCode::CREATED);

    let repo_metadata = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-life-source/repositories/project")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(repo_metadata.status(), StatusCode::OK);
    let repo_metadata: serde_json::Value = test::read_body_json(repo_metadata).await;
    assert_eq!(repo_metadata["default_branch"], "develop");

    let created_target = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/namespaces")
            .insert_header(("Authorization", authorization.as_str()))
            .set_json(serde_json::json!({ "name": "api-life-target" }))
            .to_request(),
    )
    .await;
    assert_eq!(created_target.status(), StatusCode::CREATED);

    let renamed = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri("/api/v1/namespaces/api-life-source/repositories/project")
            .insert_header(("Authorization", authorization.as_str()))
            .set_json(serde_json::json!({ "name": "renamed" }))
            .to_request(),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);

    let moved = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri("/api/v1/namespaces/api-life-source/repositories/renamed")
            .insert_header(("Authorization", authorization.as_str()))
            .set_json(serde_json::json!({ "target_namespace": "api-life-target" }))
            .to_request(),
    )
    .await;
    assert_eq!(moved.status(), StatusCode::OK);

    let source_delete = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/namespaces/api-life-source")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(source_delete.status(), StatusCode::OK);

    let handle = crate::git::bare::RepoHandle::open(&root, "api-life-target", "renamed")
        .expect("open moved repository");
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
            "Enable deletion for lifecycle test",
        )
        .expect("mark repo deletable");

    let deleted_repo = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/namespaces/api-life-target/repositories/renamed")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(deleted_repo.status(), StatusCode::OK);

    let target_delete = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/v1/namespaces/api-life-target")
            .insert_header(("Authorization", authorization.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(target_delete.status(), StatusCode::OK);
    assert!(
        db.get_namespace_by_name("api-life-target")
            .await
            .unwrap()
            .is_none()
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn api_members_and_invitations_are_namespace_scoped_and_do_not_list_secrets() {
    let root = format!("/tmp/test_twig_api_members_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let owner = auth::create_user(
        "api-members-owner".to_string(),
        "api-members-owner@example.com".to_string(),
        "password123",
    )
    .expect("create membership owner");
    db.create_user(&owner)
        .await
        .expect("persist membership owner");
    let namespace = auth::create_namespace("api-members-ns".to_string(), owner.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create member namespace");
    let outsider = auth::create_user(
        "api-members-outsider".to_string(),
        "api-members-outsider@example.com".to_string(),
        "password123",
    )
    .expect("create member outsider");
    db.create_user(&outsider)
        .await
        .expect("persist member outsider");
    let authorization = basic_authorization("api-members-owner", "password123");
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let (_, manage_token) = auth_state
        .issue_personal_access_token(
            &owner.id,
            "manage members",
            std::slice::from_ref(&namespace.id),
            &PermissionSet::new([NamespacePermission::ManageMembers]),
            None,
        )
        .await
        .expect("create management token");
    let manage_auth = format!("Bearer {manage_token}");

    let members = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-members-ns/members")
            .insert_header(("Authorization", manage_auth.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(members.status(), StatusCode::OK);
    let members: serde_json::Value = test::read_body_json(members).await;
    assert!(
        members
            .as_array()
            .unwrap()
            .iter()
            .any(|member| member["username"] == owner.username && member["role"] == "owner")
    );

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/namespaces/api-members-ns/invitations")
            .insert_header(("Authorization", manage_auth.as_str()))
            .set_json(serde_json::json!({
                "email": "invited@example.com",
                "role": "contributor",
                "expires": "7d",
            }))
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created: serde_json::Value = test::read_body_json(created).await;
    let invite_url = created["invite_url"].as_str().unwrap().to_owned();
    let invitation_token = invite_url.rsplit('/').next().unwrap().to_owned();
    assert_eq!(created["email_delivery"], "not_configured");
    assert_eq!(created["invitation"]["email"], "invited@example.com");

    let list = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-members-ns/invitations")
            .insert_header(("Authorization", manage_auth.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(list.status(), StatusCode::OK);
    let list: serde_json::Value = test::read_body_json(list).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert!(list.to_string().find(&invitation_token).is_none());
    assert!(list[0].get("token").is_none());

    let resent = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!(
                "/api/v1/namespaces/api-members-ns/invitations/{invitation_token}/resend"
            ))
            .insert_header(("Authorization", manage_auth.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(resent.status(), StatusCode::OK);
    let resent: serde_json::Value = test::read_body_json(resent).await;
    assert!(
        resent["invite_url"]
            .as_str()
            .unwrap()
            .ends_with(&invitation_token)
    );

    let (_, view_token) = auth_state
        .issue_personal_access_token(
            &owner.id,
            "view only",
            std::slice::from_ref(&namespace.id),
            &PermissionSet::new([NamespacePermission::ViewNamespace]),
            None,
        )
        .await
        .expect("create view-only token");
    let denied = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/namespaces/api-members-ns/members")
            .insert_header(("Authorization", format!("Bearer {view_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let outsider_auth = basic_authorization(&outsider.username, "password123");
    let outsider_denied = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/namespaces/api-members-ns/invitations")
            .insert_header(("Authorization", outsider_auth))
            .set_json(serde_json::json!({
                "email": "unauthorized@example.com",
                "role": "contributor",
            }))
            .to_request(),
    )
    .await;
    assert_eq!(outsider_denied.status(), StatusCode::FORBIDDEN);
    let _ = authorization;

    let _ = std::fs::remove_dir_all(&root);
}
