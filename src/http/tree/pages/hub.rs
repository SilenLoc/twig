use actix_web::{HttpRequest, HttpResponse, get, web};

use crate::auth::TwigContext;

/// The Manage sections as one flat row. `show_test` is true only for the
/// configured test user, since that is the only account the suite admits
/// without a session PIN.
pub(crate) fn render_tree_hub(
    show_test: bool,
    data_enabled: bool,
    user_management_enabled: bool,
    active: Option<&str>,
) -> maud::Markup {
    let link = |id: &str, href: &str, label: &str| {
        maud::html! {
            a class="twig-tab" aria-current=[(active == Some(id)).then_some("page")] href=(href) { (label) }
        }
    };
    maud::html! {
        nav class="twig-tabs" aria-label="Manage sections" {
            (link("account", "/settings", "Account"))
            (link("tokens", "/settings/tokens", "Access tokens"))
            (link("repositories", "/manage/repositories", "Repositories"))
            (link("namespaces", "/manage/namespaces", "Namespaces"))
            @if user_management_enabled {
                (link("users", "/manage/users", "Users"))
            }
            @if show_test {
                (link("test", "/_test", "Test"))
            }
            @if data_enabled {
                (link("data", "/manage/data", "Data"))
            }
        }
    }
}

#[get("/manage")]
pub async fn tree_page(req: HttpRequest, auth_state: web::Data<TwigContext>) -> HttpResponse {
    if crate::http::auth::session::get_username_from_request(&req, &auth_state)
        .await
        .is_none()
    {
        return HttpResponse::Found()
            .insert_header(("Location", "/auth/login"))
            .finish();
    }

    HttpResponse::Found()
        .insert_header(("Location", "/settings"))
        .finish()
}
