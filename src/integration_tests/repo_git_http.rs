use super::*;

use super::repo_content::init_repo_with_files;
use super::support::create_test_service_in;
use std::sync::{Arc, Mutex};

#[actix_web::test]
async fn test_git_http_head_is_served_by_twig_http() {
    let root = format!("/tmp/test_twig_git_http_head_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    init_repo_with_files(
        &std::path::Path::new(&root).join("public/example"),
        "",
        &[("README.md", "# Example\n")],
    );

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/public/example/HEAD")
            .insert_header(("User-Agent", "git/2.43.0"))
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/plain"
    );
    assert_eq!(
        test::read_body(response).await.as_ref(),
        b"ref: refs/heads/main\n"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_git_client_clones_and_fetches_through_twig_http() {
    use std::process::Command;

    let root =
        std::env::temp_dir().join(format!("test_twig_git_http_smart_{}", uuid::Uuid::new_v4()));
    let repository = root.join("public/example");
    std::fs::create_dir_all(&repository).unwrap();
    crate::git::repo::bare_init(&repository, "main", "Test", "test@example.com").unwrap();
    let (address, server_handle, _db, database_path) =
        start_network_git_server(&root, Vec::new()).await;

    let clone_dir = root.with_extension("clone");
    let seed_dir = root.with_extension("seed");
    let clone_url = format!("http://{address}/public/example.git");
    let ls_remote_url = clone_url.clone();
    let ls_remote = tokio::task::spawn_blocking(move || {
        Command::new("git")
            .args(["ls-remote", &ls_remote_url])
            .output()
            .unwrap()
    })
    .await
    .expect("join git ls-remote");
    assert!(ls_remote.status.success(), "{ls_remote:?}");
    assert!(
        String::from_utf8_lossy(&ls_remote.stdout).contains("refs/heads/main"),
        "native advertisement omitted main: {ls_remote:?}"
    );

    let clone_url_for_command = clone_url.clone();
    let clone_path = clone_dir.to_string_lossy().into_owned();
    let clone_output = tokio::task::spawn_blocking(move || {
        Command::new("git")
            .args(["clone", "--quiet", &clone_url_for_command, &clone_path])
            .output()
    })
    .await
    .expect("join git clone")
    .expect("run git clone");
    assert!(clone_output.status.success(), "{clone_output:?}");

    let source_path = repository.to_string_lossy().into_owned();
    let seed_path = seed_dir.to_string_lossy().into_owned();
    let clone_path = clone_dir.to_string_lossy().into_owned();
    let expected_head = tokio::task::spawn_blocking(move || {
        let run = |args: &[&str]| {
            let output = Command::new("git").args(args).output().unwrap();
            assert!(output.status.success(), "{args:?}: {output:?}");
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };

        run(&["clone", "--quiet", &source_path, &seed_path]);
        run(&["-C", &seed_path, "config", "user.name", "Twig test"]);
        run(&[
            "-C",
            &seed_path,
            "config",
            "user.email",
            "twig-test@example.com",
        ]);
        std::fs::write(
            std::path::Path::new(&seed_path).join("new-file.txt"),
            "new data\n",
        )
        .unwrap();
        run(&["-C", &seed_path, "add", "new-file.txt"]);
        run(&["-C", &seed_path, "commit", "-m", "advance main"]);
        run(&["-C", &seed_path, "push", "origin", "main"]);
        let expected = run(&["-C", &seed_path, "rev-parse", "HEAD"]);
        let fetched = Command::new("git")
            .args(["-C", &clone_path, "fetch", "origin"])
            .output()
            .unwrap();
        assert!(fetched.status.success(), "{fetched:?}");
        expected
    })
    .await
    .expect("join git fetch");

    let clone_path_for_rev = clone_dir.to_string_lossy().into_owned();
    let fetched_head = tokio::task::spawn_blocking(move || {
        let output = Command::new("git")
            .args([
                "-C",
                &clone_path_for_rev,
                "rev-parse",
                "refs/remotes/origin/main",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    })
    .await
    .expect("join rev-parse");

    assert_eq!(fetched_head, expected_head);
    server_handle.stop(true).await;
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&clone_dir);
    let _ = std::fs::remove_dir_all(&seed_dir);
    let _ = std::fs::remove_file(database_path);
}

async fn start_network_git_server(
    project_root: &std::path::Path,
    receive_hooks: Vec<Arc<dyn twig_http::ReceiveHook>>,
) -> (
    std::net::SocketAddr,
    actix_web::dev::ServerHandle,
    crate::db::Database,
    std::path::PathBuf,
) {
    let database_path = project_root.with_extension("db");
    let mut config = config::Server::new(
        ("127.0.0.1".to_string(), 0),
        "debug".to_string(),
        project_root.to_string_lossy().into_owned(),
        database_path.to_string_lossy().into_owned(),
        "test-key".to_string(),
        true,
    );
    config.git_receive_hooks = receive_hooks.into();
    let db = crate::db::Database::new(config.db_path());
    db.init_tables().await.expect("initialize test database");
    let auth_state = web::Data::new(crate::auth::TwigContext::new(
        db.clone(),
        "test-key".to_string(),
    ));
    let app_config = web::Data::new(config);
    let session_db = db.clone();
    let session_key = actix_web::cookie::Key::generate();
    let server = actix_web::HttpServer::new(move || {
        App::new()
            .app_data(app_config.clone())
            .app_data(auth_state.clone())
            .app_data(web::PayloadConfig::new(1 << 29))
            .wrap(actix_identity::IdentityMiddleware::default())
            .wrap(crate::auth::session_store::middleware(
                session_db.clone(),
                session_key.clone(),
            ))
            .configure(crate::http::routes::configure_routes)
    })
    .workers(1)
    .bind(("127.0.0.1", 0))
    .expect("bind temporary Git HTTP server");
    let address = server.addrs()[0];
    let server = server.run();
    let server_handle = server.handle();
    actix_web::rt::spawn(server);
    (address, server_handle, db, database_path)
}

struct ReceiveHookRecorder(Arc<Mutex<Vec<String>>>);

impl twig_http::ReceiveHook for ReceiveHookRecorder {
    fn pre_receive(
        &self,
        _repository: &std::path::Path,
        updates: &[twig_http::RefUpdate],
        context: &twig_http::HookContext<'_>,
    ) -> Result<(), twig_http::HookError> {
        let refs = updates
            .iter()
            .map(|update| update.refname.as_str())
            .collect::<Vec<_>>()
            .join(",");
        self.0.lock().unwrap().push(format!(
            "pre:{}:{}:{refs}",
            context.authenticated_user.unwrap_or("anonymous"),
            context.may_rewrite_protected_branches
        ));
        Ok(())
    }

    fn update(
        &self,
        _repository: &std::path::Path,
        update: &twig_http::RefUpdate,
        context: &twig_http::HookContext<'_>,
    ) -> Result<(), twig_http::HookError> {
        self.0.lock().unwrap().push(format!(
            "update:{}:{}:{}",
            context.authenticated_user.unwrap_or("anonymous"),
            context.may_rewrite_protected_branches,
            update.refname
        ));
        if update.refname == "refs/heads/reject-me" {
            return Err(twig_http::HookError::reject(
                "test update callback rejection",
            ));
        }
        Ok(())
    }

    fn post_receive(
        &self,
        _repository: &std::path::Path,
        updates: &[twig_http::RefUpdate],
        context: &twig_http::HookContext<'_>,
    ) {
        let refs = updates
            .iter()
            .map(|update| update.refname.as_str())
            .collect::<Vec<_>>()
            .join(",");
        self.0.lock().unwrap().push(format!(
            "post:{}:{}:{refs}",
            context.authenticated_user.unwrap_or("anonymous"),
            context.may_rewrite_protected_branches
        ));
    }

    fn post_update(
        &self,
        _repository: &std::path::Path,
        refnames: &[String],
        context: &twig_http::HookContext<'_>,
    ) {
        self.0.lock().unwrap().push(format!(
            "post-update:{}:{}:{}",
            context.authenticated_user.unwrap_or("anonymous"),
            context.may_rewrite_protected_branches,
            refnames.join(",")
        ));
    }
}

#[actix_web::test]
async fn test_git_client_push_uses_rust_callbacks_and_ignores_legacy_shell_hooks() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let root = std::env::temp_dir().join(format!(
        "test_twig_native_git_hooks_{}",
        uuid::Uuid::new_v4()
    ));
    let repository = root.join("public/example");
    std::fs::create_dir_all(&repository).unwrap();
    crate::git::repo::bare_init(&repository, "main", "Test", "test@example.com").unwrap();

    let legacy_marker = root.join("legacy-shell-hook-ran");
    let hooks_dir = repository.join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    let legacy_hook = format!("#!/bin/sh\ntouch '{}'\n", legacy_marker.display());
    for name in ["pre-receive", "pre-receive.twig-user"] {
        let path = hooks_dir.join(name);
        std::fs::write(&path, &legacy_hook).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let hook_events = Arc::new(Mutex::new(Vec::new()));
    let hook: Arc<dyn twig_http::ReceiveHook> =
        Arc::new(ReceiveHookRecorder(Arc::clone(&hook_events)));
    let (address, server_handle, db, database_path) =
        start_network_git_server(&root, vec![hook]).await;
    let owner = crate::auth::create_user(
        "push-owner".to_string(),
        "push-owner@example.com".to_string(),
        "owner-password",
    )
    .unwrap();
    db.create_user(&owner).await.unwrap();
    let namespace = crate::auth::create_namespace("public".to_string(), owner.id.clone());
    db.create_namespace(&namespace).await.unwrap();
    let contributor = crate::auth::create_user(
        "push-contributor".to_string(),
        "push-contributor@example.com".to_string(),
        "contributor-password",
    )
    .unwrap();
    db.create_user(&contributor).await.unwrap();
    db.conn()
        .await
        .unwrap()
        .execute(
            "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
             VALUES (?1, ?2, 'contributor', ?3)",
            turso::params![
                namespace.id,
                contributor.id,
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .await
        .unwrap();

    let working_copy = root.with_extension("working-copy");
    let public_url = format!("http://{address}/public/example.git");
    let owner_url = format!("http://push-owner:owner-password@{address}/public/example.git");
    let contributor_url =
        format!("http://push-contributor:contributor-password@{address}/public/example.git");
    let working_copy_for_git = working_copy.to_string_lossy().into_owned();
    let (rejected_update, rejected_push, pushed_head) = tokio::task::spawn_blocking(move || {
        let run = |args: &[&str]| {
            let output = Command::new("git").args(args).output().unwrap();
            assert!(output.status.success(), "{args:?}: {output:?}");
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };

        run(&["clone", "--quiet", &public_url, &working_copy_for_git]);
        run(&[
            "-C",
            &working_copy_for_git,
            "config",
            "user.name",
            "Push Owner",
        ]);
        run(&[
            "-C",
            &working_copy_for_git,
            "config",
            "user.email",
            "push-owner@example.com",
        ]);
        std::fs::write(
            std::path::Path::new(&working_copy_for_git).join("new-file.txt"),
            "initial push\n",
        )
        .unwrap();
        run(&["-C", &working_copy_for_git, "add", "new-file.txt"]);
        run(&["-C", &working_copy_for_git, "commit", "-m", "allowed push"]);
        run(&[
            "-C",
            &working_copy_for_git,
            "remote",
            "set-url",
            "origin",
            &owner_url,
        ]);
        let push = Command::new("git")
            .args(["-C", &working_copy_for_git, "push", "origin", "main"])
            .output()
            .unwrap();
        assert!(push.status.success(), "{push:?}");
        let pushed_head = run(&["-C", &working_copy_for_git, "rev-parse", "HEAD"]);

        run(&["-C", &working_copy_for_git, "checkout", "-b", "reject-me"]);
        std::fs::write(
            std::path::Path::new(&working_copy_for_git).join("rejected-file.txt"),
            "callback should reject this ref\n",
        )
        .unwrap();
        run(&["-C", &working_copy_for_git, "add", "rejected-file.txt"]);
        run(&[
            "-C",
            &working_copy_for_git,
            "commit",
            "-m",
            "rejected update callback",
        ]);
        let rejected_update = Command::new("git")
            .args(["-C", &working_copy_for_git, "push", "origin", "reject-me"])
            .output()
            .unwrap();
        assert!(
            !rejected_update.status.success(),
            "update callback should reject"
        );
        let rejected_update = String::from_utf8_lossy(&rejected_update.stderr).into_owned();

        run(&["-C", &working_copy_for_git, "checkout", "main"]);
        std::fs::write(
            std::path::Path::new(&working_copy_for_git).join("new-file.txt"),
            "rewritten push\n",
        )
        .unwrap();
        run(&["-C", &working_copy_for_git, "add", "new-file.txt"]);
        run(&[
            "-C",
            &working_copy_for_git,
            "commit",
            "--amend",
            "--no-edit",
        ]);
        run(&[
            "-C",
            &working_copy_for_git,
            "remote",
            "set-url",
            "origin",
            &contributor_url,
        ]);
        let rejected_push = Command::new("git")
            .args([
                "-C",
                &working_copy_for_git,
                "push",
                "--force",
                "origin",
                "main",
            ])
            .output()
            .unwrap();
        assert!(
            !rejected_push.status.success(),
            "force-push should be denied"
        );
        let rejected_push = String::from_utf8_lossy(&rejected_push.stderr).into_owned();
        (rejected_update, rejected_push, pushed_head)
    })
    .await
    .expect("join native callback push test");

    assert!(
        rejected_update.contains("test update callback rejection"),
        "{rejected_update}"
    );
    assert!(
        rejected_push.contains("This permission set cannot rewrite history on main"),
        "{rejected_push}"
    );
    assert!(
        !legacy_marker.exists(),
        "legacy shell hooks must remain inactive"
    );
    {
        let events = hook_events.lock().unwrap();
        assert!(
            events
                .iter()
                .any(|event| event == "pre:push-owner:true:refs/heads/main")
        );
        assert!(
            events
                .iter()
                .any(|event| event == "update:push-owner:true:refs/heads/main")
        );
        assert!(
            events
                .iter()
                .any(|event| event == "post:push-owner:true:refs/heads/main")
        );
        assert!(
            events
                .iter()
                .any(|event| event == "post-update:push-owner:true:refs/heads/main")
        );
        assert!(
            events
                .iter()
                .any(|event| event == "update:push-owner:true:refs/heads/reject-me")
        );
        assert!(
            !events
                .iter()
                .any(|event| event.contains("post:push-owner:true:refs/heads/reject-me"))
        );
        assert!(
            !events
                .iter()
                .any(|event| event.contains("push-contributor"))
        );
    }

    let source_head = Command::new("git")
        .args([
            "--git-dir",
            repository.to_str().unwrap(),
            "rev-parse",
            "refs/heads/main",
        ])
        .output()
        .unwrap();
    assert!(source_head.status.success(), "{source_head:?}");
    assert_eq!(
        String::from_utf8_lossy(&source_head.stdout).trim(),
        pushed_head
    );

    server_handle.stop(true).await;
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&working_copy);
    let _ = std::fs::remove_file(database_path);
}

#[cfg(unix)]
#[actix_web::test]
async fn test_twig_runtime_works_with_an_empty_path() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    if std::env::var_os("TWIG_NATIVE_PACK_TEST_CHILD").is_some() {
        let root = std::path::PathBuf::from(
            std::env::var_os("TWIG_NATIVE_PACK_TEST_ROOT").expect("child root path"),
        );
        return run_native_pack_server_child(&root).await;
    }

    let root = std::env::temp_dir().join(format!(
        "test_twig_empty_server_path_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let client_path = std::env::var("PATH").expect("parent PATH");
    let git = std::env::split_paths(&client_path)
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("find Git test client");
    let server_path = root.join("empty-path");
    std::fs::create_dir_all(&server_path).unwrap();
    let executable = std::env::current_exe().unwrap();
    let child_root = root.clone();
    let child_path = server_path.clone();
    let mut server = Command::new(executable)
        .args([
            "--exact",
            "integration_tests::tests::repo_git_http::test_twig_runtime_works_with_an_empty_path",
            "--nocapture",
        ])
        .env("TWIG_NATIVE_PACK_TEST_CHILD", "1")
        .env("TWIG_NATIVE_PACK_TEST_ROOT", &child_root)
        .env("PATH", &child_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start process-isolated Twig server");
    let _shutdown = ServerShutdown(root.join("shutdown"));
    let ready_file = root.join("ready-address");
    let deadline = Instant::now() + Duration::from_secs(30);
    while !ready_file.exists() {
        if server.try_wait().unwrap().is_some() {
            let output = server.wait_with_output().unwrap();
            panic!("isolated server exited before readiness: {output:?}");
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for server readiness"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let address = std::fs::read_to_string(&ready_file).unwrap();
    let public_url = format!("http://{address}/public/example.git");
    let contributor_url =
        format!("http://native-contributor:contributor-password@{address}/public/example.git");
    let clone_dir = root.with_extension("native-client");
    let clone_path = clone_dir.to_string_lossy().into_owned();
    let run_git = |args: &[&str]| {
        let output = Command::new(&git)
            .args(args)
            .env("PATH", &client_path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };

    run_git(&["ls-remote", &public_url]);
    run_git(&["clone", "--quiet", &public_url, &clone_path]);
    run_git(&[
        "-C",
        &clone_path,
        "config",
        "user.name",
        "Native Contributor",
    ]);
    run_git(&[
        "-C",
        &clone_path,
        "config",
        "user.email",
        "native-contributor@example.com",
    ]);
    std::fs::write(
        std::path::Path::new(&clone_path).join("native.txt"),
        "served with no server executables\n",
    )
    .unwrap();
    run_git(&["-C", &clone_path, "add", "native.txt"]);
    run_git(&["-C", &clone_path, "commit", "-m", "native push"]);
    run_git(&[
        "-C",
        &clone_path,
        "remote",
        "set-url",
        "origin",
        &contributor_url,
    ]);
    run_git(&["-C", &clone_path, "push", "origin", "main"]);
    run_git(&["-C", &clone_path, "fetch", "origin"]);

    let head = run_git(&["-C", &clone_path, "rev-parse", "HEAD"]);
    let remote_head = run_git(&["-C", &clone_path, "rev-parse", "refs/remotes/origin/main"]);
    assert_eq!(head, remote_head);
    assert!(
        !root.join("legacy-shell-hook-ran").exists(),
        "legacy executable hooks must remain inactive"
    );

    std::fs::write(root.join("shutdown"), "stop").unwrap();
    let output = tokio::task::spawn_blocking(move || server.wait_with_output().unwrap())
        .await
        .expect("join process-isolated server");
    assert!(output.status.success(), "{output:?}");
    let _ = std::fs::remove_dir_all(&clone_dir);
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
struct ServerShutdown(std::path::PathBuf);

#[cfg(unix)]
impl Drop for ServerShutdown {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "stop");
    }
}

#[cfg(unix)]
async fn run_native_pack_server_child(root: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let repository = root.join("public/example");
    std::fs::create_dir_all(&repository).unwrap();
    crate::git::repo::bare_init(&repository, "main", "Test", "test@example.com")
        .expect("create repository with empty PATH");

    let legacy_marker = root.join("legacy-shell-hook-ran");
    let hooks_dir = repository.join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    for hook_name in ["pre-receive", "pre-receive.twig-user"] {
        let path = hooks_dir.join(hook_name);
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf 'ran\\n' > '{}'\n",
                legacy_marker.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let (address, server_handle, db, database_path) =
        start_network_git_server(root, Vec::new()).await;
    let owner = crate::auth::create_user(
        "native-owner".to_string(),
        "native-owner@example.com".to_string(),
        "owner-password",
    )
    .unwrap();
    db.create_user(&owner).await.unwrap();
    let namespace = crate::auth::create_namespace("public".to_string(), owner.id.clone());
    db.create_namespace(&namespace).await.unwrap();
    let contributor = crate::auth::create_user(
        "native-contributor".to_string(),
        "native-contributor@example.com".to_string(),
        "contributor-password",
    )
    .unwrap();
    db.create_user(&contributor).await.unwrap();
    db.conn()
        .await
        .unwrap()
        .execute(
            "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
             VALUES (?1, ?2, 'contributor', ?3)",
            turso::params![
                namespace.id,
                contributor.id,
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .await
        .unwrap();
    std::fs::write(root.join("ready-address"), address.to_string()).unwrap();

    while !root.join("shutdown").exists() {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    server_handle.stop(true).await;
    let _ = std::fs::remove_file(database_path);
}
