use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use actix_session::Session;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;

use crate::{
    auth::{
        Namespace, NamespacePermission, TwigContext,
        device::{
            DeviceAuthorizationDecision, DeviceTokenPoll, format_user_code, hash_device_code,
            normalize_user_code,
        },
        permissions,
    },
    config,
    db::device_authorizations::DeviceAuthorizationStatus,
    http::{
        auth::session::device_return_to,
        view::{render_error, render_layout},
    },
};

const RATE_WINDOW: Duration = Duration::from_mins(1);
const MAX_DEVICE_STARTS_PER_WINDOW: usize = 10;
const MAX_CODE_LOOKUPS_PER_WINDOW: usize = 10;

#[derive(Clone, Default)]
pub struct DeviceAuthorizationRateLimiter {
    starts: Arc<Mutex<HashMap<IpAddr, VecDeque<Instant>>>>,
    code_lookups: Arc<Mutex<HashMap<IpAddr, VecDeque<Instant>>>>,
}

impl DeviceAuthorizationRateLimiter {
    pub fn allow_start(&self, peer: IpAddr, now: Instant) -> bool {
        allow_request(&self.starts, peer, now, MAX_DEVICE_STARTS_PER_WINDOW)
    }

    pub fn allow_code_lookup(&self, peer: IpAddr, now: Instant) -> bool {
        allow_request(&self.code_lookups, peer, now, MAX_CODE_LOOKUPS_PER_WINDOW)
    }
}

fn allow_request(
    buckets: &Mutex<HashMap<IpAddr, VecDeque<Instant>>>,
    peer: IpAddr,
    now: Instant,
    limit: usize,
) -> bool {
    let Ok(mut buckets) = buckets.lock() else {
        return false;
    };
    buckets.retain(|_, events| {
        prune_events(events, now);
        !events.is_empty()
    });
    let events = buckets.entry(peer).or_default();
    prune_events(events, now);
    if events.len() >= limit {
        return false;
    }
    events.push_back(now);
    true
}

fn prune_events(events: &mut VecDeque<Instant>, now: Instant) {
    while events
        .front()
        .is_some_and(|event| now.saturating_duration_since(*event) >= RATE_WINDOW)
    {
        events.pop_front();
    }
}

fn peer_ip(request: &HttpRequest) -> IpAddr {
    request
        .peer_addr()
        .map_or(IpAddr::V4(Ipv4Addr::LOCALHOST), |address| address.ip())
}

#[derive(Deserialize)]
struct DeviceCodeQuery {
    user_code: String,
}

#[derive(Deserialize)]
struct DeviceTokenRequest {
    device_code: String,
}

#[post("/api/auth/device")]
pub async fn start_device_authorization(
    request: HttpRequest,
    auth_state: web::Data<TwigContext>,
    rate_limiter: web::Data<DeviceAuthorizationRateLimiter>,
) -> HttpResponse {
    if !rate_limiter.allow_start(peer_ip(&request), Instant::now()) {
        return HttpResponse::TooManyRequests().finish();
    }

    match auth_state.start_device_authorization().await {
        Ok(challenge) => no_store(HttpResponse::Ok()).json(challenge),
        Err(error) => {
            log::error!("Failed to start device authorization: {error}");
            HttpResponse::InternalServerError().body("Failed to start device authorization.")
        }
    }
}

