use std::path::Path;
use std::time::Duration;

use super::pull_requests_api::{push_branch, seed_owner_repo, token_for, token_header};
use super::repo_git_http::start_network_git_server_with_ci;
use super::support::*;
use super::*;
use crate::auth::NamespacePermission;
use crate::config::CiSettings;
use crate::db::ci_runs::RunStatus;

const DEFINITION: &str = "[tools]\n\n[tasks.ci]\ndir = \"..\"\nrun = 'echo ci'\n";
const DISPATCH: &str = "/api/v1/repos/pr-team/tooling/actions/workflows/ci/dispatches";
const TASKS: &str = "/api/v1/repos/pr-team/tooling/actions/tasks";

/// Writes a stand-in for `mise` that reports what it was given and succeeds.
fn fake_mise(root: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let directory = root.join("bin");
    std::fs::create_dir_all(&directory).expect("bin directory");
    let path = directory.join("fake-mise");
    std::fs::write(
        &path,
        "#!/bin/sh\necho \"mise $*\"\necho \"sha=$TWIG_CI_SHA event=$TWIG_CI_EVENT input=$TWIG_INPUT_TARGET\"\n",
    )
    .expect("write fake mise");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path.display().to_string()
}

fn enabled_ci(root: &Path) -> CiSettings {
    CiSettings::enabled(
        fake_mise(root),
        Duration::from_secs(30),
        1,
        root.join("ci-cache"),
    )
}

fn config_with_ci(root: &str, ci: CiSettings) -> config::Server {
    config::Server::new(
        ("127.0.0.1".to_string(), 8080),
        "debug".to_string(),
        root.to_string(),
        format!("/tmp/test_twig_ci_api_{}.db", uuid::Uuid::new_v4()),
        "secure".to_string(),
        true,
    )
    .with_ci(ci)
}

/// Adds a branch whose only change is `.twig/mise.toml` with `definition`.
fn branch_with_definition(root: &str, branch: &str, definition: &str) {
    let repo_path = std::path::Path::new(root).join("pr-team").join("tooling");
    let repo = git2::Repository::open_bare(&repo_path).expect("open bare repo");
    let base = repo
        .find_reference("refs/heads/main")
        .expect("main")
        .peel_to_commit()
        .expect("main commit");
    let blob = repo.blob(definition.as_bytes()).expect("blob");
    let mut twig = repo.treebuilder(None).expect("tree builder");
    twig.insert("mise.toml", blob, 0o100_644).expect("insert");
    let twig_tree = twig.write().expect("write .twig");
    let mut root_tree = repo
        .treebuilder(Some(&base.tree().expect("main tree")))
        .expect("tree builder");
    root_tree
        .insert(".twig", twig_tree, 0o040_000)
        .expect("insert .twig");
    let tree = repo
        .find_tree(root_tree.write().expect("write tree"))
        .expect("tree");
    let signature = git2::Signature::now("Twig Test", "twig@example.com").expect("signature");
    repo.commit(
        Some(&format!("refs/heads/{branch}")),
        &signature,
        &signature,
        "Add CI definition",
        &tree,
        &[&base],
    )
    .expect("commit");
}

fn branch_tip(root: &str, branch: &str) -> String {
    let repo_path = std::path::Path::new(root).join("pr-team").join("tooling");
    let repo = git2::Repository::open_bare(&repo_path).expect("open bare repo");
    repo.find_reference(&format!("refs/heads/{branch}"))
        .expect("branch")
        .target()
        .expect("branch target")
        .to_string()
}

