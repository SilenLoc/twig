// ========== ACTIX API TESTS ==========

#[cfg(test)]
mod tests {
    use crate::{auth, config, db::Database, health, view};
    use actix_http::Request;
    use actix_web::{App, http::StatusCode, test, web};

    // Health endpoint test
    #[actix_web::test]
    async fn test_health_endpoint() {
        let config = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/tmp/test_git".to_string(),
            format!("/tmp/test_fig_health_{}.db", uuid::Uuid::new_v4()),
            "secure".to_string(),
            true,
            1.0,
        );

        let db = Database::new(config.db_path());
        let auth_state = web::Data::new(auth::FigContext::new(db, "secure".to_string()));
        // Initialize database synchronously (we are already inside an async test runtime)
        auth_state.db().init_tables().await.expect("init tables");
        auth_state.set_initialized();

        let config_data = web::Data::new(config);

        let app = test::init_service(
            App::new()
                .app_data(config_data)
                .app_data(auth_state.clone())
                .app_data(web::PayloadConfig::new(1 << 29))
                .service(health::health)
                .service(health::up),
        )
        .await;

        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // Helper function to create full test app service
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
        // A distinct database per call: nextest runs each test in its own
        // process, and turso takes an exclusive file lock, so a shared path
        // would make concurrent tests fail to open the database.
        let config = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            project_root.to_string(),
            format!("/tmp/test_fig_service_{}.db", uuid::Uuid::new_v4()),
            "secure".to_string(),
            true,
            1.0,
        );

        let db = Database::new(config.db_path());
        let auth_state = web::Data::new(auth::FigContext::new(db, "secure".to_string()));
        // Initialize database synchronously (we are already inside an async test runtime)
        auth_state.db().init_tables().await.expect("init tables");
        auth_state.set_initialized();

        let config_data = web::Data::new(config);
        let session_db = auth_state.db().clone();

        test::init_service(
            App::new()
                .app_data(config_data)
                .app_data(auth_state)
                .app_data(web::PayloadConfig::new(1 << 29))
                .wrap(actix_identity::IdentityMiddleware::default())
                .wrap(crate::auth::session_store::middleware(
                    session_db,
                    actix_web::cookie::Key::generate(),
                ))
                .configure(crate::configure_routes),
        )
        .await
    }

    // Rewritten health test using helper
    #[actix_web::test]
    async fn test_health_endpoint_v2() {
        let app = create_test_service().await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // Auth UI tests - from auth_ui.hurl
    #[actix_web::test]
    async fn test_invite_page_loads() {
        let app = create_test_service().await;
        let req = test::TestRequest::get().uri("/auth/invite").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("<title>Get Signup Invite · Fig</title>"));
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
        assert!(body_str.contains("<title>Create Account · Fig</title>"));
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
        assert!(body_str.contains("<title>Log In · Fig</title>"));
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
        assert!(body_str.contains("<title>Create Namespace · Fig</title>"));
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
        let db_path = format!("/tmp/test_fig_admin_data_{}.db", uuid::Uuid::new_v4());
        let server = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/tmp/test_git".to_string(),
            db_path.clone(),
            "secure".to_string(),
            false,
            1.0,
        )
        .with_admin_user(Some("dbadmin".to_string()))
        .with_test_user(Some("testadmin".to_string()));
        let db = Database::new(&db_path);
        db.init_tables().await.expect("initialize tables");
        let auth_state = web::Data::new(auth::FigContext::new(db.clone(), "secure".to_string()));

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
                .configure(crate::configure_routes),
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
        let body = test::read_body(response).await;
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("href=\"/settings\""));
        assert!(html.contains("href=\"/_test\""));
        assert!(html.contains("href=\"/tree/data\""));

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
        assert!(body_str.contains("<title>Settings · Fig</title>"));
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
        assert!(body_str.contains("<title>Settings · Fig</title>"));
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

    #[actix_web::test]
    async fn test_move_repo_between_owned_namespaces() {
        let root = format!("/tmp/test_fig_move_repo_{}", uuid::Uuid::new_v4());
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
        let root = format!("/tmp/test_fig_move_denied_{}", uuid::Uuid::new_v4());
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
    async fn test_cannot_create_namespace_with_reserved_prefix() {
        let root = format!("/tmp/test_fig_ns_reserved_{}", uuid::Uuid::new_v4());
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
        let root = PathBuf::from(format!("/tmp/fig_trav_root_{id}"));
        let repo_dir = root.join("public").join("repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        crate::git::repo::bare_init(&repo_dir, "main", "Test", "t@example.com")
            .expect("bare_init failed");

        // Secret file outside the project root, reachable only via traversal
        let outside = PathBuf::from(format!("/tmp/fig_trav_secret_{id}"));
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(
            outside.join("secret.txt"),
            format!("{SECRET_MARKER}\nroot:x:0:0:root\n"),
        )
        .unwrap();

        TraversalFixture {
            root,
            outside,
            db_path: format!("/tmp/fig_trav_{id}.db"),
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
            1.0,
        );

        let db = Database::new(config.db_path());
        let auth_state = web::Data::new(auth::FigContext::new(db, "secure".to_string()));
        auth_state.db().init_tables().await.expect("init tables");
        auth_state.set_initialized();

        test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(auth_state)
                .service(view::repo::handler)
                .service(view::repo::tab_handler)
                .service(view::repo::content_handler)
                .service(view::repo::markdown_handler),
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

        let (status, body) = body_of(&app, "/public/repo/tab/content").await;
        assert_eq!(status, StatusCode::OK);
        // Positive control: repo's own file is listed
        assert!(
            body.contains(".fig.toml"),
            "expected .fig.toml in listing: {body}"
        );
        assert!(!body.contains(SECRET_MARKER));
    }

    #[actix_web::test]
    async fn test_content_file_view_serves_repo_files_only() {
        let fixture = setup_traversal_fixture();
        let app = create_traversal_service(&fixture).await;

        let (status, body) = body_of(&app, "/public/repo/content/.fig.toml").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Created with Fig"), "{body}");
        assert!(
            body.contains(r#"<nav class="fig-crumbs fig-crumbs--page" aria-label="Breadcrumb">"#),
            "direct file views must retain the repository breadcrumb: {body}"
        );
        assert!(
            body.contains(r#"<h1 class="fig-crumb-current" aria-current="page">repo</h1>"#),
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
            "/..%2f..%2ftmp%2fnonsense/repo/tab/content",
            "/public%2f..%2f..%2fsecret/repo/tab/content",
            // repo name tries to climb out of the namespace dir
            "/public/..%2f..%2fsecret/tab/content",
            "/public/../secret/tab/content",
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
            "/public/repo/content/.fig.toml%00",
            "/public/repo/content/%00.fig.toml",
            "/public/repo/content/a%00b",
        ] {
            let (_status, body) = body_of(&app, uri).await;
            assert!(
                !body.contains("Created with Fig") || uri.contains(".fig"),
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
        let (status, body) = body_of(&app, "/public/repo/content/.fig.toml").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Created with Fig"));

        // ...but dot-segment navigation must not escape into other namespaces
        let (_status, body) =
            body_of(&app, "/public/repo/content/../../other/vault/.fig.toml").await;
        assert!(
            !body.contains("Created with Fig"),
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
            .stdin(format!("100644 blob {blob}\t.fig.toml\n"))
            .read()
            .unwrap();
        let commit = xshell::cmd!(sh, "git commit-tree {tree} -m 'make private'")
            .read()
            .unwrap();
        xshell::cmd!(sh, "git update-ref refs/heads/main {commit}")
            .run()
            .unwrap();
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
        let root = format!("/tmp/test_fig_private_ui_{}", uuid::Uuid::new_v4());
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
            .uri("/privspace/secretrepo/tab/commits")
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
        let root = format!("/tmp/test_fig_private_git_{}", uuid::Uuid::new_v4());
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
        web::Data<auth::FigContext>,
    ) {
        let mut config = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/tmp/test_git".to_string(),
            format!("/tmp/test_fig_test_user_{}.db", uuid::Uuid::new_v4()),
            "secure".to_string(),
            true,
            1.0,
        );
        if let Some(user) = admin_user {
            config = config.with_test_user(Some(user.to_string()));
        }

        let db = Database::new(config.db_path());
        let auth_state = web::Data::new(auth::FigContext::new(db, "secure".to_string()));
        auth_state.db().init_tables().await.expect("init tables");
        auth_state.set_initialized();

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
                .configure(crate::configure_routes),
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
        assert!(body_str.contains("/api/v1/tree"));
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
