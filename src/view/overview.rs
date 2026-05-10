use actix_web::HttpRequest;
use actix_web::Result as AwResult;
use actix_web::get;
use actix_web::web;
use serde::Deserialize;

use crate::auth::FigContext;
use crate::config;
use crate::view::session_auth::get_username_from_request;

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

#[get("/")]
pub async fn index(
    req: HttpRequest,
    _server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    query: web::Query<SearchQuery>,
) -> AwResult<maud::Markup> {
    let search_query = query.q.as_deref().unwrap_or("");
    let username = get_username_from_request(&req, &auth_state).await;

    let db = auth_state.db();

    // Get namespaces with owners from database
    let namespaces = if search_query.is_empty() {
        match db.get_all_namespaces_with_owners().await {
            Ok(n) => n,
            Err(e) => {
                log::error!("Failed to get namespaces: {e}");
                Vec::new()
            }
        }
    } else {
        match db.search_namespaces_with_owners(search_query).await {
            Ok(n) => n,
            Err(e) => {
                log::error!("Failed to search namespaces: {e}");
                Vec::new()
            }
        }
    };

    let content = maud::html! {
        // Header
        div class="flex justify-between items-center mb4" {
            h1 class="f3 fw6 white ma0" { "Namespaces" }
            @if username.is_some() {
                a
                    href="/auth/namespace"
                    class="pa2 bg-white black bn br1 pointer hover-bg-white-90 f6 no-underline"
                {
                    "Create namespace"
                }
            }
        }

        // Search box
        div class="mb4" {
            form
                method="GET"
                action="/"
                class="flex items-center"
            {
                input
                    type="text"
                    name="q"
                    value=(search_query)
                    placeholder="Search namespaces..."
                    class="flex-auto pa2 bg-black white ba b--white-30 br1 mr2"
                    style="outline: none;"
                    onfocus="this.style.borderColor='white'"
                    onblur="this.style.borderColor='rgba(255,255,255,0.3)'";
                button
                    type="submit"
                    class="pa2 bg-white-10 white bn br1 pointer hover-bg-white-20"
                {
                    "Search"
                }
                @if !search_query.is_empty() {
                    a
                        href="/"
                        class="ml2 pa2 link white-70 hover-white no-underline"
                    {
                        "Clear"
                    }
                }
            }
        }

        // Namespace list
        div class="ba b--white-20 br2 bg-black-20 overflow-hidden" {
            // Table header
            div class="flex pa3 bb b--white-20 bg-black-30 white-70 f6 fw6" {
                div class="flex-auto" { "Namespace" }
                div class="tr" style="width: 150px;" { "Owner" }
            }

            @if namespaces.is_empty() {
                div class="pa4 tc" {
                    @if search_query.is_empty() {
                        p class="f6 white-70" { "No namespaces yet. Create one to get started!" }
                    } @else {
                        p class="f6 white-70" { "No namespaces found matching your search." }
                    }
                }
            } @else {
                div class="flex flex-column" {
                    @for (namespace, owner) in namespaces {
                        a
                            href=(format!("{}", namespace.name))
                            class="flex pa3 bb b--white-10 link white-90 hover-white hover-bg-white-10 no-underline items-center"
                        {
                            div class="flex-auto" {
                                span class="f5" { (namespace.name) }
                            }
                            div class="tr white-60 f6" style="width: 150px;" {
                                (owner)
                            }
                        }
                    }
                }
            }
        }
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(super::render_layout(&content, username.as_deref()))
    }
}
