use super::issues_api::add_contributor;
use super::pull_requests_api::seed_owner_repo;
use super::support::*;
use super::*;

#[derive(serde::Serialize)]
struct IssueForm<'a> {
    title: &'a str,
    body: &'a str,
}

#[actix_web::test]
async fn issues_tab_is_listed_between_commits_and_pull_requests() {
    let root = format!("/tmp/test_twig_issues_ui_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (_user, namespace, repo) = seed_owner_repo(&db, &root).await;

    let (status, body) = body_of(&app, &format!("/{namespace}/{repo}/issues")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(">Issues<"), "the Issues tab is rendered");
    assert!(body.contains("There are no issues in this view."));

    let (_, tab_bar) = body_of(&app, &format!("/{namespace}/{repo}/commits")).await;
    let issues_at = tab_bar.find(">Issues<").expect("tab bar lists Issues");
    let pulls_at = tab_bar
        .find(">Pull requests<")
        .expect("tab bar lists Pull requests");
    assert!(issues_at < pulls_at, "Issues sits beside Pull requests");
}

#[actix_web::test]
async fn new_issue_form_creates_an_issue_and_shows_it() {
    let root = format!("/tmp/test_twig_issues_form_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let session = auth_state
        .create_session(user.id.clone())
        .await
        .expect("session");
    let cookie = || actix_web::cookie::Cookie::new("session", session.clone());
    let new_page = format!("/{namespace}/{repo}/issues/new");

    // Anonymous visitors get a sign-in notice and no form.
    let (_, anonymous) = body_of(&app, &new_page).await;
    assert!(anonymous.contains("Sign in to open an issue"));
    assert!(!anonymous.contains("name=\"title\""));

    // Signed in, a blank title is refused with the form shown again.
    let refused = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&new_page)
            .cookie(cookie())
            .set_form(IssueForm {
                title: "  ",
                body: "no title",
            })
            .to_request(),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let refused_body = test::read_body(refused).await;
    let refused_html = String::from_utf8(refused_body.to_vec()).unwrap();
    assert!(refused_html.contains("Title is required"));

    // A valid title creates the issue and redirects to its page.
    let created = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&new_page)
            .cookie(cookie())
            .set_form(IssueForm {
                title: "Uploader drops the last chunk",
                body: "On slow links the **final** chunk is lost.\n\n<img src=x onerror=alert(1)>",
            })
            .to_request(),
    )
    .await;
    assert_eq!(created.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        created.headers().get("Location").unwrap(),
        format!("/{namespace}/{repo}/issues/1").as_str()
    );

    let (status, page) = body_of(&app, &format!("/{namespace}/{repo}/issues/1")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("Uploader drops the last chunk"));
    assert!(page.contains("<strong>final</strong>"), "Markdown renders");
    assert!(
        !page.contains("<img src=x"),
        "raw HTML in an issue body is not rendered as markup"
    );
    assert!(
        page.contains("&lt;img src=x"),
        "the raw HTML is shown as escaped text"
    );

    let (_, list) = body_of(&app, &format!("/{namespace}/{repo}/issues")).await;
    assert!(list.contains("Uploader drops the last chunk"));
    assert!(list.contains("#1 opened"));
}

#[actix_web::test]
async fn issue_page_redirects_a_pull_request_number_and_404s_unknown_ones() {
    let root = format!("/tmp/test_twig_issues_redirect_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let repo_id = crate::git::bare::RepoHandle::open(&root, &namespace, &repo)
        .expect("open repo")
        .repo_id()
        .expect("repo id");
    db.create_pull_request(crate::db::pull_requests::NewPullRequest {
        repo_id: &repo_id,
        title: "A change",
        body: "",
        author_user_id: &user.id,
        head_branch: "feature",
        base_branch: "main",
    })
    .await
    .expect("create pull request");

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/{namespace}/{repo}/issues/1"))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers().get("Location").unwrap(),
        format!("/{namespace}/{repo}/pulls/1").as_str()
    );

    let (status, _) = body_of(&app, &format!("/{namespace}/{repo}/issues/42")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// The page as a signed-in viewer sees it, with the controls only they may use.
async fn signed_in_body(
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
            .cookie(actix_web::cookie::Cookie::new(
                "session",
                session.to_string(),
            ))
            .to_request(),
    )
    .await;
    String::from_utf8(test::read_body(response).await.to_vec()).unwrap()
}

#[derive(serde::Serialize)]
struct CommentForm<'a> {
    body: &'a str,
}

#[derive(serde::Serialize)]
struct ActionForm<'a> {
    action: &'a str,
}

