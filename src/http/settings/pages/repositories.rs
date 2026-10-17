use super::{
    HttpRequest, HttpResponse, Responder, TwigContext, config, git, info, post, render_error,
    render_success, require_namespace_owner, web,
};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
pub(super) struct DeleteRepoForm {
    pub(super) namespace: String,
    pub(super) repo_name: String,
}

#[derive(Deserialize)]
pub(super) struct MoveRepoForm {
    pub(super) source_namespace: String,
    pub(super) repo_name: String,
    pub(super) target_namespace: String,
}

#[derive(Deserialize)]
pub(super) struct RenameRepoForm {
    pub(super) namespace: String,
    pub(super) repo_name: String,
    pub(super) new_name: String,
}

/// Repositories the user may delete, one technical form row each.
pub(crate) fn render_repo_deletion_panel(
    repos_by_namespace: &[(String, Vec<git::bare::RepoInfo>)],
) -> maud::Markup {
    maud::html! {
        section
            class="twig-panel twig-panel--danger"
            aria-labelledby="settings-delete-repo-heading"
        {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="settings-delete-repo-heading" { "Delete Repository" }
            }

            @if repos_by_namespace.is_empty() {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO REPOSITORIES" }
                    p class="twig-empty-body" { "You don't have any repositories to delete." }
                }
            } @else {
                div class="twig-panel-body" {
                    div class="twig-notice twig-notice--warning" role="alert" {
                        p class="twig-eyebrow" { "CAUTION" }
                        p class="twig-notice-body" { "Select a repository to permanently delete it. This action cannot be undone." }
                    }
                    div id="delete-repo-result" aria-live="polite" {}
                }

                div class="twig-list" {
                    @for (ns, repos) in repos_by_namespace {
                        @for repo in repos {
                            form
                                class="twig-row twig-row--form"
                                hx-post="/settings/delete-repo"
                                hx-target="#delete-repo-result"
                                hx-swap="innerHTML"
                                hx-confirm=(format!("Are you sure you want to permanently delete '{}/{}'?", ns, repo.name))
                            {
                                input type="hidden" name="namespace" value=(ns);
                                input type="hidden" name="repo_name" value=(repo.name);
                                span class="twig-row-id" { (ns) "/" (repo.name) }
                                button class="twig-btn twig-btn--danger" type="submit" { "Delete" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Repositories the user may rename, one technical form row each. Renaming
/// keeps the repository in its namespace and preserves its history and
/// configuration; only its web and Git URLs change.
pub(crate) fn render_repo_rename_panel(
    repos_by_namespace: &[(String, Vec<git::bare::RepoInfo>)],
) -> maud::Markup {
    maud::html! {
        section class="twig-panel" aria-labelledby="settings-rename-repo-heading" {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="settings-rename-repo-heading" { "Rename Repository" }
            }

            @if repos_by_namespace.is_empty() {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO REPOSITORIES" }
                    p class="twig-empty-body" { "You don't have any repositories to rename." }
                }
            } @else {
                div class="twig-panel-body twig-stack twig-stack--tight" {
                    p class="twig-body-sm twig-ink-secondary" {
                        "Renaming a repository changes its web and Git URLs. Its history and configuration are preserved."
                    }
                    div id="rename-repo-result" aria-live="polite" {}
                }

                div class="twig-list" {
                    @for (ns, repos) in repos_by_namespace {
                        @for repo in repos {
                            form
                                class="twig-row twig-row--form"
                                hx-post="/settings/rename-repo"
                                hx-target="#rename-repo-result"
                                hx-swap="innerHTML"
                                "hx-status:4xx"="swap:innerHTML target:#rename-repo-result"
                                "hx-status:5xx"="swap:innerHTML target:#rename-repo-result"
                                hx-confirm=(format!("Rename '{}/{}'? Its URL will change.", ns, repo.name))
                            {
                                input type="hidden" name="namespace" value=(ns);
                                input type="hidden" name="repo_name" value=(repo.name);
                                span class="twig-row-id" { (ns) "/" (repo.name) }
                                div class="twig-field" {
                                    label class="twig-label" {
                                        "New name"
                                        input
                                            class="twig-input twig-input--mono"
                                            type="text"
                                            name="new_name"
                                            required
                                            autocomplete="off"
                                            spellcheck="false"
                                            value=(repo.name);
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
}

#[post("/settings/delete-repo")]
pub async fn delete_repo(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<DeleteRepoForm>,
) -> impl Responder {
    // Get user from session
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };

    let db = auth_state.db();

    if let Err(response) = require_namespace_owner(db, &user_id, &form.namespace).await {
        return response;
    }

    // Build the repository path
    let repo_path = Path::new(server.project_root())
        .join(&form.namespace)
        .join(&form.repo_name);

    // Check that the path exists and is a git repository
    if !repo_path.exists() {
        return HttpResponse::NotFound().body(render_error("Repository not found").into_string());
    }

    if git2::Repository::open(&repo_path).is_err() {
        return HttpResponse::BadRequest()
            .body(render_error("Path is not a valid git repository").into_string());
    }

    // Check if repository is marked as deletable in twig.toml
    let twig_config =
        git::bare::TwigConfig::load(server.project_root(), &form.namespace, &form.repo_name);
    if !twig_config.deleteable {
        return HttpResponse::Forbidden()
            .body(render_error("Repository is not marked as deletable. Set deleteable=true in .twig.toml to enable deletion.").into_string());
    }

    // Delete the repository
    match std::fs::remove_dir_all(&repo_path) {
        Ok(()) => {
            info!(
                "Deleted repository '{}/{}' by user: {}",
                form.namespace, form.repo_name, user_id
            );
            HttpResponse::Ok()
                .content_type("text/html")
                .body(render_success("Repository deleted successfully.").into_string())
        }
        Err(e) => {
            log::error!("Failed to delete repository: {e}");
            HttpResponse::InternalServerError()
                .body(render_error("Failed to delete repository").into_string())
        }
    }
}

#[post("/settings/move-repo")]
pub async fn move_repo(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<MoveRepoForm>,
) -> impl Responder {
    if !git::bare::is_safe_component(&form.source_namespace)
        || !git::bare::is_safe_component(&form.target_namespace)
    {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace name").into_string());
    }
    if let Err(message) = git::reserved::validate_repo_name(&form.repo_name) {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
    }
    if form.repo_name.trim() != form.repo_name {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid repository name").into_string());
    }
    if form.source_namespace == form.target_namespace {
        return HttpResponse::BadRequest()
            .body(render_error("Choose a different destination namespace").into_string());
    }

    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };

    let db = auth_state.db();
    for (name, description) in [
        (&form.source_namespace, "source"),
        (&form.target_namespace, "destination"),
    ] {
        let _namespace = match db.get_namespace_by_name(name).await {
            Ok(Some(namespace)) => namespace,
            Ok(None) => {
                return HttpResponse::NotFound().body(
                    render_error(&format!("{description} namespace not found")).into_string(),
                );
            }
            Err(e) => {
                log::error!("Failed to load {description} namespace: {e}");
                return HttpResponse::InternalServerError()
                    .body(render_error("Database error").into_string());
            }
        };
        if let Err(response) = require_namespace_owner(db, &user_id, name).await {
            return response;
        }
    }

    let source = Path::new(server.project_root())
        .join(&form.source_namespace)
        .join(&form.repo_name);
    let destination_namespace = Path::new(server.project_root()).join(&form.target_namespace);
    let destination = destination_namespace.join(&form.repo_name);

    if !source.exists() {
        return HttpResponse::NotFound().body(render_error("Repository not found").into_string());
    }
    if git2::Repository::open(&source).is_err() {
        return HttpResponse::BadRequest()
            .body(render_error("Path is not a valid git repository").into_string());
    }
    if destination.exists() {
        return HttpResponse::Conflict().body(
            render_error("A repository with that name already exists in the destination namespace")
                .into_string(),
        );
    }
    if let Err(e) = std::fs::create_dir_all(&destination_namespace) {
        log::error!("Failed to create destination namespace directory: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to prepare destination namespace").into_string());
    }
    if let Err(e) = std::fs::rename(&source, &destination) {
        log::error!("Failed to move repository: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to move repository").into_string());
    }

    info!(
        "Moved repository '{}/{}' to '{}/{}' by user: {}",
        form.source_namespace, form.repo_name, form.target_namespace, form.repo_name, user_id
    );
    HttpResponse::Ok().content_type("text/html").body(
        render_success(&format!(
            "Repository moved to {}/{}.",
            form.target_namespace, form.repo_name
        ))
        .into_string(),
    )
}

#[post("/settings/rename-repo")]
pub async fn rename_repo(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<RenameRepoForm>,
) -> impl Responder {
    if !git::bare::is_safe_component(&form.namespace) {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace name").into_string());
    }
    if let Err(message) = git::reserved::validate_repo_name(&form.repo_name) {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
    }
    if form.repo_name.trim() != form.repo_name {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid repository name").into_string());
    }
    if let Err(message) = git::reserved::validate_repo_name(&form.new_name) {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
    }
    if form.new_name.trim() != form.new_name {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid repository name").into_string());
    }
    if form.new_name == form.repo_name {
        return HttpResponse::BadRequest()
            .body(render_error("Choose a different repository name").into_string());
    }

    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };

    let db = auth_state.db();
    let _namespace = match db.get_namespace_by_name(&form.namespace).await {
        Ok(Some(namespace)) => namespace,
        Ok(None) => {
            return HttpResponse::NotFound()
                .body(render_error("Namespace not found").into_string());
        }
        Err(e) => {
            log::error!("Failed to load namespace: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    };
    if let Err(response) = require_namespace_owner(db, &user_id, &form.namespace).await {
        return response;
    }

    let namespace_dir = Path::new(server.project_root()).join(&form.namespace);
    let source = namespace_dir.join(&form.repo_name);
    let destination = namespace_dir.join(&form.new_name);

    if !source.exists() {
        return HttpResponse::NotFound().body(render_error("Repository not found").into_string());
    }
    if git2::Repository::open(&source).is_err() {
        return HttpResponse::BadRequest()
            .body(render_error("Path is not a valid git repository").into_string());
    }
    if destination.exists() {
        return HttpResponse::Conflict().body(
            render_error("A repository with that name already exists in this namespace")
                .into_string(),
        );
    }
    if let Err(e) = std::fs::rename(&source, &destination) {
        log::error!("Failed to rename repository: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to rename repository").into_string());
    }

    info!(
        "Renamed repository '{}/{}' to '{}/{}' by user: {}",
        form.namespace, form.repo_name, form.namespace, form.new_name, user_id
    );
    HttpResponse::Ok().content_type("text/html").body(
        render_success(&format!(
            "Repository renamed to {}/{}.",
            form.namespace, form.new_name
        ))
        .into_string(),
    )
}