#[actix_web::test]
async fn dispatch_runs_ci_and_reports_it_through_forgejo_routes() {
    let root = format!("/tmp/test_twig_ci_dispatch_{}", uuid::Uuid::new_v4());
    let ci = enabled_ci(Path::new(&root));
    let (app, db) = create_test_service_for_config(
        config_with_ci(&root, ci),
        crate::http::body::RequestBodyLimits::default(),
    )
    .await;
    let (user, _namespace, _repo) = seed_owner_repo(&db, &root).await;
    branch_with_definition(&root, "feature", DEFINITION);
    let tip = branch_tip(&root, "feature");
    let token = token_for(
        &db,
        &user,
        "pr-team",
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let (name, value) = token_header(&token);

    let dispatched = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(DISPATCH)
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({
                "ref": "feature",
                "inputs": { "TARGET": "linux" },
            }))
            .to_request(),
    )
    .await;
    assert_eq!(dispatched.status(), StatusCode::NO_CONTENT);

    // The run executes in the background; poll the Forgejo task list until it ends.
    let mut task = serde_json::Value::Null;
    for _ in 0..200 {
        let listed = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(TASKS)
                .insert_header((name, value.as_str()))
                .to_request(),
        )
        .await;
        assert_eq!(listed.status(), StatusCode::OK);
        let body: serde_json::Value = test::read_body_json(listed).await;
        assert_eq!(body["total_count"], 1);
        task = body["workflow_runs"][0].clone();
        if task["status"] == "success" || task["status"] == "failure" {
            break;
        }
        actix_web::rt::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(task["status"], "success", "task: {task}");
    assert_eq!(task["workflow_id"], "ci");
    assert_eq!(task["event"], "workflow_dispatch");
    assert_eq!(task["head_branch"], "feature");
    assert_eq!(task["head_sha"], tip);
    assert_eq!(task["run_number"], 1);
    assert!(task["created_at"].is_string() && task["updated_at"].is_string());

    let url = task["url"].as_str().expect("log url");
    let log_path = url
        .find("/api/v1/")
        .map(|index| &url[index..])
        .expect("log url has an API path");
    let log = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(log_path)
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(log.status(), StatusCode::OK);
    let log = String::from_utf8_lossy(&test::read_body(log).await).into_owned();
    assert!(
        log.contains(&format!("sha={tip} event=workflow_dispatch input=linux")),
        "log:\n{log}"
    );

    let combined = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/v1/repos/pr-team/tooling/commits/{tip}/status"
            ))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    assert_eq!(combined.status(), StatusCode::OK);
    let combined: serde_json::Value = test::read_body_json(combined).await;
    assert_eq!(combined["state"], "success");
    assert_eq!(combined["total_count"], 1);
    assert_eq!(combined["statuses"][0]["context"], "ci");

    let statuses = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/api/v1/repos/pr-team/tooling/statuses/{tip}"))
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let statuses: serde_json::Value = test::read_body_json(statuses).await;
    assert_eq!(statuses[0]["state"], "success");

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn dispatch_refuses_missing_definitions_bad_refs_and_read_only_tokens() {
    let root = format!("/tmp/test_twig_ci_refusals_{}", uuid::Uuid::new_v4());
    let ci = enabled_ci(Path::new(&root));
    let (app, db) = create_test_service_for_config(
        config_with_ci(&root, ci),
        crate::http::body::RequestBodyLimits::default(),
    )
    .await;
    let (user, _namespace, _repo) = seed_owner_repo(&db, &root).await;
    branch_with_definition(&root, "feature", DEFINITION);
    push_branch(&root, "plain", "notes.txt", "no CI here");
    let writer = token_for(
        &db,
        &user,
        "pr-team",
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let reader = token_for(&db, &user, "pr-team", &[NamespacePermission::ViewNamespace]).await;

    let cases = [
        (
            &writer,
            DISPATCH,
            serde_json::json!({ "ref": "plain" }),
            StatusCode::UNPROCESSABLE_ENTITY,
            ".twig/mise.toml",
        ),
        (
            &writer,
            DISPATCH,
            serde_json::json!({ "ref": "missing" }),
            StatusCode::UNPROCESSABLE_ENTITY,
            "does not exist",
        ),
        (
            &writer,
            DISPATCH,
            serde_json::json!({ "ref": "feature", "inputs": { "bad-name": "x" } }),
            StatusCode::UNPROCESSABLE_ENTITY,
            "letters, digits",
        ),
        (
            &writer,
            "/api/v1/repos/pr-team/tooling/actions/workflows/deploy/dispatches",
            serde_json::json!({ "ref": "feature" }),
            StatusCode::NOT_FOUND,
            "couldn't be found",
        ),
        (
            &reader,
            DISPATCH,
            serde_json::json!({ "ref": "feature" }),
            StatusCode::FORBIDDEN,
            "permission",
        ),
    ];
    for (token, uri, body, status, message) in cases {
        let (name, value) = token_header(token);
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(uri)
                .insert_header((name, value.as_str()))
                .set_json(body.clone())
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), status, "{body}");
        let error: serde_json::Value = test::read_body_json(response).await;
        assert!(
            error["message"]
                .as_str()
                .unwrap_or_default()
                .contains(message),
            "{body}: {error}"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn ci_is_off_by_default_and_its_lists_stay_empty() {
    let root = format!("/tmp/test_twig_ci_disabled_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, _namespace, _repo) = seed_owner_repo(&db, &root).await;
    branch_with_definition(&root, "feature", DEFINITION);
    let token = token_for(
        &db,
        &user,
        "pr-team",
        &[
            NamespacePermission::ViewNamespace,
            NamespacePermission::PushChanges,
        ],
    )
    .await;
    let (name, value) = token_header(&token);

    let dispatched = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(DISPATCH)
            .insert_header((name, value.as_str()))
            .set_json(serde_json::json!({ "ref": "feature" }))
            .to_request(),
    )
    .await;
    assert_eq!(dispatched.status(), StatusCode::NOT_FOUND);
    let error: serde_json::Value = test::read_body_json(dispatched).await;
    assert_eq!(error["message"], "CI is not enabled on this server");

    let listed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(TASKS)
            .insert_header((name, value.as_str()))
            .to_request(),
    )
    .await;
    let listed: serde_json::Value = test::read_body_json(listed).await;
    assert_eq!(listed["total_count"], 0);
    assert_eq!(listed["workflow_runs"], serde_json::json!([]));

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn a_push_over_git_http_queues_ci_only_for_commits_that_define_it() {
    use std::process::Command;

    let root = std::env::temp_dir().join(format!("test_twig_ci_push_{}", uuid::Uuid::new_v4()));
    let repository = root.join("public").join("example");
    std::fs::create_dir_all(&repository).unwrap();
    crate::git::repo::bare_init(&repository, "main", "Test", "test@example.com").unwrap();
    let ci = enabled_ci(&root);
    let (address, server_handle, db, database_path) = start_network_git_server_with_ci(
        &root,
        crate::http::body::RequestBodyLimits::default(),
        ci,
    )
    .await;
    let owner = crate::auth::create_user(
        "ci-pusher".to_string(),
        "ci-pusher@example.com".to_string(),
        "pusher-password",
    )
    .unwrap();
    db.create_user(&owner).await.unwrap();
    let namespace = crate::auth::create_namespace("public".to_string(), owner.id.clone());
    db.create_namespace(&namespace).await.unwrap();

    let working_copy = root.with_extension("working-copy");
    let owner_url = format!("http://ci-pusher:pusher-password@{address}/public/example.git");
    let working = working_copy.to_string_lossy().into_owned();
    let (defined_head, plain_head) = tokio::task::spawn_blocking(move || {
        let run = |args: &[&str]| {
            let output = Command::new("git").args(args).output().unwrap();
            assert!(output.status.success(), "{args:?}: {output:?}");
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };
        run(&["clone", "--quiet", &owner_url, &working]);
        run(&["-C", &working, "config", "user.name", "CI Pusher"]);
        run(&[
            "-C",
            &working,
            "config",
            "user.email",
            "ci-pusher@example.com",
        ]);

        std::fs::create_dir_all(std::path::Path::new(&working).join(".twig")).unwrap();
        std::fs::write(
            std::path::Path::new(&working).join(".twig/mise.toml"),
            DEFINITION,
        )
        .unwrap();
        run(&["-C", &working, "add", ".twig/mise.toml"]);
        run(&["-C", &working, "commit", "-m", "Define CI"]);
        run(&["-C", &working, "push", "--quiet", "origin", "main"]);
        let defined_head = run(&["-C", &working, "rev-parse", "HEAD"]);

        run(&["-C", &working, "checkout", "--quiet", "--orphan", "plain"]);
        run(&["-C", &working, "rm", "-rf", "--quiet", "."]);
        std::fs::write(std::path::Path::new(&working).join("notes.txt"), "no CI\n").unwrap();
        run(&["-C", &working, "add", "notes.txt"]);
        run(&["-C", &working, "commit", "-m", "Notes only"]);
        run(&["-C", &working, "push", "--quiet", "origin", "plain"]);
        let plain_head = run(&["-C", &working, "rev-parse", "HEAD"]);
        (defined_head, plain_head)
    })
    .await
    .unwrap();

    let repo_id = crate::git::bare::RepoHandle::open(root.to_str().unwrap(), "public", "example")
        .unwrap()
        .repo_id()
        .unwrap();

    let mut runs = Vec::new();
    for _ in 0..200 {
        let (listed, _) = db.list_ci_runs(&repo_id, 50, 0).await.unwrap();
        if listed.iter().any(|run| run.status == RunStatus::Success) {
            runs = listed;
            break;
        }
        actix_web::rt::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        runs.len(),
        1,
        "only the commit defining ci is queued: {runs:?}"
    );
    let run = &runs[0];
    assert_eq!(run.event, "push");
    assert_eq!(run.head_branch, "main");
    assert_eq!(run.head_sha, defined_head);
    assert_eq!(run.triggered_by_user_id.as_deref(), Some(owner.id.as_str()));
    assert_ne!(run.display_title, "");

    actix_web::rt::time::sleep(Duration::from_millis(300)).await;
    let (listed, _) = db.list_ci_runs(&repo_id, 50, 0).await.unwrap();
    assert!(
        listed.iter().all(|run| run.head_sha != plain_head),
        "a commit without the ci task queues nothing"
    );

    let log = db
        .find_ci_run_log(&repo_id, run.id)
        .await
        .unwrap()
        .expect("log");
    assert!(
        log.contains(&format!("sha={defined_head} event=push")),
        "log:\n{log}"
    );

    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(&database_path);
    let _ = std::fs::remove_dir_all(&working_copy);
    server_handle.stop(true).await;
}