#[actix_web::test]
async fn comment_close_and_reopen_from_the_issue_page() {
    let root = format!(
        "/tmp/test_twig_issues_conversation_{}",
        uuid::Uuid::new_v4()
    );
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let session = auth_state
        .create_session(user.id.clone())
        .await
        .expect("session");
    let cookie = || actix_web::cookie::Cookie::new("session", session.clone());
    let page = format!("/{namespace}/{repo}/issues/1");

    let opened = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/{namespace}/{repo}/issues/new"))
            .cookie(cookie())
            .set_form(IssueForm {
                title: "Discuss",
                body: "",
            })
            .to_request(),
    )
    .await;
    assert_eq!(opened.status(), StatusCode::SEE_OTHER);

    // Signed-out visitors cannot comment, and the page says so.
    let (_, anonymous) = body_of(&app, &page).await;
    assert!(anonymous.contains("Sign in to comment."));
    let refused = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{page}/comments"))
            .set_form(CommentForm { body: "anonymous" })
            .to_request(),
    )
    .await;
    assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);

    let commented = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{page}/comments"))
            .cookie(cookie())
            .set_form(CommentForm {
                body: "Thanks for the report",
            })
            .to_request(),
    )
    .await;
    assert_eq!(commented.status(), StatusCode::SEE_OTHER);
    let (_, after_comment) = body_of(&app, &page).await;
    assert!(after_comment.contains("Thanks for the report"));

    let closed = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{page}/state"))
            .cookie(cookie())
            .set_form(ActionForm { action: "close" })
            .to_request(),
    )
    .await;
    assert_eq!(closed.status(), StatusCode::SEE_OTHER);
    let after_close = signed_in_body(&app, &page, &session).await;
    assert!(after_close.contains("data-state=\"closed\""));
    assert!(after_close.contains("Reopen issue"));

    let reopened = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{page}/state"))
            .cookie(cookie())
            .set_form(ActionForm { action: "reopen" })
            .to_request(),
    )
    .await;
    assert_eq!(reopened.status(), StatusCode::SEE_OTHER);
    let after_reopen = signed_in_body(&app, &page, &session).await;
    assert!(after_reopen.contains("data-state=\"open\""));
    assert!(after_reopen.contains("Close issue"));
}

#[actix_web::test]
async fn author_edits_and_deletes_their_own_comment_on_the_page() {
    let root = format!("/tmp/test_twig_issues_edit_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let session = auth_state
        .create_session(user.id.clone())
        .await
        .expect("session");
    let cookie = || actix_web::cookie::Cookie::new("session", session.clone());
    let page = format!("/{namespace}/{repo}/issues/1");

    test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/{namespace}/{repo}/issues/new"))
            .cookie(cookie())
            .set_form(IssueForm {
                title: "Edit me",
                body: "",
            })
            .to_request(),
    )
    .await;
    test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{page}/comments"))
            .cookie(cookie())
            .set_form(CommentForm {
                body: "First draft",
            })
            .to_request(),
    )
    .await;
    let repo_id = crate::git::bare::RepoHandle::open(&root, &namespace, &repo)
        .expect("open repo")
        .repo_id()
        .expect("repo id");
    let issue = db
        .get_issue(&repo_id, 1)
        .await
        .expect("load issue")
        .expect("issue exists");
    let row = db
        .list_issue_comments(&issue.id)
        .await
        .expect("list comments")[0]
        .row_id;

    // The author sees the edit form when asked for it.
    let (_, editing) = {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("{page}?edit={row}"))
                .cookie(cookie())
                .to_request(),
        )
        .await;
        let status = response.status();
        let body = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();
        (status, body)
    };
    assert!(editing.contains(&format!("{page}/comments/{row}/edit")));
    assert!(editing.contains("First draft"));

    let edited = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{page}/comments/{row}/edit"))
            .cookie(cookie())
            .set_form(CommentForm {
                body: "Second draft",
            })
            .to_request(),
    )
    .await;
    assert_eq!(edited.status(), StatusCode::SEE_OTHER);
    let (_, after_edit) = body_of(&app, &page).await;
    assert!(after_edit.contains("Second draft"));
    assert!(after_edit.contains("edited"));

    let deleted = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{page}/comments/{row}/delete"))
            .cookie(cookie())
            .to_request(),
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::SEE_OTHER);
    let (_, after_delete) = body_of(&app, &page).await;
    assert!(!after_delete.contains("Second draft"));
}

#[derive(serde::Serialize)]
struct TextEditForm<'a> {
    title: &'a str,
    body: &'a str,
}

#[derive(serde::Serialize)]
struct AssigneeFormBody<'a> {
    action: &'a str,
    username: &'a str,
}

