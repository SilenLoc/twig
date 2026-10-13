use actix_web::http::header::HeaderMap;
use actix_web::{HttpRequest, HttpResponse, get, post, web};

use crate::auth::TwigContext;
use crate::config;
use crate::http::auth::session::get_username_from_request;

use super::views::{render_stopped_content, render_test_page};
use super::*;
use super::{render_pin_entry_page, render_pin_panel, render_runner_content};

fn get_base_url(req: &HttpRequest, server: &config::Server) -> String {
    let host = req.connection_info().host().to_string();
    let scheme = req.connection_info().scheme().to_string();
    if host.is_empty() || host.starts_with("0.0.0.0") {
        let (_, port) = server.address();
        format!("{scheme}://localhost:{port}")
    } else {
        format!("{scheme}://{host}")
    }
}

async fn authenticate_caller(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    query_pin: Option<&str>,
) -> Result<CallerRole, HttpResponse> {
    if !server.is_test_user_enabled() {
        return Err(HttpResponse::NotFound().body("Not Found"));
    }

    let Some(admin_user) = server.test_user() else {
        return Err(HttpResponse::NotFound().body("Not Found"));
    };

    let username = get_username_from_request(req, auth_state).await;
    if username
        .as_deref()
        .is_some_and(|name| server.is_test_user(name))
    {
        return Ok(CallerRole::Admin {
            username: admin_user.to_string(),
        });
    }

    let pin_from_cookie = req.cookie("test_pin").map(|c| c.value().to_string());
    let pin_from_header = req
        .headers()
        .get("X-Test-Pin")
        .and_then(|h| h.to_str().ok().map(String::from));

    let candidate_pin = query_pin
        .map(String::from)
        .or(pin_from_cookie)
        .or(pin_from_header);

    if let Some(pin) = candidate_pin
        && auth_state.validate_test_pin(&pin).await
    {
        return Ok(CallerRole::Guest { pin });
    }

    if is_hx_request(req.headers()) {
        return Err(HttpResponse::Forbidden().body(
            crate::http::view::render_error(
                "Forbidden: Test suite requires admin login or a valid session PIN.",
            )
            .into_string(),
        ));
    }

    let has_active_pin = auth_state.get_test_pin().await.is_some();
    let content = render_pin_entry_page(admin_user, has_active_pin);
    Err(HttpResponse::Forbidden().body(
        crate::http::view::render_layout(&content, username.as_deref(), Some("Test Suite Access"))
            .into_string(),
    ))
}

async fn verify_admin_only(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
) -> Result<String, HttpResponse> {
    if !server.is_test_user_enabled() {
        return Err(HttpResponse::NotFound().body("Not Found"));
    }

    let Some(admin_user) = server.test_user() else {
        return Err(HttpResponse::NotFound().body("Not Found"));
    };

    let username = get_username_from_request(req, auth_state).await;
    if !username
        .as_deref()
        .is_some_and(|name| server.is_test_user(name))
    {
        return Err(HttpResponse::Forbidden().body(
            crate::http::view::render_error(
                "Forbidden: Only the admin user can manage session PINs.",
            )
            .into_string(),
        ));
    }

    Ok(admin_user.to_string())
}

fn is_hx_request(headers: &HeaderMap) -> bool {
    headers.get("HX-Request").is_some()
}

async fn handle_test_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<TestPageQuery>,
) -> HttpResponse {
    let role = match authenticate_caller(&req, &server, &auth_state, query.pin.as_deref()).await {
        Ok(r) => r,
        Err(res) => return res,
    };

    let active_pin = auth_state.get_test_pin().await;
    let base_url = get_base_url(&req, &server);
    let data_enabled = role
        .username()
        .is_some_and(|username| server.is_configured_admin(username));
    let content = render_test_page(&role, active_pin.as_deref(), &base_url, data_enabled);

    let mut builder = HttpResponse::Ok();

    if let Some(ref pin) = query.pin
        && auth_state.validate_test_pin(pin).await
    {
        let cookie = actix_web::cookie::Cookie::build("test_pin", pin.clone())
            .path("/")
            .same_site(actix_web::cookie::SameSite::Lax)
            .http_only(true)
            .finish();
        builder.cookie(cookie);
    }

    if is_hx_request(req.headers()) {
        builder.body(content.into_string())
    } else {
        builder.body(
            crate::http::view::render_layout(&content, role.username(), Some("Test Suite"))
                .into_string(),
        )
    }
}

