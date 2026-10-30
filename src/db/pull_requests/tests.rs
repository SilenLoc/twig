use super::*;

async fn database() -> (Database, String) {
    let path = format!("/tmp/test_twig_pull_requests_{}.db", Uuid::new_v4());
    let db = Database::new(&path);
    db.init_tables().await.expect("init tables");
    (db, path)
}

fn new_request<'a>(repo_id: &'a str, head: &'a str) -> NewPullRequest<'a> {
    NewPullRequest {
        repo_id,
        title: "Add retry",
        body: "Explains the retry policy.",
        author_user_id: "user-1",
        head_branch: head,
        base_branch: "main",
    }
}

#[tokio::test]
async fn create_pull_request_allocates_sequential_numbers_per_repo() {
    let (db, path) = database().await;

    let first = db
        .create_pull_request(new_request("repo-a", "feature"))
        .await
        .unwrap();
    let second = db
        .create_pull_request(new_request("repo-a", "other"))
        .await
        .unwrap();
    let elsewhere = db
        .create_pull_request(new_request("repo-b", "feature"))
        .await
        .unwrap();

    assert_eq!((first.number, second.number), (1, 2));
    assert_eq!(elsewhere.number, 1, "numbers are per repository");
    assert_eq!(first.state, PullRequestState::Open);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn concurrent_creates_never_share_a_number() {
    let (db, path) = database().await;

    let mut tasks = Vec::new();
    for index in 0..8 {
        let db = db.clone();
        tasks.push(tokio::spawn(async move {
            let head = format!("branch-{index}");
            db.create_pull_request(NewPullRequest {
                repo_id: "repo-race",
                title: "Race",
                body: "",
                author_user_id: "user-1",
                head_branch: &head,
                base_branch: "main",
            })
            .await
            .map(|pull_request| pull_request.number)
        }));
    }
    let mut numbers = Vec::new();
    for task in tasks {
        numbers.push(task.await.unwrap().expect("create succeeds"));
    }
    numbers.sort_unstable();
    assert_eq!(numbers, (1..=8).collect::<Vec<i64>>());
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn list_filters_by_state_head_and_pages_with_has_more() {
    let (db, path) = database().await;
    let one = db
        .create_pull_request(new_request("repo", "feature"))
        .await
        .unwrap();
    db.create_pull_request(new_request("repo", "feature-two"))
        .await
        .unwrap();
    db.edit_pull_request(
        &one.id,
        &PullRequestEdit {
            state: Some(PullRequestState::Closed),
            ..Default::default()
        },
        "2026-10-30T10:00:00Z",
    )
    .await
    .unwrap();

    let open = PullRequestFilter {
        state: Some(PullRequestState::Open),
        limit: 10,
        ..Default::default()
    };
    let (rows, has_more) = db.list_pull_requests("repo", &open).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].head_branch, "feature-two");
    assert!(!has_more);

    let head = PullRequestFilter {
        head_branch: Some("feature".into()),
        limit: 10,
        ..Default::default()
    };
    let (rows, _) = db.list_pull_requests("repo", &head).await.unwrap();
    assert_eq!(
        rows.iter().map(|row| row.number).collect::<Vec<_>>(),
        vec![1]
    );

    let paged = PullRequestFilter {
        limit: 1,
        ..Default::default()
    };
    let (rows, has_more) = db.list_pull_requests("repo", &paged).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(has_more, "a second request exists beyond the first page");
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn closing_sets_closed_at_and_reopening_clears_it() {
    let (db, path) = database().await;
    let pull_request = db
        .create_pull_request(new_request("repo", "feature"))
        .await
        .unwrap();

    db.edit_pull_request(
        &pull_request.id,
        &PullRequestEdit {
            state: Some(PullRequestState::Closed),
            ..Default::default()
        },
        "2026-10-30T10:00:00Z",
    )
    .await
    .unwrap();
    let closed = db.get_pull_request("repo", 1).await.unwrap().unwrap();
    assert_eq!(closed.state, PullRequestState::Closed);
    assert_eq!(closed.closed_at.as_deref(), Some("2026-10-30T10:00:00Z"));

    db.edit_pull_request(
        &pull_request.id,
        &PullRequestEdit {
            state: Some(PullRequestState::Open),
            title: Some("Renamed".into()),
            ..Default::default()
        },
        "2026-10-30T11:00:00Z",
    )
    .await
    .unwrap();
    let reopened = db.get_pull_request("repo", 1).await.unwrap().unwrap();
    assert_eq!(reopened.state, PullRequestState::Open);
    assert_eq!(reopened.closed_at, None);
    assert_eq!(
        reopened.title, "Renamed",
        "untouched fields in the same edit still apply"
    );
    assert_eq!(reopened.body, "Explains the retry policy.");
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn find_open_and_delete_are_scoped_to_one_repository() {
    let (db, path) = database().await;
    db.create_pull_request(new_request("repo-a", "feature"))
        .await
        .unwrap();
    db.create_pull_request(new_request("repo-b", "feature"))
        .await
        .unwrap();

    assert!(
        db.find_open_pull_request("repo-a", "feature", "main")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        db.find_open_pull_request("repo-a", "feature", "release")
            .await
            .unwrap()
            .is_none()
    );

    db.delete_pull_requests_for_repo("repo-a").await.unwrap();
    assert!(db.get_pull_request("repo-a", 1).await.unwrap().is_none());
    assert!(db.get_pull_request("repo-b", 1).await.unwrap().is_some());
    let _ = std::fs::remove_file(path);
}