#[actix_web::test]
async fn author_edits_text_and_members_are_assigned_from_the_page() {
    let root = format!("/tmp/test_twig_issues_assign_ui_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let reviewer = auth::create_user(
        "ui-reviewer".to_string(),
        "ui-reviewer@example.com".to_string(),
        "password123",
    )
    .expect("create reviewer");
    db.create_user(&reviewer).await.expect("persist reviewer");
    add_contributor(&db, &namespace, &reviewer.id).await;

    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let session = auth_state
        .create_session(user.id.clone())
        .await
        .expect("session");
    let cookie = || actix_web::cookie::Cookie::new("session", session.clone());
    let page = format!("/{namespace}/{repo}/issues/1");

    test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/{namespace}/{repo}/issues/new"))
            .cookie(cookie())
            .set_form(IssueForm {
                title: "Old title",
                body: "Old body",
            })
            .to_request(),
    )
    .await;

    let edited = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{page}/edit"))
            .cookie(cookie())
            .set_form(TextEditForm {
                title: "New title",
                body: "New body",
            })
            .to_request(),
    )
    .await;
    assert_eq!(edited.status(), StatusCode::SEE_OTHER);
    let after_edit = signed_in_body(&app, &page, &session).await;
    assert!(after_edit.contains("New title"));
    assert!(after_edit.contains("New body"));
    assert!(!after_edit.contains("Old title"));

    // Anonymous visitors see who is assigned but none of the controls.
    let (_, anonymous) = body_of(&app, &page).await;
    assert!(!anonymous.contains("Assign a member"));

    let assigned = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{page}/assignees"))
            .cookie(cookie())
            .set_form(AssigneeFormBody {
                action: "add",
                username: "ui-reviewer",
            })
            .to_request(),
    )
    .await;
    assert_eq!(assigned.status(), StatusCode::SEE_OTHER);
    let after_assign = signed_in_body(&app, &page, &session).await;
    assert!(after_assign.contains("ui-reviewer"));
    assert!(after_assign.contains("Remove"));

    let removed = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("{page}/assignees"))
            .cookie(cookie())
            .set_form(AssigneeFormBody {
                action: "remove",
                username: "ui-reviewer",
            })
            .to_request(),
    )
    .await;
    assert_eq!(removed.status(), StatusCode::SEE_OTHER);
    let after_remove = signed_in_body(&app, &page, &session).await;
    assert!(!after_remove.contains("ui-reviewer"));
    assert!(after_remove.contains("No one is assigned."));
}

#[actix_web::test]
async fn issue_list_searches_and_filters_by_people() {
    let root = format!("/tmp/test_twig_issues_list_ui_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let (user, namespace, repo) = seed_owner_repo(&db, &root).await;
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let session = auth_state
        .create_session(user.id.clone())
        .await
        .expect("session");
    let cookie = || actix_web::cookie::Cookie::new("session", session.clone());
    for (title, body) in [
        ("Crash on start", "It segfaults on launch."),
        ("Docs", "The README is out of date."),
    ] {
        test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/{namespace}/{repo}/issues/new"))
                .cookie(cookie())
                .set_form(IssueFormWithBody { title, body })
                .to_request(),
        )
        .await;
    }

    let (_, by_text) = body_of(
        &app,
        &format!("/{namespace}/{repo}/issues?state=all&q=segfault"),
    )
    .await;
    assert!(by_text.contains("Crash on start"));
    assert!(
        !by_text.contains(&format!("/{namespace}/{repo}/issues/2\">Docs")),
        "the search narrows the list"
    );

    let (_, by_author) = body_of(
        &app,
        &format!(
            "/{namespace}/{repo}/issues?state=all&author={}",
            user.username
        ),
    )
    .await;
    assert!(by_author.contains("Docs") && by_author.contains("Crash on start"));

    let (_, nobody) = body_of(
        &app,
        &format!("/{namespace}/{repo}/issues?state=all&author=ghost"),
    )
    .await;
    assert!(nobody.contains("There are no issues in this view."));
    assert!(
        nobody.contains("Open") && nobody.contains("Closed"),
        "the state filters stay"
    );

    let (_, tabs) = body_of(
        &app,
        &format!("/{namespace}/{repo}/issues?state=all&q=segfault"),
    )
    .await;
    assert!(
        tabs.contains("issues?state=closed&amp;q=segfault")
            || tabs.contains("issues?state=closed&q=segfault"),
        "tab links keep the search"
    );
}

#[derive(serde::Serialize)]
struct IssueFormWithBody<'a> {
    title: &'a str,
    body: &'a str,
}