#[get("/auth/device")]
pub async fn device_authorization_page(
    request: HttpRequest,
    query: web::Query<DeviceCodeQuery>,
    session: Session,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    rate_limiter: web::Data<DeviceAuthorizationRateLimiter>,
) -> HttpResponse {
    if !rate_limiter.allow_code_lookup(peer_ip(&request), Instant::now()) {
        return no_store(HttpResponse::TooManyRequests()).finish();
    }
    let normalized_code = match normalize_user_code(&query.user_code) {
        Ok(code) => code,
        Err(message) => return no_store(HttpResponse::BadRequest()).body(message),
    };
    let user_code = format_user_code(&normalized_code);
    let code_hash = hash_device_code(&normalized_code);
    let authorization = match auth_state
        .db()
        .find_device_authorization_by_user_code_hash(&code_hash)
        .await
    {
        Ok(Some(authorization)) => authorization,
        Ok(None) => {
            return render_error_response(
                HttpResponse::NotFound(),
                "This Tree authorization code is invalid or expired.",
            );
        }
        Err(error) => {
            log::error!("Failed to load device authorization: {error}");
            return render_error_response(
                HttpResponse::InternalServerError(),
                "Failed to load the Tree authorization request.",
            );
        }
    };
    if authorization.status != DeviceAuthorizationStatus::Pending {
        return render_error_response(
            HttpResponse::Gone(),
            "This Tree authorization request is no longer pending.",
        );
    }

    let Some(user_id) = auth_state.user_id_from_request(&request).await else {
        return device_login_redirect(&user_code);
    };
    let user = match auth_state.db().get_user_by_id(&user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return render_error_response(
                HttpResponse::Unauthorized(),
                "Session expired. Please log in again.",
            );
        }
        Err(error) => {
            log::error!("Failed to load user for device authorization: {error}");
            return render_error_response(
                HttpResponse::InternalServerError(),
                "Failed to load your Twig account.",
            );
        }
    };
    let namespaces = match permissions::namespaces_for_user(
        auth_state.db(),
        &user.id,
        server.is_configured_admin(&user.username),
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(namespaces) => namespaces,
        Err(error) => {
            log::error!("Failed to load namespaces for device authorization: {error}");
            return render_error_response(
                HttpResponse::InternalServerError(),
                "Failed to load your Twig namespaces.",
            );
        }
    };
    if namespaces.is_empty() {
        return render_error_response(
            HttpResponse::Forbidden(),
            "Your account has no namespaces that Tree can access.",
        );
    }

    let csrf_token = crate::auth::generate_token();
    if let Err(error) = session.insert(csrf_session_key(&code_hash), &csrf_token) {
        log::error!("Failed to store device authorization CSRF state: {error}");
        return render_error_response(
            HttpResponse::InternalServerError(),
            "Failed to prepare the Tree authorization form.",
        );
    }

    let host = request.connection_info().host().to_owned();
    let content = render_approval_page(&user.username, &host, &user_code, &csrf_token, &namespaces);
    no_store(HttpResponse::Ok())
        .insert_header(("Referrer-Policy", "no-referrer"))
        .content_type("text/html")
        .body(render_layout(&content, Some(&user.username), Some("Authorize Tree")).into_string())
}

#[post("/auth/device/approve")]
pub async fn decide_device_authorization(
    request: HttpRequest,
    session: Session,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    rate_limiter: web::Data<DeviceAuthorizationRateLimiter>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    if !rate_limiter.allow_code_lookup(peer_ip(&request), Instant::now()) {
        return no_store(HttpResponse::TooManyRequests()).finish();
    }
    let raw_code = form.get("user_code").map_or("", String::as_str);
    let normalized_code = match normalize_user_code(raw_code) {
        Ok(code) => code,
        Err(message) => return render_error_response(HttpResponse::BadRequest(), &message),
    };
    let formatted_code = format_user_code(&normalized_code);
    let code_hash = hash_device_code(&normalized_code);
    let csrf_key = csrf_session_key(&code_hash);
    let expected_csrf = match session.get::<String>(&csrf_key) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return render_error_response(
                HttpResponse::Forbidden(),
                "The authorization form expired. Reload the page and try again.",
            );
        }
        Err(error) => {
            log::error!("Failed to read device authorization CSRF state: {error}");
            return render_error_response(
                HttpResponse::InternalServerError(),
                "Failed to verify the authorization form.",
            );
        }
    };
    if form.get("csrf_token").map(String::as_str) != Some(expected_csrf.as_str()) {
        return render_error_response(
            HttpResponse::Forbidden(),
            "The authorization form expired. Reload the page and try again.",
        );
    }
    session.remove(&csrf_key);

    let Some(user_id) = auth_state.user_id_from_request(&request).await else {
        return render_error_response(
            HttpResponse::Unauthorized(),
            "Please log in before approving Tree.",
        );
    };
    let decision = match form.get("decision").map(String::as_str) {
        Some("deny") => DeviceAuthorizationDecision::Deny,
        Some("approve") => {
            match approved_device_decision(&auth_state, &server, &user_id, &form).await {
                Ok(decision) => decision,
                Err(response) => return *response,
            }
        }
        _ => {
            return render_error_response(
                HttpResponse::BadRequest(),
                "Choose whether to approve or deny Tree.",
            );
        }
    };

    match auth_state
        .decide_device_authorization(&server, &user_id, &formatted_code, decision)
        .await
    {
        Ok(()) => no_store(HttpResponse::Ok())
            .insert_header(("Referrer-Policy", "no-referrer"))
            .content_type("text/html")
            .body(render_decision_result(&formatted_code).into_string()),
        Err(message) => render_error_response(HttpResponse::BadRequest(), &message),
    }
}

