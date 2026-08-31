// ========== ACTIX API TESTS ==========

#[cfg(test)]
mod tests {
    use crate::{assets, auth, config, db::Database, health, view};
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
                .app_data(auth_state)
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

        test::init_service(
            App::new()
                .app_data(config_data)
                .app_data(auth_state)
                .app_data(web::PayloadConfig::new(1 << 29))
                .service(health::health)
                .service(health::up)
                .service(assets::assets)
                .service(view::auth::invite_page)
                .service(view::auth::signup_page)
                .service(view::auth::login_page)
                .service(view::auth::namespace_page)
                .service(auth::handlers::create_invite_ui_handler)
                .service(auth::handlers::signup_ui_handler)
                .service(auth::handlers::login_ui_handler)
                .service(auth::handlers::create_namespace_ui_handler)
                .service(auth::handlers::logout_ui_handler)
                .service(view::settings::settings_page)
                .service(view::settings::update_email)
                .service(view::settings::delete_repo)
                .service(view::settings::delete_namespace)
                .service(view::overview::index)
                .service(view::namespace::handler)
                .service(view::namespace::create_repo_form_handler)
                .service(view::namespace::create_repo_handler)
                .service(view::repo::handler)
                .service(view::repo::tab_handler)
                .service(view::repo::slide_handler)
                .service(view::repo::markdown_handler)
                .service(view::repo::content_handler),
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

    // Settings tests - from settings.hurl
    #[actix_web::test]
    async fn test_settings_page_without_login() {
        let app = create_test_service().await;
        let req = test::TestRequest::get().uri("/settings").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
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

    /// Signs a user up and logs them in, returning the session cookie.
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
            .find(|c| c.name() == "session")
            .expect("login should set a session cookie")
            .into_owned()
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
}
