use super::*;

use super::auth_and_invitations::signup_and_login;
use super::repo_content::init_repo_with_files;
use super::support::{create_test_service_in, create_test_service_with_db_and_body_limits};

#[actix_web::test]
async fn oversized_git_push_does_not_create_namespace_or_repository() {
    let root = format!("/tmp/test_twig_git_body_limit_{}", uuid::Uuid::new_v4());
    let limits = crate::http::body::RequestBodyLimits::for_tests(8, 16, 2, 1);
    let (app, db) = create_test_service_with_db_and_body_limits(&root, limits).await;
    let _session = signup_and_login(&app, "gitbodyowner").await;
    let authorization = super::repo_access_and_routes::basic_auth("gitbodyowner", "password123");

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/new-space/new-repo/git-receive-pack")
            .insert_header(("User-Agent", "git/2.43.0"))
            .insert_header(("Content-Type", "application/x-git-receive-pack-request"))
            .insert_header(("Authorization", authorization.as_str()))
            .set_payload(b"0123456789abcdefg".to_vec())
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(
        db.get_namespace_by_name("new-space")
            .await
            .unwrap()
            .is_none(),
        "an oversized push must not create the namespace record"
    );
    assert!(
        !std::path::Path::new(&root)
            .join("new-space/new-repo")
            .exists(),
        "an oversized push must not create a repository"
    );

    let oversized_public_read = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/public/missing-repo/git-upload-pack")
            .insert_header(("User-Agent", "git/2.43.0"))
            .insert_header(("Content-Type", "application/x-git-upload-pack-request"))
            .set_payload(b"123456789".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(
        oversized_public_read.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "public Git reads use the smaller request-body ceiling"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn unauthorized_git_write_is_rejected_before_reading_its_body() {
    use std::io::{Read, Write};

    let root = std::env::temp_dir().join(format!(
        "test_twig_git_auth_before_body_{}",
        uuid::Uuid::new_v4()
    ));
    let (address, server_handle, db, database_path) = start_network_git_server(&root).await;
    let response = tokio::task::spawn_blocking(move || {
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        stream
            .write_all(
                b"POST /untrusted-space/repo/git-receive-pack HTTP/1.1\r\n\
                  Host: 127.0.0.1\r\n\
                  User-Agent: git/2.43.0\r\n\
                  Content-Type: application/x-git-receive-pack-request\r\n\
                  Content-Length: 2097152\r\n\
                  Connection: close\r\n\r\n",
            )
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        String::from_utf8_lossy(&response).into_owned()
    })
    .await
    .expect("join unauthenticated request");

    assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    assert!(
        db.get_namespace_by_name("untrusted-space")
            .await
            .unwrap()
            .is_none(),
        "an unauthenticated push must not create a namespace"
    );
    server_handle.stop(true).await;
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(database_path);
}

#[actix_web::test]
async fn chunked_public_git_read_is_stopped_at_the_small_body_limit() {
    use std::io::{Read, Write};

    let root = std::env::temp_dir().join(format!(
        "test_twig_git_chunked_limit_{}",
        uuid::Uuid::new_v4()
    ));
    let limits = crate::http::body::RequestBodyLimits::for_tests(8, 16, 2, 1);
    let (address, server_handle, _db, database_path) =
        start_network_git_server_with_limits(&root, limits).await;
    let response = tokio::task::spawn_blocking(move || {
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        stream
            .write_all(
                b"POST /public/missing-repo/git-upload-pack HTTP/1.1\r\n\
                  Host: 127.0.0.1\r\n\
                  User-Agent: git/2.43.0\r\n\
                  Content-Type: application/x-git-upload-pack-request\r\n\
                  Transfer-Encoding: chunked\r\n\
                  Connection: close\r\n\r\n\
                  9\r\n123456789\r\n0\r\n\r\n",
            )
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        String::from_utf8_lossy(&response).into_owned()
    })
    .await
    .expect("join chunked public Git request");

    assert!(response.starts_with("HTTP/1.1 413"), "{response}");
    server_handle.stop(true).await;
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(database_path);
}

#[actix_web::test]
async fn test_git_http_head_is_served_by_git_http_backend() {
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
async fn test_git_client_clones_and_fetches_through_git_http_backend() {
    use std::process::Command;

    let root =
        std::env::temp_dir().join(format!("test_twig_git_http_smart_{}", uuid::Uuid::new_v4()));
    let repository = root.join("public/example");
    std::fs::create_dir_all(&repository).unwrap();
    crate::git::repo::bare_init(&repository, "main", "Test", "test@example.com").unwrap();
    let (address, server_handle, _db, database_path) = start_network_git_server(&root).await;

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

type NetworkGitServer = (
    std::net::SocketAddr,
    actix_web::dev::ServerHandle,
    crate::db::Database,
    std::path::PathBuf,
);

async fn start_network_git_server(project_root: &std::path::Path) -> NetworkGitServer {
    start_network_git_server_with_limits(
        project_root,
        crate::http::body::RequestBodyLimits::default(),
    )
    .await
}

async fn start_network_git_server_with_limits(
    project_root: &std::path::Path,
    request_body_limits: crate::http::body::RequestBodyLimits,
) -> NetworkGitServer {
    let database_path = project_root.with_extension("db");
    let config = config::Server::new(
        ("127.0.0.1".to_string(), 0),
        "debug".to_string(),
        project_root.to_string_lossy().into_owned(),
        database_path.to_string_lossy().into_owned(),
        "test-key".to_string(),
        true,
    );
    let db = crate::db::Database::new(config.db_path());
    db.init_tables().await.expect("initialize test database");
    let auth_state = web::Data::new(crate::auth::TwigContext::new(
        db.clone(),
        "test-key".to_string(),
    ));
    let app_config = web::Data::new(config);
    let request_body_limits = web::Data::new(request_body_limits);
    let session_db = db.clone();
    let session_key = actix_web::cookie::Key::generate();
    let server = actix_web::HttpServer::new(move || {
        App::new()
            .app_data(app_config.clone())
            .app_data(auth_state.clone())
            .app_data(request_body_limits.clone())
            .app_data(web::PayloadConfig::new(
                crate::http::body::DEFAULT_SMALL_BODY_LIMIT,
            ))
            .wrap(actix_identity::IdentityMiddleware::default())
            .wrap(crate::auth::session_store::middleware(
                session_db.clone(),
                session_key.clone(),
            ))
            .wrap(actix_web::middleware::from_fn(
                crate::http::security::same_origin_guard,
            ))
            .wrap(actix_web::middleware::from_fn(
                crate::http::security::security_headers,
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

#[actix_web::test]
async fn test_git_http_extra_headers_accept_scoped_bearer_and_basic_token_passwords() {
    use base64::Engine;
    use std::process::Command;

    let root = std::env::temp_dir().join(format!(
        "test_twig_git_http_personal_tokens_{}",
        uuid::Uuid::new_v4()
    ));
    let repository = root.join("token-space/example");
    std::fs::create_dir_all(&repository).unwrap();
    crate::git::repo::bare_init(&repository, "main", "Test", "test@example.com").unwrap();

    let (address, server_handle, db, database_path) = start_network_git_server(&root).await;
    let owner = crate::auth::create_user(
        "token-push-owner".to_string(),
        "token-push-owner@example.com".to_string(),
        "owner-password",
    )
    .expect("create namespace owner");
    db.create_user(&owner).await.expect("store owner");
    let namespace = crate::auth::create_namespace("token-space".to_string(), owner.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("store namespace");

    let auth_state = crate::auth::TwigContext::new(db.clone(), "test-key".to_string());
    let namespace_ids = [namespace.id.clone()];
    let (_, push_token) = auth_state
        .issue_personal_access_token(
            &owner.id,
            "Push-only Git token",
            &namespace_ids,
            &crate::auth::PermissionSet::new([crate::auth::NamespacePermission::PushChanges]),
            None,
        )
        .await
        .expect("create push token");
    let (_, read_token) = auth_state
        .issue_personal_access_token(
            &owner.id,
            "Read-only Git token",
            &namespace_ids,
            &crate::auth::PermissionSet::new([crate::auth::NamespacePermission::ViewNamespace]),
            None,
        )
        .await
        .expect("create read token");

    let working_copy = root.with_extension("working-copy");
    let clone_url = format!("http://{address}/token-space/example.git");
    let working_copy_path = working_copy.to_string_lossy().into_owned();
    let bearer_header = format!("Bearer {push_token}");
    let basic_value =
        base64::engine::general_purpose::STANDARD.encode(format!("not-the-owner:{push_token}"));
    let basic_header = format!("Basic {basic_value}");
    let read_header = format!("Bearer {read_token}");

    let (bearer_push, basic_password_push, read_only_push) =
        tokio::task::spawn_blocking(move || {
            let run = |args: &[&str]| {
                let output = Command::new("git").args(args).output().unwrap();
                assert!(output.status.success(), "{args:?}: {output:?}");
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            };
            let push_with_header = |header: &str| {
                Command::new("git")
                    .arg("-c")
                    .arg(format!("http.extraHeader=Authorization: {header}"))
                    .args(["-C", &working_copy_path, "push", "origin", "main"])
                    .output()
                    .unwrap()
            };

            run(&["clone", "--quiet", &clone_url, &working_copy_path]);
            run(&[
                "-C",
                &working_copy_path,
                "config",
                "user.name",
                "Token Push",
            ]);
            run(&[
                "-C",
                &working_copy_path,
                "config",
                "user.email",
                "token-push@example.com",
            ]);

            std::fs::write(
                std::path::Path::new(&working_copy_path).join("bearer.txt"),
                "Bearer push\n",
            )
            .unwrap();
            run(&["-C", &working_copy_path, "add", "bearer.txt"]);
            run(&["-C", &working_copy_path, "commit", "-m", "Bearer push"]);
            let bearer_push = push_with_header(&bearer_header);

            std::fs::write(
                std::path::Path::new(&working_copy_path).join("basic.txt"),
                "Basic token password push\n",
            )
            .unwrap();
            run(&["-C", &working_copy_path, "add", "basic.txt"]);
            run(&[
                "-C",
                &working_copy_path,
                "commit",
                "-m",
                "Basic token password push",
            ]);
            let basic_password_push = push_with_header(&basic_header);

            std::fs::write(
                std::path::Path::new(&working_copy_path).join("read-only.txt"),
                "This push must be denied\n",
            )
            .unwrap();
            run(&["-C", &working_copy_path, "add", "read-only.txt"]);
            run(&["-C", &working_copy_path, "commit", "-m", "Denied push"]);
            let read_only_push = push_with_header(&read_header);
            (bearer_push, basic_password_push, read_only_push)
        })
        .await
        .expect("join Git token pushes");

    assert!(bearer_push.status.success(), "{bearer_push:?}");
    assert!(
        basic_password_push.status.success(),
        "Basic token-password push failed: {basic_password_push:?}"
    );
    assert!(
        !read_only_push.status.success(),
        "read-only token must not push: {read_only_push:?}"
    );

    server_handle.stop(true).await;
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&working_copy);
    let _ = std::fs::remove_file(database_path);
}

#[actix_web::test]
async fn test_git_client_push_uses_managed_and_custom_shell_hooks() {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let root = std::env::temp_dir().join(format!(
        "test_twig_native_git_hooks_{}",
        uuid::Uuid::new_v4()
    ));
    let repository = root.join("public/example");
    std::fs::create_dir_all(&repository).unwrap();
    crate::git::repo::bare_init(&repository, "main", "Test", "test@example.com").unwrap();

    let custom_marker = root.join("custom-shell-hook-ran");
    let hooks_dir = repository.join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    let custom_hook = hooks_dir.join("pre-receive.twig-user");
    let hook_script = format!(
        "#!/bin/sh\nprintf '%s\\n' called > '{}'\nwhile read old_oid new_oid ref_name; do\n  if [ \"$ref_name\" = refs/heads/reject-me ]; then\n    echo 'Twig test hook rejected refs/heads/reject-me' >&2\n    exit 1\n  fi\ndone\n",
        custom_marker.display()
    );
    std::fs::write(&custom_hook, hook_script).unwrap();
    std::fs::set_permissions(&custom_hook, std::fs::Permissions::from_mode(0o755)).unwrap();

    let (address, server_handle, db, database_path) = start_network_git_server(&root).await;
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
        assert!(!rejected_update.status.success(), "{rejected_update:?}");
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
    .expect("join Git HTTP backend push test");

    assert!(
        rejected_update.contains("Twig test hook rejected refs/heads/reject-me"),
        "{rejected_update}"
    );
    assert!(
        rejected_push.contains("Twig: Contributors cannot rewrite history on main."),
        "{rejected_push}"
    );
    assert!(
        custom_marker.exists(),
        "custom shell hook should run for a successful push"
    );

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
async fn test_twig_runtime_works_with_git_on_a_restricted_path() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    if std::env::var_os("TWIG_NATIVE_PACK_TEST_CHILD").is_some() {
        let root = std::path::PathBuf::from(
            std::env::var_os("TWIG_NATIVE_PACK_TEST_ROOT").expect("child root path"),
        );
        return run_native_pack_server_child(&root).await;
    }

    let root = std::env::temp_dir().join(format!(
        "test_twig_restricted_server_path_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let client_path = std::env::var("PATH").expect("parent PATH");
    let git = std::env::split_paths(&client_path)
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("find Git test client");
    let server_path = root.join("restricted-path");
    std::fs::create_dir_all(&server_path).unwrap();
    let git_directory = git.parent().expect("Git executable directory");
    for tool in ["git", "mktemp", "rm", "cat", "dirname"] {
        let executable = if tool == "git" {
            git.clone()
        } else {
            std::env::split_paths(&client_path)
                .map(|directory| directory.join(tool))
                .find(|candidate| candidate.is_file())
                .unwrap_or_else(|| git_directory.join(tool))
        };
        std::os::unix::fs::symlink(executable, server_path.join(tool)).unwrap();
    }
    let executable = std::env::current_exe().unwrap();
    let child_root = root.clone();
    let child_path = server_path.clone();
    let mut server = Command::new(executable)
        .args([
            "--exact",
            "integration_tests::tests::repo_git_http::test_twig_runtime_works_with_git_on_a_restricted_path",
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
        "served with only Git and receive-hook tools on the server PATH\n",
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
    let repository = root.join("public/example");
    std::fs::create_dir_all(&repository).unwrap();
    crate::git::repo::bare_init(&repository, "main", "Test", "test@example.com")
        .expect("create repository with git on PATH");

    let (address, server_handle, db, database_path) = start_network_git_server(root).await;
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
