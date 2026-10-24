use crate::auth::TwigContext;
use crate::auth::permissions::{self, NamespacePermission};
use crate::config;
use crate::git;
use actix_web::{HttpRequest, HttpResponse, get, web};

use super::hub::render_tree_hub;
use super::repositories::load_namespace_page_data;

const NAMESPACE_SEARCH_SCRIPT: &str = r"
(() => {
  const input = document.getElementById('namespace-search');
  if (!input) return;
  const rows = Array.from(document.querySelectorAll('[data-namespace-row]'));
  const empty = document.getElementById('namespace-search-empty');
  input.addEventListener('input', () => {
    const query = input.value.trim().toLowerCase();
    let shown = 0;
    for (const row of rows) {
      const match = row.dataset.namespaceRow.includes(query);
      row.hidden = !match;
      if (match) shown++;
    }
    empty.hidden = shown > 0;
  });
})();
";

/// One searchable, scrollable list of the namespaces the user can manage. Each
/// row links to that namespace's settings page.
pub(super) fn render_namespaces_page(manageable_namespaces: &[String]) -> maud::Markup {
    maud::html! {
        section class="twig-panel" {
            @if manageable_namespaces.is_empty() {                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO NAMESPACES" }
                    p class="twig-empty-body" { "You don't have any namespaces to manage." }
                }
            } @else {
                div class="twig-panel-body twig-stack twig-stack--tight" {
                    input
                        class="twig-input twig-input--mono"
                        type="search"
                        id="namespace-search"
                        placeholder="Search namespaces"
                        aria-label="Search namespaces"
                        autocomplete="off"
                        spellcheck="false";
                }

                div class="twig-list twig-list--scroll" {
                    @for name in manageable_namespaces {
                        div class="twig-row twig-row--form" data-namespace-row=(name.to_lowercase()) {
                            span class="twig-row-id" { (name) }
                            div class="twig-row-actions" {
                                a class="twig-btn" href=(format!("/manage/namespaces/{name}/settings")) { "Settings" }
                            }
                        }
                    }
                }
                p id="namespace-search-empty" class="twig-empty-body" hidden { "No namespaces match your search." }
                script { (maud::PreEscaped(NAMESPACE_SEARCH_SCRIPT)) }
            }
        }
    }
}

/// Settings for one namespace: the viewer's home-page pin, rename, move its
/// repositories, and delete it.
pub(super) fn render_namespace_settings(
    namespace: &str,
    pinned: bool,
    repos: &[git::bare::RepoInfo],
    repo_manageable: bool,
    move_targets: &[String],
    deletable: bool,
) -> maud::Markup {
    maud::html! {
        div class="twig-pagehead" {
            nav class="twig-crumbs twig-crumbs--page" aria-label="Breadcrumb" {
                a href="/manage/namespaces" { "Namespaces" }
                span class="twig-crumb-sep" aria-hidden="true" { "/" }
                span { (namespace) }
                span class="twig-crumb-sep" aria-hidden="true" { "/" }
                span aria-current="page" { "Settings" }
            }
        }

        div class="twig-bento" {
            (crate::http::settings::pages::render_namespace_pin_panel(namespace, pinned))
            (render_namespace_rename_panel(namespace))
            @if repo_manageable {
                (render_namespace_move_panel(namespace, repos, move_targets))
            }
            (render_namespace_deletion_panel(namespace, deletable))
        }
    }
}

fn render_namespace_rename_panel(namespace: &str) -> maud::Markup {
    maud::html! {
        section class="twig-panel" aria-labelledby="namespace-rename-heading" {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="namespace-rename-heading" { "Rename Namespace" }
            }
            div class="twig-panel-body twig-stack twig-stack--tight" {
                p class="twig-body-sm twig-ink-secondary" {
                    "Renaming a namespace changes the URLs of every repository inside it. Their history and configuration are preserved."
                }
                form
                    class="twig-form"
                    hx-post="/settings/rename-namespace"
                    hx-target="#rename-namespace-result"
                    hx-swap="innerHTML"
                    "hx-status:4xx"="swap:innerHTML target:#rename-namespace-result"
                    "hx-status:5xx"="swap:innerHTML target:#rename-namespace-result"
                    hx-confirm=(format!("Rename namespace '{namespace}'? Every repository URL inside it will change."))
                {
                    input type="hidden" name="namespace" value=(namespace);
                    div class="twig-field" {
                        label class="twig-label" {
                            "New name"
                            input
                                class="twig-input twig-input--mono"
                                type="text"
                                name="new_name"
                                required
                                minlength="2"
                                autocomplete="off"
                                spellcheck="false"
                                value=(namespace);
                        }
                    }
                    div class="twig-form-actions" {
                        button class="twig-btn twig-btn--primary" type="submit" { "Rename" }
                    }
                }
                div id="rename-namespace-result" aria-live="polite" {}
            }
        }
    }
}

