use std::path::Path;

use actix_web::Result as AwResult;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use log::info;
use serde::Deserialize;

use super::{render_error, render_success};
use crate::auth::FigContext;
use crate::config;
use crate::git;

#[derive(Deserialize)]
struct UpdateEmailForm {
    email: String,
}

#[derive(Deserialize)]
struct DeleteRepoForm {
    namespace: String,
    repo_name: String,
}

/// Renders the settings page body shared by the full-page and HTMX responses.
fn render_settings(
    user: &crate::auth::User,
    repos_by_namespace: &[(String, Vec<git::bare::RepoInfo>)],
    deletable_namespaces: &[String],
) -> maud::Markup {
    maud::html! {
        div class="mb3 mb4-ns tf-kicker white-50" {
            a href="/" class="link white-50 hover-white no-underline" { "Home" }
            span class="mh2" { "/" }
            span class="white" { "Settings" }
        }

        h1 class="tf-hero white mb3 mb4-ns" { "Settings" }

        (render_profile_section(user))
        (render_repo_deletion_section(repos_by_namespace))
        (render_namespace_deletion_section(deletable_namespaces))
    }
}

/// Username and email form.
fn render_profile_section(user: &crate::auth::User) -> maud::Markup {
    maud::html! {
    div class="ba b--white-20 pa3 pa4-ns bg-black-20 mb3 mb4-ns" {
        h2 class="tf-section white mb3" { "Profile Information" }

        div class="mb3 mb4-ns" {
            label class="db tf-kicker white-50 mb2" { "Username" }
            p class="f5 white ma0" { (user.username) }
        }

        div class="mb3 mb4-ns" {
            label class="db tf-kicker white-50 mb2" { "Email" }
            @match &user.email {
                Some(email) => {
                    p class="f5 white ma0" style="word-break: break-all;" { (email) }
                }
                None => {
                    p class="f5 white-40 ma0" { "Not set" }
                }
            }
        }

        hr class="bt b--white-20 mv3 mv4-ns";

        h3 class="f5 fw6 white mb3" { "Update Email" }

        form
            hx-post="/settings/email"
            hx-target="#settings-result"
            hx-swap="innerHTML"
        {
            div class="mb3" {
                label class="db tf-kicker white-50 mb2" for="email" { "Email Address" }
                input
                    type="email"
                    name="email"
                    id="email"
                    required
                    value=(user.email.as_deref().unwrap_or(""))
                    class="tf-input db w-100"
                    placeholder="Enter your email address";
            }
            button
                type="submit"
                class="tf-btn"
            {
                "Save Email"
            }
        }

        div id="settings-result" {}
    }
    }
}

