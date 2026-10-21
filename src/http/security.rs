use actix_web::{
    Error,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    http::header::{HeaderName, HeaderValue},
    middleware::Next,
};

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
