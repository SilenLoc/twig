use actix_web::{App, HttpResponse, HttpServer, Responder, get, web};
use env_logger::Env;
use log::info;

mod assets;
mod config;
mod git;

#[get("/health")]
async fn health() -> impl Responder {
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
            .service(assets::assets)
    })
    .bind(bind_address)?
    .run()
    .await
}
