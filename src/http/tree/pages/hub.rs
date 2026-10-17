use actix_web::{HttpRequest, HttpResponse, get, web};

use crate::auth::TwigContext;

pub(crate) fn render_tree_hub(
    test_enabled: bool,
    data_enabled: bool,
    user_management_enabled: bool,
    active: Option<&str>,
) -> maud::Markup {
    maud::html! {
        nav class="twig-tabs" aria-label="Tree sections" {
            a class="twig-tab" aria-current=[(active == Some("account")).then_some("page")] href="/settings" { "Account" }
            a class="twig-tab" aria-current=[(active == Some("repositories")).then_some("page")] href="/tree/repositories" { "Repository" }
            a class="twig-tab" aria-current=[(active == Some("namespaces")).then_some("page")] href="/tree/namespaces" { "Namespace" }
            @if test_enabled {
                a class="twig-tab" aria-current=[(active == Some("test")).then_some("page")] href="/_test" { "Test" }
            }
            @if data_enabled {
                a class="twig-tab" aria-current=[(active == Some("data")).then_some("page")] href="/tree/data" { "Data" }
            }
            @if user_management_enabled {
                a class="twig-tab" aria-current=[(active == Some("users")).then_some("page")] href="/tree/users" { "User Management" }
            }
        }
    }
}

#[get("/tree")]
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
