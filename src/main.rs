use actix_web::{
    App, HttpServer,
    dev::Service,
    guard,
    web::{self},
};
use db::Database;
use env_logger::Env;
use log::{info, warn};
use sentry::integrations::log::LogFilter;

mod api;
mod assets;
mod auth;
mod config;
mod db;
mod git;
mod git_backend;
mod health;
mod info;
mod integration_tests;
mod md;
mod view;

/// Initialises Sentry before the async runtime starts, as the SDK requires.
/// The DSN is read from `SENTRY_DSN`; when it is unset `sentry::init` is a
/// no-op and the application runs uninstrumented.
fn init_sentry(config: &config::Server) -> sentry::ClientInitGuard {
    sentry::init(
        sentry::ClientOptions::new()
            .maybe_release(sentry::release_name!())
            .send_default_pii(true)
            .max_request_body_size(sentry::MaxRequestBodySize::Always)
            // Capture all traces/spans; lower this in production if needed.
            .traces_sample_rate(config.traces_sample_rate())
            .enable_logs(true)
            // errors and warns become events + logs; everything else is a breadcrumb + log
            .before_send_log(|log| {
                if log.level == sentry::protocol::LogLevel::Trace {
                    return None;
                }
                Some(log)
            }),
    )
}

/// Wraps `env_logger` with `SentryLogger` so every `log::*` call reaches both
/// the console and Sentry (as logs, plus breadcrumbs/events for errors).
fn init_logging(config: &config::Server) {
    let log_filter = format!(
        "{},libsql=warn,turso=warn,tracing::span=warn",
        config.log_level()
    );

    let env_logger =
        env_logger::Builder::from_env(Env::default().default_filter_or(log_filter)).build();
    let logger = sentry::integrations::log::SentryLogger::with_dest(env_logger).filter(|log| {
        if log.level() == log::Level::Error || log.level() == log::Level::Warn {
            LogFilter::Event | LogFilter::Log
        } else if log.target().starts_with("libsql")
            || log.target().starts_with("turso")
            || log.target().starts_with("tracing::span")
        {
            LogFilter::Log
        } else {
            LogFilter::Breadcrumb | LogFilter::Log
        }
    });
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
    auth_state: web::Data<auth::FigContext>,
    config: web::Data<config::Server>,
    dev_session: web::Data<tokio::sync::OnceCell<String>>,
) {
    actix_web::rt::spawn(async move {
        loop {
            match auth_state.db().init_tables().await {
                Ok(()) => {
                    info!("Database initialized successfully");
                    auth_state.set_initialized();

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

/// Registers every route, grouped and ordered exactly as the inline chain
/// used to be. Order still matters: static/UI routes must be registered
/// before the dynamic `/{namespace}` and `/{namespace}/{repo}` patterns they
/// would otherwise be shadowed by.
pub(crate) fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health)
        .service(health::up)
        .service(assets::assets)
        .service(api::tree_endpoint)
        .service(view::info::index)
        // Auth UI endpoints (HTML forms)
        .service(view::auth::invite_page)
        .service(view::auth::signup_page)
        .service(view::auth::login_page)
        .service(view::auth::namespace_page)
        .service(auth::handlers::create_invite_ui_handler)
        .service(auth::handlers::signup_ui_handler)
        .service(auth::handlers::login_ui_handler)
        .service(auth::handlers::create_namespace_ui_handler)
        .service(auth::handlers::logout_ui_handler)
        // Web UI endpoints (MUST come before git routes to avoid pattern conflicts)
        .service(view::overview::index)
        // Settings page MUST come before namespace handler (which matches /{namespace})
        .service(view::settings::settings_page)
        .service(view::settings::update_email)
        .service(view::settings::move_repo)
        .service(view::settings::delete_repo)
        .service(view::settings::delete_namespace)
        .service(view::namespace::handler)
        .service(view::namespace::create_repo_form_handler)
        .service(view::namespace::create_repo_handler)
        .service(view::repo::handler)
        .service(view::repo::tab_handler)
        .service(view::repo::slide_handler)
        .service(view::repo::markdown_handler)
        .service(view::repo::content_handler)
        // Git endpoints with auth
        .service(git::repo::init)
        .route(
            "/{namespace}/{repo}/{endpoint:.*}",
            web::get().guard(is_git()).to(git::git_handler),
        )
        .route(
            "/{namespace}/{repo}/{endpoint:.*}",
            web::post().guard(is_git()).to(git::git_handler),
        );
}

fn main() -> std::io::Result<()> {
    let config = config::from_env();
    let _sentry_guard = init_sentry(&config);
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

        let auth_state = web::Data::new(auth::FigContext::new(db, api_key));
        let dev_session_token: web::Data<tokio::sync::OnceCell<String>> =
            web::Data::new(tokio::sync::OnceCell::new());

        spawn_database_init(
            auth_state.clone(),
            config.clone(),
            dev_session_token.clone(),
        );

        let bind_address = config.address();

        HttpServer::new(move || {
            let reset_db = config.reset_db();
            let dev_session_for_mw = dev_session_token.clone();

            App::new()
                .app_data(config.clone())
                .app_data(auth_state.clone())
                // Increase payload limit to 512MB for large git pushes
                .app_data(web::PayloadConfig::new(1 << 29))
                // Dev convenience: when RESET_DB is true, auto-attach the seeded
                // admin session cookie to any request that doesn't already carry
                // one, so local browsing never requires a manual login.
                .wrap_fn(move |mut req, srv| {
                    if reset_db
                        && req.cookie("session").is_none()
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
                // Sentry middleware: capture server errors and start a transaction
                // per request. Added last so it is the outermost wrap (first to process).
                .wrap(
                    sentry::integrations::actix::Sentry::builder()
                        .capture_server_errors(true)
                        .start_transaction(true)
                        .finish(),
                )
                .configure(configure_routes)
        })
        .bind(bind_address)?
        .run()
        .await
    })
}

pub(crate) fn is_git() -> impl guard::Guard {
    guard::fn_guard(|ctx| {
        ctx.head()
            .headers
            .get("User-Agent")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ua| ua.starts_with("git/"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{HttpResponse, test as aw_test, web};

    #[actix_web::test]
    async fn test_is_git_guard_matches_git_user_agent() {
        let app = aw_test::init_service(
            actix_web::App::new().route(
                "/{namespace}/{repo}/{endpoint:.*}",
                web::get()
                    .guard(is_git())
                    .to(|| async { HttpResponse::Ok().body("git") }),
            ),
        )
        .await;

        let req = aw_test::TestRequest::get()
            .uri("/ns/repo/info/refs")
            .insert_header(("User-Agent", "git/2.43.0"))
            .to_request();
        let resp = aw_test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_is_git_guard_rejects_non_git_user_agent() {
        let app = aw_test::init_service(
            actix_web::App::new().route(
                "/{namespace}/{repo}/{endpoint:.*}",
                web::get()
                    .guard(is_git())
                    .to(|| async { HttpResponse::Ok().body("git") }),
            ),
        )
        .await;

        let req = aw_test::TestRequest::get()
            .uri("/ns/repo/info/refs")
            .insert_header(("User-Agent", "Mozilla/5.0"))
            .to_request();
        let resp = aw_test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_is_git_guard_rejects_missing_user_agent() {
        let app = aw_test::init_service(
            actix_web::App::new().route(
                "/{namespace}/{repo}/{endpoint:.*}",
                web::get()
                    .guard(is_git())
                    .to(|| async { HttpResponse::Ok().body("git") }),
            ),
        )
        .await;

        let req = aw_test::TestRequest::get()
            .uri("/ns/repo/info/refs")
            .to_request();
        let resp = aw_test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }
}
