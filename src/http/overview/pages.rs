use std::collections::HashSet;

use actix_web::HttpRequest;
use actix_web::Result as AwResult;
use actix_web::{get, web};
use serde::Deserialize;

use crate::auth::TwigContext;
use crate::config;
use crate::db::Database;
use twig_git as git;

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
    #[serde(default)]
    more: bool,
}

/// The viewer's place in the index: the active search, whether the full list
/// is expanded, and the namespace ids they have pinned. `pinned` is `None` for
/// anonymous visitors, who always see the full list.
struct ListView<'a> {
    search_query: &'a str,
    more: bool,
    pinned: Option<&'a HashSet<String>>,
}

/// What the index lists for one viewer.
struct Listing {
    namespaces: Vec<(crate::auth::Namespace, String)>,
    repo_hits: Vec<(String, String, bool)>,
    pinned: Option<HashSet<String>>,
}

/// Renders the namespace listing shared by the full-page and HTMX responses.
fn render_index(
    view: &ListView<'_>,
    username: Option<&str>,
    namespaces: &[(crate::auth::Namespace, String)],
    repo_hits: &[(String, String, bool)],
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
            form class="twig-search" role="search" method="GET" action="/"
                hx-get="/"
                hx-target="#index-results"
                hx-swap="innerHTML"
                hx-push-url="true"
                hx-indicator="#index-search-status" {
                label class="twig-sr" for="index-search" { "Search namespaces and repositories" }
                input
                    class="twig-input twig-input--mono"
                    id="index-search"
                    type="search"
                    name="q"
                    value=(view.search_query)
                    placeholder="Search namespaces and repositories..."
                    hx-get="/"
                    hx-target="#index-results"
                    hx-swap="innerHTML"
                    hx-push-url="true"
                    hx-indicator="#index-search-status"
                    hx-trigger="input changed delay:200ms";
                span id="index-search-status" class="twig-search-status" aria-hidden="true" {}
                (render_clear(view.search_query, false))
            }

            section class="twig-panel twig-panel--flush" {
                div id="index-results" class="twig-panel-body" aria-live="polite" {
                    (render_results(view, username, namespaces, repo_hits))
                }
            }
        }
    }
}

