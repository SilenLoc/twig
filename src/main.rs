use actix_web::{
    App, HttpResponse, HttpServer, Responder, get, guard,
    web::{self},
};
use db::Database;
use env_logger::Env;
use log::{info, warn};

mod assets;
mod auth;
mod config;
mod db;
mod git;
mod git_backend;
mod md;
mod view;

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok()
}

#[get("/up")]
async fn up() -> impl Responder {
    HttpResponse::Ok().finish()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = config::from_env();

    let log_filter = format!(
        "{},libsql=warn,turso=warn,tracing::span=warn",
        config.log_level()
    );
    env_logger::Builder::from_env(Env::default().default_filter_or(log_filter)).init();

    info!("{config}");

    let config = web::Data::new(config);

    // Check if we should reset the database
    config.maybe_reset_database();

    // Initialize database
    let db_path = config.db_path().to_string();
    let api_key = config.effective_api_key();

    let db = Database::new(&db_path);

    let auth_state = web::Data::new(auth::FigContext::new(db, api_key));

    {
        let auth_state = auth_state.clone();
        actix_web::rt::spawn(async move {
            loop {
                match auth_state.db().init_tables().await {
                    Ok(()) => {
                        info!("Database initialized successfully");
                        auth_state.set_initialized();
                        return;
                    }
                    Err(e) => {
                        warn!("Database init failed (will retry): {e}");
                        actix_web::rt::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }

    let bind_address = config.address();

    HttpServer::new(move || {
        App::new()
            .app_data(config.clone())
            .app_data(auth_state.clone())
            // Increase payload limit to 512MB for large git pushes
            .app_data(web::PayloadConfig::new(1 << 29))
            .service(health)
            .service(up)
            .service(assets::assets)
            // Auth UI endpoints (HTML forms)
            .service(view::auth::ticket_page)
            .service(view::auth::signup_page)
            .service(view::auth::login_page)
            .service(view::auth::namespace_page)
            .service(auth::handlers::create_ticket_ui_handler)
            .service(auth::handlers::signup_ui_handler)
            .service(auth::handlers::login_ui_handler)
            .service(auth::handlers::create_namespace_ui_handler)
            .service(auth::handlers::logout_ui_handler)
            // Web UI endpoints (MUST come before git routes to avoid pattern conflicts)
            .service(view::overview::index)
            // Settings page MUST come before namespace handler (which matches /{namespace})
            .service(view::settings::settings_page)
            .service(view::settings::update_email)
            .service(view::settings::delete_repo)
            .service(view::settings::delete_namespace)
            .service(view::namespace::handler)
            .service(view::namespace::create_repo_form_handler)
            .service(view::namespace::create_repo_handler)
            .service(view::repo::handler)
            .service(view::repo::tab_handler)
            .service(view::repo::slide_handler)
            .service(view::repo::markdown_handler)
            // Git endpoints with auth
            .service(git::repo::init)
            .route(
                "/{namespace}/{repo}/{endpoint:.*}",
                web::get().guard(is_git()).to(git::git_handler),
            )
            .route(
                "/{namespace}/{repo}/{endpoint:.*}",
                web::post().guard(is_git()).to(git::git_handler),
            )
    })
    .bind(bind_address)?
    .run()
    .await
}

fn is_git() -> impl guard::Guard {
    guard::fn_guard(|ctx| {
        ctx.head()
            .headers
            .get("User-Agent")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ua| ua.starts_with("git/"))
    })
}

// ========== ACTIX API TESTS ==========
// These tests replace the hurl tests with native Actix-web tests

#[cfg(test)]
mod tests {
    use super::*;
    use actix_http::Request;
    use actix_web::{http::StatusCode, test, web};

    // Health endpoint test
    #[actix_web::test]
    async fn test_health_endpoint() {
        let config = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/tmp/test_git".to_string(),
            "/tmp/test_fig.db".to_string(),
            "secure".to_string(),
            true,
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
                .service(health)
                .service(up),
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
        let config = config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            "/tmp/test_git".to_string(),
            "/tmp/test_fig.db".to_string(),
            "secure".to_string(),
            true,
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
                .service(health)
                .service(up)
                .service(assets::assets)
                .service(view::auth::ticket_page)
                .service(view::auth::signup_page)
                .service(view::auth::login_page)
                .service(view::auth::namespace_page)
                .service(auth::handlers::create_ticket_ui_handler)
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
                .service(view::repo::markdown_handler),
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
    async fn test_ticket_page_loads() {
        let app = create_test_service().await;
        let req = test::TestRequest::get().uri("/auth/ticket").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Get Signup Ticket"));
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
        assert!(body_str.contains("Signup Ticket"));
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
    async fn test_generate_ticket_with_invalid_api_key() {
        let app = create_test_service().await;
        let req = test::TestRequest::post()
            .uri("/auth/ticket")
            .set_form(&[("api_key", "invalid_key")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Invalid API key"));
    }

    #[actix_web::test]
    async fn test_generate_ticket_with_valid_api_key() {
        let app = create_test_service().await;
        let req = test::TestRequest::post()
            .uri("/auth/ticket")
            .set_form(&[("api_key", "secure")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Ticket Generated!"));
    }

    #[actix_web::test]
    async fn test_signup_with_invalid_ticket() {
        let app = create_test_service().await;
        let req = test::TestRequest::post()
            .uri("/auth/signup")
            .set_form(&[
                ("ticket", "invalid-ticket-code"),
                ("username", "uiuser"),
                ("email", "uiuser@example.com"),
                ("password", "password123"),
            ])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("Invalid ticket"));
    }

    #[actix_web::test]
    async fn test_login_with_wrong_password() {
        let app = create_test_service().await;
        let req = test::TestRequest::post()
            .uri("/auth/login")
            .set_form(&[("username", "nonexistent"), ("password", "wrongpassword")])
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
            .set_form(&[("name", "shouldfail_ns")])
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
            .set_form(&[("email", "invalid-email")])
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = test::read_body(resp).await;
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("valid email address"));
    }
}
