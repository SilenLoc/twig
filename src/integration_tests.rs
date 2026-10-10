#[cfg(test)]
mod tests {
    use crate::{auth, config, db::Database, http::repository::pages as repo_pages};
    use actix_http::Request;
    use actix_web::{App, http::StatusCode, test, web};

    async fn create_test_service() -> impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    > {
        create_test_service_in("/tmp/test_git").await
    }

    // Helper function to create full test app service rooted at a given
    // PROJECT_ROOT, for tests that assert on repositories written to disk.
    async fn create_test_service_in(
        project_root: &str,
    ) -> impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    > {
        create_test_service_with_db_in(project_root).await.0
    }

    async fn create_test_service_with_db_in(
        project_root: &str,
    ) -> (
        impl actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        Database,
    ) {
        // A distinct database per call: nextest runs each test in its own
        // process, and turso takes an exclusive file lock, so a shared path
        // would make concurrent tests fail to open the database.
        let config = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            project_root.to_string(),
            format!("/tmp/test_twig_service_{}.db", uuid::Uuid::new_v4()),
            "secure".to_string(),
            true,
        );

        let db = Database::new(config.db_path());
        let auth_state = web::Data::new(auth::TwigContext::new(db, "secure".to_string()));
        // Initialize database synchronously (we are already inside an async test runtime)
        auth_state.db().init_tables().await.expect("init tables");

        let config_data = web::Data::new(config);
        let test_db = auth_state.db().clone();
        let session_db = test_db.clone();

