use actix_web::{
    Error, HttpResponse,
    body::{EitherBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    http::header::{HeaderName, HeaderValue},
    middleware::Next,
};

use crate::config;

/// Rejects unsafe requests that provide cross-origin metadata. Session cookies
/// also use SameSite=Lax as a fallback for browsers that omit these headers.
pub async fn same_origin_guard<B>(
    req: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<EitherBody<B>>, Error>
where
    B: MessageBody + 'static,
{
    if should_check_origin(&req)
        && !is_device_approval(&req)
        && !origin_matches(&req, req.app_data::<actix_web::web::Data<config::Server>>())
    {
        return Ok(req
            .into_response(HttpResponse::Forbidden().body("Cross-origin request rejected"))
            .map_into_right_body());
    }

    next.call(req)
        .await
        .map(ServiceResponse::map_into_left_body)
}

/// Device approval has its own one-time, session-bound CSRF token. Some device
/// authorization clients open the approval page from a different origin, so
/// allow that form submission through the origin check without relaxing other
/// state-changing routes.
fn is_device_approval(req: &ServiceRequest) -> bool {
    req.method() == actix_web::http::Method::POST && req.path() == "/auth/device/approve"
}

/// Applies safe response defaults without overriding route-specific cache or
/// content-type headers. Inline-script allowances preserve current HTMX usage;
/// a stricter CSP requires moving those handlers into external assets.
pub async fn security_headers<B>(
    req: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<B>, Error>
where
    B: MessageBody + 'static,
{
    let mut response = next.call(req).await?;
    let headers = response.headers_mut();
    insert_default_header(headers, "x-content-type-options", "nosniff");
    insert_default_header(headers, "x-frame-options", "DENY");
    insert_default_header(headers, "referrer-policy", "no-referrer");
    insert_default_header(headers, "cache-control", "no-store");
    insert_default_header(
        headers,
        "content-security-policy",
        "default-src 'self'; base-uri 'self'; object-src 'none'; frame-ancestors 'none'; form-action 'self'; img-src 'self' data: https:; font-src 'self' https://fonts.gstatic.com; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; script-src 'self' 'unsafe-inline' 'unsafe-eval'; connect-src 'self'",
    );
    Ok(response)
}

fn insert_default_header(
    headers: &mut actix_web::http::header::HeaderMap,
    name: &'static str,
    value: &'static str,
) {
    let name = HeaderName::from_static(name);
    if !headers.contains_key(&name) {
        headers.insert(name, HeaderValue::from_static(value));
    }
}

fn should_check_origin(req: &ServiceRequest) -> bool {
    if matches!(
        *req.method(),
        actix_web::http::Method::GET
            | actix_web::http::Method::HEAD
            | actix_web::http::Method::OPTIONS
    ) {
        return false;
    }

    req.headers().contains_key(actix_web::http::header::ORIGIN)
        || req.headers().contains_key(actix_web::http::header::REFERER)
        || req
            .headers()
            .get("Sec-Fetch-Site")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|site| site.eq_ignore_ascii_case("cross-site"))
}

fn origin_matches(
    req: &ServiceRequest,
    server: Option<&actix_web::web::Data<config::Server>>,
) -> bool {
    let supplied = req
        .headers()
        .get(actix_web::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(|value| (value, false))
        .or_else(|| {
            req.headers()
                .get(actix_web::http::header::REFERER)
                .and_then(|value| value.to_str().ok())
                .map(|value| (value, true))
        });
    let Some((supplied, allow_path)) = supplied else {
        return false;
    };
    let Some(supplied_origin) = parse_web_origin(supplied, allow_path) else {
        return false;
    };

    let expected = server
        .and_then(|server| server.public_base_url())
        .map_or_else(
            || {
                let connection = req.connection_info();
                format!("{}://{}", connection.scheme(), connection.host())
            },
            str::to_string,
        );
    parse_web_origin(&expected, true).is_some_and(|origin| origin == supplied_origin)
}

fn parse_web_origin(value: &str, allow_path: bool) -> Option<(String, String)> {
    let (scheme, rest) = value.trim().split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https") {
        return None;
    }

    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let suffix = &rest[authority_end..];
    if !allow_path && !suffix.is_empty() && suffix != "/" {
        return None;
    }

    Some((scheme, authority.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_origins_and_referers_without_accepting_opaque_values() {
        assert_eq!(
            parse_web_origin("https://Twig.example:8443", false),
            Some(("https".to_string(), "twig.example:8443".to_string()))
        );
        assert_eq!(
            parse_web_origin("https://twig.example/settings", true),
            Some(("https".to_string(), "twig.example".to_string()))
        );
        assert!(parse_web_origin("null", false).is_none());
        assert!(parse_web_origin("https://attacker.example/path", false).is_none());
    }

    #[actix_web::test]
    async fn only_device_approval_bypasses_the_same_origin_check() {
        use actix_web::{App, test, web};

        let app = test::init_service(
            App::new()
                .wrap(actix_web::middleware::from_fn(same_origin_guard))
                .route(
                    "/auth/device/approve",
                    web::post().to(|| async { HttpResponse::Ok() }),
                )
                .route(
                    "/auth/login",
                    web::post().to(|| async { HttpResponse::Ok() }),
                ),
        )
        .await;

        let cross_origin_approval = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/auth/device/approve")
                .insert_header(("Origin", "https://tree.example"))
                .to_request(),
        )
        .await;
        assert_eq!(
            cross_origin_approval.status(),
            actix_web::http::StatusCode::OK
        );

        let cross_origin_login = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/auth/login")
                .insert_header(("Origin", "https://tree.example"))
                .to_request(),
        )
        .await;
        assert_eq!(
            cross_origin_login.status(),
            actix_web::http::StatusCode::FORBIDDEN
        );

        let other_method = test::call_service(
            &app,
            test::TestRequest::put()
                .uri("/auth/device/approve")
                .insert_header(("Origin", "https://tree.example"))
                .to_request(),
        )
        .await;
        assert_eq!(
            other_method.status(),
            actix_web::http::StatusCode::FORBIDDEN
        );
    }
}
