use actix_identity::IdentityMiddleware;
use actix_web::{
    App, HttpServer,
    dev::Service,
    web::{self},
};
use db::Database;
use env_logger::Env;
use log::{info, warn};

mod auth;
mod config;
mod db;
mod email;
mod git;
mod http;
mod info;
mod integration_tests;
mod md;

fn init_logging(config: &config::Server) {
    let log_filter = format!(
        "{},libsql=warn,turso=warn,tracing::span=warn",
        config.log_level()
    );

    let logger =
        env_logger::Builder::from_env(Env::default().default_filter_or(log_filter)).build();
    log::set_boxed_logger(Box::new(logger)).expect("install logger");
    let max_level = config
        .log_level()
        .parse::<log::LevelFilter>()
        .unwrap_or(log::LevelFilter::Info);
    log::set_max_level(max_level);
}

/// Retries `init_tables` in the background until it succeeds, so a database
/// that is not ready yet does not stop the server from accepting connections.
///
/// When `RESET_DB` is true, also seeds a dev `admin`/`admin` user once tables
/// are ready, storing the resulting session token in `dev_session` so the
/// auto-login middleware can pick it up for local browsing.
fn spawn_database_init(
    auth_state: web::Data<auth::TwigContext>,
    config: web::Data<config::Server>,
    dev_session: web::Data<tokio::sync::OnceCell<String>>,
) {
    actix_web::rt::spawn(async move {
        loop {
            match auth_state.db().init_tables().await {
                Ok(()) => {
                    info!("Database initialized successfully");

                    if config.reset_db() {
                        match auth::seed_dev_admin(&auth_state, config.project_root()).await {
                            Ok(token) => {
                                warn!(
                                    "RESET_DB is true: auto-logging in as 'admin' (password 'admin') for local browsing"
                                );
                                let _ = dev_session.set(token);
                            }
                            Err(e) => warn!("Failed to seed dev admin user: {e}"),
                        }
                    }

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

fn main() -> std::io::Result<()> {
    let config = config::from_env();
    init_logging(&config);

    actix_web::rt::System::new().block_on(async move {
        info!("{config}");

        let config = web::Data::new(config);

        // Check if we should reset the database
        config.maybe_reset_database();

        // Initialize database
        let db_path = config.db_path().to_string();
        let api_key = config.effective_api_key();

        let db = Database::new(&db_path);

        let auth_state = web::Data::new(auth::TwigContext::new(db, api_key));
        let dev_session_token: web::Data<tokio::sync::OnceCell<String>> =
            web::Data::new(tokio::sync::OnceCell::new());

        spawn_database_init(
            auth_state.clone(),
            config.clone(),
            dev_session_token.clone(),
        );

        let bind_address = config.address();
        let session_key = config.session_key();

        HttpServer::new(move || {
            let reset_db = config.reset_db();
            let dev_session_for_mw = dev_session_token.clone();
            let session_key = session_key.clone();

            App::new()
                .app_data(config.clone())
                .app_data(auth_state.clone())
                // Shared payload ceiling for large Git pushes and binary uploads.
                .app_data(web::PayloadConfig::new(1 << 29))
                .wrap(IdentityMiddleware::default())
                .wrap(auth::session_store::middleware(
                    auth_state.db().clone(),
                    session_key,
                ))
                // Dev convenience: when RESET_DB is true, auto-attach the seeded
                // admin session cookie to any request that doesn't already carry
                // one, so local browsing never requires a manual login.
                .wrap_fn(move |mut req, srv| {
                    if reset_db
                        && req.cookie("session").is_none()
                        && req.cookie("id").is_none()
                        && let Some(token) = dev_session_for_mw.get()
                    {
                        let cookie_header = format!("session={token}");
                        if let Ok(value) =
                            actix_web::http::header::HeaderValue::from_str(&cookie_header)
                        {
                            req.headers_mut()
                                .insert(actix_web::http::header::COOKIE, value);
                        }
                    }
                    srv.call(req)
                })
                .configure(http::routes::configure_routes)
        })
        .bind(bind_address)?
        .run()
        .await
    })
}
