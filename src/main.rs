use actix_web::{
    App, HttpResponse, HttpServer, Responder, get, guard,
    web::{self},
};
use env_logger::Env;
use log::info;

mod assets;
mod config;
mod git;
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

    let bind_address = config.address();

    HttpServer::new(move || {
        App::new()
            .app_data(config.clone())
            .service(health)
            .service(up)
            .service(assets::assets)
            .service(git::repo::init)
            .route(
                "/{namespace}/{repo}/{endpoint:.*}",
                web::get().guard(is_git()).to(git::git_handler),
            )
            .route(
                "/{namespace}/{repo}/{endpoint:.*}",
                web::post().guard(is_git()).to(git::git_handler),
            )
            .service(view::repo::handler)
            .service(view::namespace::handler)
            .service(view::index)
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