#[get("/_test")]
pub async fn test_page_alias(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<TestPageQuery>,
) -> HttpResponse {
    handle_test_page(req, server, auth_state, query).await
}

#[post("/_test/pin/create")]
pub async fn test_pin_create(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> HttpResponse {
    if let Err(res) = verify_admin_only(&req, &server, &auth_state).await {
        return res;
    }

    let pin = generate_random_pin();
    if let Err(e) = auth_state.set_test_pin(&pin).await {
        log::error!("Failed to save test pin to database: {e}");
        return HttpResponse::InternalServerError().body("Failed to save test pin");
    }
    let base_url = get_base_url(&req, &server);

    let panel = render_pin_panel(true, Some(&pin), &base_url);
    HttpResponse::Ok().body(panel.into_string())
}

#[post("/_test/pin/remove")]
pub async fn test_pin_remove(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> HttpResponse {
    if let Err(res) = verify_admin_only(&req, &server, &auth_state).await {
        return res;
    }

    if let Err(e) = auth_state.clear_test_pin().await {
        log::error!("Failed to remove test pin from database: {e}");
        return HttpResponse::InternalServerError().body("Failed to remove test pin");
    }
    let base_url = get_base_url(&req, &server);

    let panel = render_pin_panel(true, None, &base_url);
    HttpResponse::Ok().body(panel.into_string())
}

#[get("/_test/pin/qr")]
pub async fn test_pin_qr(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<TestPageQuery>,
) -> HttpResponse {
    if let Err(res) = authenticate_caller(&req, &server, &auth_state, query.pin.as_deref()).await {
        return res;
    }

    let Some(active_pin) = auth_state.get_test_pin().await else {
        return HttpResponse::NotFound().body("No active PIN");
    };

    let base_url = get_base_url(&req, &server);
    let redirect_url = format!("{base_url}/_test?pin={active_pin}");

    match generate_qr_svg(&redirect_url) {
        Ok(svg) => HttpResponse::Ok().content_type("image/svg+xml").body(svg),
        Err(e) => {
            log::debug!("Failed to generate QR SVG: {e}");
            HttpResponse::InternalServerError().body("QR generation failed")
        }
    }
}

#[get("/_test/runner")]
pub async fn test_runner(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<TestPageQuery>,
) -> HttpResponse {
    if let Err(res) = authenticate_caller(&req, &server, &auth_state, query.pin.as_deref()).await {
        return res;
    }

    HttpResponse::Ok().body(render_runner_content().into_string())
}

#[get("/_test/stopped")]
pub async fn test_stopped(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<TestPageQuery>,
) -> HttpResponse {
    if let Err(res) = authenticate_caller(&req, &server, &auth_state, query.pin.as_deref()).await {
        return res;
    }

    HttpResponse::Ok().body(render_stopped_content().into_string())
}

#[get("/_test/ping")]
pub async fn test_ping(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<TestPageQuery>,
) -> HttpResponse {
    if let Err(res) = authenticate_caller(&req, &server, &auth_state, query.pin.as_deref()).await {
        return res;
    }

    HttpResponse::Ok().body("pong")
}

#[get("/_test/feature-check")]
pub async fn test_feature_check(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<TestPageQuery>,
) -> HttpResponse {
    if let Err(res) = authenticate_caller(&req, &server, &auth_state, query.pin.as_deref()).await {
        return res;
    }

    let markup = maud::html! {
        span class="twig-data" { "all systems operational" }
    };
    HttpResponse::Ok().body(markup.into_string())
}
