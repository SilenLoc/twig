use super::pull_requests_api::{push_branch, seed_owner_repo};
use super::support::*;
use super::*;

#[derive(serde::Serialize)]
struct CreateForm<'a> {
    base: &'a str,
    head: &'a str,
    title: &'a str,
    body: &'a str,
}

#[actix_web::test]
async fn pull_requests_pages_create_list_and_show_a_request() {
    let root = format!("/tmp/test_twig_pr_ui_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let session = auth_state
        .create_session(user.id.clone())
        .await
        .expect("session");
    let cookie = || actix_web::cookie::Cookie::new("session", session.clone());
    let tab = format!("/{namespace}/{repo}/pulls");

    // Anonymous visitors see the tab but no way to open a request.
    let (status, body) = body_of(&app, &tab).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Pull requests"), "tab label is rendered");
    assert!(body.contains("There are no pull requests in this view."));
    assert!(!body.contains("New pull request"));
    let (_, anonymous_compare) = body_of(&app, &format!("{tab}/new")).await;
    assert!(
        !anonymous_compare.contains("name=\"title\""),
        "no form without sign-in"
    );

    // Signed in: the compare page shows the commits the head adds.
    let compare = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{tab}/new?base=main&head=feature/retry"))
            .cookie(cookie())
            .to_request(),
    )
    .await;
    assert_eq!(compare.status(), StatusCode::OK);
    let compare_html = String::from_utf8(test::read_body(compare).await.to_vec()).unwrap();
    assert!(
        compare_html.contains("Add retry.txt"),
        "commit summary is listed"
    );
    assert!(compare_html.contains("name=\"title\""));

    // Creating the request redirects to its conversation page.
    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/new"))
            .cookie(cookie())
            .set_form(CreateForm {
                base: "main",
                head: "feature/retry",
                title: "Add retry",
                body: "Explains the policy.",
            })
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        created.headers().get("Location").unwrap(),
        &format!("/{namespace}/{repo}/pulls/1")
    );

    let (status, listed) = body_of(&app, &tab).await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed.contains("Add retry"), "row shows the title");
    assert!(listed.contains("1 Open"), "open count is shown");

    let (status, page) = body_of(&app, &format!("{tab}/1")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("Add retry"));
    assert!(page.contains("Explains the policy."));
    assert!(
        page.contains("wants to merge 1 commit into"),
        "byline matches Forgejo wording"
    );

    // A second request for the same branches is refused and the form is shown again.
    let duplicate = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/new"))
            .cookie(cookie())
            .set_form(CreateForm {
                base: "main",
                head: "feature/retry",
                title: "Again",
                body: "",
            })
            .to_request(),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let duplicate_html = String::from_utf8(test::read_body(duplicate).await.to_vec()).unwrap();
    assert!(duplicate_html.contains("already exists"));

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn pull_request_creation_redirects_visitors_who_are_not_signed_in() {
    let root = format!("/tmp/test_twig_pr_ui_anon_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (_user, namespace, repo) = seed_owner_repo(&db, &root).await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/{namespace}/{repo}/pulls/new"))
            .set_form(CreateForm {
                base: "main",
                head: "feature/retry",
                title: "Anonymous",
                body: "",
            })
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(
        response
            .headers()
            .get("Location")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("/auth/login")
    );

    let (status, listed) = body_of(&app, &format!("/{namespace}/{repo}/pulls")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!listed.contains("Anonymous"), "nothing was created");

    let _ = std::fs::remove_dir_all(&root);
}

#[derive(serde::Serialize)]
struct CommentForm<'a> {
    body: &'a str,
}

#[derive(serde::Serialize)]
struct StateChange<'a> {
    action: &'a str,
}

#[derive(serde::Serialize)]
struct EditChange<'a> {
    title: &'a str,
    body: &'a str,
}

#[derive(serde::Serialize)]
struct ReactionChange<'a> {
    target: &'a str,
    content: &'a str,
    on: &'a str,
}

