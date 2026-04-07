use actix_web::{HttpRequest, HttpResponse, Responder, get};

const TCSS: &str = include_str!("../assets/t.css");
const HTMX: &str = include_str!("../assets/h.js");
const HTMX_RESPONSE_TARGETS: &str = include_str!("../assets/hx-response-targets.js");

// Cache static assets for 1 year (in seconds)
const CACHE_CONTROL_VALUE: &str = "public, max-age=31536000, immutable";

#[get("/assets/{filename:.*}")]
pub async fn assets(req: HttpRequest) -> impl Responder {
    let path = req.match_info().query("filename");

    match path {
        "t.css" => HttpResponse::Ok()
            .content_type("text/css; charset=utf-8")
            .insert_header(("Cache-Control", CACHE_CONTROL_VALUE))
            .body(TCSS),
        "h.js" => HttpResponse::Ok()
            .content_type("application/javascript; charset=utf-8")
            .insert_header(("Cache-Control", CACHE_CONTROL_VALUE))
            .body(HTMX),
        "hx-response-targets.js" => HttpResponse::Ok()
            .content_type("application/javascript; charset=utf-8")
            .insert_header(("Cache-Control", CACHE_CONTROL_VALUE))
            .body(HTMX_RESPONSE_TARGETS),
        _ => HttpResponse::NotFound().body("Not found"),
    }
}