        let app = test::init_service(
            App::new()
                .app_data(config_data)
                .app_data(auth_state)
                .app_data(web::PayloadConfig::new(1 << 29))
                .wrap(actix_identity::IdentityMiddleware::default())
                .wrap(crate::auth::session_store::middleware(
                    session_db,
                    actix_web::cookie::Key::generate(),
                ))
                .configure(crate::http::routes::configure_routes),
        )
        .await;
        (app, test_db)
    }

    #[actix_web::test]
    async fn test_health_endpoint() {
        let app = create_test_service().await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_invite_page_loads() {
        let app = create_test_service().await;
        let req = test::TestRequest::get().uri("/auth/invite").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("<title>Get Signup Invite · Twig</title>"));
        assert!(body_str.contains("Get Signup Invite"));
        assert!(body_str.contains("API Key"));
    }

    #[actix_web::test]
    async fn test_signup_page_loads() {
        let app = create_test_service().await;
        let req = test::TestRequest::get().uri("/auth/signup").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("<title>Create Account · Twig</title>"));
        assert!(body_str.contains("Create Account"));
        assert!(body_str.contains("Signup Invite"));
    }

    #[actix_web::test]
    async fn test_login_page_loads() {
        let app = create_test_service().await;
        let req = test::TestRequest::get().uri("/auth/login").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("<title>Log In · Twig</title>"));
        assert!(body_str.contains("Log In"));
        assert!(body_str.contains("Username"));
    }

    #[actix_web::test]
    async fn test_namespace_page_loads() {
        let app = create_test_service().await;
        let req = test::TestRequest::get().uri("/auth/namespace").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("<title>Create Namespace · Twig</title>"));
        assert!(body_str.contains("Create Namespace"));
    }

    #[actix_web::test]
    async fn test_generate_invite_with_invalid_api_key() {
        let app = create_test_service().await;
        let req = test::TestRequest::post()
            .uri("/auth/invite")
            .set_form([("api_key", "invalid_key")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Invalid API key"));
    }

    #[actix_web::test]
    async fn test_generate_invite_with_valid_api_key() {
        let app = create_test_service().await;
        let req = test::TestRequest::post()
            .uri("/auth/invite")
            .set_form([("api_key", "secure")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Invite Generated!"));
    }

    #[actix_web::test]
    async fn test_signup_with_invalid_invite() {
        let app = create_test_service().await;
        let req = test::TestRequest::post()
            .uri("/auth/signup")
            .set_form([
                ("invite", "invalid-invite-code"),
                ("username", "uiuser"),
                ("email", "uiuser@example.com"),
                ("password", "password123"),
            ])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Invalid invite"));
    }

    #[actix_web::test]
    async fn test_login_with_wrong_password() {
        let app = create_test_service().await;
        let req = test::TestRequest::post()
            .uri("/auth/login")
            .set_form([("username", "nonexistent"), ("password", "wrongpassword")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Invalid credentials"));
    }

    #[actix_web::test]
    async fn test_create_namespace_without_session() {
        let app = create_test_service().await;
        let req = test::TestRequest::post()
            .uri("/auth/namespace")
            .set_form([("name", "shouldfail_ns")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Not logged in"));
    }

    #[actix_web::test]
    async fn test_tree_hub_and_admin_data_view_are_gated_and_paginated() {
        let db_path = format!("/tmp/test_twig_admin_data_{}.db", uuid::Uuid::new_v4());
        let server = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/tmp/test_git".to_string(),
            db_path.clone(),
            "secure".to_string(),
            false,
        )
        .with_admin_user(Some("dbadmin".to_string()))
        .with_test_user(Some("testadmin".to_string()));
        let db = Database::new(&db_path);
        db.init_tables().await.expect("initialize tables");
        let auth_state = web::Data::new(auth::TwigContext::new(db.clone(), "secure".to_string()));

        let admin = auth::create_user(
            "dbadmin".to_string(),
            "dbadmin@example.com".to_string(),
            "password123",
        )
        .unwrap();
        db.create_user(&admin).await.unwrap();
        let admin_token = auth_state.create_session(admin.id).await.unwrap();
        let other = auth::create_user(
            "ordinary".to_string(),
            "ordinary@example.com".to_string(),
            "password123",
        )
        .unwrap();
        db.create_user(&other).await.unwrap();
        let other_token = auth_state.create_session(other.id).await.unwrap();

        db.conn()
            .await
            .unwrap()
            .execute("CREATE TABLE scroll_fixture (id INTEGER)", ())
            .await
            .unwrap();
        for id in 0..51 {
            db.conn()
                .await
                .unwrap()
                .execute(
                    "INSERT INTO scroll_fixture (id) VALUES (?1)",
                    turso::params![id],
                )
                .await
                .unwrap();
        }

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(server))
                .app_data(auth_state)
                .wrap(actix_identity::IdentityMiddleware::default())
                .wrap(crate::auth::session_store::middleware(
                    db.clone(),
                    actix_web::cookie::Key::generate(),
                ))
                .configure(crate::http::routes::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/tree")
                .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(response.headers().get("Location").unwrap(), "/settings");

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/settings")
                .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("aria-current=\"page\" href=\"/settings\""));

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/tree/repositories")
                .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("aria-current=\"page\" href=\"/tree/repositories\">Repository</a>"));
        assert!(html.contains("You don't have any repositories to delete."));

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/tree/data?table=scroll_fixture")
                .cookie(actix_web::cookie::Cookie::new("session", &other_token))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/tree/data?table=scroll_fixture")
                .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("scroll_fixture"));
        assert!(html.contains(">49</td>"));
        assert!(!html.contains(">50</td>"));
        assert!(html.contains("hx-trigger=\"revealed\""));

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/tree/data/rows?table=scroll_fixture&offset=50")
                .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
                .to_request(),
        )
        .await;
        let body = test::read_body(response).await;
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains(">50</td>"));

        // An offset past the end of the table is an empty page with no load-more
        // link, so the scroll chain terminates instead of repeating rows.
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/tree/data/rows?table=scroll_fixture&offset=100")
                .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.is_empty(), "rows past the end render nothing: {html}");

        // A hand-crafted absurd offset is rejected rather than walked.
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/tree/data/rows?table=scroll_fixture&offset=99999999999")
                .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let _ = std::fs::remove_file(db_path);
    }

    #[actix_web::test]
    async fn test_settings_page_without_login_is_an_htmx_recovery_fragment() {
        let app = create_test_service().await;
        let req = test::TestRequest::get()
            .uri("/settings")
            .insert_header(("HX-Request", "true"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(!body_str.contains("<!DOCTYPE html>"));
        assert!(body_str.contains("href=\"/auth/login\""));
        assert!(body_str.contains("Not logged in"));
    }

    #[actix_web::test]
    async fn test_settings_page_with_expired_session_is_a_full_recovery_page() {
        let app = create_test_service().await;
        let req = test::TestRequest::get()
            .uri("/settings")
            .cookie(actix_web::cookie::Cookie::new("session", "expired"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("<!DOCTYPE html>"));
        assert!(body_str.contains("<title>Account · Twig</title>"));
        assert!(body_str.contains("href=\"/auth/login\""));
        assert!(body_str.contains("Session expired"));
    }

    // Settings tests - from settings.hurl
    #[actix_web::test]
    async fn test_settings_page_without_login() {
        let app = create_test_service().await;
        let req = test::TestRequest::get().uri("/settings").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("<!DOCTYPE html>"));
        assert!(body_str.contains("<title>Account · Twig</title>"));
        assert!(body_str.contains("href=\"/auth/login\""));
        assert!(body_str.contains("Not logged in"));
    }

    #[actix_web::test]
    async fn test_update_email_invalid_format() {
        let app = create_test_service().await;
        let req = test::TestRequest::post()
            .uri("/settings/email")
            .set_form([("email", "invalid-email")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("valid email address"));
    }

    // ========== TICKET REPOSITORY PROVISIONING ==========

    /// Pulls the 64-hex-character invite token out of the success fragment.
    fn extract_invite(html: &str) -> String {
        html.split(|c: char| !c.is_ascii_hexdigit())
            .find(|token| token.len() == 64)
            .expect("invite token in response")
            .to_string()
    }

    /// Signs a user up and logs them in, returning the identity session cookie.
    async fn signup_and_login<S>(app: &S, username: &str) -> actix_web::cookie::Cookie<'static>
    where
        S: actix_web::dev::Service<
                Request,
                Response = actix_web::dev::ServiceResponse,
                Error = actix_web::Error,
            >,
    {
        let req = test::TestRequest::post()
            .uri("/auth/invite")
            .set_form([("api_key", "secure")])
            .to_request();
        let resp = test::call_service(app, req).await;
        assert!(
            resp.status().is_success(),
            "invite generation should succeed"
        );
        let body = test::read_body(resp).await;
        let invite = extract_invite(&String::from_utf8_lossy(&body));

        let email = format!("{username}@example.com");
        let req = test::TestRequest::post()
            .uri("/auth/signup")
            .set_form([
                ("invite", invite.as_str()),
                ("username", username),
                ("email", email.as_str()),
                ("password", "password123"),
            ])
            .to_request();
        let resp = test::call_service(app, req).await;
        assert!(resp.status().is_success(), "signup should succeed");

        let req = test::TestRequest::post()
            .uri("/auth/login")
            .set_form([("username", username), ("password", "password123")])
            .to_request();
        let resp = test::call_service(app, req).await;
        assert!(resp.status().is_success(), "login should succeed");

        resp.response()
            .cookies()
            .find(|c| c.name() == "id")
            .expect("login should set an identity cookie")
            .into_owned()
    }

    async fn create_namespace<S>(app: &S, session: actix_web::cookie::Cookie<'static>, name: &str)
    where
        S: actix_web::dev::Service<
                Request,
                Response = actix_web::dev::ServiceResponse,
                Error = actix_web::Error,
            >,
    {
        let req = test::TestRequest::post()
            .uri("/auth/namespace")
            .cookie(session)
            .set_form([("name", name)])
            .to_request();
        let resp = test::call_service(app, req).await;
        assert!(
            resp.status().is_success(),
            "namespace creation should succeed"
        );
    }

    fn init_text_repo(repo_path: &std::path::Path, path: &str, content: &[u8]) -> git2::Oid {
        init_repo_files(repo_path, &[(path, content, git2::FileMode::Blob.into())])
    }

    fn init_repo_files(repo_path: &std::path::Path, files: &[(&str, &[u8], i32)]) -> git2::Oid {
        std::fs::create_dir_all(repo_path).unwrap();
        let repo = git2::Repository::init_bare(repo_path).unwrap();
        let empty_builder = repo.treebuilder(None).unwrap();
        let empty_tree_id = empty_builder.write().unwrap();
        let empty_tree = repo.find_tree(empty_tree_id).unwrap();
        let mut update = git2::build::TreeUpdateBuilder::new();
        for (path, content, mode) in files {
            let blob = repo.blob(content).unwrap();
            let mode = match *mode {
                0o100_644 => git2::FileMode::Blob,
                0o100_755 => git2::FileMode::BlobExecutable,
                0o120_000 => git2::FileMode::Link,
                _ => panic!("unsupported test file mode: {mode:o}"),
            };
            update.upsert(path, blob, mode);
        }
        let tree_oid = update.create_updated(&repo, &empty_tree).unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let signature = git2::Signature::now("Seed", "seed@example.com").unwrap();
        let commit = repo
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "seed file",
                &tree,
                &[],
            )
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();
        commit
    }

    #[actix_web::test]
    async fn repository_editor_requires_access_and_commits_with_compare_and_swap() {
        let root = format!("/tmp/test_twig_editor_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let session = signup_and_login(&app, "editorowner").await;
        create_namespace(&app, session.clone(), "editspace").await;
        let repo_path = std::path::Path::new(&root).join("editspace/repo");
        let original_head = init_text_repo(&repo_path, "README.md", b"# Before\n");

        let unauthenticated = test::TestRequest::get()
            .uri("/editspace/repo/edit/README.md")
            .to_request();
        let response = test::call_service(&app, unauthenticated).await;
        assert_eq!(response.status(), StatusCode::FOUND);

        let page = test::TestRequest::get()
            .uri("/editspace/repo/edit/README.md")
            .cookie(session.clone())
            .to_request();
        let response = test::call_service(&app, page).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("# Before"));
        assert!(html.contains(&original_head.to_string()));
        assert!(html.contains("Commit changes"));

        let unauthorized_save = test::TestRequest::post()
            .uri("/editspace/repo/edit/README.md")
            .set_json(serde_json::json!({
                "content": "# Unauthorized\n",
                "message": "Should not write",
                "expected_head": original_head.to_string()
            }))
            .to_request();
        let response = test::call_service(&app, unauthorized_save).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let save = test::TestRequest::post()
            .uri("/editspace/repo/edit/README.md")
            .cookie(session.clone())
            .set_json(serde_json::json!({
                "content": "# After\n",
                "message": "Update README",
                "expected_head": original_head.to_string()
            }))
            .to_request();
        let response = test::call_service(&app, save).await;
        assert_eq!(response.status(), StatusCode::OK);
        let result: serde_json::Value = test::read_body_json(response).await;
        let location = result["location"].as_str().unwrap();
        assert!(location.contains("committed="));

        let saved_page = test::TestRequest::get()
            .uri(location)
            .cookie(session.clone())
            .to_request();
        let response = test::call_service(&app, saved_page).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        assert!(String::from_utf8_lossy(&body).contains("Changes committed successfully."));

        let handle = crate::git::bare::RepoHandle::open(&root, "editspace", "repo").unwrap();
        assert_eq!(
            handle.read_blob_bytes("README.md").unwrap().as_deref(),
            Some(&b"# After\n"[..])
        );
        let commits = handle.get_commits(2).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].author(), "editorowner");

        let stale_save = test::TestRequest::post()
            .uri("/editspace/repo/edit/README.md")
            .cookie(session)
            .set_json(serde_json::json!({
                "content": "# Stale\n",
                "message": "Stale README",
                "expected_head": original_head.to_string()
            }))
            .to_request();
        let response = test::call_service(&app, stale_save).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            handle.read_blob_bytes("README.md").unwrap().as_deref(),
            Some(&b"# After\n"[..])
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn repository_editor_denies_authenticated_users_without_namespace_access() {
        let root = format!("/tmp/test_twig_editor_access_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let owner_session = signup_and_login(&app, "editaccessowner").await;
        create_namespace(&app, owner_session, "ownededitspace").await;
        let repo_path = std::path::Path::new(&root).join("ownededitspace/repo");
        init_text_repo(&repo_path, "README.md", b"# Private to namespace\n");
        let other_session = signup_and_login(&app, "editaccessother").await;

        let request = test::TestRequest::get()
            .uri("/ownededitspace/repo/edit/README.md")
            .cookie(other_session)
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn repository_editor_edits_non_markdown_files_as_exact_source_text() {
        let root = format!("/tmp/test_twig_editor_source_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let session = signup_and_login(&app, "editorsourceowner").await;
        create_namespace(&app, session.clone(), "editsourcespace").await;
        let repo_path = std::path::Path::new(&root).join("editsourcespace/repo");
        let head = init_text_repo(&repo_path, "src/main.rs", b"fn main() {}\n");

        let page = test::TestRequest::get()
            .uri("/editsourcespace/repo/edit/src/main.rs")
            .cookie(session.clone())
            .to_request();
        let response = test::call_service(&app, page).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Source editor"));
        assert!(html.contains("fn main() {}"));
        assert!(!html.contains("quill-"), "source files must not load Quill");

        let save = test::TestRequest::post()
            .uri("/editsourcespace/repo/edit/src/main.rs")
            .cookie(session)
            .set_json(serde_json::json!({
                "content": "fn main() { println!(\"edited\"); }\n",
                "message": "Edit Rust source",
                "expected_head": head.to_string()
            }))
            .to_request();
        let response = test::call_service(&app, save).await;
        assert_eq!(response.status(), StatusCode::OK);
        let handle = crate::git::bare::RepoHandle::open(&root, "editsourcespace", "repo").unwrap();
        assert_eq!(
            handle.read_blob_bytes("src/main.rs").unwrap().as_deref(),
            Some(&b"fn main() { println!(\"edited\"); }\n"[..])
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn repository_editor_saves_conflicted_draft_as_prefixed_file_on_latest_head() {
        let root = format!(
            "/tmp/test_twig_editor_conflict_copy_{}",
            uuid::Uuid::new_v4()
        );
        let app = create_test_service_in(&root).await;
        let session = signup_and_login(&app, "editorcopyowner").await;
        create_namespace(&app, session.clone(), "editcopyspace").await;
        let repo_path = std::path::Path::new(&root).join("editcopyspace/repo");
        let original_head = init_text_repo(&repo_path, "README.md", b"# Original\n");
        let handle = crate::git::bare::RepoHandle::open(&root, "editcopyspace", "repo").unwrap();

        let concurrent = handle
            .commit_file(
                "README.md",
                b"# Concurrent edit\n",
                original_head,
                "Other editor",
                "other@example.com",
                "Concurrent update",
            )
            .unwrap();
        let crate::git::bare::CommitFileOutcome::Committed(concurrent_head) = concurrent else {
            panic!("the concurrent update should commit");
        };

        let stale_save = test::TestRequest::post()
            .uri("/editcopyspace/repo/edit/README.md")
            .cookie(session.clone())
            .set_json(serde_json::json!({
                "content": "# My preserved draft\n",
                "message": "My draft",
                "expected_head": original_head.to_string()
            }))
            .to_request();
        let response = test::call_service(&app, stale_save).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = test::read_body(response).await;
        assert!(String::from_utf8_lossy(&body).contains("save it as a new file"));

        let save_copy = test::TestRequest::post()
            .uri("/editcopyspace/repo/edit/README.md")
            .cookie(session.clone())
            .set_json(serde_json::json!({
                "content": "# My preserved draft\n",
                "message": "My draft",
                "expected_head": original_head.to_string(),
                "save_conflict_copy": true
            }))
            .to_request();
        let response = test::call_service(&app, save_copy).await;
        assert_eq!(response.status(), StatusCode::OK);
        let result: serde_json::Value = test::read_body_json(response).await;
        let location = result["location"].as_str().unwrap();
        assert!(location.contains("?committed="));
        assert!(location.contains("&conflict_copy=true"));
        let (copy_path, _) = location
            .strip_prefix("/editcopyspace/repo/content/")
            .unwrap()
            .split_once('?')
            .unwrap();
        let mut name_parts = copy_path.splitn(4, '-');
        assert_eq!(name_parts.next(), Some("editorcopyowner"));
        let date = name_parts.next().unwrap();
        let time = name_parts.next().unwrap();
        assert_eq!(date.len(), 8);
        assert!(date.chars().all(|character| character.is_ascii_digit()));
        assert_eq!(time.len(), 6);
        assert!(time.chars().all(|character| character.is_ascii_digit()));
        assert_eq!(name_parts.next(), Some("README.md"));

        assert_eq!(
            handle.read_blob_bytes("README.md").unwrap().as_deref(),
            Some(&b"# Concurrent edit\n"[..]),
            "the other user's update must remain untouched"
        );
        assert_eq!(
            handle.read_blob_bytes(copy_path).unwrap().as_deref(),
            Some(&b"# My preserved draft\n"[..])
        );
        let repo = git2::Repository::open(&repo_path).unwrap();
        let copy_commit = repo
            .find_commit(handle.head_oid().unwrap().unwrap())
            .unwrap();
        assert_eq!(copy_commit.parent_id(0).unwrap(), concurrent_head);
        assert_eq!(copy_commit.author().name().unwrap(), "editorcopyowner");
        drop(copy_commit);
        drop(repo);

        let saved_page = test::TestRequest::get()
            .uri(location)
            .cookie(session.clone())
            .to_request();
        let response = test::call_service(&app, saved_page).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        assert!(String::from_utf8_lossy(&body).contains("Draft saved as a conflict copy."));

        // A later change may delete the file being edited entirely. The
        // already-open editor must still be able to persist its draft copy.
        let repo = git2::Repository::open(&repo_path).unwrap();
        let tip = repo.head().unwrap().peel_to_commit().unwrap();
        let tip_id = tip.id();
        let tree_id = {
            let tree = tip.tree().unwrap();
            let mut update = git2::build::TreeUpdateBuilder::new();
            update.remove("README.md");
            update.create_updated(&repo, &tree).unwrap()
        };
        let new_tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Other editor", "other@example.com").unwrap();
        let deleted_head = repo
            .commit(
                None,
                &signature,
                &signature,
                "Delete README",
                &new_tree,
                &[&tip],
            )
            .unwrap();
        repo.reference_matching(
            "refs/heads/main",
            deleted_head,
            true,
            tip_id,
            "Delete README",
        )
        .unwrap();
        drop(new_tree);
        drop(tip);
        drop(repo);

        let after_delete = test::TestRequest::post()
            .uri("/editcopyspace/repo/edit/README.md")
            .cookie(session.clone())
            .set_json(serde_json::json!({
                "content": "# Draft after deletion\n",
                "message": "Keep my deleted-file draft",
                "expected_head": concurrent_head.to_string(),
                "save_conflict_copy": true
            }))
            .to_request();
        let response = test::call_service(&app, after_delete).await;
        assert_eq!(response.status(), StatusCode::OK);
        let result: serde_json::Value = test::read_body_json(response).await;
        let location = result["location"].as_str().unwrap();
        let (second_copy_path, _) = location
            .strip_prefix("/editcopyspace/repo/content/")
            .unwrap()
            .split_once('?')
            .unwrap();
        assert_eq!(handle.read_blob_bytes("README.md").unwrap(), None);
        assert_eq!(
            handle.read_blob_bytes(second_copy_path).unwrap().as_deref(),
            Some(&b"# Draft after deletion\n"[..])
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn repository_editor_refuses_ignored_binary_invalid_and_non_regular_files() {
        let root = format!("/tmp/test_twig_editor_files_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let session = signup_and_login(&app, "editorfileowner").await;
        create_namespace(&app, session.clone(), "editfilespace").await;
        let large = vec![b'x'; 1024 * 1024 + 1];
        let repo_path = std::path::Path::new(&root).join("editfilespace/repo");
        init_repo_files(
            &repo_path,
            &[
                (
                    ".twig.toml",
                    b"ignore_for_view = [\"hidden.txt\"]\n",
                    0o100_644,
                ),
                ("README.md", b"# Allowed\n", 0o100_644),
                ("hidden.txt", b"secret\n", 0o100_644),
                ("binary.bin", b"\0\x01\x02", 0o100_644),
                ("invalid.txt", b"\xff", 0o100_644),
                ("large.txt", &large, 0o100_644),
                ("link.txt", b"README.md", 0o120_000),
            ],
        );

        for (path, expected) in [
            ("hidden.txt", StatusCode::NOT_FOUND),
            ("missing.txt", StatusCode::NOT_FOUND),
            ("binary.bin", StatusCode::UNSUPPORTED_MEDIA_TYPE),
            ("invalid.txt", StatusCode::UNSUPPORTED_MEDIA_TYPE),
            ("large.txt", StatusCode::PAYLOAD_TOO_LARGE),
            ("link.txt", StatusCode::UNSUPPORTED_MEDIA_TYPE),
        ] {
            let request = test::TestRequest::get()
                .uri(&format!("/editfilespace/repo/edit/{path}"))
                .cookie(session.clone())
                .to_request();
            let response = test::call_service(&app, request).await;
            assert_eq!(response.status(), expected, "{path}");
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn repository_editor_rejects_empty_or_unchanged_commits_without_history_changes() {
        let root = format!("/tmp/test_twig_editor_validation_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let session = signup_and_login(&app, "editorvalidator").await;
        create_namespace(&app, session.clone(), "editvalidspace").await;
        let repo_path = std::path::Path::new(&root).join("editvalidspace/repo");
        let head = init_text_repo(&repo_path, "README.md", b"# Stable\n");

        for (message, expected_status) in [
            ("", StatusCode::BAD_REQUEST),
            (&"x".repeat(201), StatusCode::BAD_REQUEST),
        ] {
            let request = test::TestRequest::post()
                .uri("/editvalidspace/repo/edit/README.md")
                .cookie(session.clone())
                .set_json(serde_json::json!({
                    "content": "# Changed\n",
                    "message": message,
                    "expected_head": head.to_string()
                }))
                .to_request();
            let response = test::call_service(&app, request).await;
            assert_eq!(response.status(), expected_status);
        }

        let unchanged = test::TestRequest::post()
            .uri("/editvalidspace/repo/edit/README.md")
            .cookie(session.clone())
            .set_json(serde_json::json!({
                "content": "# Stable\n",
                "message": "No change",
                "expected_head": head.to_string()
            }))
            .to_request();
        let response = test::call_service(&app, unchanged).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let oversized = test::TestRequest::post()
            .uri("/editvalidspace/repo/edit/README.md")
            .cookie(session)
            .set_json(serde_json::json!({
                "content": "x".repeat(1024 * 1024 + 1),
                "message": "Too large",
                "expected_head": head.to_string()
            }))
            .to_request();
        let response = test::call_service(&app, oversized).await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let handle = crate::git::bare::RepoHandle::open(&root, "editvalidspace", "repo").unwrap();
        assert_eq!(handle.head_oid().unwrap(), Some(head));
        assert_eq!(handle.get_commits(5).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_move_repo_between_owned_namespaces() {
        let root = format!("/tmp/test_twig_move_repo_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let session = signup_and_login(&app, "moveowner").await;
        create_namespace(&app, session.clone(), "source").await;
        create_namespace(&app, session.clone(), "target").await;

        let source = std::path::Path::new(&root).join("source/repo");
        std::fs::create_dir_all(&source).unwrap();
        crate::git::repo::bare_init(&source, "main", "Test", "test@example.com").unwrap();

        let req = test::TestRequest::post()
            .uri("/settings/move-repo")
            .cookie(session)
            .set_form([
                ("source_namespace", "source"),
                ("repo_name", "repo"),
                ("target_namespace", "target"),
            ])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(!source.exists());
        let destination = std::path::Path::new(&root).join("target/repo");
        assert!(git2::Repository::open(destination).is_ok());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_move_repo_rejects_namespace_owned_by_another_user() {
        let root = format!("/tmp/test_twig_move_denied_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let owner_session = signup_and_login(&app, "sourceowner").await;
        create_namespace(&app, owner_session.clone(), "ownedsource").await;
        let other_session = signup_and_login(&app, "targetowner").await;
        create_namespace(&app, other_session, "foreigntarget").await;

        let source = std::path::Path::new(&root).join("ownedsource/repo");
        std::fs::create_dir_all(&source).unwrap();
        crate::git::repo::bare_init(&source, "main", "Test", "test@example.com").unwrap();

        let req = test::TestRequest::post()
            .uri("/settings/move-repo")
            .cookie(owner_session)
            .set_form([
                ("source_namespace", "ownedsource"),
                ("repo_name", "repo"),
                ("target_namespace", "foreigntarget"),
            ])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert!(source.exists());
        assert!(
            !std::path::Path::new(&root)
                .join("foreigntarget/repo")
                .exists()
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_rename_repo_within_owned_namespace() {
        let root = format!("/tmp/test_twig_rename_repo_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let session = signup_and_login(&app, "renameowner").await;
        create_namespace(&app, session.clone(), "acme").await;

        let source = std::path::Path::new(&root).join("acme/old-name");
        std::fs::create_dir_all(&source).unwrap();
        crate::git::repo::bare_init(&source, "main", "Test", "test@example.com").unwrap();

        let req = test::TestRequest::post()
            .uri("/settings/rename-repo")
            .cookie(session)
            .set_form([
                ("namespace", "acme"),
                ("repo_name", "old-name"),
                ("new_name", "new-name"),
            ])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(!source.exists());
        let destination = std::path::Path::new(&root).join("acme/new-name");
        assert!(git2::Repository::open(destination).is_ok());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_rename_repo_rejects_foreign_namespace_and_existing_name() {
        let root = format!("/tmp/test_twig_rename_denied_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let owner_session = signup_and_login(&app, "renameowner").await;
        create_namespace(&app, owner_session.clone(), "owned").await;
        let other_session = signup_and_login(&app, "renameother").await;
        create_namespace(&app, other_session, "foreign").await;

        let repo = std::path::Path::new(&root).join("owned/repo");
        std::fs::create_dir_all(&repo).unwrap();
        crate::git::repo::bare_init(&repo, "main", "Test", "test@example.com").unwrap();
        let taken = std::path::Path::new(&root).join("owned/taken");
        std::fs::create_dir_all(&taken).unwrap();
        crate::git::repo::bare_init(&taken, "main", "Test", "test@example.com").unwrap();

        // Another user's namespace is off limits.
        let req = test::TestRequest::post()
            .uri("/settings/rename-repo")
            .cookie(owner_session.clone())
            .set_form([
                ("namespace", "foreign"),
                ("repo_name", "repo"),
                ("new_name", "renamed"),
            ])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // Renaming onto an existing repository name conflicts.
        let req = test::TestRequest::post()
            .uri("/settings/rename-repo")
            .cookie(owner_session)
            .set_form([
                ("namespace", "owned"),
                ("repo_name", "repo"),
                ("new_name", "taken"),
            ])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        assert!(repo.exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_rename_namespace_moves_directory_and_record() {
        let root = format!("/tmp/test_twig_rename_ns_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let session = signup_and_login(&app, "nsrenameowner").await;
        create_namespace(&app, session.clone(), "oldns").await;

        let source = std::path::Path::new(&root).join("oldns");
        let repo = source.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        crate::git::repo::bare_init(&repo, "main", "Test", "test@example.com").unwrap();

        let req = test::TestRequest::post()
            .uri("/settings/rename-namespace")
            .cookie(session.clone())
            .set_form([("namespace", "oldns"), ("new_name", "newns")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(!source.exists());
        assert!(git2::Repository::open(std::path::Path::new(&root).join("newns/repo")).is_ok());

        // The database record moved too: renaming again from the new name works.
        let req = test::TestRequest::post()
            .uri("/settings/rename-namespace")
            .cookie(session)
            .set_form([("namespace", "newns"), ("new_name", "final")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(std::path::Path::new(&root).join("final/repo").is_dir());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_rename_namespace_rejects_foreign_owner_and_existing_name() {
        let root = format!("/tmp/test_twig_rename_ns_denied_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let owner_session = signup_and_login(&app, "nsrenameowner").await;
        create_namespace(&app, owner_session.clone(), "ownedns").await;
        create_namespace(&app, owner_session.clone(), "taken").await;
        let other_session = signup_and_login(&app, "nsrenameother").await;
        create_namespace(&app, other_session, "foreignns").await;

        // Another user's namespace is off limits.
        let req = test::TestRequest::post()
            .uri("/settings/rename-namespace")
            .cookie(owner_session.clone())
            .set_form([("namespace", "foreignns"), ("new_name", "renamedns")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert!(std::path::Path::new(&root).join("foreignns").is_dir());

        // Renaming onto an existing namespace name conflicts.
        let req = test::TestRequest::post()
            .uri("/settings/rename-namespace")
            .cookie(owner_session)
            .set_form([("namespace", "ownedns"), ("new_name", "taken")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        assert!(std::path::Path::new(&root).join("ownedns").is_dir());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_cannot_create_namespace_with_reserved_prefix() {
        let root = format!("/tmp/test_twig_ns_reserved_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let session = signup_and_login(&app, "underscoreuser").await;

        let req = test::TestRequest::post()
            .uri("/auth/namespace")
            .cookie(session)
            .set_form([("name", "_admin")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(
            resp.status().is_client_error(),
            "namespace names starting with '_' must be refused"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    // ========== CONTENT TAB TRAVERSAL SECURITY TESTS ==========

    /// Marker embedded in a secret file placed OUTSIDE the project root.
    /// No response may ever contain this string.
    const SECRET_MARKER: &str = "TOP_SECRET_TRAVERSAL_MARKER_9f3a";

    struct TraversalFixture {
        root: std::path::PathBuf,
        outside: std::path::PathBuf,
        db_path: String,
    }

    impl Drop for TraversalFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
            let _ = std::fs::remove_dir_all(&self.outside);
            let _ = std::fs::remove_file(&self.db_path);
        }
    }

    /// Creates a project root containing `public/repo` (a real bare repo with
    /// an initial commit) plus a secret directory OUTSIDE the project root.
    fn setup_traversal_fixture() -> TraversalFixture {
        use std::path::PathBuf;

        let id = uuid::Uuid::new_v4().to_string();
        let root = PathBuf::from(format!("/tmp/twig_trav_root_{id}"));
        let repo_dir = root.join("public").join("repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        crate::git::repo::bare_init(&repo_dir, "main", "Test", "t@example.com")
            .expect("bare_init failed");

        // Secret file outside the project root, reachable only via traversal
        let outside = PathBuf::from(format!("/tmp/twig_trav_secret_{id}"));
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(
            outside.join("secret.txt"),
            format!("{SECRET_MARKER}\nroot:x:0:0:root\n"),
        )
        .unwrap();

        TraversalFixture {
            root,
            outside,
            db_path: format!("/tmp/twig_trav_{id}.db"),
        }
    }

    async fn create_traversal_service(
        fixture: &TraversalFixture,
    ) -> impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    > {
        let config = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "error".to_string(),
            fixture.root.to_string_lossy().into_owned(),
            fixture.db_path.clone(),
            "secure".to_string(),
            true,
        );

        let db = Database::new(config.db_path());
        let auth_state = web::Data::new(auth::TwigContext::new(db, "secure".to_string()));
        auth_state.db().init_tables().await.expect("init tables");

        test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(auth_state)
                .service(repo_pages::handler)
                .service(repo_pages::tab_handler)
                .service(repo_pages::content_tab_handler)
                .service(repo_pages::commits_tab_handler)
                .service(repo_pages::content_handler)
                .service(repo_pages::markdown_handler),
        )
        .await
    }

    async fn body_of(
        app: &impl actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        uri: &str,
    ) -> (StatusCode, String) {
        let req = test::TestRequest::get().uri(uri).to_request();
        let resp = test::call_service(app, req).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[actix_web::test]
    async fn test_content_tab_lists_repo_files() {
        let fixture = setup_traversal_fixture();
        let app = create_traversal_service(&fixture).await;

        let (status, body) = body_of(&app, "/public/repo/content").await;
        assert_eq!(status, StatusCode::OK);
        // Positive control: repo's own file is listed
        assert!(
            body.contains(".twig.toml"),
            "expected .twig.toml in listing: {body}"
        );
        assert!(!body.contains(SECRET_MARKER));
    }

    #[actix_web::test]
    async fn test_content_file_view_serves_repo_files_only() {
        let fixture = setup_traversal_fixture();
        let app = create_traversal_service(&fixture).await;

        let (status, body) = body_of(&app, "/public/repo/content/.twig.toml").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Created with Twig"), "{body}");
        assert!(
            body.contains(r#"<nav class="twig-crumbs twig-crumbs--page" aria-label="Breadcrumb">"#),
            "direct file views must retain the repository breadcrumb: {body}"
        );
        assert!(
            body.contains(r#"<h1 class="twig-crumb-current" aria-current="page">repo</h1>"#),
            "direct file views must identify the repository in the breadcrumb: {body}"
        );
        assert!(!body.contains(SECRET_MARKER));
    }

    #[actix_web::test]
    async fn test_content_rejects_dotdot_segments() {
        let fixture = setup_traversal_fixture();
        let app = create_traversal_service(&fixture).await;

        for uri in [
            "/public/repo/content/../../../../../etc/passwd",
            "/public/repo/content/src/../../../etc/passwd",
            "/public/repo/content/..%2f..%2f..%2fetc%2fpasswd",
            "/public/repo/content/%2e%2e/%2e%2e/secret.txt",
            "/public/repo/content/..%5c..%5csecret.txt",
            "/public/repo/content/%2fetc%2fpasswd",
            "/public/repo/content/..",
            "/public/repo/content/a//b",
        ] {
            let (_status, body) = body_of(&app, uri).await;
            assert!(
                !body.contains(SECRET_MARKER),
                "SECRET LEAKED via {uri}: {body}"
            );
            assert!(!body.contains("root:x:0:0"), "passwd leaked via {uri}");
            assert!(
                body.contains("Path not found"),
                "expected rejection page for {uri}: {body}"
            );
        }
    }

    #[actix_web::test]
    async fn test_content_rejects_namespace_and_repo_traversal() {
        let fixture = setup_traversal_fixture();
        let app = create_traversal_service(&fixture).await;

        for uri in [
            // namespace tries to climb out of the project root
            "/..%2f..%2ftmp%2fnonsense/repo/content",
            "/public%2f..%2f..%2fsecret/repo/content",
            // repo name tries to climb out of the namespace dir
            "/public/..%2f..%2fsecret/content",
            "/public/../secret/content",
        ] {
            let (_status, body) = body_of(&app, uri).await;
            assert!(
                !body.contains(SECRET_MARKER),
                "SECRET LEAKED via {uri}: {body}"
            );
            assert!(!body.contains("root:x:0:0"), "passwd leaked via {uri}");
        }
    }

    #[actix_web::test]
    async fn test_content_rejects_nul_byte_paths() {
        let fixture = setup_traversal_fixture();
        let app = create_traversal_service(&fixture).await;

        for uri in [
            "/public/repo/content/.twig.toml%00",
            "/public/repo/content/%00.twig.toml",
            "/public/repo/content/a%00b",
        ] {
            let (_status, body) = body_of(&app, uri).await;
            assert!(
                !body.contains("Created with Twig") || uri.contains(".twig"),
                "unexpected content served for {uri}",
            );
            assert!(!body.contains(SECRET_MARKER), "leak via {uri}");
        }
    }

    #[actix_web::test]
    async fn test_markdown_view_rejects_traversal() {
        let fixture = setup_traversal_fixture();
        let app = create_traversal_service(&fixture).await;

        for uri in [
            "/public/repo/md/../../secret.txt",
            "/public/repo/md/..%2f..%2fsecret.txt",
            "/public/repo/md/%2e%2e%2f%2e%2e%2fsecret.txt",
        ] {
            let (_status, body) = body_of(&app, uri).await;
            assert!(
                !body.contains(SECRET_MARKER),
                "SECRET LEAKED via {uri}: {body}"
            );
        }
    }

    #[actix_web::test]
    async fn test_content_cannot_reach_sibling_namespace_via_dots() {
        let fixture = setup_traversal_fixture();
        // A sibling namespace with its own repo inside the same root
        let sibling = fixture.root.join("other").join("vault");
        std::fs::create_dir_all(&sibling).unwrap();
        crate::git::repo::bare_init(&sibling, "main", "Test", "t@example.com").unwrap();

        let app = create_traversal_service(&fixture).await;

        // Normal access to a public path still works...
        let (status, body) = body_of(&app, "/public/repo/content/.twig.toml").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Created with Twig"));

        // ...but dot-segment navigation must not escape into other namespaces
        let (_status, body) =
            body_of(&app, "/public/repo/content/../../other/vault/.twig.toml").await;
        assert!(
            !body.contains("Created with Twig"),
            "escaped into sibling namespace: {body}"
        );
        assert!(body.contains("Path not found"), "{body}");
    }

    // ========== PRIVATE REPOSITORY TESTS ==========

    fn init_private_repo(repo_path: &std::path::Path) {
        std::fs::create_dir_all(repo_path).unwrap();
        crate::git::repo::bare_init(repo_path, "main", "Test", "test@example.com").unwrap();
        let sh = xshell::Shell::new().unwrap();
        let _p = sh.push_dir(repo_path);
        let blob = xshell::cmd!(sh, "git hash-object -w --stdin")
            .stdin("private = true\n")
            .read()
            .unwrap();
        let tree = xshell::cmd!(sh, "git mktree")
            .stdin(format!("100644 blob {blob}\t.twig.toml\n"))
            .read()
            .unwrap();
        let commit = xshell::cmd!(sh, "git commit-tree {tree} -m 'make private'")
            .read()
            .unwrap();
        xshell::cmd!(sh, "git update-ref refs/heads/main {commit}")
            .run()
            .unwrap();
    }

    /// Builds a bare repository whose tree contains the given `.twig.toml` and,
    /// when non-empty, a `paper/` directory holding the supplied pages.
    fn init_repo_with_paper(repo_path: &std::path::Path, twig: &str, paper_pages: &[(&str, &str)]) {
        use std::fmt::Write as _;

        std::fs::create_dir_all(repo_path).unwrap();
        crate::git::repo::bare_init(repo_path, "main", "Test", "test@example.com").unwrap();
        let sh = xshell::Shell::new().unwrap();
        let _p = sh.push_dir(repo_path);

        let hash = |content: &str| -> String {
            xshell::cmd!(sh, "git hash-object -w --stdin")
                .stdin(content)
                .read()
                .unwrap()
        };

        let twig_blob = hash(twig);
        let mut entries = format!("100644 blob {twig_blob}\t.twig.toml\n");

        if !paper_pages.is_empty() {
            let mut paper_entries = String::new();
            for (name, content) in paper_pages {
                let blob = hash(content);
                writeln!(paper_entries, "100644 blob {blob}\t{name}").unwrap();
            }
            let paper_tree = xshell::cmd!(sh, "git mktree")
                .stdin(paper_entries)
                .read()
                .unwrap();
            writeln!(entries, "040000 tree {paper_tree}\tpaper").unwrap();
        }

        let tree = xshell::cmd!(sh, "git mktree")
            .stdin(entries)
            .read()
            .unwrap();
        let commit = xshell::cmd!(sh, "git commit-tree {tree} -m 'paper'")
            .read()
            .unwrap();
        xshell::cmd!(sh, "git update-ref refs/heads/main {commit}")
            .run()
            .unwrap();
    }

    /// Builds a bare repository with the given `.twig.toml` plus arbitrary
    /// extra files. Nested paths build their subtrees bottom-up, since
    /// `mktree` rejects slashes.
    fn init_repo_with_files(repo_path: &std::path::Path, twig: &str, files: &[(&str, &str)]) {
        use std::fmt::Write as _;
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        // Builds the tree holding `files`, whose paths all live under one
        // directory: blobs straight into `mktree`, deeper paths into a
        // recursive subtree named by their first path segment.
        fn tree(
            files: &[(&str, &str)],
            blob: &impl Fn(&str) -> String,
            mktree: &impl Fn(&str) -> String,
        ) -> String {
            let mut entries = String::new();
            let mut nested: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();
            for (name, content) in files {
                match name.split_once('/') {
                    None => {
                        let hash = blob(content);
                        let _ = writeln!(entries, "100644 blob {hash}\t{name}");
                    }
                    Some((dir, rest)) => match nested.iter_mut().find(|(d, _)| *d == dir) {
                        Some((_, group)) => group.push((rest, *content)),
                        None => nested.push((dir, vec![(rest, *content)])),
                    },
                }
            }
            for (dir, group) in &nested {
                let subtree = tree(group, blob, mktree);
                let _ = writeln!(entries, "040000 tree {subtree}\t{dir}");
            }
            mktree(&entries)
        }

        std::fs::create_dir_all(repo_path).unwrap();
        crate::git::repo::bare_init(repo_path, "main", "Test", "test@example.com").unwrap();
        let sh = xshell::Shell::new().unwrap();
        let _p = sh.push_dir(repo_path);

        let git_pipe = |args: &[&str], input: &str| -> String {
            let mut child = Command::new("git")
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .current_dir(repo_path)
                .spawn()
                .unwrap();
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(input.as_bytes())
                .unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "{args:?}: {out:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };

        let blob = |content: &str| git_pipe(&["hash-object", "-w", "--stdin"], content);
        let mktree = |input: &str| git_pipe(&["mktree"], input);

        let mut all: Vec<(&str, &str)> = vec![(".twig.toml", twig)];
        all.extend_from_slice(files);
        let root = tree(&all, &blob, &mktree);

        let commit = xshell::cmd!(sh, "git commit-tree {root} -m 'files'")
            .read()
            .unwrap();
        xshell::cmd!(sh, "git update-ref refs/heads/main {commit}")
            .run()
            .unwrap();
    }

    async fn get_body(
        app: &impl actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        uri: &str,
    ) -> String {
        let req = test::TestRequest::get().uri(uri).to_request();
        let resp = test::call_service(app, req).await;
        String::from_utf8(test::read_body(resp).await.to_vec()).unwrap()
    }

    #[actix_web::test]
    async fn test_paper_tab_renders_lazily_loaded_pages() {
        let root = format!("/tmp/test_twig_paper_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        init_repo_with_paper(
            &std::path::Path::new(&root).join("pub/book"),
            "[paper]\ndir = \"paper\"\n",
            &[("01.md", "# First\n"), ("02.md", "# Second\n")],
        );

        let body = get_body(&app, "/pub/book").await;
        assert!(body.contains(">Paper<"), "paper tab missing: {body}");
        assert!(
            body.contains("href=\"/pub/book/paper\""),
            "the tab points at the paper view: {body}"
        );

        let body = get_body(&app, "/pub/book/paper").await;
        assert!(body.contains("paper-container"), "{body}");
        assert!(body.contains("data-twig-paper-font=\"sans\""), "{body}");
        assert!(
            body.contains(r#"hx-trigger="revealed""#),
            "pages must lazy load on scroll: {body}"
        );
        assert!(
            body.contains("/pub/book/paper/paper/01.md"),
            "the page endpoint is wired: {body}"
        );
        assert!(
            body.contains(r#"id="paper-01.md""#),
            "each page carries an anchor: {body}"
        );

        let body = get_body(&app, "/pub/book/paper/paper/01.md").await;
        assert!(
            body.contains(r#"<h1 id="paper-01.md--first">First</h1>"#),
            "headings are anchored: {body}"
        );

        let body = get_body(&app, "/pub/book/paper/.twig.toml").await;
        assert!(
            body.contains("Paper page not found."),
            "files outside the paper dir are refused: {body}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_scripts_tab_is_hidden_until_configured() {
        let root = format!("/tmp/test_twig_scripts_hidden_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        init_repo_with_files(
            &std::path::Path::new(&root).join("pub/plain"),
            "tabs = []\n",
            &[("README.md", "# Plain\n")],
        );

        let body = get_body(&app, "/pub/plain").await;
        assert!(
            !body.contains(">Scripts<"),
            "no [scripts] section means no Scripts tab: {body}"
        );

        // Even a direct visit stays inert: no groups render.
        let body = get_body(&app, "/pub/plain/scripts").await;
        assert!(body.contains("NO SCRIPTS"), "{body}");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_scripts_tab_shows_groups_with_curl_commands() {
        let root = format!("/tmp/test_twig_scripts_tab_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        init_repo_with_files(
            &std::path::Path::new(&root).join("pub/toolbox"),
            r#"
[scripts.linux]
name = "Linux"
scripts = [{ name = "Install", path = "scripts/install.sh", shell = "sh" }]

[scripts.linux.maintenance]
scripts = [{ name = "Cleanup", path = "scripts/cleanup.sh" }]
"#,
            &[
                ("README.md", "# Toolbox\n"),
                ("scripts/install.sh", "#!/bin/sh\necho install\n"),
                ("scripts/cleanup.sh", "#!/bin/bash\necho cleanup\n"),
            ],
        );

        // The tab appears on the repository home and points at its own route.
        let body = get_body(&app, "/pub/toolbox").await;
        assert!(body.contains(">Scripts<"), "Scripts tab missing: {body}");
        assert!(
            body.contains("href=\"/pub/toolbox/scripts\""),
            "the tab points at the scripts view: {body}"
        );

        // The default group renders with its sub-tab bar and run command.
        let body = get_body(&app, "/pub/toolbox/scripts").await;
        assert!(body.contains("scripts-container"), "{body}");
        assert!(body.contains(">Linux<"), "the group is a sub-tab: {body}");
        assert!(
            body.contains("href=\"/pub/toolbox/scripts/linux\""),
            "sub-tabs link by group key: {body}"
        );
        assert!(
            body.contains(
                "curl -fsSL 'http://localhost:8080/pub/toolbox/raw/scripts/install.sh' | sh"
            ),
            "the run command names the raw route and the configured shell: {body}"
        );
        assert!(
            body.contains("data-twig-copy"),
            "every entry carries a copy button: {body}"
        );

        // A nested group renders its own scripts under its key.
        let body = get_body(&app, "/pub/toolbox/scripts/linux/maintenance").await;
        assert!(
            body.contains(
                "curl -fsSL 'http://localhost:8080/pub/toolbox/raw/scripts/cleanup.sh' | bash"
            ),
            "nested groups resolve and default to bash: {body}"
        );

        // An unknown group key falls back to the first configured group.
        let body = get_body(&app, "/pub/toolbox/scripts/bogus").await;
        assert!(
            body.contains("curl -fsSL 'http://localhost:8080/pub/toolbox/raw/scripts/install.sh'"),
            "unknown keys fall back to the first group: {body}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_http_binary_upload_and_download_work_for_public_and_private_repos() {
        use base64::Engine as _;

        let root = format!("/tmp/test_twig_binary_upload_{}", uuid::Uuid::new_v4());
        let (app, db) = create_test_service_with_db_in(&root).await;
        let owner = auth::create_user(
            "binary-owner".to_string(),
            "binary-owner@example.com".to_string(),
            "correct horse battery staple",
        )
        .expect("create upload user");
        db.create_user(&owner).await.expect("store upload user");
        let outsider = auth::create_user(
            "binary-outsider".to_string(),
            "binary-outsider@example.com".to_string(),
            "correct horse battery staple",
        )
        .expect("create unrelated user");
        db.create_user(&outsider)
            .await
            .expect("store unrelated user");
        let namespace = auth::create_namespace("pub".to_string(), owner.id.clone());
        db.create_namespace(&namespace)
            .await
            .expect("create upload namespace");

        init_repo_with_files(
            &std::path::Path::new(&root).join("pub/toolbox"),
            r#"
[scripts.linux]
scripts = [{ name = "Install", path = "scripts/install.sh" }]
"#,
            &[("scripts/install.sh", "#!/bin/sh\necho install\n")],
        );
        init_repo_with_files(
            &std::path::Path::new(&root).join("pub/plain"),
            "\n",
            &[("README.md", "No Scripts tab here.\n")],
        );
        init_repo_with_files(
            &std::path::Path::new(&root).join("pub/private-tool"),
            r#"
private = true
[scripts.linux]
scripts = [{ name = "Install", path = "install.sh" }]
"#,
            &[("install.sh", "#!/bin/sh\n")],
        );

        let basic = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD
                .encode("binary-owner:correct horse battery staple")
        );
        let outsider_basic = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD
                .encode("binary-outsider:correct horse battery staple")
        );
        let anonymous_upload = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/pub/toolbox/binaries/1.4.2/example-linux-x86_64")
                .set_payload(b"unauthorized".to_vec())
                .to_request(),
        )
        .await;
        assert_eq!(anonymous_upload.status(), StatusCode::UNAUTHORIZED);
        let unrelated_upload = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/pub/toolbox/binaries/1.4.2/example-linux-x86_64")
                .insert_header(("Authorization", outsider_basic.as_str()))
                .set_payload(b"not allowed".to_vec())
                .to_request(),
        )
        .await;
        assert_eq!(unrelated_upload.status(), StatusCode::FORBIDDEN);

        let linux_bytes = b"locally built linux binary\0bytes";
        let upload_linux = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/pub/toolbox/binaries/v1.4.2/example-linux-x86_64")
                .insert_header(("Authorization", basic.as_str()))
                .set_payload(linux_bytes.to_vec())
                .to_request(),
        )
        .await;
        assert_eq!(upload_linux.status(), StatusCode::CREATED);
        let replacement_bytes = b"replacement linux binary\0bytes";
        let replace_linux = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/pub/toolbox/binaries/1.4.2/example-linux-x86_64")
                .insert_header(("Authorization", basic.as_str()))
                .set_payload(replacement_bytes.to_vec())
                .to_request(),
        )
        .await;
        assert_eq!(replace_linux.status(), StatusCode::OK);

        let mac_bytes = b"locally built macOS binary";
        let upload_mac = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/pub/toolbox/binaries/1.4.2/example-darwin-arm64")
                .insert_header(("Authorization", basic.as_str()))
                .set_payload(mac_bytes.to_vec())
                .to_request(),
        )
        .await;
        assert_eq!(upload_mac.status(), StatusCode::CREATED);

        let page = get_body(&app, "/pub/toolbox/scripts").await;
        assert!(page.contains("Available binaries"), "{page}");
        assert!(page.contains("v1.4.2"), "{page}");
        assert!(page.contains("example-linux-x86_64"), "{page}");
        assert!(page.contains("example-darwin-arm64"), "{page}");
        assert!(
            page.contains("data-twig-copy"),
            "install command needs a Copy button: {page}"
        );
        assert!(
            page.contains("/pub/toolbox/binaries/latest/…"),
            "the page explains the direct script URL: {page}"
        );

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/pub/toolbox/binaries/1.4.2/example-linux-x86_64")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/octet-stream"
        );
        assert_eq!(
            response.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert!(
            response
                .headers()
                .get("content-disposition")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("example-linux-x86_64")
        );
        assert_eq!(test::read_body(response).await.as_ref(), replacement_bytes);

        let latest = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/pub/toolbox/binaries/latest/example-linux-x86_64")
                .to_request(),
        )
        .await;
        assert_eq!(latest.status(), StatusCode::OK);
        assert_eq!(test::read_body(latest).await.as_ref(), replacement_bytes);

        // Direct downloads remain available when the repository has no Scripts tab.
        let plain_upload = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/pub/plain/binaries/1.0.0/tool")
                .insert_header(("Authorization", basic.as_str()))
                .set_payload(b"plain repo asset".to_vec())
                .to_request(),
        )
        .await;
        assert_eq!(plain_upload.status(), StatusCode::CREATED);
        let direct = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/pub/plain/binaries/1.0.0/tool")
                .to_request(),
        )
        .await;
        assert_eq!(direct.status(), StatusCode::OK);
        assert_eq!(test::read_body(direct).await.as_ref(), b"plain repo asset");

        let private_upload = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/pub/private-tool/binaries/1.0.0/private-linux")
                .insert_header(("Authorization", basic.as_str()))
                .set_payload(b"private asset".to_vec())
                .to_request(),
        )
        .await;
        assert_eq!(private_upload.status(), StatusCode::CREATED);
        let private_anonymous = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/pub/private-tool/binaries/1.0.0/private-linux")
                .to_request(),
        )
        .await;
        assert_eq!(private_anonymous.status(), StatusCode::UNAUTHORIZED);
        db.create_token("binary-private-session", &owner.id)
            .await
            .expect("create authenticated session token");
        let private_authenticated = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/pub/private-tool/binaries/1.0.0/private-linux")
                .cookie(actix_web::cookie::Cookie::new(
                    "session",
                    "binary-private-session",
                ))
                .to_request(),
        )
        .await;
        assert_eq!(private_authenticated.status(), StatusCode::OK);
        assert_eq!(
            test::read_body(private_authenticated).await.as_ref(),
            b"private asset"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_binary_uploads_retain_semver_releases_and_latest_resolves_by_asset() {
        use base64::Engine as _;

        let root = format!("/tmp/test_twig_binary_semver_{}", uuid::Uuid::new_v4());
        let (app, db) = create_test_service_with_db_in(&root).await;
        let owner = auth::create_user(
            "semver-owner".to_string(),
            "semver-owner@example.com".to_string(),
            "correct horse battery staple",
        )
        .expect("create upload user");
        db.create_user(&owner).await.expect("store upload user");
        db.create_namespace(&auth::create_namespace("pub".to_string(), owner.id.clone()))
            .await
            .expect("create upload namespace");
        init_repo_with_files(
            &std::path::Path::new(&root).join("pub/versions"),
            r#"
[scripts.linux]
scripts = [{ name = "Install", path = "install.sh" }]
"#,
            &[("install.sh", "#!/bin/sh\n")],
        );

        let basic = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD
                .encode("semver-owner:correct horse battery staple")
        );
        for (version, filename, bytes) in [
            ("v1.9.0", "linux", &b"linux-1.9"[..]),
            ("1.10.0", "macos", &b"macos-1.10"[..]),
            ("1.8.0", "linux", &b"linux-1.8"[..]),
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::put()
                    .uri(&format!("/pub/versions/binaries/{version}/{filename}"))
                    .insert_header(("Authorization", basic.as_str()))
                    .set_payload(bytes.to_vec())
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        let too_old = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/pub/versions/binaries/1.7.0/linux")
                .insert_header(("Authorization", basic.as_str()))
                .set_payload(b"too old".to_vec())
                .to_request(),
        )
        .await;
        assert_eq!(too_old.status(), StatusCode::CONFLICT);

        let invalid = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/pub/versions/binaries/latest/linux")
                .insert_header(("Authorization", basic.as_str()))
                .set_payload(b"reserved".to_vec())
                .to_request(),
        )
        .await;
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

        let page = get_body(&app, "/pub/versions/scripts").await;
        let v110 = page.find("v1.10.0").expect("newest version rendered");
        let v19 = page.find("v1.9.0").expect("second version rendered");
        let v18 = page.find("v1.8.0").expect("third version rendered");
        assert!(
            v110 < v19 && v19 < v18,
            "versions are in SemVer order: {page}"
        );
        assert!(
            !page.contains("v1.7.0"),
            "too-old release was not retained: {page}"
        );

        let latest_linux = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/pub/versions/binaries/latest/linux")
                .to_request(),
        )
        .await;
        assert_eq!(latest_linux.status(), StatusCode::OK);
        assert_eq!(test::read_body(latest_linux).await.as_ref(), b"linux-1.9");

        let latest_macos = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/pub/versions/binaries/latest/macos")
                .to_request(),
        )
        .await;
        assert_eq!(latest_macos.status(), StatusCode::OK);
        assert_eq!(test::read_body(latest_macos).await.as_ref(), b"macos-1.10");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_raw_route_serves_repo_files_and_refuses_ignored_paths() {
        let root = format!("/tmp/test_twig_scripts_raw_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        init_repo_with_files(
            &std::path::Path::new(&root).join("pub/toolbox"),
            r#"
ignore_for_view = ["secrets/"]
[scripts.linux]
scripts = [{ name = "Install", path = "scripts/install.sh" }]
"#,
            &[
                ("scripts/install.sh", "#!/bin/bash\necho install\n"),
                ("secrets/key.txt", "DO NOT SERVE\n"),
            ],
        );

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/pub/toolbox/raw/scripts/install.sh")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("Content-Type").unwrap(),
            "text/plain; charset=utf-8"
        );
        let body = test::read_body(response).await;
        assert_eq!(body, "#!/bin/bash\necho install\n");

        for uri in [
            "/pub/toolbox/raw/secrets/key.txt",
            "/pub/toolbox/raw/missing.sh",
            "/pub/toolbox/raw/../../secrets/key.txt",
            "/pub/toolbox/raw/%2e%2e/secrets/key.txt",
            "/pub/toolbox/raw/",
        ] {
            let response =
                test::call_service(&app, test::TestRequest::get().uri(uri).to_request()).await;
            assert_eq!(
                response.status(),
                StatusCode::NOT_FOUND,
                "expected 404 for {uri}"
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_legacy_tab_url_redirects_to_the_tab_route() {
        let root = format!("/tmp/test_twig_tab_redirect_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        init_repo_with_paper(
            &std::path::Path::new(&root).join("pub/book"),
            "[paper]\ndir = \"paper\"\n",
            &[("01.md", "# First\n")],
        );

        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/pub/book/tab/paper")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            response.headers().get("Location").unwrap(),
            "/pub/book/paper",
            "the legacy tab URL points at the tab's own route"
        );

        // An unknown tab name falls back to the repository home.
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/pub/book/tab/bogus")
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        assert_eq!(response.headers().get("Location").unwrap(), "/pub/book");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_invalid_twig_toml_shows_a_diagnostic() {
        let root = format!("/tmp/test_twig_badconfig_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        init_repo_with_paper(
            &std::path::Path::new(&root).join("pub/broken"),
            "this = = not valid\n",
            &[],
        );

        let body = get_body(&app, "/pub/broken").await;
        assert!(body.contains("CONFIG ERROR"), "{body}");
        assert!(
            body.contains("error: could not parse configuration"),
            "the parse failure is surfaced, not swallowed: {body}"
        );
        assert!(body.contains("help: fix the syntax"), "{body}");
        assert!(
            body.contains(".twig.toml"),
            "the offending file is named: {body}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    fn basic_auth(username: &str, password: &str) -> String {
        use base64::Engine;
        let creds = format!("{username}:{password}");
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(creds.as_bytes())
        )
    }

    #[actix_web::test]
    async fn test_private_repository_ui_visibility_gated_on_login() {
        let root = format!("/tmp/test_twig_private_ui_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let owner_session = signup_and_login(&app, "privowner").await;
        create_namespace(&app, owner_session.clone(), "privspace").await;

        let pub_repo = std::path::Path::new(&root).join("privspace/pubrepo");
        std::fs::create_dir_all(&pub_repo).unwrap();
        crate::git::repo::bare_init(&pub_repo, "main", "Test", "test@example.com").unwrap();

        let priv_repo = std::path::Path::new(&root).join("privspace/secretrepo");
        init_private_repo(&priv_repo);

        // Anonymous namespace listing: only public repo should appear
        let req = test::TestRequest::get().uri("/privspace").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
        assert!(body.contains("pubrepo"), "public repo must be visible");
        assert!(
            !body.contains("secretrepo"),
            "private repo must NOT be visible to anonymous visitors"
        );

        // Logged-in namespace listing: both public and private repos appear
        let req = test::TestRequest::get()
            .uri("/privspace")
            .cookie(owner_session.clone())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
        assert!(body.contains("pubrepo"), "public repo must be visible");
        assert!(
            body.contains("secretrepo"),
            "private repo must be visible to logged-in users"
        );

        // Anonymous direct access to private repo: auth error
        let req = test::TestRequest::get()
            .uri("/privspace/secretrepo")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
        assert!(
            body.contains("Not logged in. Please log in first."),
            "must show auth error notice"
        );
        assert!(body.contains("href=\"/auth/login\""));

        // Anonymous tab access: auth error
        let req = test::TestRequest::get()
            .uri("/privspace/secretrepo/commits")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
        assert!(body.contains("Not logged in. Please log in first."));

        // Anonymous content access: auth error
        let req = test::TestRequest::get()
            .uri("/privspace/secretrepo/content/")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
        assert!(body.contains("Not logged in. Please log in first."));

        // Logged-in direct access to private repo: succeeds with repo page
        let req = test::TestRequest::get()
            .uri("/privspace/secretrepo")
            .cookie(owner_session)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
        assert!(!body.contains("Not logged in. Please log in first."));
        assert!(body.contains("privspace/secretrepo"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[actix_web::test]
    async fn test_private_repository_git_read_gated_behind_auth() {
        let root = format!("/tmp/test_twig_private_git_{}", uuid::Uuid::new_v4());
        let app = create_test_service_in(&root).await;
        let owner_session = signup_and_login(&app, "gitprivowner").await;
        let other_session = signup_and_login(&app, "gitprivother").await;
        create_namespace(&app, owner_session, "gitspace").await;
        create_namespace(&app, other_session, "otherspace").await;

        let pub_repo = std::path::Path::new(&root).join("gitspace/pubrepo");
        std::fs::create_dir_all(&pub_repo).unwrap();
        crate::git::repo::bare_init(&pub_repo, "main", "Test", "test@example.com").unwrap();

        let priv_repo = std::path::Path::new(&root).join("gitspace/privaterepo");
        init_private_repo(&priv_repo);

        // Public repo git read (unauthenticated) -> 200 OK
        let req = test::TestRequest::get()
            .uri("/gitspace/pubrepo/info/refs?service=git-upload-pack")
            .insert_header(("User-Agent", "git/2.43.0"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // The regular Git smart-push handshake remains available and still
        // requires credentials, independently of the binary HTTP endpoints.
        let req = test::TestRequest::get()
            .uri("/gitspace/pubrepo/info/refs?service=git-receive-pack")
            .insert_header(("User-Agent", "git/2.43.0"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let req = test::TestRequest::get()
            .uri("/gitspace/pubrepo/info/refs?service=git-receive-pack")
            .insert_header(("User-Agent", "git/2.43.0"))
            .insert_header(("Authorization", basic_auth("gitprivowner", "password123")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Private repo git read (unauthenticated) -> 401 Unauthorized with WWW-Authenticate
        let req = test::TestRequest::get()
            .uri("/gitspace/privaterepo/info/refs?service=git-upload-pack")
            .insert_header(("User-Agent", "git/2.43.0"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let auth_header = resp
            .headers()
            .get("WWW-Authenticate")
            .expect("must contain WWW-Authenticate header")
            .to_str()
            .unwrap();
        assert!(auth_header.contains("Basic"));

        // Private repo git read with .git suffix (unauthenticated) -> 401 Unauthorized
        let req = test::TestRequest::get()
            .uri("/gitspace/privaterepo.git/info/refs?service=git-upload-pack")
            .insert_header(("User-Agent", "git/2.43.0"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Private repo git read with wrong password -> 401 Unauthorized
        let req = test::TestRequest::get()
            .uri("/gitspace/privaterepo/info/refs?service=git-upload-pack")
            .insert_header(("User-Agent", "git/2.43.0"))
            .insert_header((
                "Authorization",
                basic_auth("gitprivowner", "wrong-password"),
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Private repo git read with valid user but no namespace access -> 403 Forbidden
        let req = test::TestRequest::get()
            .uri("/gitspace/privaterepo/info/refs?service=git-upload-pack")
            .insert_header(("User-Agent", "git/2.43.0"))
            .insert_header(("Authorization", basic_auth("gitprivother", "password123")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // Private repo git read with owner credentials -> 200 OK
        let req = test::TestRequest::get()
            .uri("/gitspace/privaterepo/info/refs?service=git-upload-pack")
            .insert_header(("User-Agent", "git/2.43.0"))
            .insert_header(("Authorization", basic_auth("gitprivowner", "password123")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Private repo git read with .git suffix with owner credentials -> 200 OK
        let req = test::TestRequest::get()
            .uri("/gitspace/privaterepo.git/info/refs?service=git-upload-pack")
            .insert_header(("User-Agent", "git/2.43.0"))
            .insert_header(("Authorization", basic_auth("gitprivowner", "password123")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let _ = std::fs::remove_dir_all(&root);
    }

    async fn create_test_service_with_test_user(
        admin_user: Option<&str>,
    ) -> (
        impl actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        web::Data<auth::TwigContext>,
    ) {
        let mut config = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/tmp/test_git".to_string(),
            format!("/tmp/test_twig_test_user_{}.db", uuid::Uuid::new_v4()),
            "secure".to_string(),
            true,
        );
        if let Some(user) = admin_user {
            config = config.with_test_user(Some(user.to_string()));
        }

        let db = Database::new(config.db_path());
        let auth_state = web::Data::new(auth::TwigContext::new(db, "secure".to_string()));
        auth_state.db().init_tables().await.expect("init tables");

        let config_data = web::Data::new(config);
        let session_db = auth_state.db().clone();
        let app = test::init_service(
            App::new()
                .app_data(config_data)
                .app_data(auth_state.clone())
                .app_data(web::PayloadConfig::new(1 << 29))
                .wrap(actix_identity::IdentityMiddleware::default())
                .wrap(crate::auth::session_store::middleware(
                    session_db,
                    actix_web::cookie::Key::generate(),
                ))
                .configure(crate::http::routes::configure_routes),
        )
        .await;

        (app, auth_state)
    }

    #[actix_web::test]
    async fn test_test_page_404_when_test_user_unset() {
        let (app, _) = create_test_service_with_test_user(None).await;
        let req = test::TestRequest::get().uri("/_test").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_test_page_forbidden_when_unauthenticated() {
        let (app, _) = create_test_service_with_test_user(Some("admin")).await;
        let req = test::TestRequest::get().uri("/_test").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn test_test_page_and_rapid_runner_when_authenticated_as_admin() {
        let (app, auth_state) = create_test_service_with_test_user(Some("admin")).await;
        let user = auth::create_user(
            "admin".to_string(),
            "admin@example.com".to_string(),
            "password123",
        )
        .unwrap();
        auth_state.db().create_user(&user).await.unwrap();
        let token = auth_state.create_session(user.id).await.unwrap();

        // 1. Load main test page
        let req = test::TestRequest::get()
            .uri("/_test")
            .cookie(actix_web::cookie::Cookie::new("session", &token))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Test Suite"));
        assert!(body_str.contains("id=\"btn-start\""));
        assert!(body_str.contains("id=\"btn-stop\""));
        assert!(body_str.contains("id=\"test-runner-container\""));
        assert!(body_str.contains("id=\"test-runner-idle\""));
        assert!(!body_str.contains("id=\"test-runner\""));

        // 2. Load runner content (very low trigger on page, even smaller on contents)
        let req = test::TestRequest::get()
            .uri("/_test/runner")
            .cookie(actix_web::cookie::Cookie::new("session", &token))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("hx-trigger=\"every 25ms\""));
        assert!(body_str.contains("hx-trigger=\"load, every 5ms\""));
        assert!(body_str.contains("/health"));
        assert!(body_str.contains("/api/tree"));
        assert!(!body_str.contains("/settings"));
        assert!(!body_str.contains("delete-namespace"));

        // 3. Load stopped content
        let req = test::TestRequest::get()
            .uri("/_test/stopped")
            .cookie(actix_web::cookie::Cookie::new("session", &token))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("id=\"test-runner-stopped\""));
        assert!(body_str.contains("Test run interrupted"));

        // 4. Ping and feature-check endpoints
        let req = test::TestRequest::get()
            .uri("/_test/ping")
            .cookie(actix_web::cookie::Cookie::new("session", &token))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let req = test::TestRequest::get()
            .uri("/_test/feature-check")
            .cookie(actix_web::cookie::Cookie::new("session", &token))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_test_page_pin_and_qr_code_multi_user_lifecycle() {
        let (app, auth_state) = create_test_service_with_test_user(Some("admin")).await;
        let admin = auth::create_user(
            "admin".to_string(),
            "admin@example.com".to_string(),
            "password123",
        )
        .unwrap();
        auth_state.db().create_user(&admin).await.unwrap();
        let admin_token = auth_state.create_session(admin.id).await.unwrap();

        // 1. Non-admin cannot create a PIN -> 403 Forbidden
        let req = test::TestRequest::post()
            .uri("/_test/pin/create")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // 2. Admin creates PIN -> 200 OK with PIN and QR code SVG
        let req = test::TestRequest::post()
            .uri("/_test/pin/create")
            .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("id=\"active-pin-display\""));
        assert!(body_str.contains("<svg"));
        assert!(body_str.contains("Remove PIN"));

        let active_pin = auth_state
            .get_test_pin()
            .await
            .expect("active pin must be set");
        assert_eq!(active_pin.len(), 6);
        // Verify PIN is saved in the database directly
        assert_eq!(
            auth_state.db().get_test_pin().await.unwrap().as_deref(),
            Some(active_pin.as_str())
        );

        // 3. Guest visits with PIN in query param -> 200 OK, sets test_pin cookie
        let req = test::TestRequest::get()
            .uri(&format!("/_test?pin={active_pin}"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify test_pin cookie was attached
        let pin_cookie = resp
            .response()
            .cookies()
            .find(|c| c.name() == "test_pin")
            .expect("must set test_pin cookie");
        assert_eq!(pin_cookie.value(), active_pin);

        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("GUEST (PIN:"));

        // 4. Guest runs the test runner with test_pin cookie -> 200 OK
        let req = test::TestRequest::get()
            .uri("/_test/runner")
            .cookie(actix_web::cookie::Cookie::new("test_pin", &active_pin))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("hx-trigger=\"every 25ms\""));
        assert!(body_str.contains("hx-trigger=\"load, every 5ms\""));

        // 5. Guest requests QR endpoint -> 200 OK with SVG
        let req = test::TestRequest::get()
            .uri("/_test/pin/qr")
            .cookie(actix_web::cookie::Cookie::new("test_pin", &active_pin))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers().get("content-type").unwrap(), "image/svg+xml");

        // 6. Admin removes PIN -> 200 OK
        let req = test::TestRequest::post()
            .uri("/_test/pin/remove")
            .cookie(actix_web::cookie::Cookie::new("session", &admin_token))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(auth_state.get_test_pin().await, None);
        assert_eq!(auth_state.db().get_test_pin().await.unwrap(), None);

        // 7. Guest trying to run runner with now-revoked PIN -> 403 Forbidden!
        let req = test::TestRequest::get()
            .uri("/_test/runner")
            .cookie(actix_web::cookie::Cookie::new("test_pin", &active_pin))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // 8. Guest trying to access main test page with old PIN -> 403 Forbidden!
        let req = test::TestRequest::get()
            .uri(&format!("/_test?pin={active_pin}"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
