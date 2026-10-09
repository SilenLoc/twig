use std::path::Path;

use actix_web::Result as AwResult;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use log::info;
use serde::Deserialize;

use crate::auth::TwigContext;
use crate::config;
use crate::git;
use crate::http::view::{render_error, render_error_with_action, render_success};

#[derive(Deserialize)]
struct UpdateEmailForm {
    email: String,
}

#[derive(Deserialize)]
struct DeleteRepoForm {
    namespace: String,
    repo_name: String,
}

#[derive(Deserialize)]
struct MoveRepoForm {
    source_namespace: String,
    repo_name: String,
    target_namespace: String,
}

#[derive(Deserialize)]
struct RenameRepoForm {
    namespace: String,
    repo_name: String,
    new_name: String,
}

#[derive(Deserialize)]
struct RenameNamespaceForm {
    namespace: String,
    new_name: String,
}

/// Renders the settings page body shared by the full-page and HTMX responses.
fn render_settings(user: &crate::auth::User) -> maud::Markup {
    maud::html! {
        nav class="twig-crumbs twig-crumbs--page" aria-label="Breadcrumb" {
            a href="/" { "Namespaces" }
            span class="twig-crumb-sep" aria-hidden="true" { "/" }
            span aria-current="page" { "Account" }
        }

        section class="twig-pagehead" {
        }

        div class="twig-bento" {
            (render_profile_panel(user))
        }
    }
}

/// Identity values plus the email update form.
fn render_profile_panel(user: &crate::auth::User) -> maud::Markup {
    maud::html! {
        section class="twig-panel" aria-labelledby="settings-profile-heading" {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="settings-profile-heading" { "Account" }
            }
            div class="twig-panel-body twig-stack" {
                div class="twig-field" {
                    span class="twig-label" { "Username" }
                    p class="twig-body twig-break" { (user.username) }
                }

                div class="twig-field" {
                    span class="twig-label" { "Email" }
                    @match &user.email {
                        Some(email) => { p class="twig-body twig-break" { (email) } }
                        None => { p class="twig-body twig-ink-tertiary" { "Not set" } }
                    }
                }

                form
                    class="twig-form"
                    aria-labelledby="settings-email-heading"
                    hx-post="/settings/email"
                    hx-target="#settings-result"
                    hx-swap="innerHTML"
                {
                    h3 class="twig-subsection" id="settings-email-heading" { "Update Email" }

                    div class="twig-field" {
                        label class="twig-label" for="email" { "Email Address" }
                        input
                            class="twig-input"
                            type="email"
                            name="email"
                            id="email"
                            required
                            value=(user.email.as_deref().unwrap_or(""))
                            placeholder="Enter your email address";
                    }

                    div class="twig-form-actions" {
                        button class="twig-btn twig-btn--primary" type="submit" { "Save Email" }
                    }
                }

                div id="settings-result" aria-live="polite" {}
            }
        }
    }
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

#[get("/settings")]
pub async fn settings_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> AwResult<maud::Markup> {
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        let message = if req.cookie("session").is_some() {
            "Session expired. Please log in again."
        } else {
            "Not logged in. Please log in first."
        };
        return Ok(render_settings_auth_error(&req, message));
    };

    let db = auth_state.db();

    let Ok(Some(user)) = db.get_user_by_id(&user_id).await else {
        return Ok(render_error("Failed to load user."));
    };

    let content = render_settings(&user);

    let content = maud::html! {
        (crate::http::tree::pages::render_tree_hub(
            server.is_test_user_enabled(),
            server.is_configured_admin(&user.username),
            Some("account"),
        ))
        (content)
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(crate::http::view::render_layout(
            &content,
            Some(&user.username),
            Some("Account"),
        ))
    }
}

fn render_settings_auth_error(req: &HttpRequest, message: &str) -> maud::Markup {
    let content = render_error_with_action(message, "/auth/login", "Log in");
    if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::http::view::render_layout(&content, None, Some("Account"))
    }
}

