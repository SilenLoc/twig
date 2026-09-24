use actix_web::http::header::HeaderMap;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use argon2::password_hash::rand_core::RngCore;
use qrcode::QrCode;
use qrcode::render::svg;
use rand::rngs::OsRng;
use serde::Deserialize;

use super::session_auth::get_username_from_request;
use crate::auth::FigContext;
use crate::config;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TestPageQuery {
    pub pin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallerRole {
    Admin { username: String },
    Guest { pin: String },
}

impl CallerRole {
    #[must_use]
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Admin { .. })
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::Admin { username } => format!("ADMIN: {username}"),
            Self::Guest { pin } => format!("GUEST (PIN: {pin})"),
        }
    }

    #[must_use]
    pub fn username(&self) -> Option<&str> {
        match self {
            Self::Admin { username } => Some(username.as_str()),
            Self::Guest { .. } => None,
        }
    }
}

#[must_use]
pub fn generate_random_pin() -> String {
    let mut buf = [0u8; 4];
    OsRng.fill_bytes(&mut buf);
    let num = (u32::from_be_bytes(buf) % 900_000) + 100_000;
    format!("{num:06}")
}

pub fn generate_qr_svg(url: &str) -> Result<String, String> {
    let code = QrCode::new(url.as_bytes()).map_err(|e| e.to_string())?;
    let svg_str = code
        .render::<svg::Color>()
        .min_dimensions(180, 180)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Ok(svg_str)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestEndpoint {
    pub id: &'static str,
    pub category: &'static str,
    pub method: &'static str,
    pub url: String,
}

fn core_system_endpoints() -> Vec<TestEndpoint> {
    vec![
        TestEndpoint {
            id: "health",
            category: "System",
            method: "GET",
            url: "/health".to_string(),
        },
        TestEndpoint {
            id: "up",
            category: "System",
            method: "GET",
            url: "/up".to_string(),
        },
        TestEndpoint {
            id: "tree",
            category: "API",
            method: "GET",
            url: "/api/tree".to_string(),
        },
    ]
}

fn docs_endpoints() -> Vec<TestEndpoint> {
    vec![
        TestEndpoint {
            id: "info",
            category: "Docs",
            method: "GET",
            url: "/_info".to_string(),
        },
        TestEndpoint {
            id: "info-about",
            category: "Docs",
            method: "GET",
            url: "/_info?tab=about".to_string(),
        },
        TestEndpoint {
            id: "info-docs",
            category: "Docs",
            method: "GET",
            url: "/_info?tab=docs".to_string(),
        },
        TestEndpoint {
            id: "info-git",
            category: "Docs",
            method: "GET",
            url: "/_info?tab=docs&page=git-backend".to_string(),
        },
        TestEndpoint {
            id: "info-ui",
            category: "Docs",
            method: "GET",
            url: "/_info?tab=docs&page=ui".to_string(),
        },
        TestEndpoint {
            id: "info-env",
            category: "Docs",
            method: "GET",
            url: "/_info?tab=docs&page=environment-variables".to_string(),
        },
    ]
}

fn asset_endpoints() -> Vec<TestEndpoint> {
    vec![
        TestEndpoint {
            id: "css-fig",
            category: "Assets",
            method: "GET",
            url: "/assets/fig.css".to_string(),
        },
        TestEndpoint {
            id: "css-t",
            category: "Assets",
            method: "GET",
            url: "/assets/t.css".to_string(),
        },
        TestEndpoint {
            id: "js-h",
            category: "Assets",
            method: "GET",
            url: "/assets/h.js".to_string(),
        },
        TestEndpoint {
            id: "svg-fig",
            category: "Assets",
            method: "GET",
            url: "/assets/fig.svg".to_string(),
        },
    ]
}

fn ui_endpoints() -> Vec<TestEndpoint> {
    vec![
        TestEndpoint {
            id: "overview",
            category: "UI",
            method: "GET",
            url: "/".to_string(),
        },
        TestEndpoint {
            id: "overview-search",
            category: "UI",
            method: "GET",
            url: "/?q=test".to_string(),
        },
    ]
}

fn feature_endpoints() -> Vec<TestEndpoint> {
    vec![
        TestEndpoint {
            id: "test-ping",
            category: "Features",
            method: "GET",
            url: "/_test/ping".to_string(),
        },
        TestEndpoint {
            id: "test-feature-check",
            category: "Features",
            method: "GET",
            url: "/_test/feature-check".to_string(),
        },
    ]
}

pub fn list_test_endpoints() -> Vec<TestEndpoint> {
    let mut endpoints = core_system_endpoints();
    endpoints.extend(docs_endpoints());
    endpoints.extend(asset_endpoints());
    endpoints.extend(ui_endpoints());
    endpoints.extend(feature_endpoints());
    endpoints
}

fn render_pagehead(role: &CallerRole) -> maud::Markup {
    maud::html! {
        section class="fig-pagehead" {
            nav class="fig-crumbs fig-crumbs--page" aria-label="Breadcrumb" {
                a href="/" { "Fig" }
                span class="fig-crumbs-sep" aria-hidden="true" { "/" }
                span aria-current="page" { "Test Suite" }
            }
            div class="fig-cluster" {
                div {
                    p class="fig-eyebrow" { (role.display_name()) }
                    h1 class="fig-title" { "Test Suite & Rapid Endpoint Runner" }
                }
                div class="fig-cluster" {
                    button id="btn-start"
                        class="fig-btn fig-btn--primary"
                        hx-get="/_test/runner"
                        hx-target="#test-runner-container"
                        hx-swap="innerHTML" {
                        "Start"
                    }
                    button id="btn-stop"
                        class="fig-btn fig-btn--ghost"
                        hx-get="/_test/stopped"
                        hx-target="#test-runner-container"
                        hx-swap="innerHTML" {
                        "Stop"
                    }
                }
            }
        }
    }
}

pub fn render_pin_panel(is_admin: bool, active_pin: Option<&str>, base_url: &str) -> maud::Markup {
    maud::html! {
        section id="pin-management-panel" class="fig-panel" aria-label="Multi-user PIN and QR code access" {
            div class="fig-panel-head" {
                span class="fig-eyebrow" { "MULTI-USER ACCESS" }
                span class="fig-title" { "Session PIN & QR Code" }
            }
            div class="fig-panel-body" {
                @if let Some(pin) = active_pin {
                    div class="fig-stack" {
                        div class="fig-cluster" {
                            div {
                                p class="fig-label" { "Active Session PIN" }
                                p id="active-pin-display" class="fig-data-lg" { (pin) }
                            }
                            @if is_admin {
                                button id="btn-remove-pin"
                                    class="fig-btn fig-btn--danger"
                                    hx-post="/_test/pin/remove"
                                    hx-target="#pin-management-panel"
                                    hx-swap="outerHTML" {
                                    "Remove PIN"
                                }
                            }
                        }

                        @let redirect_url = format!("{base_url}/_test?pin={pin}");
                        div class="fig-panel" {
                            div class="fig-panel-body" {
                                div class="fig-cluster" {
                                    @if let Ok(qr_svg) = generate_qr_svg(&redirect_url) {
                                        div id="qr-code-container" class="fig-panel" {
                                            (maud::PreEscaped(qr_svg))
                                        }
                                    }
                                    div class="fig-stack fig-stack--tight" {
                                        p class="fig-eyebrow" { "SCAN TO JOIN" }
                                        p class="fig-body-sm" {
                                            "Scan this QR code or use the link below to allow multiple users to run tests simultaneously. Access is revoked when the admin removes the PIN."
                                        }
                                        p class="fig-label" { "Redirect Link:" }
                                        a id="pin-direct-link" class="fig-data" href=(redirect_url) { (redirect_url) }
                                    }
                                }
                            }
                        }
                    }
                } @else if is_admin {
                    div class="fig-cluster" {
                        div class="fig-stack fig-stack--tight" {
                            p class="fig-body-sm" {
                                "No PIN active. Create a session PIN to let multiple users access and run the test suite concurrently."
                            }
                        }
                        button id="btn-create-pin"
                            class="fig-btn fig-btn--primary"
                            hx-post="/_test/pin/create"
                            hx-target="#pin-management-panel"
                            hx-swap="outerHTML" {
                            "Create Session PIN & QR Code"
                        }
                    }
                } @else {
                    p class="fig-body-sm" {
                        "Connected via Session PIN. You can run all test endpoints concurrently until the admin ends the session."
                    }
                }
            }
        }
    }
}

#[must_use]
pub fn render_pin_entry_page(admin_user: &str, has_active_pin: bool) -> maud::Markup {
    maud::html! {
        div class="fig-stack" {
            div class="fig-notice fig-notice--warn" role="alert" {
                p class="fig-eyebrow" { "ACCESS RESTRICTED" }
                p class="fig-notice-body" {
                    (format!("The test suite is restricted to admin '{admin_user}' or users with an active session PIN."))
                }
            }

            section class="fig-panel" {
                div class="fig-panel-head" {
                    span class="fig-eyebrow" { "SESSION PIN" }
                    span class="fig-title" { "Enter PIN to Join" }
                }
                div class="fig-panel-body" {
                    @if has_active_pin {
                        form method="GET" action="/_test" class="fig-form fig-form--narrow" {
                            div class="fig-field" {
                                label class="fig-label" for="pin-input" { "Enter 6-digit Session PIN" }
                                input id="pin-input" class="fig-input fig-input--mono" type="text" name="pin" placeholder="e.g. 123456" required;
                            }
                            div class="fig-form-actions" {
                                button class="fig-btn fig-btn--primary" type="submit" { "Join Session" }
                                a class="fig-btn fig-btn--ghost" href="/auth/login" { "Log in as Admin" }
                            }
                        }
                    } @else {
                        div class="fig-empty fig-empty--void" {
                            p class="fig-eyebrow" { "NO ACTIVE SESSION" }
                            p class="fig-empty-body" {
                                "There is currently no active session PIN. Please ask the admin to generate a PIN on the test page, or log in as admin."
                            }
                            div class="fig-cluster" {
                                a class="fig-btn fig-btn--primary" href="/auth/login" { "Log in as Admin" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[must_use]
pub fn render_test_page(
    role: &CallerRole,
    active_pin: Option<&str>,
    base_url: &str,
    data_enabled: bool,
) -> maud::Markup {
    maud::html! {
        (super::tree::render_tree_hub(true, data_enabled, Some("test")))
        (render_pagehead(role))
        (render_pin_panel(role.is_admin(), active_pin, base_url))
        div id="test-runner-container" {
            (render_idle_content())
        }
    }
}

#[must_use]
pub fn render_idle_content() -> maud::Markup {
    maud::html! {
        div id="test-runner-idle" class="fig-panel" {
            div class="fig-panel-head" {
                span class="fig-eyebrow" { "IDLE" }
                span class="fig-title" { "Test run has not started" }
            }
            div class="fig-panel-body" {
                p class="fig-body-sm" {
                    "Press Start to run every read-only endpoint and feature check. Press Stop at any time to interrupt the run."
                }
            }
        }
    }
}

#[must_use]
pub fn render_stopped_content() -> maud::Markup {
    maud::html! {
        div id="test-runner-stopped" class="fig-panel" {
            div class="fig-panel-head" {
                span class="fig-eyebrow" { "STOPPED" }
                span class="fig-title" { "Test run interrupted" }
            }
            div class="fig-panel-body" {
                p class="fig-body-sm" {
                    "The run has been stopped. Press Start to run the endpoints again."
                }
            }
        }
    }
}

pub fn render_runner_content() -> maud::Markup {
    let endpoints = list_test_endpoints();
    maud::html! {
        div id="test-runner"
            class="fig-stack"
            hx-get="/_test/runner"
            hx-trigger="every 25ms"
            hx-target="#test-runner"
            hx-swap="outerHTML" {
            div class="fig-cluster" {
                span class="fig-label" { "Runner active: triggering all read-only endpoints every 5ms (container polling every 25ms)" }
            }
            div class="fig-bento" {
                @for ep in endpoints {
                    div class="fig-panel" {
                        div class="fig-panel-head" {
                            span class="fig-eyebrow" { (ep.category) " · " (ep.method) }
                            span class="fig-title" { (ep.url) }
                        }
                        div class="fig-panel-body"
                            hx-get=(ep.url)
                            hx-trigger="load, every 5ms"
                            hx-swap="none"
                            data-ep-id=(ep.id) {
                            span id=(format!("res-{}", ep.id)) class="fig-data" {
                                "Active"
                            }
                        }
                    }
                }
            }
        }
    }
}

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
    auth_state: &web::Data<FigContext>,
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
            super::render_error(
                "Forbidden: Test suite requires admin login or a valid session PIN.",
            )
            .into_string(),
        ));
    }

    let has_active_pin = auth_state.get_test_pin().await.is_some();
    let content = render_pin_entry_page(admin_user, has_active_pin);
    Err(HttpResponse::Forbidden().body(
        super::render_layout(&content, username.as_deref(), Some("Test Suite Access"))
            .into_string(),
    ))
}

async fn verify_admin_only(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<FigContext>,
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
            super::render_error("Forbidden: Only the admin user can manage session PINs.")
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
    auth_state: web::Data<FigContext>,
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
        builder
            .body(super::render_layout(&content, role.username(), Some("Test Suite")).into_string())
    }
}

#[get("/_test")]
pub async fn test_page_alias(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    query: web::Query<TestPageQuery>,
) -> HttpResponse {
    handle_test_page(req, server, auth_state, query).await
}

#[post("/_test/pin/create")]
pub async fn test_pin_create(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
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
    auth_state: web::Data<FigContext>,
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
    auth_state: web::Data<FigContext>,
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
    auth_state: web::Data<FigContext>,
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
    auth_state: web::Data<FigContext>,
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
    auth_state: web::Data<FigContext>,
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
    auth_state: web::Data<FigContext>,
    query: web::Query<TestPageQuery>,
) -> HttpResponse {
    if let Err(res) = authenticate_caller(&req, &server, &auth_state, query.pin.as_deref()).await {
        return res;
    }

    let markup = maud::html! {
        span class="fig-data" { "all systems operational" }
    };
    HttpResponse::Ok().body(markup.into_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_stopped_content() {
        let markup = render_stopped_content().into_string();
        assert!(markup.contains("id=\"test-runner-stopped\""));
        assert!(markup.contains("Test run interrupted"));
    }

    #[test]
    fn test_render_test_page_is_idle_until_started() {
        let role = CallerRole::Admin {
            username: "admin".to_string(),
        };
        let markup = render_test_page(&role, None, "http://localhost:8080", false).into_string();
        assert!(markup.contains("id=\"test-runner-container\""));
        assert!(markup.contains("id=\"btn-start\""));
        assert!(markup.contains("id=\"btn-stop\""));
        assert!(markup.contains("hx-get=\"/_test/runner\""));
        assert!(markup.contains("hx-get=\"/_test/stopped\""));
        assert!(markup.contains("id=\"test-runner-idle\""));
        assert!(!markup.contains("id=\"test-runner\""));
        assert!(!markup.contains("hx-trigger=\"every 25ms\""));
    }

    #[test]
    fn test_endpoint_catalog_uses_current_api_and_info_routes() {
        let endpoints = list_test_endpoints();
        let urls: Vec<_> = endpoints
            .iter()
            .map(|endpoint| endpoint.url.as_str())
            .collect();
        assert!(urls.contains(&"/api/tree"));
        assert!(urls.contains(&"/_info?tab=docs&page=git-backend"));
        assert!(urls.contains(&"/_info?tab=docs&page=ui"));
        assert!(urls.contains(&"/_info?tab=docs&page=environment-variables"));
        assert!(
            !urls
                .iter()
                .any(|url| url.contains("/api/v1/") || url.contains("doc="))
        );
    }
}