/// The namespace's repositories, each movable to another namespace the user can
/// manage repositories in.
fn render_namespace_move_panel(
    namespace: &str,
    repos: &[git::bare::RepoInfo],
    move_targets: &[String],
) -> maud::Markup {
    maud::html! {
        section class="twig-panel" aria-labelledby="namespace-move-heading" {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="namespace-move-heading" { "Move Repositories" }
            }

            @if repos.is_empty() {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO REPOSITORIES" }
                    p class="twig-empty-body" { "This namespace has no repositories to move." }
                }
            } @else {
                div class="twig-panel-body twig-stack twig-stack--tight" {
                    p class="twig-body-sm twig-ink-secondary" {
                        "Moving a repository changes its web and Git URLs. Its history and configuration are preserved."
                    }
                    @if move_targets.is_empty() {
                        p class="twig-empty-body" { "There is no other namespace you can move repositories to." }
                    }
                    div id="move-repo-result" aria-live="polite" {}
                }

                div class="twig-list" {
                    @for repo in repos {
                        div class="twig-row" {
                            div class="twig-stack twig-stack--tight" {
                                span class="twig-row-id" { (namespace) "/" (repo.name) }
                                span class="twig-label" { "Move to namespace" }
                            }
                            div class="twig-form-actions" aria-label=(format!("Move {}/{} to namespace", namespace, repo.name)) {
                                @for target in move_targets {
                                    form
                                        hx-post="/settings/move-repo"
                                        hx-target="#move-repo-result"
                                        hx-swap="innerHTML"
                                        "hx-status:4xx"="swap:innerHTML target:#move-repo-result"
                                        "hx-status:5xx"="swap:innerHTML target:#move-repo-result"
                                        hx-confirm=(format!("Move '{}/{}' to the '{}' namespace? Its URL will change.", namespace, repo.name, target))
                                    {
                                        input type="hidden" name="source_namespace" value=(namespace);
                                        input type="hidden" name="repo_name" value=(repo.name);
                                        input type="hidden" name="target_namespace" value=(target);
                                        button class="twig-btn twig-btn--ghost" type="submit" { (target) }
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

/// Deletion is offered only once the namespace holds no repositories.
fn render_namespace_deletion_panel(namespace: &str, deletable: bool) -> maud::Markup {
    maud::html! {
        section
            class="twig-panel twig-panel--danger"
            aria-labelledby="namespace-delete-heading"
        {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="namespace-delete-heading" { "Delete Namespace" }
            }

            @if deletable {
                div class="twig-panel-body twig-stack twig-stack--tight" {
                    div class="twig-notice twig-notice--warning" role="alert" {
                        p class="twig-eyebrow" { "CAUTION" }
                        p class="twig-notice-body" { "Deleting this namespace permanently removes it. This action cannot be undone." }
                    }
                    div id="delete-namespace-result" aria-live="polite" {}
                }
                form
                    hx-post="/settings/delete-namespace/"
                    hx-target="#delete-namespace-result"
                    hx-swap="innerHTML"
                    hx-confirm=(format!("Are you sure you want to permanently delete the namespace '{namespace}'? This cannot be undone."))
                {
                    input type="hidden" name="namespace" value=(namespace);
                    button class="twig-btn twig-btn--danger" type="submit" { "Delete Namespace" }
                }
            } @else {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NOT EMPTY" }
                    p class="twig-empty-body" { "Delete every repository in this namespace before deleting it." }
                }
            }
        }
    }
}

fn render_namespace_error(username: &str, message: &str) -> maud::Markup {
    crate::http::view::render_layout(
        &crate::http::view::render_error(message),
        Some(username),
        Some("Namespace"),
    )
}

#[get("/manage/namespaces")]
pub async fn namespaces_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> HttpResponse {
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Found()
            .insert_header(("Location", "/auth/login"))
            .finish();
    };
    let db = auth_state.db();
    let Some(user) = (match db.get_user_by_id(&user_id).await {
        Ok(user) => user,
        Err(error) => {
            log::error!("Failed to load user for namespace tree: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    }) else {
        return HttpResponse::Unauthorized()
            .insert_header(("Location", "/auth/login"))
            .finish();
    };
    let is_site_admin = server.is_configured_admin(&user.username);
    let page_data = match load_namespace_page_data(db, &server, &user_id, is_site_admin).await {
        Ok(page_data) => page_data,
        Err(error) => {
            log::error!("Failed to load namespace tree data: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let content = maud::html! {
        (render_tree_hub(
            server.is_test_user(&user.username),
            is_site_admin,
            page_data.user_management_enabled,
            Some("namespaces"),
        ))
        div class="twig-bento" {
            (render_namespaces_page(&page_data.manageable_namespaces))
        }
    };
    let content = if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::http::view::render_layout(&content, Some(&user.username), Some("Namespace"))
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}

#[get("/manage/namespaces/{namespace}/settings")]
pub async fn namespace_settings_page(
    req: HttpRequest,
    path: web::Path<String>,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> HttpResponse {
    let namespace = path.into_inner();
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Found()
            .insert_header(("Location", "/auth/login"))
            .finish();
    };
    let db = auth_state.db();
    let Some(user) = (match db.get_user_by_id(&user_id).await {
        Ok(user) => user,
        Err(error) => {
            log::error!("Failed to load user for namespace settings: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    }) else {
        return HttpResponse::Unauthorized()
            .insert_header(("Location", "/auth/login"))
            .finish();
    };
    let is_site_admin = server.is_configured_admin(&user.username);

    let namespace_id = match db.get_namespace_by_name(&namespace).await {
        Ok(Some(record)) => record.id,
        Ok(None) => {
            return HttpResponse::NotFound()
                .content_type("text/html")
                .body(render_namespace_error(&user.username, "Namespace not found").into_string());
        }
        Err(error) => {
            log::error!("Failed to load namespace for settings: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    match permissions::has(
        db,
        &user_id,
        &namespace,
        is_site_admin,
        NamespacePermission::ManageNamespace,
    )
    .await
    {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Forbidden().content_type("text/html").body(
                render_namespace_error(&user.username, "Required namespace permission is missing")
                    .into_string(),
            );
        }
        Err(error) => {
            log::error!("Failed to check namespace permission: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    }

    let page_data = match load_namespace_page_data(db, &server, &user_id, is_site_admin).await {
        Ok(page_data) => page_data,
        Err(error) => {
            log::error!("Failed to load namespace settings data: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let repo_manageable = page_data.repository_namespace_names.contains(&namespace);
    let repos = page_data
        .repos_by_namespace
        .iter()
        .find(|(name, _)| name == &namespace)
        .map(|(_, repos)| repos.as_slice())
        .unwrap_or_default();
    let move_targets: Vec<String> = page_data
        .repository_namespace_names
        .iter()
        .filter(|name| name.as_str() != namespace.as_str())
        .cloned()
        .collect();
    let deletable = !git::bare::namespace::has_any_repository(server.project_root(), &namespace);
    let pinned = match db.get_pinned_namespace_ids(&user_id).await {
        Ok(ids) => ids.contains(&namespace_id),
        Err(error) => {
            log::error!("Failed to load namespace pins for settings: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let content = maud::html! {
        (render_tree_hub(
            server.is_test_user(&user.username),
            is_site_admin,
            page_data.user_management_enabled,
            Some("namespaces"),
        ))
        (render_namespace_settings(&namespace, pinned, repos, repo_manageable, &move_targets, deletable))
    };
    let content = if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::http::view::render_layout(&content, Some(&user.username), Some("Namespace"))
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}
