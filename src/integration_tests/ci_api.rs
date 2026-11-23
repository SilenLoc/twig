use std::path::Path;
use std::time::Duration;

use super::repo_git_http::start_network_git_server_with_ci;
use crate::config::CiSettings;
use crate::db::ci_runs::RunStatus;

const DEFINITION: &str = "[tools]\n\n[tasks.ci]\ndir = \"..\"\nrun = 'echo ci'\n";

/// Writes a stand-in for `mise` that reports what it was given and succeeds.
fn fake_mise(root: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let directory = root.join("bin");
    std::fs::create_dir_all(&directory).expect("bin directory");
    let path = directory.join("fake-mise");
    std::fs::write(
        &path,
        "#!/bin/sh\necho \"mise $*\"\necho \"sha=$TWIG_CI_SHA event=$TWIG_CI_EVENT\"\n",
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
