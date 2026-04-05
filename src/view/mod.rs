use actix_web::HttpRequest;
use actix_web::Result as AwResult;
use actix_web::get;
use actix_web::web;
use maud::DOCTYPE;
use serde::Deserialize;

use crate::auth::AuthState;
use crate::config;

pub mod admin;
pub mod auth;
pub mod namespace;
pub mod repo;
pub mod settings;

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

/// Helper function to get the username from the session cookie if logged in
async fn get_username_from_request(
    req: &HttpRequest,
    auth_state: &web::Data<AuthState>,
) -> Option<String> {
    let token = req.cookie("session")?;
    let user_id = auth_state.validate_token(token.value()).await?;
    let user = auth_state.db.get_user_by_id(&user_id).await.ok()??;
    Some(user.username)
}

#[get("/")]
pub async fn index(
    req: HttpRequest,
    _server: web::Data<config::Server>,
    auth_state: web::Data<AuthState>,
    query: web::Query<SearchQuery>,
) -> AwResult<maud::Markup> {
    let search_query = query.q.as_deref().unwrap_or("");
    let username = get_username_from_request(&req, &auth_state).await;

    // Get namespaces with owners from database
    let namespaces = if search_query.is_empty() {
        auth_state
            .db
            .get_all_namespaces_with_owners()
            .await
            .unwrap_or_default()
    } else {
        auth_state
            .db
            .search_namespaces_with_owners(search_query)
            .await
            .unwrap_or_default()
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
        Ok(render_layout(&content, username.as_deref()))
    }
}

pub fn render_layout(main_content: &maud::Markup, username: Option<&str>) -> maud::Markup {
    maud::html! {
        (DOCTYPE)
        html class="h-100" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Fig" }
                link rel="stylesheet" href="/assets/t.css";
                script src="/assets/h.js" {}
                script src="/assets/hx-response-targets.js" {}
            }
            body hx-ext="response-targets" class="w-100 sans-serif ma0 bg-black white" style="height: 100vh; overflow: hidden;" {
                nav class="dt w-100 bg-black bb b--white-20 fixed top-0 left-0 right-0 z-1" {
                    div class="dtc v-mid pa3" {
                        a href="/" class="link white-90 hover-white no-underline fw6 f4" {
                            "Fig"
                        }
                    }
                    div class="dtc v-mid tr pa3" {
                        @match username {
                            Some(name) => {
                                span class="white-70 f6 mr3" { (name) }
                                a href="/settings" class="link white-70 hover-white no-underline f6 mr3" {
                                    "Settings"
                                }
                                form method="POST" action="/auth/logout" class="dib ma0" {
                                    button
                                        type="submit"
                                        class="link white-70 hover-white no-underline f6 bg-transparent bn pointer pa0"
                                    {
                                        "Logout"
                                    }
                                }
                            }
                            None => {
                                a href="/auth/login" class="link white-70 hover-white no-underline f6 mr3" {
                                    "Login"
                                }
                                a href="/auth/signup" class="link white-70 hover-white no-underline f6" {
                                    "Signup"
                                }
                            }
                        }
                    }
                }
                main id="feature" class="flex flex-column" style="padding-top: 5rem; padding-left: 10px; padding-right: 10px; padding-bottom: 10px; height: 100vh; overflow: hidden;" {
                    div class="w-100 flex-auto" style="overflow: hidden; display: flex; flex-direction: column;" {
                        (main_content)
                    }
                }
            }
        }
    }
}
