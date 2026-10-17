use actix_web::HttpRequest;
use actix_web::Result as AwResult;
use actix_web::get;
use actix_web::web;
use serde::Deserialize;

use crate::auth::TwigContext;
use crate::config;
use crate::http::auth::session::get_username_from_request;
use twig_git as git;

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

/// Renders the namespace listing shared by the full-page and HTMX responses.
fn render_index(
    search_query: &str,
    username: Option<&str>,
    namespaces: &[(crate::auth::Namespace, String)],
    repo_hits: &[(String, String)],
) -> maud::Markup {
    maud::html! {
        section class="twig-pagehead" {
            @if username.is_some() {
                div class="twig-cluster" {
                    a class="twig-btn twig-btn--primary" href="/auth/namespace" { "Create namespace" }
                }
            }
        }


        div class="twig-stack" {
            form class="twig-search" role="search" method="GET" action="/" {
                label class="twig-sr" for="index-search" { "Search namespaces and repositories" }
                input
                    class="twig-input twig-input--mono"
                    id="index-search"
                    type="search"
                    name="q"
                    value=(search_query)
                    placeholder="Search namespaces and repositories...";
                button type="submit" class="twig-btn twig-btn--ghost" { "Search" }
                @if !search_query.is_empty() {
                    a class="twig-btn twig-btn--quiet" href="/" { "Clear" }
                }
            }

            section class="twig-panel twig-panel--flush" {
                div class="twig-panel-body" aria-live="polite" {
                    @if namespaces.is_empty() && repo_hits.is_empty() {
                        @if search_query.is_empty() {
                            div class="twig-empty" {
                                p class="twig-eyebrow" { "NO NAMESPACES" }
                                p class="twig-empty-body" { "No namespaces yet. Create one to get started!" }
                            }
                        } @else {
                            div class="twig-empty" {
                                p class="twig-eyebrow" { "NO MATCHES" }
                                p class="twig-empty-body" {
                                    "No namespaces or repositories found matching your search."
                                }
                            }
                        }
                    } @else {
                        @if !namespaces.is_empty() {
                            div class="twig-colhead" {
                                span { "Namespace" }
                                span { "Owner" }
                            }
                            nav class="twig-list" aria-label="Namespaces" {
                                @for (namespace, owner) in namespaces {
                                    a class="twig-row" href=(namespace.name) {
                                        span class="twig-row-id" { (namespace.name) }
                                        span class="twig-row-meta" {
                                            span class="twig-sr" { "Owner: " }
                                            (owner)
                                        }
                                    }
                                }
                            }
                        }
                        @if !repo_hits.is_empty() {
                            div class="twig-colhead" {
                                span { "Repository" }
                                span { "Namespace" }
                            }
                            nav class="twig-list" aria-label="Repository matches" {
                                @for (namespace, repo) in repo_hits {
                                    a class="twig-row" href=(format!("{namespace}/{repo}")) {
                                        span class="twig-row-id" { (repo) }
                                        span class="twig-row-meta" {
                                            span class="twig-sr" { "Namespace: " }
                                            (namespace)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Repositories matched by the index search, across every namespace on disk.
/// Private repositories stay hidden from anonymous visitors. Returns
/// `(namespace, repository)` pairs sorted by namespace, then repository.
///
/// The scan matches repository *names* only; opening a repository is needed
/// just for the anonymous private check on names that already matched.
fn search_repo_hits(
    root: &str,
    namespaces: &[(crate::auth::Namespace, String)],
    query: &str,
    signed_in: bool,
) -> Vec<(String, String)> {
    let mut hits: Vec<(String, String)> = namespaces
        .iter()
        .flat_map(|(namespace, _)| {
            git::bare::search_repo_names(root, &namespace.name, query)
                .into_iter()
                .filter(|repo| {
                    signed_in || !git::bare::is_repo_private(root, &namespace.name, repo)
                })
                .map(move |repo| (namespace.name.clone(), repo))
        })
        .collect();
    hits.sort();
    hits
}

#[get("/")]
pub async fn index(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<SearchQuery>,
) -> AwResult<maud::Markup> {
    let search_query = query.q.as_deref().unwrap_or("");
    let username = get_username_from_request(&req, &auth_state).await;

    let db = auth_state.db();

    // Get namespaces with owners from database
    let all_namespaces = match db.get_all_namespaces_with_owners().await {
        Ok(n) => n,
        Err(e) => {
            log::error!("Failed to get namespaces: {e}");
            Vec::new()
        }
    };

    // Repository hits are collected only while a query is active, and always
    // over every namespace on disk, so a repo hit can surface even when its
    // namespace's name does not match the query.
    let repo_hits = if search_query.is_empty() {
        Vec::new()
    } else {
        search_repo_hits(
            server.project_root(),
            &all_namespaces,
            search_query,
            username.is_some(),
        )
    };

    // The visible namespace list narrows by name only while searching.
    let namespaces = if search_query.is_empty() {
        all_namespaces
    } else {
        match db.search_namespaces_with_owners(search_query).await {
            Ok(n) => n,
            Err(e) => {
                log::error!("Failed to search namespaces: {e}");
                Vec::new()
            }
        }
    };

    let content = render_index(search_query, username.as_deref(), &namespaces, &repo_hits);

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(crate::http::view::render_layout(
            &content,
            username.as_deref(),
            Some("Namespaces"),
        ))
    }
}

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
