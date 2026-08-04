use actix_web::{HttpResponse, Responder, get};

#[get("/health")]
pub async fn health() -> impl Responder {
    HttpResponse::Ok()
}

#[get("/up")]
pub async fn up() -> impl Responder {
    HttpResponse::Ok().finish()
}
