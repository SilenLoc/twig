use actix_web::{
    App, HttpResponse, HttpServer, Responder, get, guard,
    web::{self},
};
use env_logger::Env;
use log::info;

mod assets;
mod auth;
mod config;
mod db;
mod git;
mod git_backend;
mod view;

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok()
}

#[get("/up")]
async fn up() -> impl Responder {
    HttpResponse::Ok()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = config::from_env();

    env_logger::Builder::from_env(Env::default().default_filter_or(config.log_level())).init();

    info!("{config}");

    let config = web::Data::new(config);

    // Initialize auth state
    let db_path = config.db_path().to_string();

    // Check if we should reset the database
    if config.reset_db() {
        log::warn!(
            "RESET_DB is set to true, deleting database file: {}",
            db_path
        );
        if std::path::Path::new(&db_path).exists() {
            if let Err(e) = std::fs::remove_file(&db_path) {
                log::error!("Failed to delete database file: {}", e);
            } else {
                log::info!("Database file deleted successfully");
            }
        }
    }

    let api_key = if config.api_key().is_empty() {
        // Generate a random API key if not provided
        let key = auth::generate_token();
        log::warn!("No API_KEY set, using generated key: {}", key);
        key
    } else {
        config.api_key().to_string()
    };

    let auth_state = match auth::AuthState::new(&db_path, api_key).await {
        Ok(state) => web::Data::new(state),
        Err(e) => {
            log::error!("Failed to initialize auth state: {}", e);
            return Err(std::io::Error::other(e));
        }
    };

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
            .service(view::namespace::handler)
            .service(view::namespace::create_repo_form_handler)
            .service(view::namespace::create_repo_handler)
            .service(view::repo::handler)
            .service(view::repo::tab_handler)
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