async fn approved_device_decision(
    auth_state: &TwigContext,
    server: &config::Server,
    user_id: &str,
    form: &HashMap<String, String>,
) -> Result<DeviceAuthorizationDecision, Box<HttpResponse>> {
    let user = match auth_state.db().get_user_by_id(user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err(Box::new(render_error_response(
                HttpResponse::Unauthorized(),
                "Session expired. Please log in again.",
            )));
        }
        Err(error) => {
            log::error!("Failed to load user for device approval: {error}");
            return Err(Box::new(render_error_response(
                HttpResponse::InternalServerError(),
                "Failed to load your Twig account.",
            )));
        }
    };
    let accessible = match permissions::namespaces_for_user(
        auth_state.db(),
        &user.id,
        server.is_configured_admin(&user.username),
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(namespaces) => namespaces,
        Err(error) => {
            log::error!("Failed to load accessible namespaces for Tree: {error}");
            return Err(Box::new(render_error_response(
                HttpResponse::InternalServerError(),
                "Failed to load your Twig namespaces.",
            )));
        }
    };
    let all_namespaces = form
        .get("all_namespaces")
        .is_some_and(|value| value == "true");
    let selected_ids = selected_form_values(form, "namespace_");
    let namespaces = permissions::select_namespaces(&accessible, all_namespaces, &selected_ids)
        .map_err(|message| Box::new(render_error_response(HttpResponse::BadRequest(), &message)))?;
    let selected_permission_names = selected_form_values(form, "permission_")
        .into_iter()
        .collect::<Vec<_>>();
    let selected_permissions = permissions::select_permissions(
        form.get("all_read").is_some_and(|value| value == "true"),
        form.get("all_write").is_some_and(|value| value == "true"),
        &selected_permission_names,
    )
    .map_err(|message| Box::new(render_error_response(HttpResponse::BadRequest(), &message)))?;
    Ok(DeviceAuthorizationDecision::Approve {
        namespace_ids: namespaces
            .into_iter()
            .map(|namespace| namespace.id)
            .collect(),
        permissions: selected_permissions,
    })
}

fn device_login_redirect(user_code: &str) -> HttpResponse {
    let return_to = device_return_to(user_code)
        .expect("normalized device code should produce a safe return target");
    let encoded_return_to = return_to
        .replace('/', "%2F")
        .replace('?', "%3F")
        .replace('=', "%3D");
    let location = format!("/auth/login?return_to={encoded_return_to}");
    no_store(HttpResponse::Found())
        .insert_header(("Location", location))
        .insert_header(("Referrer-Policy", "no-referrer"))
        .finish()
}

#[post("/api/auth/device/token")]
pub async fn poll_device_authorization(
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    request: web::Json<DeviceTokenRequest>,
) -> HttpResponse {
    if !is_device_code(&request.device_code) {
        return no_store(HttpResponse::Ok()).json(DeviceTokenPoll::Expired);
    }
    match auth_state
        .poll_device_authorization(&server, &request.device_code)
        .await
    {
        Ok(status) => no_store(HttpResponse::Ok()).json(status),
        Err(error) => {
            log::error!("Failed to poll Tree device authorization: {error}");
            HttpResponse::InternalServerError().body("Failed to check device authorization.")
        }
    }
}

fn selected_form_values(form: &HashMap<String, String>, prefix: &str) -> BTreeSet<String> {
    form.iter()
        .filter_map(|(key, value)| {
            (value == "true")
                .then(|| key.strip_prefix(prefix))
                .flatten()
                .map(str::to_owned)
        })
        .collect()
}