/// Fetches a page as the signed-in user behind `session`.
async fn get_as(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    uri: &str,
    session: &str,
) -> String {
    let response = test::call_service(
        app,
        test::TestRequest::get()
            .uri(uri)
            .cookie(session_cookie(session))
            .to_request(),
    )
    .await;
    String::from_utf8(test::read_body(response).await.to_vec()).unwrap()
}

fn session_cookie(session: &str) -> actix_web::cookie::Cookie<'static> {
    actix_web::cookie::Cookie::new("session", session.to_string())
}

#[actix_web::test]
async fn conversation_comments_state_edit_and_reactions_work_from_the_page() {
    use super::pull_requests_api::seed_contributor;
    let root = format!("/tmp/test_twig_pr_conv_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    let contributor = seed_contributor(&db, &namespace, "pr-ui-contributor").await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let owner_session = auth_state
        .create_session(owner.id.clone())
        .await
        .expect("owner session");
    let contributor_session = auth_state
        .create_session(contributor.id.clone())
        .await
        .expect("contributor session");
    let tab = format!("/{namespace}/{repo}/pulls");

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/new"))
            .cookie(session_cookie(&owner_session))
            .set_form(CreateForm {
                base: "main",
                head: "feature/retry",
                title: "Retry",
                body: "Backoff **policy**.",
            })
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::SEE_OTHER);

    // Anonymous visitors read the conversation but cannot comment.
    let (status, anonymous) = body_of(&app, &format!("{tab}/1")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        anonymous.contains("<strong>policy</strong>"),
        "description is rendered Markdown"
    );
    assert!(anonymous.contains("Sign in to comment"));
    assert!(!anonymous.contains("name=\"body\""));

    // A signed-in contributor comments; raw HTML in the comment is shown as text.
    let commented = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/1/comments"))
            .cookie(session_cookie(&contributor_session))
            .set_form(CommentForm {
                body: "Nice <script>alert(1)</script> *work*",
            })
            .to_request(),
    )
    .await;
    assert_eq!(commented.status(), StatusCode::SEE_OTHER);
    let (_, page) = body_of(&app, &format!("{tab}/1")).await;
    assert!(page.contains("<em>work</em>"));
    assert!(
        !page.contains("<script>alert(1)</script>"),
        "raw HTML never reaches the page"
    );
    assert!(page.contains("&lt;script&gt;"));

    // The contributor can edit their comment, but the owner cannot.
    let comment_id = db
        .list_pull_request_comments(
            &db.get_pull_request(&owner_repo_id(&root), 1)
                .await
                .unwrap()
                .unwrap()
                .id,
        )
        .await
        .unwrap()[0]
        .id;
    let edit_uri = format!("{tab}/1/comments/{comment_id}/edit");
    let owner_edit = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&edit_uri)
            .cookie(session_cookie(&owner_session))
            .set_form(CommentForm {
                body: "Owner rewrite",
            })
            .to_request(),
    )
    .await;
    assert_eq!(owner_edit.status(), StatusCode::FORBIDDEN);
    let own_edit = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&edit_uri)
            .cookie(session_cookie(&contributor_session))
            .set_form(CommentForm {
                body: "Edited by author",
            })
            .to_request(),
    )
    .await;
    assert_eq!(own_edit.status(), StatusCode::SEE_OTHER);
    let (_, page) = body_of(&app, &format!("{tab}/1")).await;
    assert!(page.contains("Edited by author"));
    assert!(page.contains("edited"), "an edited comment says so");

    // Reactions toggle: on adds the viewer's reaction, off removes it.
    let reacted = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/1/reactions"))
            .cookie(session_cookie(&contributor_session))
            .set_form(ReactionChange {
                target: "pull",
                content: "heart",
                on: "true",
            })
            .to_request(),
    )
    .await;
    assert_eq!(reacted.status(), StatusCode::SEE_OTHER);
    let signed_in_page = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{tab}/1"))
            .cookie(session_cookie(&contributor_session))
            .to_request(),
    )
    .await;
    let page = String::from_utf8(test::read_body(signed_in_page).await.to_vec()).unwrap();
    assert!(
        page.contains("aria-pressed=\"true\""),
        "the viewer's reaction is pressed"
    );

    // The author closes the request from the page, then reopens it.
    let closed = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/1/state"))
            .cookie(session_cookie(&owner_session))
            .set_form(StateChange { action: "close" })
            .to_request(),
    )
    .await;
    assert_eq!(closed.status(), StatusCode::SEE_OTHER);
    let page = get_as(&app, &format!("{tab}/1"), &owner_session).await;
    assert!(page.contains("data-state=\"closed\""));
    assert!(page.contains("Reopen pull request"));

    // Editing the title and description from the edit form.
    let edited = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/1/edit"))
            .cookie(session_cookie(&owner_session))
            .set_form(EditChange {
                title: "Retry with jitter",
                body: "Adds jitter.",
            })
            .to_request(),
    )
    .await;
    assert_eq!(edited.status(), StatusCode::SEE_OTHER);
    let form = get_as(&app, &format!("{tab}/1/edit"), &owner_session).await;
    assert!(
        form.contains("name=\"title\""),
        "the edit form is offered to the author"
    );
    let anonymous_form = body_of(&app, &format!("{tab}/1/edit")).await.1;
    assert!(
        !anonymous_form.contains("name=\"title\""),
        "the edit form is not offered to visitors"
    );
    let (_, page) = body_of(&app, &format!("{tab}/1")).await;
    assert!(page.contains("Retry with jitter"));

    let _ = std::fs::remove_dir_all(&root);
}

