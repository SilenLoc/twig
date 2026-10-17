use actix_web::{HttpRequest, HttpResponse, get, web};
use serde::Deserialize;

use crate::{auth::TwigContext, config};
use twig_git as git;

mod data;
#[cfg(test)]
use crate::db::data::TablePage;
#[cfg(test)]
use data::{DATA_PAGE_SIZE, render_data_page};
pub use data::{data_page, data_rows};

#[derive(Debug, Deserialize)]
pub struct DataQuery {
    table: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RowsQuery {
    table: String,
    offset: usize,
}

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

fn render_namespaces_page(
    repos_by_namespace: &[(String, Vec<git::bare::RepoInfo>)],
    owned_namespaces: &[String],
    deletable_namespaces: &[String],
) -> maud::Markup {
    let has_repositories = repos_by_namespace
        .iter()
        .any(|(_, repos)| !repos.is_empty());

    maud::html! {
        section class="twig-panel" aria-labelledby="namespace-repos-heading" {
            header class="twig-panel-head" {
                h1 class="twig-eyebrow" id="namespace-repos-heading" { "Repositories" }
                p class="twig-row-meta" { "Choose a namespace to move each repository." }
            }
            @if !has_repositories {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO REPOSITORIES" }
                    p class="twig-empty-body" { "You don't have any repositories in your namespaces." }
                }
            } @else {
                div class="twig-panel-body twig-stack twig-stack--tight" {
                    p class="twig-body-sm twig-ink-secondary" {
                        "Moving a repository changes its web and Git URLs. Its history and configuration are preserved."
                    }
                    div id="move-repo-result" aria-live="polite" {}
                }
                @if !repos_by_namespace.iter().any(|(source, repos)| {
                    !repos.is_empty() && owned_namespaces.iter().any(|target| target != source)
                }) {
                    p class="twig-empty-body" { "Create another namespace you own to move repositories." }
                }
                div class="twig-list" {
                    @for (source, repos) in repos_by_namespace {
                        @for repo in repos {
                            div class="twig-row" {
                                div class="twig-stack twig-stack--tight" {
                                    span class="twig-row-id" { (source) "/" (repo.name) }
                                    span class="twig-label" { "Move to namespace" }
                                }
                                div class="twig-form-actions" aria-label=(format!("Move {}/{} to namespace", source, repo.name)) {
                                    @for target in owned_namespaces {
                                        @if target != source {
                                            form
                                                hx-post="/settings/move-repo"
                                                hx-target="#move-repo-result"
                                                hx-swap="innerHTML"
                                                "hx-status:4xx"="swap:innerHTML target:#move-repo-result"
                                                "hx-status:5xx"="swap:innerHTML target:#move-repo-result"
                                                hx-confirm=(format!("Move '{}/{}' to the '{}' namespace? Its URL will change.", source, repo.name, target))
                                            {
                                                input type="hidden" name="source_namespace" value=(source);
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
        (render_namespace_rename_panel(owned_namespaces))
        (render_namespace_deletion_panel(deletable_namespaces))
    }
}

/// Namespaces the user owns, each with a field for a new name.
fn render_namespace_rename_panel(owned_namespaces: &[String]) -> maud::Markup {
    maud::html! {
        section class="twig-panel" aria-labelledby="namespace-rename-heading" {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="namespace-rename-heading" { "Rename Namespace" }
            }

            @if owned_namespaces.is_empty() {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO NAMESPACES" }
                    p class="twig-empty-body" { "You don't have any namespaces to rename." }
                }
            } @else {
                div class="twig-panel-body twig-stack twig-stack--tight" {
                    p class="twig-body-sm twig-ink-secondary" {
                        "Renaming a namespace changes the URLs of every repository inside it. Their history and configuration are preserved."
                    }
                    div id="rename-namespace-result" aria-live="polite" {}
                }

                div class="twig-list" {
                    @for ns_name in owned_namespaces {
                        form
                            class="twig-row twig-row--form"
                            hx-post="/settings/rename-namespace"
                            hx-target="#rename-namespace-result"
                            hx-swap="innerHTML"
                            "hx-status:4xx"="swap:innerHTML target:#rename-namespace-result"
                            "hx-status:5xx"="swap:innerHTML target:#rename-namespace-result"
                            hx-confirm=(format!("Rename namespace '{ns_name}'? Every repository URL inside it will change."))
                        {
                            input type="hidden" name="namespace" value=(ns_name);
                            span class="twig-row-id" { (ns_name) }
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
                                        value=(ns_name);
                                }
                            }
                            button class="twig-btn twig-btn--primary" type="submit" { "Rename" }
                        }
                    }
                }
            }
        }
    }
}

/// Namespaces the user owns that hold no repositories, and so can be deleted.
fn render_namespace_deletion_panel(deletable_namespaces: &[String]) -> maud::Markup {
    maud::html! {
        section
            class="twig-panel twig-panel--danger"
            aria-labelledby="namespace-delete-heading"
        {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="namespace-delete-heading" { "Delete Namespace" }
            }

            @if deletable_namespaces.is_empty() {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO NAMESPACES" }
                    p class="twig-empty-body" { "No namespaces available for deletion. You can only delete namespaces you own that have no repositories." }
                }
            } @else {
                div class="twig-panel-body" {
                    div class="twig-notice twig-notice--warning" role="alert" {
                        p class="twig-eyebrow" { "CAUTION" }
                        p class="twig-notice-body" { "Select a namespace to permanently delete it. This action cannot be undone." }
                    }
                    div id="delete-namespace-result" aria-live="polite" {}
                }

                div class="twig-list" {
                    @for ns_name in deletable_namespaces {
                        form
                            class="twig-row twig-row--form"
                            hx-post="/settings/delete-namespace/"
                            hx-target="#delete-namespace-result"
                            hx-swap="innerHTML"
                            hx-confirm=(format!("Are you sure you want to permanently delete the namespace '{}'? This cannot be undone.", ns_name))
                        {
                            input type="hidden" name="namespace" value=(ns_name);
                            span class="twig-row-id" { (ns_name) }
                            button class="twig-btn twig-btn--danger" type="submit" { "Delete" }
                        }
                    }
                }
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

#[get("/tree/namespaces")]
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
    let namespaces = match db.get_owned_namespaces_for_user(&user_id).await {
        Ok(namespaces) => namespaces,
        Err(error) => {
            log::error!("Failed to load namespaces for namespace tree: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let owned_namespaces: Vec<String> = namespaces
        .iter()
        .map(|namespace| namespace.name.clone())
        .collect();
    let user_management_enabled =
        server.is_configured_admin(&user.username) || !owned_namespaces.is_empty();
    let deletable_namespaces: Vec<String> = owned_namespaces
        .iter()
        .filter(|namespace| {
            !git::bare::namespace::has_any_repository(server.project_root(), namespace)
        })
        .cloned()
        .collect();
    let repos_by_namespace: Vec<(String, Vec<git::bare::RepoInfo>)> = owned_namespaces
        .iter()
        .filter_map(|namespace| {
            let repos: Vec<_> = git::bare::get_repos_with_info(server.project_root(), namespace)
                .into_iter()
                .filter(|repo| git::reserved::validate_repo_name(&repo.name).is_ok())
                .collect();
            (!repos.is_empty()).then(|| (namespace.clone(), repos))
        })
        .collect();
    let content = maud::html! {
        (render_tree_hub(
            server.is_test_user_enabled(),
            server.is_configured_admin(&user.username),
            user_management_enabled,
            Some("namespaces"),
        ))
        (render_namespaces_page(
            &repos_by_namespace,
            &owned_namespaces,
            &deletable_namespaces,
        ))
    };
    let content = if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::http::view::render_layout(&content, Some(&user.username), Some("Namespaces"))
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;

#[get("/tree/repositories")]
pub async fn repositories_page(
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
            log::error!("Failed to load user for repository tree: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    }) else {
        return HttpResponse::Unauthorized()
            .insert_header(("Location", "/auth/login"))
            .finish();
    };
    let namespaces = match db.get_owned_namespaces_for_user(&user_id).await {
        Ok(namespaces) => namespaces,
        Err(error) => {
            log::error!("Failed to load namespaces for repository tree: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let user_management_enabled =
        server.is_configured_admin(&user.username) || !namespaces.is_empty();
    let mut renameable_repos_by_namespace: Vec<(String, Vec<git::bare::RepoInfo>)> = Vec::new();
    let mut deletable_repos_by_namespace: Vec<(String, Vec<git::bare::RepoInfo>)> = Vec::new();
    for namespace in namespaces {
        let renameable_repos: Vec<_> =
            git::bare::get_repos_with_info(server.project_root(), &namespace.name)
                .into_iter()
                .filter(|repo| git::reserved::validate_repo_name(&repo.name).is_ok())
                .collect();
        if !renameable_repos.is_empty() {
            renameable_repos_by_namespace.push((namespace.name.clone(), renameable_repos));
        }

        let deletable_repos: Vec<_> =
            git::bare::get_repos_with_info(server.project_root(), &namespace.name)
                .into_iter()
                .filter(|repo| {
                    git::bare::TwigConfig::load(server.project_root(), &namespace.name, &repo.name)
                        .deleteable
                })
                .collect();
        if !deletable_repos.is_empty() {
            deletable_repos_by_namespace.push((namespace.name, deletable_repos));
        }
    }

    let content = maud::html! {
        (render_tree_hub(
            server.is_test_user_enabled(),
            server.is_configured_admin(&user.username),
            user_management_enabled,
            Some("repositories"),
        ))
        div class="twig-bento" {
            (crate::http::settings::pages::render_repo_rename_panel(&renameable_repos_by_namespace))
            (crate::http::settings::pages::render_repo_deletion_panel(&deletable_repos_by_namespace))
        }
    };
    let content = if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::http::view::render_layout(&content, Some(&user.username), Some("Repository"))
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}
