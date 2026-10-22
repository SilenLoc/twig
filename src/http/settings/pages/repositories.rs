use std::path::Path;

use actix_web::{HttpRequest, HttpResponse, Responder, post, web};
use log::info;

use crate::auth::TwigContext;
use crate::auth::permissions::NamespacePermission;
use crate::config;
use crate::db::Database;
use crate::git;
use crate::http::view::{render_error, render_success};

use super::shared::require_namespace_permission;
use super::{DeleteRepoForm, MoveRepoForm, RenameRepoForm};

async fn require_move_namespace_permissions(
    db: &Database,
    server: &config::Server,
    user_id: &str,
    source_namespace: &str,
    target_namespace: &str,
) -> Result<(), HttpResponse> {
    for (namespace, description) in [
        (source_namespace, "source"),
        (target_namespace, "destination"),
    ] {
        match db.get_namespace_by_name(namespace).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                return Err(HttpResponse::NotFound().body(
                    render_error(&format!("{description} namespace not found")).into_string(),
                ));
            }
            Err(error) => {
                log::error!("Failed to load {description} namespace: {error}");
                return Err(HttpResponse::InternalServerError()
                    .body(render_error("Database error").into_string()));
            }
        }
        require_namespace_permission(
            db,
            server,
            user_id,
            namespace,
            NamespacePermission::ManageRepositories,
        )
        .await?;
    }

    Ok(())
}

/// A repository listed on the manage page. `deletable` mirrors the
/// `deleteable` flag in its `.twig.toml`; only those rows get a Delete control.
pub(crate) struct ManagedRepo {
    pub(crate) namespace: String,
    pub(crate) repo: git::bare::RepoInfo,
    pub(crate) deletable: bool,
}

const REPO_SEARCH_SCRIPT: &str = r#"
(() => {
  const input = document.getElementById('repo-search');
  if (!input) return;
  const rows = Array.from(document.querySelectorAll('[data-repo-row]'));
  const empty = document.getElementById('repo-search-empty');
  input.addEventListener('input', () => {
    const query = input.value.trim().toLowerCase();
    let shown = 0;
    for (const row of rows) {
      const match = row.dataset.repoRow.includes(query);
      row.hidden = !match;
      if (match) shown++;
    }
    empty.hidden = shown > 0;
  });
})();
"#;

/// One searchable, scrollable list of the user's repositories. Each row offers
/// Rename, and Delete when the repository is marked deletable.
pub(crate) fn render_repo_manage_panel(repos: &[ManagedRepo]) -> maud::Markup {
    maud::html! {
        section class="twig-panel" aria-labelledby="manage-repo-heading" {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="manage-repo-heading" { "Repositories" }
            }

            @if repos.is_empty() {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO REPOSITORIES" }
                    p class="twig-empty-body" { "You don't have any repositories to manage." }
                }
            } @else {
                div class="twig-panel-body twig-stack twig-stack--tight" {
                    input
                        class="twig-input twig-input--mono"
                        type="search"
                        id="repo-search"
                        placeholder="Search repositories"
                        aria-label="Search repositories"
                        autocomplete="off"
                        spellcheck="false";
                    div id="repo-manage-result" aria-live="polite" {}
                }

                div class="twig-list twig-list--scroll" {
                    @for item in repos {
                        @let full = format!("{}/{}", item.namespace, item.repo.name);
                        div class="twig-row twig-row--form" data-repo-row=(full.to_lowercase()) {
                            span class="twig-row-id" { (full) }
                            div class="twig-row-actions" {
                                details class="twig-repo-rename" {
                                    summary class="twig-btn" { "Rename" }
                                    form
                                        class="twig-repo-rename-form"
                                        hx-post="/settings/rename-repo"
                                        hx-target="#repo-manage-result"
                                        hx-swap="innerHTML"
                                        hx-confirm=(format!("Rename '{}'? Its URL will change.", full))
                                    {
                                        input type="hidden" name="namespace" value=(item.namespace);
                                        input type="hidden" name="repo_name" value=(item.repo.name);
                                        input
                                            class="twig-input twig-input--mono"
                                            type="text"
                                            name="new_name"
                                            required
                                            autocomplete="off"
                                            spellcheck="false"
                                            aria-label="New name"
                                            value=(item.repo.name);
                                        button class="twig-btn twig-btn--primary" type="submit" { "Rename" }
                                    }
                                }
                                @if item.deletable {
                                    form
                                        hx-post="/settings/delete-repo"
                                        hx-target="#repo-manage-result"
                                        hx-swap="innerHTML"
                                        hx-confirm=(format!("Are you sure you want to permanently delete '{}'?", full))
                                    {
                                        input type="hidden" name="namespace" value=(item.namespace);
                                        input type="hidden" name="repo_name" value=(item.repo.name);
                                        button class="twig-btn twig-btn--danger" type="submit" { "Delete" }
                                    }
                                }
                            }
                        }
                    }
                }
                p id="repo-search-empty" class="twig-empty-body" hidden { "No repositories match your search." }
                script { (maud::PreEscaped(REPO_SEARCH_SCRIPT)) }
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
    if let Err(message) = git::reserved::validate_namespace_name(&form.namespace) {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
    }
    if let Err(message) = git::reserved::validate_repo_name(&form.repo_name) {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
    }
    // Get user from session
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };

    let db = auth_state.db();

    if let Err(response) = require_namespace_permission(
        db,
        &server,
        &user_id,
        &form.namespace,
        NamespacePermission::ManageRepositories,
    )
    .await
    {
        return response;
    }

    // Build the repository path
    let Ok(namespace_path) =
        git::repo::checked_namespace_path(Path::new(server.project_root()), &form.namespace)
    else {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace path").into_string());
    };
    let repo_path = namespace_path.join(&form.repo_name);

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
    if let Err(message) = git::reserved::validate_namespace_name(&form.source_namespace) {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
    }
    if let Err(message) = git::reserved::validate_namespace_name(&form.target_namespace) {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
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
    if let Err(response) = require_move_namespace_permissions(
        db,
        &server,
        &user_id,
        &form.source_namespace,
        &form.target_namespace,
    )
    .await
    {
        return response;
    }

    let root = Path::new(server.project_root());
    let Ok(source_namespace) = git::repo::checked_namespace_path(root, &form.source_namespace)
    else {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace path").into_string());
    };
    let source = source_namespace.join(&form.repo_name);
    let Ok(destination_namespace) = git::repo::checked_namespace_path(root, &form.target_namespace)
    else {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace path").into_string());
    };
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
    let destination_namespace =
        match git::repo::ensure_namespace_directory(root, &form.target_namespace) {
            Ok(path) => path,
            Err(error) => {
                log::error!("Failed to prepare destination namespace directory: {error}");
                return HttpResponse::InternalServerError()
                    .body(render_error("Failed to prepare destination namespace").into_string());
            }
        };
    let destination = destination_namespace.join(&form.repo_name);
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
    if let Err(message) = git::reserved::validate_namespace_name(&form.namespace) {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
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
    match db.get_namespace_by_name(&form.namespace).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound()
                .body(render_error("Namespace not found").into_string());
        }
        Err(e) => {
            log::error!("Failed to load namespace: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    }
    if let Err(response) = require_namespace_permission(
        db,
        &server,
        &user_id,
        &form.namespace,
        NamespacePermission::ManageRepositories,
    )
    .await
    {
        return response;
    }

    let Ok(namespace_dir) =
        git::repo::checked_namespace_path(Path::new(server.project_root()), &form.namespace)
    else {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace path").into_string());
    };
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
