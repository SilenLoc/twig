use super::fixtures::seed_owner_repo;
use super::repo_access_and_routes::init_private_repo;
use super::support::*;
use super::*;

use crate::db::issues::{Issue, IssueEdit, IssueState, NewIssue};
use crate::git::bare::RepoHandle;

/// Loads a page as the given session, or anonymously, and checks it answered 200.
async fn page_as(
    app: &impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    uri: &str,
    session: Option<&str>,
) -> String {
    let mut request = test::TestRequest::get().uri(uri);
    if let Some(session) = session {
        request = request.cookie(actix_web::cookie::Cookie::new(
            "session",
            session.to_string(),
        ));
    }
    let response = test::call_service(app, request.to_request()).await;
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    String::from_utf8(test::read_body(response).await.to_vec()).unwrap()
}

fn repo_id_of(root: &str, namespace: &str, repo: &str) -> String {
    RepoHandle::open(root, namespace, repo)
        .expect("open repository")
        .repo_id()
        .expect("repository id")
}

async fn open_issue(db: &crate::db::Database, repo_id: &str, title: &str, author: &str) -> Issue {
    db.create_issue(NewIssue {
        repo_id,
        title,
        body: "",
        author_user_id: author,
        assignee_user_ids: &[],
    })
    .await
    .expect("create issue")
}

#[actix_web::test]
async fn global_list_shows_only_the_issues_the_viewer_can_read() {
    let root = format!("/tmp/test_twig_global_issues_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    // `pr-team/tooling` is public. `pr-team/secret` is private, in the same namespace.
    let (owner, _, _) = seed_owner_repo(&db, &root).await;
    init_private_repo(&std::path::Path::new(&root).join("pr-team").join("secret"));

    // `libs/widgets` is public, in a namespace the owner does not belong to.
    let lib_owner = auth::create_user(
        "lib-owner".to_string(),
        "lib-owner@example.com".to_string(),
        "password123",
    )
    .expect("create lib owner");
    db.create_user(&lib_owner).await.expect("persist lib owner");
    db.create_namespace(&auth::create_namespace(
        "libs".to_string(),
        lib_owner.id.clone(),
    ))
    .await
    .expect("persist namespace");
    let widgets = std::path::Path::new(&root).join("libs").join("widgets");
    std::fs::create_dir_all(&widgets).expect("widgets dir");
    twig_git::repo::bare_init(&widgets, "main", "Test", "test@example.com").expect("init widgets");

    let drifter = auth::create_user(
        "drifter".to_string(),
        "drifter@example.com".to_string(),
        "password123",
    )
    .expect("create drifter");
    db.create_user(&drifter).await.expect("persist drifter");

    let tooling = repo_id_of(&root, "pr-team", "tooling");
    let secret = repo_id_of(&root, "pr-team", "secret");
    let gadgets = repo_id_of(&root, "libs", "widgets");
    open_issue(&db, &tooling, "Public crash", &owner.id).await;
    let shipped = open_issue(&db, &tooling, "Shipped fix", &owner.id).await;
    db.edit_issue(
        &shipped.id,
        &IssueEdit {
            state: Some(IssueState::Closed),
            ..Default::default()
        },
        "2026-11-23T10:00:00Z",
    )
    .await
    .expect("close issue");
    open_issue(&db, &secret, "Secret plan", &owner.id).await;
    open_issue(&db, &gadgets, "Widget bug", &lib_owner.id).await;

    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let owner_session = auth_state
        .create_session(owner.id.clone())
        .await
        .expect("owner session");
    let drifter_session = auth_state
        .create_session(drifter.id.clone())
        .await
        .expect("drifter session");

    // Anonymous visitors see the public repositories only.
    let anonymous = page_as(&app, "/issues", None).await;
    assert!(anonymous.contains("Public crash"));
    assert!(anonymous.contains("Widget bug"));
    assert!(
        !anonymous.contains("Secret plan"),
        "private issues stay hidden"
    );
    assert!(
        !anonymous.contains("/pr-team/secret/"),
        "no link to a private repository"
    );
    assert!(anonymous.contains("2 Open"), "two open public issues");
    assert!(anonymous.contains("1 Closed"));
    assert!(
        anonymous.contains("href=\"/issues\""),
        "the masthead links to the list"
    );

    // Signed in without membership, a visitor sees the same as anonymous.
    let drifting = page_as(&app, "/issues", Some(&drifter_session)).await;
    assert!(!drifting.contains("Secret plan"));
    assert!(drifting.contains("2 Open"));

    // A namespace member sees the private repository too, with its own link.
    let owned = page_as(&app, "/issues", Some(&owner_session)).await;
    assert!(owned.contains("Secret plan"));
    assert!(owned.contains("3 Open"));
    assert!(owned.contains("href=\"/pr-team/tooling/issues/1\""));
    assert!(
        owned.contains("pr-team/secret"),
        "the row names its repository"
    );
}

#[actix_web::test]
async fn global_list_filters_by_state_search_and_account() {
    let root = format!("/tmp/test_twig_global_filters_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, _, _) = seed_owner_repo(&db, &root).await;
    let tooling = repo_id_of(&root, "pr-team", "tooling");
    open_issue(&db, &tooling, "Widget overflow", &owner.id).await;
    let closed = open_issue(&db, &tooling, "Login loop", &owner.id).await;
    db.edit_issue(
        &closed.id,
        &IssueEdit {
            state: Some(IssueState::Closed),
            ..Default::default()
        },
        "2026-11-23T10:00:00Z",
    )
    .await
    .expect("close issue");

    let closed_only = page_as(&app, "/issues?state=closed", None).await;
    assert!(closed_only.contains("Login loop"));
    assert!(!closed_only.contains("Widget overflow"));

    let searched = page_as(&app, "/issues?state=all&q=WIDGET", None).await;
    assert!(searched.contains("Widget overflow"));
    assert!(!searched.contains("Login loop"));

    let by_owner = page_as(&app, "/issues?author=pr-owner", None).await;
    assert!(by_owner.contains("Widget overflow"));

    let unknown = page_as(&app, "/issues?author=nobody-here", None).await;
    assert!(
        unknown.contains("There are no issues in this view."),
        "an unknown author lists nothing"
    );
    assert!(!unknown.contains("Widget overflow"));
}
