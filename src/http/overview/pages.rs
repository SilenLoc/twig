use std::collections::HashSet;

use actix_web::HttpRequest;
use actix_web::Result as AwResult;
use actix_web::{HttpResponse, get, post, web};
use serde::Deserialize;

use crate::auth::TwigContext;
use crate::config;
use crate::db::Database;
use crate::http::view::render_error;
use twig_git as git;

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
    #[serde(default)]
    more: bool,
}

#[derive(Deserialize)]
struct PinForm {
    namespace: String,
    pinned: bool,
    #[serde(default)]
    q: String,
    #[serde(default)]
    more: bool,
}

/// The viewer's place in the index: the active search, whether the full list
/// is expanded, and the namespace ids they have pinned. `pinned` is `None` for
/// anonymous visitors, who cannot pin and always see the full list.
struct ListView<'a> {
    search_query: &'a str,
    more: bool,
    pinned: Option<&'a HashSet<String>>,
}

/// What the index lists for one viewer.
struct Listing {
    namespaces: Vec<(crate::auth::Namespace, String)>,
    repo_hits: Vec<(String, String)>,
    pinned: Option<HashSet<String>>,
}

/// Renders the namespace listing shared by the full-page and HTMX responses.
fn render_index(
    view: &ListView<'_>,
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
                button type="submit" class="twig-btn twig-btn--ghost" { "Search" }
                (render_clear(view.search_query, false))
            }

            section class="twig-panel twig-panel--flush" {
                div id="index-results" class="twig-panel-body" aria-live="polite" {
                    (render_results(view, namespaces, repo_hits))
                }
            }
        }
    }
}

/// The results region that htmx swaps as the query changes.
fn render_results(
    view: &ListView<'_>,
    namespaces: &[(crate::auth::Namespace, String)],
    repo_hits: &[(String, String)],
) -> maud::Markup {
    let search_query = view.search_query;
    let grouped = view.pinned.filter(|_| search_query.is_empty());
    maud::html! {
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
            @if let Some(pinned) = grouped {
                (render_pinned_sections(view, pinned, namespaces))
            } @else if !namespaces.is_empty() {
                (render_namespace_list(view, "Namespaces", namespaces.iter()))
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
            p class="twig-empty-body twig-list-note" { "Pin a namespace to keep it at the top." }
        } @else {
            p class="twig-eyebrow twig-list-label" { "PINNED" }
            (render_namespace_list(
                view,
                "Pinned namespaces",
                namespaces.iter().filter(|entry| is_pinned(entry)),
            ))
        }
        @if view.more {
            p class="twig-eyebrow twig-list-label" { "OTHERS" }
            (render_namespace_list(
                view,
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
    view: &ListView<'_>,
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
                (render_namespace_row(view, namespace, owner))
            }
        }
    }
}

/// The link stays a plain anchor; the pin control sits beside it, because a
/// button nested inside the link would be invalid markup.
fn render_namespace_row(
    view: &ListView<'_>,
    namespace: &crate::auth::Namespace,
    owner: &str,
) -> maud::Markup {
    let pin_state = view.pinned.map(|pinned| pinned.contains(&namespace.id));
    maud::html! {
        div class="twig-row-pin" {
            a class="twig-row" href=(namespace.name) {
                span class="twig-row-id" { (namespace.name) }
                span class="twig-row-meta" {
                    span class="twig-sr" { "Owner: " }
                    (owner)
                }
            }
            @if let Some(is_pinned) = pin_state {
                form
                    class="twig-row-pin-form"
                    hx-post="/settings/pin-namespace"
                    hx-target="#index-results"
                    hx-swap="innerHTML"
                    "hx-status:4xx"="swap:innerHTML target:#index-results"
                    "hx-status:5xx"="swap:innerHTML target:#index-results"
                {
                    input type="hidden" name="namespace" value=(namespace.name);
                    input type="hidden" name="pinned" value=(if is_pinned { "false" } else { "true" });
                    @if !view.search_query.is_empty() {
                        input type="hidden" name="q" value=(view.search_query);
                    }
                    @if view.more {
                        input type="hidden" name="more" value="true";
                    }
                    @if is_pinned {
                        button
                            class="twig-btn twig-btn--quiet"
                            type="submit"
                            aria-label=(format!("Unpin {}", namespace.name)) { "Unpin" }
                    } @else {
                        button
                            class="twig-btn twig-btn--quiet"
                            type="submit"
                            aria-label=(format!("Pin {}", namespace.name)) { "Pin" }
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
            (render_results(&view, &listing.namespaces, &listing.repo_hits))
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

/// Pins or unpins a namespace for the signed-in user, then returns the
/// refreshed results so the list reorders in place.
#[post("/settings/pin-namespace")]
pub async fn pin_namespace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<PinForm>,
) -> HttpResponse {
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };

    let db = auth_state.db();
    let namespace = match db.get_namespace_by_name(&form.namespace).await {
        Ok(Some(namespace)) => namespace,
        Ok(None) => {
            return HttpResponse::NotFound()
                .body(render_error("Namespace not found").into_string());
        }
        Err(e) => {
            log::error!("Failed to load namespace for pinning: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    };

    if let Err(e) = db
        .set_namespace_pinned(&user_id, &namespace.id, form.pinned)
        .await
    {
        log::error!("Failed to update namespace pin: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to update pin").into_string());
    }

    let listing = load_listing(db, server.project_root(), &form.q, Some(&user_id)).await;
    let view = ListView {
        search_query: &form.q,
        more: form.more,
        pinned: listing.pinned.as_ref(),
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_results(&view, &listing.namespaces, &listing.repo_hits).into_string())
}

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
