use super::*;

use super::support::*;

use super::auth_and_invitations::{create_namespace, signup_and_login};

#[actix_web::test]
async fn pinned_namespaces_lead_the_home_page_until_more_is_chosen() {
    let root = format!("/tmp/test_twig_namespace_pins_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let session = signup_and_login(&app, "pinowner").await;
    create_namespace(&app, session.clone(), "pinalpha").await;
    create_namespace(&app, session.clone(), "pinbeta").await;
    create_namespace(&app, session.clone(), "pingamma").await;

    let request = test::TestRequest::get()
        .uri("/")
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let home = String::from_utf8_lossy(&body);
    assert!(
        home.contains("Pin a namespace from its settings to keep it at the top.")
            && home.contains("Show 3 more")
            && !home.contains("href=\"pinalpha\"")
            && !home.contains("pin-namespace"),
        "with nothing pinned, every namespace waits behind More and the rows carry no pin controls: {home}"
    );

    let request = test::TestRequest::get()
        .uri("/manage/namespaces/pinbeta/settings")
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let settings = String::from_utf8_lossy(&body);
    assert!(
        settings.contains("Pin to home")
            && settings.contains("hx-post=\"/settings/pin-namespace\""),
        "namespace settings offers to pin the namespace: {settings}"
    );

    let request = test::TestRequest::post()
        .uri("/settings/pin-namespace")
        .cookie(session.clone())
        .insert_header(("HX-Request", "true"))
        .set_form([("namespace", "pinbeta"), ("pinned", "true")])
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let fragment = String::from_utf8_lossy(&body);
    assert!(
        fragment.contains("Unpin from home")
            && fragment.contains("name=\"pinned\" value=\"false\""),
        "the pin response flips the settings control: {fragment}"
    );

    let request = test::TestRequest::get()
        .uri("/")
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, request).await;
    let body = test::read_body(response).await;
    let home = String::from_utf8_lossy(&body);
    assert!(
        home.contains("aria-label=\"Pinned namespaces\"")
            && home.contains("href=\"pinbeta\"")
            && home.contains("Show 2 more")
            && !home.contains("href=\"pinalpha\""),
        "the pinned namespace is listed first after a reload, the rest stay behind More: {home}"
    );

    let request = test::TestRequest::get()
        .uri("/?more=true")
        .cookie(session.clone())
        .to_request();
    let response = test::call_service(&app, request).await;
    let body = test::read_body(response).await;
    let expanded = String::from_utf8_lossy(&body);
    assert!(
        expanded.contains("href=\"pinalpha\"") && expanded.contains(">Show fewer</button>"),
        "More reveals the unpinned namespaces: {expanded}"
    );

    let request = test::TestRequest::post()
        .uri("/settings/pin-namespace")
        .cookie(session.clone())
        .insert_header(("HX-Request", "true"))
        .set_form([("namespace", "pinbeta"), ("pinned", "false")])
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = test::read_body(response).await;
    let fragment = String::from_utf8_lossy(&body);
    assert!(
        fragment.contains("Pin to home") && fragment.contains("name=\"pinned\" value=\"true\""),
        "unpinning offers to pin again: {fragment}"
    );

    let request = test::TestRequest::post()
        .uri("/settings/pin-namespace")
        .insert_header(("HX-Request", "true"))
        .set_form([("namespace", "pinbeta"), ("pinned", "true")])
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "pinning needs a signed-in user"
    );

    let request = test::TestRequest::post()
        .uri("/settings/pin-namespace")
        .cookie(session)
        .insert_header(("HX-Request", "true"))
        .set_form([("namespace", "pinmissing"), ("pinned", "true")])
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let _ = std::fs::remove_dir_all(&root);
}
