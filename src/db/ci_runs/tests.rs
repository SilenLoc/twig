use super::*;
use uuid::Uuid;

/// One run by ID, read back through the repository's listing.
async fn find(db: &Database, repo_id: &str, id: i64) -> Option<CiRun> {
    let (runs, _) = db.list_ci_runs(repo_id, 50, 0).await.unwrap();
    runs.into_iter().find(|run| run.id == id)
}

async fn database() -> (Database, String) {
    let path = format!("/tmp/test_twig_ci_runs_{}.db", Uuid::new_v4());
    let db = Database::new(&path);
    db.init_tables().await.expect("init tables");
    (db, path)
}

fn new_run<'a>(repo_id: &'a str, sha: &'a str) -> NewCiRun<'a> {
    NewCiRun {
        repo_id,
        workflow: "ci",
        event: "push",
        head_branch: "main",
        head_sha: sha,
        display_title: "Fix the build",
        triggered_by_user_id: Some("user-1"),
    }
}

#[tokio::test]
async fn runs_are_numbered_per_repository_and_listed_newest_first() {
    let (db, path) = database().await;

    let first = db.create_ci_run(new_run("repo-a", "aaa")).await.unwrap();
    let second = db.create_ci_run(new_run("repo-a", "bbb")).await.unwrap();
    let elsewhere = db.create_ci_run(new_run("repo-b", "ccc")).await.unwrap();

    assert_eq!((first.run_number, second.run_number), (1, 2));
    assert_eq!(elsewhere.run_number, 1, "numbers are per repository");
    assert_eq!(first.status, RunStatus::Waiting);

    let (runs, total) = db.list_ci_runs("repo-a", 20, 0).await.unwrap();
    assert_eq!(total, 2);
    assert_eq!(
        runs.iter().map(|run| run.id).collect::<Vec<_>>(),
        vec![second.id, first.id],
        "newest first"
    );

    let (page, total) = db.list_ci_runs("repo-a", 1, 1).await.unwrap();
    assert_eq!(total, 2);
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].id, first.id);

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn runs_are_only_visible_within_their_repository() {
    let (db, path) = database().await;
    let run = db.create_ci_run(new_run("repo-a", "aaa")).await.unwrap();

    assert!(find(&db, "repo-b", run.id).await.is_none());
    assert!(
        db.find_ci_run_log("repo-b", run.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(find(&db, "repo-a", run.id).await.is_some());

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn a_run_moves_through_started_and_finished_with_its_log() {
    let (db, path) = database().await;
    let run = db.create_ci_run(new_run("repo-a", "aaa")).await.unwrap();

    db.mark_ci_run_started(run.id).await.unwrap();
    let running = find(&db, "repo-a", run.id).await.unwrap();
    assert_eq!(running.status, RunStatus::Running);
    assert!(running.started_at.is_some());

    db.finish_ci_run(run.id, RunStatus::Failure, "boom\n")
        .await
        .unwrap();
    let finished = find(&db, "repo-a", run.id).await.unwrap();
    assert_eq!(finished.status, RunStatus::Failure);
    assert!(finished.finished_at.is_some());
    assert_eq!(
        db.find_ci_run_log("repo-a", run.id)
            .await
            .unwrap()
            .as_deref(),
        Some("boom\n")
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn runs_left_unfinished_by_a_restart_are_closed_as_failed() {
    let (db, path) = database().await;
    let queued = db.create_ci_run(new_run("repo-a", "aaa")).await.unwrap();
    let running = db.create_ci_run(new_run("repo-a", "bbb")).await.unwrap();
    db.mark_ci_run_started(running.id).await.unwrap();
    let done = db.create_ci_run(new_run("repo-a", "ccc")).await.unwrap();
    db.finish_ci_run(done.id, RunStatus::Success, "ok")
        .await
        .unwrap();

    db.fail_unfinished_ci_runs("restarted").await.unwrap();

    for id in [queued.id, running.id] {
        let run = find(&db, "repo-a", id).await.unwrap();
        assert_eq!(run.status, RunStatus::Failure);
        let log = db.find_ci_run_log("repo-a", id).await.unwrap().unwrap();
        assert!(log.contains("restarted"));
    }
    let untouched = find(&db, "repo-a", done.id).await.unwrap();
    assert_eq!(untouched.status, RunStatus::Success);

    let _ = std::fs::remove_file(path);
}
