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
        div class="pt3 pt4-ns" {
            h1 class="tf-hero white ma0" { "Namespaces" }
        }

        div class="flex flex-wrap justify-between items-end mb3 mb4-ns" {
            @if username.is_some() {
                a
                    href="/auth/namespace"
                    class="tf-btn"
                {
                    "Create namespace"
                }
            }
        }

        div class="mb3 mb4-ns" {
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
                    class="tf-input flex-auto mr2"
                    style="min-width: 0;";
                button
                    type="submit"
                    class="tf-btn tf-btn-ghost"
                {
                    "Search"
                }
                @if !search_query.is_empty() {
                    a
                        href="/"
                        class="ml2 pa2 link white-70 hover-white no-underline tf-kicker"
                    {
                        "Clear"
                    }
                }
            }
        }

        div class="ba b--white-20 bg-black-20 overflow-hidden overflow-x-auto" {
            div class="flex pa3 bb b--white-20 white-50 f6 fw6 tf-kicker" {
                div class="flex-auto" { "Namespace" }
                div class="tr dn db-ns" style="min-width: 150px;" { "Owner" }
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
                            class="flex pa3 bb b--white-10 link white hover-white hover-bg-white-10 no-underline items-baseline"
                        {
                            div class="flex-auto" {
                                span class="f4 fw6" style="letter-spacing: -0.01em;" { (namespace.name) }
                            }
                            div class="tr white-50 f6 dn db-ns" style="min-width: 150px;" {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_query_deserialization() {
        let query: SearchQuery = serde_urlencoded::from_str("q=rust").unwrap();
        assert_eq!(query.q, Some("rust".to_string()));
    }

    #[test]
    fn test_search_query_empty() {
        let query: SearchQuery = serde_urlencoded::from_str("").unwrap();
        assert_eq!(query.q, None);
    }
}