#[post("/settings/email")]
pub async fn update_email(
    req: HttpRequest,
    auth_state: web::Data<TwigContext>,
    form: web::Form<UpdateEmailForm>,
) -> impl Responder {
    // Validate email format (basic validation) before requiring a session,
    // so malformed input is rejected as BAD_REQUEST regardless of auth state.
    if form.email.is_empty() || !form.email.contains('@') {
        return HttpResponse::BadRequest()
            .body(render_error("Please enter a valid email address").into_string());
    }

    // Get user from session
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };

    let db = auth_state.db();

    match db.update_user_email(&user_id, &form.email).await {
        Ok(()) => {
            info!("Updated email for user: {user_id}");
            HttpResponse::Ok()
                .content_type("text/html")
                .body(render_success("Email updated successfully!").into_string())
        }
        Err(e) => {
            log::error!("Failed to update email: {e}");
            HttpResponse::InternalServerError()
                .body(render_error("Failed to update email").into_string())
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

    // Verify user has access to the namespace
    match db
        .user_has_namespace_access(&user_id, &form.namespace)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Forbidden()
                .body(render_error("Access denied to namespace").into_string());
        }
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
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
        let namespace = match db.get_namespace_by_name(name).await {
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
        if namespace.owner_id != user_id {
            return HttpResponse::Forbidden().body(
                render_error("Repositories can only be moved between namespaces you own")
                    .into_string(),
            );
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
    let namespace = match db.get_namespace_by_name(&form.namespace).await {
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
    if namespace.owner_id != user_id {
        return HttpResponse::Forbidden().body(
            render_error("Repositories can only be renamed by their namespace owner").into_string(),
        );
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

#[post("/settings/rename-namespace")]
pub async fn rename_namespace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<RenameNamespaceForm>,
) -> impl Responder {
    if !git::bare::is_safe_component(&form.namespace) {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace name").into_string());
    }
    if form.new_name.len() < 2 {
        return HttpResponse::BadRequest()
            .body(render_error("Namespace name must be at least 2 characters").into_string());
    }
    if form.new_name.starts_with('_') {
        return HttpResponse::BadRequest()
            .body(render_error("Namespace names starting with '_' are reserved").into_string());
    }
    if !git::bare::is_safe_component(&form.new_name) {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace name").into_string());
    }
    if form.new_name.trim() != form.new_name {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace name").into_string());
    }
    if form.new_name == form.namespace {
        return HttpResponse::BadRequest()
            .body(render_error("Choose a different namespace name").into_string());
    }

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
            log::error!("Failed to load namespace: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    };
    if namespace.owner_id != user_id {
        return HttpResponse::Forbidden()
            .body(render_error("Only the namespace owner can rename it").into_string());
    }

    match db.get_namespace_by_name(&form.new_name).await {
        Ok(Some(_)) => {
            return HttpResponse::Conflict()
                .body(render_error("Namespace already exists").into_string());
        }
        Ok(None) => {}
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    }

    let root = Path::new(server.project_root());
    let source = root.join(&form.namespace);
    let destination = root.join(&form.new_name);

    if destination.exists() {
        return HttpResponse::Conflict().body(
            render_error("A directory with that namespace name already exists").into_string(),
        );
    }

    let moved_directory = source.exists();
    if moved_directory && let Err(e) = std::fs::rename(&source, &destination) {
        log::error!("Failed to rename namespace directory: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to rename namespace").into_string());
    }

    if let Err(e) = db.rename_namespace(&namespace.id, &form.new_name).await {
        log::error!("Failed to rename namespace: {e}");
        if moved_directory {
            let _ = std::fs::rename(&destination, &source);
        }
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to rename namespace").into_string());
    }

    info!(
        "Renamed namespace '{}' to '{}' by user: {}",
        form.namespace, form.new_name, user_id
    );
    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_success(&format!("Namespace renamed to {}.", form.new_name)).into_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamespaceForm {
    pub namespace: String,
}

#[post("/settings/delete-namespace/")]
pub async fn delete_namespace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<NamespaceForm>,
) -> impl Responder {
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };

    let db = auth_state.db();

    let namespace = match db.get_namespace_by_name(&form.namespace).await {
        Ok(Some(ns)) => ns,
        Ok(None) => {
            return HttpResponse::NotFound()
                .body(render_error("Namespace not found").into_string());
        }
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    };

    if namespace.owner_id != user_id {
        return HttpResponse::Forbidden()
            .body(render_error("Only the namespace owner can delete it").into_string());
    }

    if git::bare::namespace::has_any_repository(server.project_root(), &form.namespace) {
        return HttpResponse::BadRequest().body(
            render_error(
                "Cannot delete namespace: it still contains repositories. Delete all repositories first.",
            )
            .into_string(),
        );
    }

    if let Err(e) = db.delete_namespace(&namespace.id).await {
        log::error!("Failed to delete namespace: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to delete namespace").into_string());
    }

    let namespace_dir = Path::new(server.project_root()).join(&form.namespace);
    if namespace_dir.is_dir() {
        let _ = std::fs::remove_dir(&namespace_dir);
    }

    info!(
        "Deleted namespace '{}' by user: {}",
        form.namespace, user_id
    );

    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_success("Namespace deleted successfully.").into_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::view::test_util::{classes_in, index_of};

    fn user_fixture(email: Option<&str>) -> crate::auth::User {
        crate::auth::User {
            id: "11111111-2222-3333-4444-555555555555".to_string(),
            username: "silen".to_string(),
            email: email.map(str::to_owned),
            password_hash: "argon2-hash".to_string(),
            created_at: "2026-08-31T09:30:00Z".to_string(),
        }
    }

    fn repo_fixture(name: &str) -> git::bare::RepoInfo {
        git::bare::RepoInfo {
            name: name.to_string(),
            last_commit_date: None,
            is_private: false,
        }
    }

    fn populated_html() -> String {
        render_settings(&user_fixture(Some("silen@example.com"))).into_string()
    }

    fn empty_html() -> String {
        render_settings(&user_fixture(None)).into_string()
    }

    fn repo_panel_html() -> String {
        render_repo_deletion_panel(&[("acme".to_string(), vec![repo_fixture("twig")])])
            .into_string()
    }

    fn empty_repo_panel_html() -> String {
        render_repo_deletion_panel(&[]).into_string()
    }

    fn rename_panel_html() -> String {
        render_repo_rename_panel(&[("acme".to_string(), vec![repo_fixture("twig")])]).into_string()
    }

    fn empty_rename_panel_html() -> String {
        render_repo_rename_panel(&[]).into_string()
    }

    #[test]
    fn test_settings_leads_with_crumbs_and_page_head() {
        let html = populated_html();
        assert!(
            html.contains(
                "<nav class=\"twig-crumbs twig-crumbs--page\" aria-label=\"Breadcrumb\">"
            ),
            "breadcrumb must be a labelled nav landmark: {html}"
        );
        assert!(
            html.contains("<span class=\"twig-crumb-sep\" aria-hidden=\"true\">/</span>"),
            "separators are decorative: {html}"
        );
        assert!(
            html.contains("<span aria-current=\"page\">Account</span>"),
            "final crumb names the account page: {html}"
        );
        let crumbs = index_of(&html, "twig-crumbs");
        let head = index_of(&html, "<section class=\"twig-pagehead\">");
        assert!(
            crumbs < head && head < index_of(&html, "twig-bento"),
            "crumbs sit above the page head and bento: {html}"
        );
    }

    #[test]
    fn test_account_page_renders_account_panel_only() {
        let html = populated_html();
        assert_eq!(
            html.matches("class=\"twig-bento\"").count(),
            1,
            "one bento container: {html}"
        );
        assert_eq!(
            html.matches("<section class=\"twig-panel").count(),
            1,
            "only the account panel appears here: {html}"
        );
        assert_eq!(
            html.matches("class=\"twig-panel-head\"").count(),
            1,
            "every panel declares a head: {html}"
        );
        assert_eq!(
            html.matches("<h2 class=\"twig-eyebrow\"").count(),
            1,
            "panel headings are h2 eyebrows under the page h1: {html}"
        );
        let heading_id = "settings-profile-heading";
        assert!(
            html.contains(&format!("aria-labelledby=\"{heading_id}\"")),
            "panel {heading_id} must be named by its heading: {html}"
        );
        assert!(
            html.contains(&format!(
                "<h2 class=\"twig-eyebrow\" id=\"{heading_id}\">Account</h2>"
            )),
            "missing heading {heading_id}: {html}"
        );
        assert!(
            !html.contains("<h4"),
            "panels never nest a fourth heading level: {html}"
        );
        assert!(!html.contains("Delete Repository"), "{html}");
    }

    #[test]
    fn test_settings_does_not_render_repository_moves() {
        let html = populated_html();
        assert!(!html.contains("/settings/move-repo"), "{html}");
        assert!(!html.contains("Move Repository"), "{html}");
        assert!(!html.contains("Delete Repository"), "{html}");
    }

    #[test]
    fn test_settings_deletion_panels_carry_danger_and_caution_affordances() {
        let html = repo_panel_html();
        assert_eq!(
            html.matches("twig-panel twig-panel--danger").count(),
            1,
            "the repository panel takes the danger variant: {html}"
        );
        assert_eq!(
            html.matches("<div class=\"twig-notice twig-notice--warning\" role=\"alert\">")
                .count(),
            1,
            "the deletion panel warns before it lists targets: {html}"
        );
        assert_eq!(
            html.matches(">CAUTION<").count(),
            1,
            "the signal word carries the meaning, not the colour: {html}"
        );
        assert!(
            html.contains(
                "Select a repository to permanently delete it. This action cannot be undone."
            ),
            "{html}"
        );
        assert_eq!(
            html.matches(
                "<button class=\"twig-btn twig-btn--danger\" type=\"submit\">Delete</button>"
            )
            .count(),
            1,
            "repository deletion uses a full-height danger button: {html}"
        );
    }

    #[test]
    fn test_settings_delete_repo_rows_preserve_the_htmx_contract() {
        let html = repo_panel_html();
        for attribute in [
            "hx-post=\"/settings/delete-repo\"",
            "hx-target=\"#delete-repo-result\"",
            "hx-swap=\"innerHTML\"",
            "hx-confirm=\"Are you sure you want to permanently delete 'acme/twig'?\"",
        ] {
            assert!(
                html.contains(attribute),
                "missing exact attribute {attribute}: {html}"
            );
        }
        assert!(
            html.contains("<form class=\"twig-row twig-row--form\""),
            "deletion targets are technical form rows: {html}"
        );
        assert!(
            html.contains("<span class=\"twig-row-id\">acme/twig</span>"),
            "the row identifier names the same object as the confirm text: {html}"
        );
        for hidden in [
            "<input type=\"hidden\" name=\"namespace\" value=\"acme\">",
            "<input type=\"hidden\" name=\"repo_name\" value=\"twig\">",
        ] {
            assert!(html.contains(hidden), "missing payload {hidden}: {html}");
        }
    }

    #[test]
    fn test_settings_rename_repo_rows_preserve_the_htmx_contract() {
        let html = rename_panel_html();
        for attribute in [
            "hx-post=\"/settings/rename-repo\"",
            "hx-target=\"#rename-repo-result\"",
            "hx-swap=\"innerHTML\"",
            "hx-confirm=\"Rename 'acme/twig'? Its URL will change.\"",
        ] {
            assert!(
                html.contains(attribute),
                "missing exact attribute {attribute}: {html}"
            );
        }
        assert!(
            html.contains("<form class=\"twig-row twig-row--form\""),
            "rename targets are technical form rows: {html}"
        );
        assert!(
            html.contains("<span class=\"twig-row-id\">acme/twig</span>"),
            "the row identifier names the repository being renamed: {html}"
        );
        for hidden in [
            "<input type=\"hidden\" name=\"namespace\" value=\"acme\">",
            "<input type=\"hidden\" name=\"repo_name\" value=\"twig\">",
        ] {
            assert!(html.contains(hidden), "missing payload {hidden}: {html}");
        }
        assert!(
            html.contains(
                "<input class=\"twig-input twig-input--mono\" type=\"text\" name=\"new_name\" required autocomplete=\"off\" spellcheck=\"false\" value=\"twig\">"
            ),
            "the new-name input is labelled, required and prefilled: {html}"
        );
        assert!(
            html.contains(
                "<button class=\"twig-btn twig-btn--primary\" type=\"submit\">Rename</button>"
            ),
            "{html}"
        );
        assert!(
            html.contains("Renaming a repository changes its web and Git URLs."),
            "{html}"
        );
    }

    #[test]
    fn test_settings_rename_panel_empty_state_offers_no_targets() {
        let html = empty_rename_panel_html();
        assert!(html.contains("Rename Repository"), "{html}");
        assert!(html.contains(">NO REPOSITORIES<"), "{html}");
        assert!(
            html.contains("You don't have any repositories to rename."),
            "{html}"
        );
        assert!(!html.contains("hx-confirm"), "{html}");
        assert!(!html.contains("twig-btn--primary"), "{html}");
    }

    #[test]
    fn test_settings_does_not_render_namespace_deletion_controls() {
        let html = populated_html();
        assert!(!html.contains("Delete Namespace"), "{html}");
        assert!(!html.contains("delete-namespace"), "{html}");
    }

    #[test]
    fn test_settings_email_form_preserves_the_htmx_contract() {
        let html = populated_html();
        for attribute in [
            "hx-post=\"/settings/email\"",
            "hx-target=\"#settings-result\"",
            "hx-swap=\"innerHTML\"",
        ] {
            assert!(
                html.contains(attribute),
                "missing exact attribute {attribute}: {html}"
            );
        }
        assert!(
            html.contains("<label class=\"twig-label\" for=\"email\">Email Address</label>"),
            "every input keeps a real label: {html}"
        );
        assert!(
            html.contains(
                "<input class=\"twig-input\" type=\"email\" name=\"email\" id=\"email\" required value=\"silen@example.com\""
            ),
            "the email input keeps its type, requirement and current value: {html}"
        );
        assert!(
            html.contains(
                "<button class=\"twig-btn twig-btn--primary\" type=\"submit\">Save Email</button>"
            ),
            "{html}"
        );
    }

    #[test]
    fn test_settings_result_targets_announce_swaps() {
        let html = populated_html();
        assert!(
            html.contains("<div id=\"settings-result\" aria-live=\"polite\">"),
            "the email swap target must be a polite live region: {html}"
        );
    }

    #[test]
    fn test_settings_keeps_identity_visible_and_email_break_safe() {
        let html = populated_html();
        assert!(
            html.contains("<span class=\"twig-label\">Username</span>"),
            "{html}"
        );
        assert!(
            html.contains("<p class=\"twig-body twig-break\">silen</p>"),
            "{html}"
        );
        assert!(
            html.contains("<p class=\"twig-body twig-break\">silen@example.com</p>"),
            "long addresses must wrap rather than overflow: {html}"
        );

        let unset = empty_html();
        assert!(
            unset.contains("<p class=\"twig-body twig-ink-tertiary\">Not set</p>"),
            "a missing email reads as tertiary ink, not as an error: {unset}"
        );
        assert!(unset.contains("silen"), "username stays visible: {unset}");
    }

    #[test]
    fn test_settings_empty_states_name_their_condition_and_offer_no_targets() {
        let html = empty_repo_panel_html();
        assert_eq!(
            html.matches("<div class=\"twig-empty\">").count(),
            1,
            "repository tab degrades to an empty state: {html}"
        );
        assert!(html.contains(">NO REPOSITORIES<"), "{html}");
        assert!(
            html.contains("You don't have any repositories to delete."),
            "{html}"
        );
        assert!(
            !html.contains("hx-confirm"),
            "no deletion affordance without a deletable object: {html}"
        );
        assert!(
            !html.contains("twig-btn--danger"),
            "no danger control without a deletable object: {html}"
        );
    }

    #[test]
    fn test_settings_uses_only_twig_design_system_classes() {
        for html in [
            populated_html(),
            empty_html(),
            repo_panel_html(),
            empty_repo_panel_html(),
            rename_panel_html(),
            empty_rename_panel_html(),
        ] {
            let classes = classes_in(&html);
            assert!(!classes.is_empty(), "settings should carry classes: {html}");
            for class in classes {
                assert!(
                    class.starts_with("twig-"),
                    "non design-system class {class:?} in settings: {html}"
                );
            }
            assert!(
                !html.contains("style=\""),
                "no inline style attributes: {html}"
            );
        }
    }

    #[test]
    fn test_namespace_form_deserialization() {
        let form: NamespaceForm = serde_urlencoded::from_str("namespace=my-namespace").unwrap();
        assert_eq!(form.namespace, "my-namespace");
    }

    #[test]
    fn test_update_email_form_deserialization() {
        let form: UpdateEmailForm = serde_urlencoded::from_str("email=test@example.com").unwrap();
        assert_eq!(form.email, "test@example.com");
    }

    #[test]
    fn test_delete_repo_form_deserialization() {
        let form: DeleteRepoForm =
            serde_urlencoded::from_str("namespace=ns&repo_name=repo").unwrap();
        assert_eq!(form.namespace, "ns");
        assert_eq!(form.repo_name, "repo");
    }

    #[test]
    fn test_move_repo_form_deserialization() {
        let form: MoveRepoForm = serde_urlencoded::from_str(
            "source_namespace=source&repo_name=repo&target_namespace=target",
        )
        .unwrap();
        assert_eq!(form.source_namespace, "source");
        assert_eq!(form.repo_name, "repo");
        assert_eq!(form.target_namespace, "target");
    }

    #[test]
    fn test_rename_repo_form_deserialization() {
        let form: RenameRepoForm =
            serde_urlencoded::from_str("namespace=ns&repo_name=old&new_name=new").unwrap();
        assert_eq!(form.namespace, "ns");
        assert_eq!(form.repo_name, "old");
        assert_eq!(form.new_name, "new");
    }

    #[test]
    fn test_rename_namespace_form_deserialization() {
        let form: RenameNamespaceForm =
            serde_urlencoded::from_str("namespace=oldns&new_name=newns").unwrap();
        assert_eq!(form.namespace, "oldns");
        assert_eq!(form.new_name, "newns");
    }
}