/// Per-namespace list of repositories the user may delete.
fn render_repo_deletion_section(
    repos_by_namespace: &[(String, Vec<git::bare::RepoInfo>)],
) -> maud::Markup {
    maud::html! {
    div class="ba b--white-20 pa3 pa4-ns bg-black-20" {
        h2 class="tf-section white mb3" { "Delete Repository" }

        @if repos_by_namespace.is_empty() {
            p class="f6 white-40 ma0" { "You don't have any repositories to delete." }
        } @else {
            p class="f6 white-70 mb3" { "Select a repository to permanently delete it. This action cannot be undone." }

            div id="delete-repo-result" {}

            @for (ns, repos) in repos_by_namespace {
                div class="mb3" {
                    h4 class="tf-kicker white-50 mb2" { (ns) }
                    div class="flex flex-column" {
                        @for repo in repos {
                            form
                                class="flex flex-wrap justify-between items-center pa2 bb b--white-10"
                                hx-post="/settings/delete-repo"
                                hx-target="#delete-repo-result"
                                hx-swap="innerHTML"
                                hx-confirm=(format!("Are you sure you want to permanently delete '{}/{}'?", ns, repo.name))
                            {
                                input type="hidden" name="namespace" value=(ns);
                                input type="hidden" name="repo_name" value=(repo.name);
                                span class="f6 white mb1 mb0-ns" { (repo.name) }
                                button
                                    type="submit"
                                    class="tf-btn tf-btn-danger f7"
                                    style="padding: 0.4rem 0.9rem;"
                                {
                                    "Delete"
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

/// Namespaces the user owns that hold no repositories, and so can be deleted.
fn render_namespace_deletion_section(deletable_namespaces: &[String]) -> maud::Markup {
    maud::html! {
    div class="ba b--white-20 pa3 pa4-ns bg-black-20 mt3 mt4-ns" {
        h2 class="tf-section white mb3" { "Delete Namespace" }

        @if deletable_namespaces.is_empty() {
            p class="f6 white-40 ma0" { "No namespaces available for deletion. You can only delete namespaces you own that have no repositories." }
        } @else {
            p class="f6 white-70 mb3" { "Select a namespace to permanently delete it. This action cannot be undone." }

            div id="delete-namespace-result" {}

            div class="flex flex-column" {
                @for ns_name in deletable_namespaces {
                    form
                        class="flex flex-wrap justify-between items-center pa2 bb b--white-10"
                        hx-post="/settings/delete-namespace/"
                        hx-target="#delete-namespace-result"
                        hx-swap="innerHTML"
                        hx-confirm=(format!("Are you sure you want to permanently delete the namespace '{}'? This cannot be undone.", ns_name))
                    {
                        input type="hidden" name="namespace" value=(ns_name);
                        span class="f6 white mb1 mb0-ns" { (ns_name) }
                        button
                            type="submit"
                            class="tf-btn tf-btn-danger f7"
                            style="padding: 0.4rem 0.9rem;"
                        {
                            "Delete"
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
    auth_state: web::Data<FigContext>,
) -> AwResult<maud::Markup> {
    // Get user from session
    let Some(cookie) = req.cookie("session") else {
        return Ok(maud::html! {
            div class="ba b--red br2 pa3 bg-dark-red" {
                p class="f6 white ma0" { "Not logged in. Please log in first." }
            }
        });
    };
    let token = cookie.value().to_string();

    let Some(user_id) = auth_state.validate_token(&token).await else {
        return Ok(maud::html! {
            div class="ba b--red br2 pa3 bg-dark-red" {
                p class="f6 white ma0" { "Session expired. Please log in again." }
            }
        });
    };

    let db = auth_state.db();

    let Ok(Some(user)) = db.get_user_by_id(&user_id).await else {
        return Ok(maud::html! {
            div class="ba b--red br2 pa3 bg-dark-red" {
                p class="f6 white ma0" { "Failed to load user." }
            }
        });
    };

    // Load namespaces and repos the user has access to
    let namespaces = match db.get_namespaces_for_user(&user_id).await {
        Ok(n) => n,
        Err(e) => {
            log::error!("Failed to get namespaces: {e}");
            Vec::new()
        }
    };

    let mut repos_by_namespace: Vec<(String, Vec<git::bare::RepoInfo>)> = Vec::new();
    let mut deletable_namespaces: Vec<String> = Vec::new();
    for ns in &namespaces {
        let has_repos = git::bare::namespace::has_any_repository(server.project_root(), &ns.name);
        if ns.owner_id == user_id && !has_repos {
            deletable_namespaces.push(ns.name.clone());
        }
        let repos = git::bare::get_repos_with_info(server.project_root(), &ns.name);
        let deletable_repos: Vec<_> = repos
            .into_iter()
            .filter(|repo| {
                let config =
                    git::bare::FigConfig::load(server.project_root(), &ns.name, &repo.name);
                config.deleteable
            })
            .collect();
        if !deletable_repos.is_empty() {
            repos_by_namespace.push((ns.name.clone(), deletable_repos));
        }
    }

    let content = render_settings(&user, &repos_by_namespace, &deletable_namespaces);

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(crate::view::render_layout(&content, Some(&user.username)))
    }
}

#[post("/settings/email")]
pub async fn update_email(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
    form: web::Form<UpdateEmailForm>,
) -> impl Responder {
    // Validate email format (basic validation) before requiring a session,
    // so malformed input is rejected as BAD_REQUEST regardless of auth state.
    if form.email.is_empty() || !form.email.contains('@') {
        return HttpResponse::BadRequest()
            .body(render_error("Please enter a valid email address").into_string());
    }

    // Get user from session
    let Some(cookie) = req.cookie("session") else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };
    let token = cookie.value().to_string();

    let Some(user_id) = auth_state.validate_token(&token).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Session expired. Please log in again.").into_string());
    };

    let db = auth_state.db();

    // Update email in database
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
    auth_state: web::Data<FigContext>,
    form: web::Form<DeleteRepoForm>,
) -> impl Responder {
    // Get user from session
    let Some(cookie) = req.cookie("session") else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };
    let token = cookie.value().to_string();

    let Some(user_id) = auth_state.validate_token(&token).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Session expired. Please log in again.").into_string());
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

    // Check if repository is marked as deletable in fig.toml
    let fig_config =
        git::bare::FigConfig::load(server.project_root(), &form.namespace, &form.repo_name);
    if !fig_config.deleteable {
        return HttpResponse::Forbidden()
            .body(render_error("Repository is not marked as deletable. Set deleteable=true in .fig.toml to enable deletion.").into_string());
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

#[derive(Debug, Clone, Deserialize)]
pub struct NamespaceForm {
    pub namespace: String,
}

#[post("/settings/delete-namespace/")]
pub async fn delete_namespace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    form: web::Form<NamespaceForm>,
) -> impl Responder {
    let Some(cookie) = req.cookie("session") else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };
    let token = cookie.value().to_string();

    let Some(user_id) = auth_state.validate_token(&token).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Session expired. Please log in again.").into_string());
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
}
