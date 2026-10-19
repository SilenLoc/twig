use crate::auth::TwigContext;
use crate::config;
use crate::git;
use actix_web::{HttpRequest, HttpResponse, get, web};

use super::hub::render_tree_hub;
use super::repositories::load_namespace_page_data;

pub(super) fn render_namespaces_page(
    repos_by_namespace: &[(String, Vec<git::bare::RepoInfo>)],
    repository_namespaces: &[String],
    manageable_namespaces: &[String],
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
                    !repos.is_empty()
                        && repository_namespaces
                            .iter()
                            .any(|target| target != source)
                }) {
                    p class="twig-empty-body" { "There is no other namespace you can move repositories to." }
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
                                    @for target in repository_namespaces {
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
        (render_namespace_rename_panel(manageable_namespaces))
        (render_namespace_deletion_panel(deletable_namespaces))
    }
}

/// Namespaces the user can manage, each with a field for a new name.
pub(super) fn render_namespace_rename_panel(owned_namespaces: &[String]) -> maud::Markup {
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
            server.is_test_user_enabled(),
            is_site_admin,
            page_data.user_management_enabled,
            Some("namespaces"),
        ))
        (render_namespaces_page(
            &page_data.repos_by_namespace,
            &page_data.repository_namespace_names,
            &page_data.manageable_namespaces,
            &page_data.deletable_namespaces,
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