/// The repository identity the test service stored for `pr-team/tooling`.
fn owner_repo_id(root: &str) -> String {
    let path = std::path::Path::new(root).join("pr-team").join("tooling");
    let repo = git2::Repository::open_bare(&path).expect("open bare repo");
    let config = repo.config().expect("config").get_string("twig.id");
    config.expect("repository identity is set once a request exists")
}

#[derive(serde::Serialize)]
struct ReviewChange<'a> {
    event: &'a str,
    body: &'a str,
}

#[derive(serde::Serialize)]
struct ReviewerChange<'a> {
    action: &'a str,
    reviewer: &'a str,
}

#[actix_web::test]
async fn reviews_and_reviewers_are_offered_and_recorded_from_the_page() {
    use super::pull_requests_api::seed_contributor;
    let root = format!("/tmp/test_twig_pr_review_ui_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    let contributor = seed_contributor(&db, &namespace, "pr-ui-reviewer").await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let owner_session = auth_state
        .create_session(owner.id.clone())
        .await
        .expect("owner session");
    let contributor_session = auth_state
        .create_session(contributor.id.clone())
        .await
        .expect("contributor session");
    let tab = format!("/{namespace}/{repo}/pulls");

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/new"))
            .cookie(session_cookie(&owner_session))
            .set_form(CreateForm {
                base: "main",
                head: "feature/retry",
                title: "Retry",
                body: "",
            })
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::SEE_OTHER);

    // The author sees the reviewers panel but no review buttons on their own request.
    let author_page = get_as(&app, &format!("{tab}/1"), &owner_session).await;
    assert!(author_page.contains("Reviewers"));
    assert!(
        !author_page.contains("value=\"APPROVED\""),
        "authors cannot approve their own request"
    );

    // The author asks the contributor to review.
    let requested = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/1/reviewers"))
            .cookie(session_cookie(&owner_session))
            .set_form(ReviewerChange {
                action: "add",
                reviewer: &contributor.username,
            })
            .to_request(),
    )
    .await;
    assert_eq!(requested.status(), StatusCode::SEE_OTHER);
    let page = get_as(&app, &format!("{tab}/1"), &owner_session).await;
    assert!(
        page.contains("pr-ui-reviewer"),
        "the requested reviewer is listed"
    );

    // The contributor sees the review buttons and approves.
    let reviewer_page = get_as(&app, &format!("{tab}/1"), &contributor_session).await;
    assert!(
        reviewer_page.contains("value=\"APPROVED\""),
        "non-authors with push can approve"
    );
    let approved = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/1/reviews"))
            .cookie(session_cookie(&contributor_session))
            .set_form(ReviewChange {
                event: "APPROVED",
                body: "Looks right.",
            })
            .to_request(),
    )
    .await;
    assert_eq!(approved.status(), StatusCode::SEE_OTHER);
    let (_, page) = body_of(&app, &format!("{tab}/1")).await;
    assert!(
        page.contains("approved these changes"),
        "the timeline records the verdict"
    );
    assert!(page.contains("Looks right."));

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn commits_and_files_views_show_the_change_and_place_inline_comments() {
    let root = format!("/tmp/test_twig_pr_files_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    push_branch(&root, "feature/retry", "retry.txt", "retry policy\n");
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let session = auth_state
        .create_session(owner.id.clone())
        .await
        .expect("session");
    let tab = format!("/{namespace}/{repo}/pulls");

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/new"))
            .cookie(session_cookie(&session))
            .set_form(CreateForm {
                base: "main",
                head: "feature/retry",
                title: "Retry",
                body: "",
            })
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::SEE_OTHER);

    let (status, commits) = body_of(&app, &format!("{tab}/1/commits")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(commits.contains("Add retry.txt"), "the commit is listed");
    assert!(
        commits.contains("1 commit<"),
        "the count is singular for one commit"
    );

    let files = get_as(&app, &format!("{tab}/1/files"), &session).await;
    assert!(
        files.contains("1 changed file"),
        "the change summary is shown"
    );
    assert!(files.contains("retry.txt"));
    assert!(files.contains("retry policy"), "the added line is shown");
    assert!(
        files.contains("twig-pr-diff"),
        "the diff is rendered as a table"
    );
    assert!(
        files.contains("name=\"line\""),
        "signed-in pushers get the inline form"
    );

    // An inline comment on added line 1 is stored and shown under that line.
    let inline = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/1/files/comments"))
            .cookie(session_cookie(&session))
            .set_form(InlineChange {
                path: "retry.txt",
                side: "new",
                line: 1,
                body: "Name this constant.",
            })
            .to_request(),
    )
    .await;
    assert_eq!(inline.status(), StatusCode::SEE_OTHER);
    let files = get_as(&app, &format!("{tab}/1/files"), &session).await;
    assert!(
        files.contains("Name this constant."),
        "the inline comment is shown on the page"
    );
    let conversation = get_as(&app, &format!("{tab}/1"), &session).await;
    assert!(
        conversation.contains("Conversation") && conversation.contains("Files changed"),
        "tab bar is shown"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[derive(serde::Serialize)]
struct InlineChange<'a> {
    path: &'a str,
    side: &'a str,
    line: i64,
    body: &'a str,
}

#[derive(serde::Serialize)]
struct MergeChange<'a> {
    method: &'a str,
}