/// The results region that htmx swaps as the query changes.
fn render_results(
    view: &ListView<'_>,
    username: Option<&str>,
    namespaces: &[(crate::auth::Namespace, String)],
    repo_hits: &[(String, String, bool)],
) -> maud::Markup {
    let search_query = view.search_query;
    let grouped = view.pinned.filter(|_| search_query.is_empty());
    maud::html! {
        @if namespaces.is_empty() && repo_hits.is_empty() {
            @if search_query.is_empty() {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO NAMESPACES" }
                    p class="twig-empty-body" {
                        @if username.is_some() {
                            "No namespaces yet. Create one to get started!"
                        } @else {
                            "No namespaces yet."
                        }
                    }
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
            @if let Some(pinned) = grouped {
                (render_pinned_sections(view, pinned, namespaces))
            } @else if !namespaces.is_empty() {
                @if !repo_hits.is_empty() {
                    p class="twig-eyebrow twig-list-label" { "NAMESPACES" }
                }
                (render_namespace_list("Namespaces", namespaces.iter()))
            }
            @if !repo_hits.is_empty() {
                @if !namespaces.is_empty() {
                    p class="twig-eyebrow twig-list-label" { "REPOSITORIES" }
                }
                div class="twig-colhead" {
                    span { "Repository" }
                    span { "Namespace" }
                }
                nav class="twig-list" aria-label="Repository matches" {
                    @for (namespace, repo, private) in repo_hits {
                        a class="twig-row" href=(format!("{namespace}/{repo}")) {
                            span class="twig-row-id" { (repo) }
                            span class="twig-row-meta" {
                                @if *private {
                                    span class="twig-tag" { "Private" }
                                }
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

/// On the home page a signed-in viewer sees pinned namespaces first. The rest
/// stay behind a toggle until they are expanded or a search is active.
fn render_pinned_sections(
    view: &ListView<'_>,
    pinned: &HashSet<String>,
    namespaces: &[(crate::auth::Namespace, String)],
) -> maud::Markup {
    let is_pinned = |entry: &&(crate::auth::Namespace, String)| pinned.contains(&entry.0.id);
    let pinned_count = namespaces.iter().filter(|entry| is_pinned(entry)).count();
    let other_count = namespaces.len() - pinned_count;
    maud::html! {
        @if pinned_count == 0 {
            p class="twig-empty-body twig-list-note" { "Pin a namespace from its settings to keep it at the top." }
        } @else {
            p class="twig-eyebrow twig-list-label" { "PINNED" }
            (render_namespace_list(
                "Pinned namespaces",
                namespaces.iter().filter(|entry| is_pinned(entry)),
            ))
        }
        @if view.more {
            p class="twig-eyebrow twig-list-label" { "OTHERS" }
            (render_namespace_list(
                "Other namespaces",
                namespaces.iter().filter(|entry| !is_pinned(entry)),
            ))
        }
        @if other_count > 0 {
            (render_more_toggle(view.more, other_count))
        }
    }
}

fn render_more_toggle(expanded: bool, other_count: usize) -> maud::Markup {
    maud::html! {
        div class="twig-list-more" {
            @if expanded {
                button
                    class="twig-btn twig-btn--quiet"
                    type="button"
                    hx-get="/"
                    hx-target="#index-results"
                    hx-swap="innerHTML"
                    hx-push-url="true" { "Show fewer" }
            } @else {
                button
                    class="twig-btn twig-btn--quiet"
                    type="button"
                    hx-get="/?more=true"
                    hx-target="#index-results"
                    hx-swap="innerHTML"
                    hx-push-url="true" { "Show " (other_count) " more" }
            }
        }
    }
}

/// One column-headed list of namespace rows. `label` names the list for
/// assistive technology.
fn render_namespace_list<'a>(
    label: &str,
    rows: impl Iterator<Item = &'a (crate::auth::Namespace, String)>,
) -> maud::Markup {
    maud::html! {
        div class="twig-colhead" {
            span { "Namespace" }
            span { "Owner" }
        }
        nav class="twig-list" aria-label=(label) {
            @for (namespace, owner) in rows {
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
}

/// The clear control sits outside the results region, so HTMX responses
/// replace it out-of-band whenever the query changes.
fn render_clear(search_query: &str, oob: bool) -> maud::Markup {
    maud::html! {
        span id="index-clear" hidden[search_query.is_empty()] hx-swap-oob=[oob.then_some("outerHTML")] {
            @if !search_query.is_empty() {
                a class="twig-btn twig-btn--quiet" href="/" { "Clear" }
            }
        }
    }
}

/// Repositories matched by the index search, across every namespace on disk.
/// Private repositories stay hidden from anonymous visitors. Returns
/// `(namespace, repository, is_private)` triples sorted by namespace, then
/// repository.
///
/// The scan matches repository *names* only; opening a repository is needed
/// just for the anonymous private check on names that already matched.
fn search_repo_hits(
    root: &str,
    namespaces: &[(crate::auth::Namespace, String)],
    query: &str,
    signed_in: bool,
) -> Vec<(String, String, bool)> {
    let mut hits: Vec<(String, String, bool)> = namespaces
        .iter()
        .flat_map(|(namespace, _)| {
            git::bare::search_repo_names(root, &namespace.name, query)
                .into_iter()
                .filter_map(|repo| {
                    let private = git::bare::is_repo_private(root, &namespace.name, &repo);
                    (signed_in || !private).then(|| (namespace.name.clone(), repo, private))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    hits.sort();
    hits
}

/// Loads what the index lists for one viewer. Anonymous visitors (`user_id` is
/// `None`) have no pins.
async fn load_listing(
    db: &Database,
    root: &str,
    search_query: &str,
    user_id: Option<&str>,
) -> Listing {
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
        search_repo_hits(root, &all_namespaces, search_query, user_id.is_some())
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

    let pinned = match user_id {
        Some(user_id) => match db.get_pinned_namespace_ids(user_id).await {
            Ok(ids) => Some(ids),
            Err(e) => {
                log::error!("Failed to get pinned namespaces: {e}");
                Some(HashSet::new())
            }
        },
        None => None,
    };

    Listing {
        namespaces,
        repo_hits,
        pinned,
    }
}

#[get("/")]
pub async fn index(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<SearchQuery>,
) -> AwResult<maud::Markup> {
    let search_query = query.q.as_deref().unwrap_or("");
    let db = auth_state.db();
    let user_id = auth_state.user_id_from_request(&req).await;
    let username = match user_id.as_deref() {
        Some(user_id) => db
            .get_user_by_id(user_id)
            .await
            .ok()
            .flatten()
            .map(|user| user.username),
        None => None,
    };

    let listing = load_listing(db, server.project_root(), search_query, user_id.as_deref()).await;
    let view = ListView {
        search_query,
        more: query.more,
        pinned: listing.pinned.as_ref(),
    };

    if req.headers().get("HX-Request").is_some() {
        return Ok(maud::html! {
            (render_results(&view, username.as_deref(), &listing.namespaces, &listing.repo_hits))
            (render_clear(search_query, true))
        });
    }

    let content = render_index(
        &view,
        username.as_deref(),
        &listing.namespaces,
        &listing.repo_hits,
    );
    Ok(crate::http::view::render_layout(
        &content,
        username.as_deref(),
        Some("Namespaces"),
    ))
}

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
