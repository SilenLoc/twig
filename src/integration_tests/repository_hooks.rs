use super::support::*;
use super::*;

#[actix_web::test]
async fn hooks_tab_and_navigation_require_repository_management_permission() {
    let (app, manager_session, _, contributor_session, _, project_root) =
        create_admin_test_service().await;
    let repo_path = std::path::Path::new(&project_root).join("acme/hook-demo");
    std::fs::create_dir_all(&repo_path).unwrap();
    crate::git::repo::bare_init(&repo_path, "main", "Test", "test@example.com").unwrap();
    let custom_hook = repo_path.join("hooks/pre-receive.twig-user");
    std::fs::write(&custom_hook, "#!/bin/sh\necho SAFE_INSPECTION_MARKER\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&custom_hook, std::fs::Permissions::from_mode(0o750)).unwrap();
    }

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/acme/hook-demo/hooks")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/acme/hook-demo/hooks")
            .cookie(actix_web::cookie::Cookie::new(
                "session",
                contributor_session.clone(),
            ))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/acme/hook-demo/hooks")
            .cookie(actix_web::cookie::Cookie::new(
                "session",
                manager_session.clone(),
            ))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("Git server hooks"), "{body}");
    assert!(
        body.contains("MISSING"),
        "the uninstalled managed hook is surfaced: {body}"
    );
    assert!(body.contains("Custom pre-receive hooks"), "{body}");
    assert!(body.contains("SAFE_INSPECTION_MARKER"), "{body}");
    assert!(body.contains("NO RECORDED RUNS"), "{body}");
    assert!(body.contains("aria-current=\"page\""), "{body}");

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/acme/hook-demo")
            .cookie(actix_web::cookie::Cookie::new("session", manager_session))
            .to_request(),
    )
    .await;
    let body = test::read_body(response).await;
    assert!(
        String::from_utf8(body.to_vec())
            .unwrap()
            .contains("href=\"/acme/hook-demo/hooks\""),
        "managers can reach Hooks from the repository tab bar"
    );

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/acme/hook-demo")
            .cookie(actix_web::cookie::Cookie::new(
                "session",
                contributor_session,
            ))
            .to_request(),
    )
    .await;
    let body = test::read_body(response).await;
    assert!(
        !String::from_utf8(body.to_vec())
            .unwrap()
            .contains(">Hooks</a>"),
        "non-managers do not see the hook-management link"
    );

    let _ = std::fs::remove_dir_all(&project_root);
}

#[cfg(unix)]
#[actix_web::test]
async fn hook_mutations_require_manager_access_and_confirm_permanent_deletion() {
    use std::os::unix::fs::PermissionsExt;

    let (app, manager_session, _, contributor_session, _, project_root) =
        create_admin_test_service().await;
    let repo_path = std::path::Path::new(&project_root).join("acme/hook-actions");
    std::fs::create_dir_all(&repo_path).unwrap();
    crate::git::repo::bare_init(&repo_path, "main", "Test", "test@example.com").unwrap();
    twig_git::repo::install_pre_receive_hook(&repo_path).unwrap();

    let source = "#!/bin/sh\necho keep this script intact\n";
    let custom_hook = repo_path.join("hooks/pre-receive.twig-user");
    std::fs::write(&custom_hook, source).unwrap();
    std::fs::set_permissions(&custom_hook, std::fs::Permissions::from_mode(0o750)).unwrap();

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/acme/hook-actions/hooks/pre-receive.twig-user/disable")
            .cookie(actix_web::cookie::Cookie::new(
                "session",
                contributor_session,
            ))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_ne!(
        std::fs::metadata(&custom_hook)
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0,
        "a contributor cannot disable a hook"
    );

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/acme/hook-actions/hooks/pre-receive.twig-user/disable")
            .cookie(actix_web::cookie::Cookie::new(
                "session",
                manager_session.clone(),
            ))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        std::fs::metadata(&custom_hook)
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0
    );
    assert_eq!(std::fs::read_to_string(&custom_hook).unwrap(), source);

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/acme/hook-actions/hooks/pre-receive.twig-user/delete")
            .cookie(actix_web::cookie::Cookie::new(
                "session",
                manager_session.clone(),
            ))
            .set_form([("confirm", "no")])
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        custom_hook.exists(),
        "deletion requires explicit confirmation"
    );

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/acme/hook-actions/hooks/pre-receive.twig-user/delete")
            .cookie(actix_web::cookie::Cookie::new("session", manager_session))
            .set_form([("confirm", "yes")])
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(!custom_hook.exists());
    assert!(
        std::fs::read_to_string(repo_path.join("hooks/pre-receive"))
            .unwrap()
            .contains("TWIG_MANAGED_PRE_RECEIVE_V1"),
        "custom hook deletion leaves Twig's managed policy in place"
    );

    let _ = std::fs::remove_dir_all(&project_root);
}

#[cfg(unix)]
#[actix_web::test]
async fn hooks_page_shows_a_real_custom_hook_run_after_a_push_hook_execution() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    let (app, manager_session, _, _, _, project_root) = create_admin_test_service().await;
    let repo_path = std::path::Path::new(&project_root).join("acme/hook-history");
    std::fs::create_dir_all(&repo_path).unwrap();
    crate::git::repo::bare_init(&repo_path, "main", "Test", "test@example.com").unwrap();
    twig_git::repo::install_pre_receive_hook(&repo_path).unwrap();
    let custom_hook = repo_path.join("hooks/pre-receive.twig-user");
    std::fs::write(
        &custom_hook,
        "#!/bin/sh\nprintf 'RUN_STDOUT_MARKER\\n'\nprintf 'RUN_STDERR_MARKER\\n' >&2\nexit 0\n",
    )
    .unwrap();
    std::fs::set_permissions(&custom_hook, std::fs::Permissions::from_mode(0o750)).unwrap();

    let updates = format!("{} {} refs/heads/feature\n", "0".repeat(40), "1".repeat(40));
    let mut hook = Command::new(repo_path.join("hooks/pre-receive"))
        .current_dir(&repo_path)
        .env("GIT_DIR", &repo_path)
        .env("TWIG_NAMESPACE_ROLE", "owner")
        .env("REMOTE_USER", "hook-history-user")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run installed pre-receive hook");
    hook.stdin
        .as_mut()
        .unwrap()
        .write_all(updates.as_bytes())
        .unwrap();
    let output = hook.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("RUN_STDOUT_MARKER"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("RUN_STDERR_MARKER"));

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/acme/hook-history/hooks")
            .cookie(actix_web::cookie::Cookie::new("session", manager_session))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();
    assert!(body.contains("pre-receive.twig-user"), "{body}");
    assert!(body.contains("PASSED"), "{body}");
    assert!(body.contains("hook-history-user"), "{body}");
    assert!(body.contains("refs/heads/feature"), "{body}");
    assert!(body.contains("RUN_STDOUT_MARKER"), "{body}");
    assert!(body.contains("RUN_STDERR_MARKER"), "{body}");

    let _ = std::fs::remove_dir_all(&project_root);
}