#[actix_web::test]
async fn merge_box_merges_the_request_from_the_page() {
    let root = format!("/tmp/test_twig_pr_merge_ui_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (owner, namespace, repo) = seed_owner_repo(&db, &root).await;
    push_branch(&root, "feature/retry", "retry.txt", "retry");
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let session = auth_state
        .create_session(owner.id.clone())
        .await
        .expect("session");
    let tab = format!("/{namespace}/{repo}/pulls");

    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/new"))
            .cookie(session_cookie(&session))
            .set_form(CreateForm {
                base: "main",
                head: "feature/retry",
                title: "Retry",
                body: "",
            })
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::SEE_OTHER);

    let page = get_as(&app, &format!("{tab}/1"), &session).await;
    assert!(page.contains("This branch has no conflicts with the base branch."));
    assert!(
        page.contains("Merge pull request"),
        "pushers see the merge box"
    );
    assert!(page.contains("Squash and merge"));

    let merged = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{tab}/1/merge"))
            .cookie(session_cookie(&session))
            .set_form(MergeChange { method: "squash" })
            .to_request(),
    )
    .await;
    assert_eq!(merged.status(), StatusCode::SEE_OTHER);

    let page = get_as(&app, &format!("{tab}/1"), &session).await;
    assert!(
        page.contains("data-state=\"merged\""),
        "the request shows as merged"
    );
    assert!(
        !page.contains("Merge pull request"),
        "the merge box is gone once merged"
    );
    let list = get_as(&app, &tab, &session).await;
    assert!(
        list.contains("0 Open"),
        "the merged request leaves the open list"
    );

    let _ = std::fs::remove_dir_all(&root);
}
