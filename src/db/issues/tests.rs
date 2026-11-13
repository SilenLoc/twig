use super::*;
use crate::db::pull_requests::NewPullRequest;
use uuid::Uuid;

async fn database() -> (Database, String) {
    let path = format!("/tmp/test_twig_issues_{}.db", Uuid::new_v4());
    let db = Database::new(&path);
    db.init_tables().await.expect("init tables");
    (db, path)
}

fn new_issue<'a>(repo_id: &'a str, title: &'a str, body: &'a str) -> NewIssue<'a> {
    NewIssue {
        repo_id,
        title,
        body,
        author_user_id: "user-1",
        assignee_user_ids: &[],
    }
}

fn everything() -> IssueFilter {
    IssueFilter {
        limit: 50,
        ..Default::default()
    }
}

#[tokio::test]
async fn create_issue_shares_numbers_with_pull_requests() {
    let (db, path) = database().await;

    let first = db
        .create_issue(new_issue("repo-a", "Crash", "It crashes."))
        .await
        .unwrap();
    let pull = db
        .create_pull_request(NewPullRequest {
            repo_id: "repo-a",
            title: "Fix crash",
            body: "",
            author_user_id: "user-1",
            head_branch: "fix",
            base_branch: "main",
        })
        .await
        .unwrap();
    let second = db
        .create_issue(new_issue("repo-a", "Slow start", ""))
        .await
        .unwrap();
    let elsewhere = db
        .create_issue(new_issue("repo-b", "Other repo", ""))
        .await
        .unwrap();

    assert_eq!(first.number, 1);
    assert_eq!(
        pull.number, 2,
        "a pull request takes the next shared number"
    );
    assert_eq!(second.number, 3, "an issue takes the next shared number");
    assert_eq!(elsewhere.number, 1, "numbers are per repository");
    assert_eq!(first.state, IssueState::Open);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn pull_request_after_issue_takes_next_shared_number() {
    let (db, path) = database().await;

    db.create_issue(new_issue("repo-a", "One", ""))
        .await
        .unwrap();
    db.create_issue(new_issue("repo-a", "Two", ""))
        .await
        .unwrap();
    let pull = db
        .create_pull_request(NewPullRequest {
            repo_id: "repo-a",
            title: "Change",
            body: "",
            author_user_id: "user-1",
            head_branch: "feature",
            base_branch: "main",
        })
        .await
        .unwrap();

    assert_eq!(pull.number, 3);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn list_filters_by_state_text_author_and_assignee() {
    let (db, path) = database().await;
    let bob = "user-bob".to_string();
    let assigned = [bob.clone()];

    let crash = db
        .create_issue(NewIssue {
            repo_id: "repo-a",
            title: "Crash on start",
            body: "Segfaults on launch.",
            author_user_id: "user-1",
            assignee_user_ids: &assigned,
        })
        .await
        .unwrap();
    db.create_issue(NewIssue {
        repo_id: "repo-a",
        title: "Docs",
        body: "The README is out of date.",
        author_user_id: "user-2",
        assignee_user_ids: &[],
    })
    .await
    .unwrap();
    db.edit_issue(
        &crash.id,
        &IssueEdit {
            state: Some(IssueState::Closed),
            ..Default::default()
        },
        "2026-11-13T10:00:00Z",
    )
    .await
    .unwrap();

    let open = IssueFilter {
        state: Some(IssueState::Open),
        ..everything()
    };
    let (rows, _) = db.list_issues("repo-a", &open).await.unwrap();
    assert_eq!(
        rows.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(),
        ["Docs"]
    );

    let text = IssueFilter {
        query: Some("SEGFAULT".to_string()),
        ..everything()
    };
    let (rows, _) = db.list_issues("repo-a", &text).await.unwrap();
    assert_eq!(rows.len(), 1, "text matches the body, case-insensitively");
    assert_eq!(rows[0].title, "Crash on start");

    let by_author = IssueFilter {
        author_user_id: Some("user-2".to_string()),
        ..everything()
    };
    let (rows, _) = db.list_issues("repo-a", &by_author).await.unwrap();
    assert_eq!(
        rows.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(),
        ["Docs"]
    );

    let by_assignee = IssueFilter {
        assignee_user_id: Some(bob),
        state: None,
        ..everything()
    };
    let (rows, _) = db.list_issues("repo-a", &by_assignee).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].assignee_user_ids, vec!["user-bob".to_string()]);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn list_pages_with_has_more_and_count() {
    let (db, path) = database().await;
    for index in 0..3 {
        db.create_issue(new_issue("repo-a", &format!("Item {index}"), ""))
            .await
            .unwrap();
    }
    let page = IssueFilter {
        limit: 2,
        ..Default::default()
    };

    let (rows, has_more) = db.list_issues("repo-a", &page).await.unwrap();
    assert_eq!(rows.iter().map(|i| i.number).collect::<Vec<_>>(), [3, 2]);
    assert!(has_more, "a third row exists");
    let (rows, has_more) = db
        .list_issues(
            "repo-a",
            &IssueFilter {
                offset: 2,
                ..page.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(rows.iter().map(|i| i.number).collect::<Vec<_>>(), [1]);
    assert!(!has_more);
    assert_eq!(db.count_issues("repo-a", &everything()).await.unwrap(), 3);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn edit_issue_close_sets_closed_at_and_reopen_clears_it() {
    let (db, path) = database().await;
    let issue = db
        .create_issue(new_issue("repo-a", "Flaky", ""))
        .await
        .unwrap();

    db.edit_issue(
        &issue.id,
        &IssueEdit {
            state: Some(IssueState::Closed),
            ..Default::default()
        },
        "2026-11-13T10:00:00Z",
    )
    .await
    .unwrap();
    let closed = db.get_issue("repo-a", issue.number).await.unwrap().unwrap();
    assert_eq!(closed.state, IssueState::Closed);
    assert_eq!(closed.closed_at.as_deref(), Some("2026-11-13T10:00:00Z"));
    assert_eq!(db.count_issues_by_state("repo-a").await.unwrap(), (0, 1));

    db.edit_issue(
        &issue.id,
        &IssueEdit {
            state: Some(IssueState::Open),
            ..Default::default()
        },
        "2026-11-13T11:00:00Z",
    )
    .await
    .unwrap();
    let reopened = db.get_issue("repo-a", issue.number).await.unwrap().unwrap();
    assert_eq!(reopened.state, IssueState::Open);
    assert_eq!(reopened.closed_at, None);
    assert_eq!(db.count_issues_by_state("repo-a").await.unwrap(), (1, 0));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn delete_issues_for_repo_leaves_other_repositories() {
    let (db, path) = database().await;
    db.create_issue(new_issue("repo-a", "Gone", ""))
        .await
        .unwrap();
    let kept = db
        .create_issue(new_issue("repo-b", "Kept", ""))
        .await
        .unwrap();

    db.delete_issues_for_repo("repo-a").await.unwrap();

    assert_eq!(db.count_issues("repo-a", &everything()).await.unwrap(), 0);
    assert!(db.get_issue_by_id(&kept.id).await.unwrap().is_some());
    let _ = std::fs::remove_file(path);
}

#[test]
fn like_pattern_matches_wildcard_characters_literally() {
    assert_eq!(like_pattern("50%_\\"), "%50\\%\\_\\\\%");
    assert_eq!(like_pattern("Crash"), "%crash%");
}

#[tokio::test]
async fn issue_comments_add_list_update_and_delete() {
    use super::comments::issue_comment_api_id;
    let (db, path) = database().await;
    let issue = db
        .create_issue(new_issue("repo-a", "Discuss", ""))
        .await
        .unwrap();
    let other = db
        .create_issue(new_issue("repo-a", "Elsewhere", ""))
        .await
        .unwrap();

    let first = db
        .add_issue_comment(&issue.id, "user-1", "First")
        .await
        .unwrap();
    let second = db
        .add_issue_comment(&issue.id, "user-2", "Second")
        .await
        .unwrap();
    db.add_issue_comment(&other.id, "user-1", "Not here")
        .await
        .unwrap();

    let listed = db.list_issue_comments(&issue.id).await.unwrap();
    assert_eq!(
        listed.iter().map(|c| c.body.as_str()).collect::<Vec<_>>(),
        ["First", "Second"],
        "only this issue's comments, oldest first"
    );
    assert_eq!(db.count_issue_comments(&issue.id).await.unwrap(), 2);
    assert_ne!(
        issue_comment_api_id(first.row_id),
        issue_comment_api_id(second.row_id)
    );

    db.update_issue_comment(first.row_id, "Edited", "2026-11-13T12:00:00Z")
        .await
        .unwrap();
    let edited = db.get_issue_comment(first.row_id).await.unwrap().unwrap();
    assert_eq!(edited.body, "Edited");
    assert_eq!(edited.updated_at.as_deref(), Some("2026-11-13T12:00:00Z"));

    db.delete_issue_comment(second.row_id).await.unwrap();
    assert!(db.get_issue_comment(second.row_id).await.unwrap().is_none());
    assert_eq!(db.count_issue_comments(&issue.id).await.unwrap(), 1);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn replace_issue_assignees_replaces_the_whole_set() {
    let (db, path) = database().await;
    let first = ["user-a".to_string(), "user-b".to_string()];
    let issue = db
        .create_issue(NewIssue {
            repo_id: "repo-a",
            title: "Assign",
            body: "",
            author_user_id: "user-1",
            assignee_user_ids: &first,
        })
        .await
        .unwrap();
    assert_eq!(issue.assignee_user_ids, first.to_vec());

    db.replace_issue_assignees(&issue.id, &["user-c".to_string()], "2026-11-13T12:00:00Z")
        .await
        .unwrap();
    let replaced = db.get_issue_by_id(&issue.id).await.unwrap().unwrap();
    assert_eq!(replaced.assignee_user_ids, vec!["user-c".to_string()]);

    db.replace_issue_assignees(&issue.id, &[], "2026-11-13T12:01:00Z")
        .await
        .unwrap();
    let cleared = db.get_issue_by_id(&issue.id).await.unwrap().unwrap();
    assert!(
        cleared.assignee_user_ids.is_empty(),
        "an empty list clears the issue"
    );
    let _ = std::fs::remove_file(path);
}
