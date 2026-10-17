use super::super::*;

/// Pulls the 64-hex-character invite token out of the success fragment.
pub(in crate::integration_tests::tests) fn extract_invite(html: &str) -> String {
    html.split(|c: char| !c.is_ascii_hexdigit())
        .find(|token| token.len() == 64)
        .expect("invite token in response")
        .to_string()
}

/// Signs a user up and logs them in, returning the identity session cookie.
pub(in crate::integration_tests::tests) async fn signup_and_login<S>(
    app: &S,
    username: &str,
) -> actix_web::cookie::Cookie<'static>
where
    S: actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let req = test::TestRequest::post()
        .uri("/auth/invite")
        .set_form([("api_key", "secure")])
        .to_request();
    let resp = test::call_service(app, req).await;
    assert!(
        resp.status().is_success(),
        "invite generation should succeed"
    );
    let body = test::read_body(resp).await;
    let invite = extract_invite(&String::from_utf8_lossy(&body));

    let email = format!("{username}@example.com");
    let req = test::TestRequest::post()
        .uri("/auth/signup")
        .set_form([
            ("invite", invite.as_str()),
            ("username", username),
            ("email", email.as_str()),
            ("password", "password123"),
        ])
        .to_request();
    let resp = test::call_service(app, req).await;
    assert!(resp.status().is_success(), "signup should succeed");

    let req = test::TestRequest::post()
        .uri("/auth/login")
        .set_form([("username", username), ("password", "password123")])
        .to_request();
    let resp = test::call_service(app, req).await;
    assert!(resp.status().is_success(), "login should succeed");

    resp.response()
        .cookies()
        .find(|c| c.name() == "id")
        .expect("login should set an identity cookie")
        .into_owned()
}

pub(in crate::integration_tests::tests) async fn create_namespace<S>(
    app: &S,
    session: actix_web::cookie::Cookie<'static>,
    name: &str,
) where
    S: actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let req = test::TestRequest::post()
        .uri("/auth/namespace")
        .cookie(session)
        .set_form([("name", name)])
        .to_request();
    let resp = test::call_service(app, req).await;
    assert!(
        resp.status().is_success(),
        "namespace creation should succeed"
    );
}

pub(in crate::integration_tests::tests) fn basic_auth(username: &str, password: &str) -> String {
    use base64::Engine;
    let creds = format!("{username}:{password}");
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(creds.as_bytes())
    )
}