fn is_device_code(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn csrf_session_key(code_hash: &str) -> String {
    format!("tree-device-csrf:{code_hash}")
}

fn no_store(mut builder: actix_web::HttpResponseBuilder) -> actix_web::HttpResponseBuilder {
    builder.insert_header(("Cache-Control", "no-store"));
    builder
}

fn render_error_response(builder: actix_web::HttpResponseBuilder, message: &str) -> HttpResponse {
    no_store(builder)
        .content_type("text/html")
        .body(render_error(message).into_string())
}

fn render_approval_page(
    username: &str,
    host: &str,
    user_code: &str,
    csrf_token: &str,
    namespaces: &[Namespace],
) -> maud::Markup {
    maud::html! {
        section class="card twig-panel" aria-labelledby="tree-approval-title" {
            header class="twig-panel-head" {
                h1 class="card-title twig-eyebrow" id="tree-approval-title" { "AUTHORIZE TREE" }
                p class="twig-body-sm twig-ink-secondary" { "Twig server: " (host) }
            }
            div class="card-body twig-panel-body twig-stack" {
                p class="twig-body-sm twig-ink-secondary" {
                    "Signed in as " (username) ". Approving creates a Twig access token named Tree CLI."
                }
                p class="twig-body-sm twig-ink-secondary" {
                    "Only approve if this code matches the code shown in Tree."
                }
                code class="twig-code" { (user_code) }
                form method="post" action="/auth/device/approve" class="twig-form" {
                    input type="hidden" name="user_code" value=(user_code);
                    input type="hidden" name="csrf_token" value=(csrf_token);
                    fieldset class="fieldset twig-stack twig-stack--tight" {
                        legend class="fieldset-legend twig-subsection" { "1. Choose namespaces" }
                        p class="twig-body-sm twig-ink-tertiary" {
                            "Tree can only access namespaces you select."
                        }
                        label class="label" {
                            input class="checkbox" type="checkbox" name="all_namespaces" value="true";
                            " All namespaces I can access"
                        }
                        @for namespace in namespaces {
                            label class="label" {
                                input
                                    class="checkbox"
                                    type="checkbox"
                                    name=(format!("namespace_{}", namespace.id))
                                    value="true";
                                " " (namespace.name)
                            }
                        }
                    }
                    fieldset class="fieldset twig-stack twig-stack--tight" {
                        legend class="fieldset-legend twig-subsection" { "2. Choose permissions" }
                        p class="twig-body-sm twig-ink-tertiary" {
                            "These permissions apply to every selected namespace. You can only grant actions your account can perform in all of them."
                        }
                        section class="twig-stack twig-stack--tight" aria-labelledby="tree-read-permissions" {
                            h2 class="twig-subsection" id="tree-read-permissions" { "Read permissions" }
                            label class="label" {
                                input class="checkbox" type="checkbox" name="all_read" value="true";
                                " Select all read permissions"
                            }
                            @for permission in NamespacePermission::READ {
                                (render_permission_checkbox(permission))
                            }
                        }
                        section class="twig-stack twig-stack--tight" aria-labelledby="tree-write-permissions" {
                            h2 class="twig-subsection" id="tree-write-permissions" { "Write permissions" }
                            label class="label" {
                                input class="checkbox" type="checkbox" name="all_write" value="true";
                                " Select all write permissions"
                            }
                            @for permission in NamespacePermission::WRITE {
                                (render_permission_checkbox(permission))
                            }
                        }
                    }
                    p class="twig-body-sm twig-ink-tertiary" {
                        "Git will use this access only for this Twig server. You can revoke it later from Account Settings."
                    }
                    div class="card-actions twig-form-actions" {
                        button class="btn twig-btn twig-btn--quiet" type="submit" name="decision" value="deny" {
                            "Deny"
                        }
                        button class="btn twig-btn twig-btn--primary" type="submit" name="decision" value="approve" {
                            "Approve and connect"
                        }
                    }
                }
            }
        }
    }
}

fn render_permission_checkbox(permission: NamespacePermission) -> maud::Markup {
    let label = match permission {
        NamespacePermission::ViewNamespace => "View namespace and repositories",
        _ => permission.display_name(),
    };
    maud::html! {
        label class="label" {
            input
                class="checkbox"
                type="checkbox"
                name=(format!("permission_{}", permission.as_str()))
                value="true";
            " " (label)
        }
    }
}

fn render_decision_result(user_code: &str) -> maud::Markup {
    maud::html! {
        main class="twig-stack twig-form--narrow" {
            section class="card twig-panel" {
                div class="card-body twig-panel-body" {
                    h1 class="card-title" { "Tree authorization updated" }
                    p class="twig-body-sm twig-ink-secondary" {
                        "You can return to Tree. The request code was " (user_code) "."
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_codes_normalize_case_whitespace_and_separator() {
        assert_eq!(normalize_user_code("wqjh-8k2m").unwrap(), "WQJH8K2M");
        assert_eq!(normalize_user_code(" WQJH 8K2M ").unwrap(), "WQJH8K2M");
        assert!(normalize_user_code("WQJH-8K2!").is_err());
    }

    #[test]
    fn user_codes_are_formatted_in_two_groups() {
        assert_eq!(format_user_code("WQJH8K2M"), "WQJH-8K2M");
    }
}
