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

    let db = Database::new(&db_path)
        .await
        .expect("Failed to initialize database");

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
