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

#[get("/settings")]
pub async fn settings_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
) -> AwResult<maud::Markup> {
    // Get user from session
    let token = match req.cookie("session") {
        Some(cookie) => cookie.value().to_string(),
        None => {
            return Ok(maud::html! {
                div class="ba b--red br2 pa3 bg-dark-red" {
                    p class="f6 white ma0" { "Not logged in. Please log in first." }
                }
            });
        }
    };

    let user_id = match auth_state.validate_token(&token).await {
        Some(user_id) => user_id,
        None => {
            return Ok(maud::html! {
                div class="ba b--red br2 pa3 bg-dark-red" {
                    p class="f6 white ma0" { "Session expired. Please log in again." }
                }
            });
        }
    };

    let db = auth_state.db();

    let user = match db.get_user_by_id(&user_id).await {
        Ok(Some(user)) => user,
        _ => {
            return Ok(maud::html! {
                div class="ba b--red br2 pa3 bg-dark-red" {
                    p class="f6 white ma0" { "Failed to load user." }
                }
            });
        }
    };

    // Load namespaces and repos the user has access to
    let namespaces = db
        .get_namespaces_for_user(&user_id)
        .await
        .unwrap_or_default();

    let mut repos_by_namespace: Vec<(String, Vec<git::bare::RepoInfo>)> = Vec::new();
    for ns in namespaces {
        let repos =
            git::bare::get_repos_with_info(server.project_root(), &ns.name).unwrap_or_default();
        let deletable_repos: Vec<_> = repos
            .into_iter()
            .filter(|repo| {
                let config =
                    git::bare::FigConfig::load(server.project_root(), &ns.name, &repo.name);
                config.deleteable
            })
            .collect();
        if !deletable_repos.is_empty() {
            repos_by_namespace.push((ns.name, deletable_repos));
        }
    }

    let content = maud::html! {
        // Breadcrumb navigation
        div class="mb4 f6 white-70" {
            a href="/" class="link white-70 hover-white no-underline" { "Home" }
            span class="mh2" { "/" }
            span class="white" { "Settings" }
        }

        h1 class="f3 fw6 white mb4" { "User Settings" }

        div class="ba b--white-20 br2 pa4 bg-black-20 mb4" {
            h2 class="f4 fw6 white mb3" { "Profile Information" }

            div class="mb4" {
                label class="db f6 white-70 mb2" { "Username" }
                p class="f5 white ma0" { (user.username) }
            }

            div class="mb4" {
                label class="db f6 white-70 mb2" { "Email" }
                @match &user.email {
                    Some(email) => {
                        p class="f5 white ma0" { (email) }
                    }
                    None => {
                        p class="f5 white-50 ma0" { "Not set" }
                    }
                }
            }

            hr class="bt b--white-20 mv4";

            h3 class="f5 fw6 white mb3" { "Update Email" }

            form
                hx-post="/settings/email"
                hx-target="#settings-result"
                hx-swap="innerHTML"
                class="mb3"
            {
                div class="mb3" {
                    label class="db f6 white-70 mb2" for="email" { "Email Address" }
                    input
                        type="email"
                        name="email"
                        id="email"
                        required
                        value=(user.email.as_deref().unwrap_or(""))
                        class="db w-100 pa2 bg-black white ba b--white-30 br1"
                        placeholder="Enter your email address";
                }
                button
                    type="submit"
                    class="pa2 bg-white black bn br1 pointer hover-bg-white-90"
                {
                    "Save Email"
                }
            }

            div id="settings-result" {}
        }

        div class="ba b--white-20 br2 pa4 bg-black-20" {
            h2 class="f4 fw6 white mb3" { "Delete Repository" }

            @if repos_by_namespace.is_empty() {
                p class="f6 white-50 ma0" { "You don't have any repositories to delete." }
            } @else {
                p class="f6 white-70 mb3" { "Select a repository to permanently delete it. This action cannot be undone." }

                div id="delete-repo-result" {}

                @for (ns, repos) in repos_by_namespace {
                    div class="mb3" {
                        h4 class="f6 fw6 white-70 mb2" { (ns) }
                        div class="flex flex-column" {
                            @for repo in repos {
                                form
                                    class="flex justify-between items-center pa2 bb b--white-10"
                                    hx-post="/settings/delete-repo"
                                    hx-target="#delete-repo-result"
                                    hx-swap="innerHTML"
                                    hx-confirm=(format!("Are you sure you want to permanently delete '{}/{}'?", ns, repo.name))
                                {
                                    input type="hidden" name="namespace" value=(ns);
                                    input type="hidden" name="repo_name" value=(repo.name);
                                    span class="f6 white" { (repo.name) }
                                    button
                                        type="submit"
                                        class="pa1 bg-dark-red white bn br1 pointer hover-bg-red f6"
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
    };

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
    // Get user from session
    let token = match req.cookie("session") {
        Some(cookie) => cookie.value().to_string(),
        None => {
            return HttpResponse::Unauthorized()
                .body(render_error("Not logged in. Please log in first.").into_string());
        }
    };

    let user_id = match auth_state.validate_token(&token).await {
        Some(user_id) => user_id,
        None => {
            return HttpResponse::Unauthorized()
                .body(render_error("Session expired. Please log in again.").into_string());
        }
    };

    // Validate email format (basic validation)
    if form.email.is_empty() || !form.email.contains('@') {
        return HttpResponse::BadRequest()
            .body(render_error("Please enter a valid email address").into_string());
    }

    let db = auth_state.db();

    // Update email in database
    match db.update_user_email(&user_id, &form.email).await {
        Ok(_) => {
            info!("Updated email for user: {}", user_id);
            HttpResponse::Ok()
                .content_type("text/html")
                .body(render_success("Email updated successfully!").into_string())
        }
        Err(e) => {
            log::error!("Failed to update email: {}", e);
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
    let token = match req.cookie("session") {
        Some(cookie) => cookie.value().to_string(),
        None => {
            return HttpResponse::Unauthorized()
                .body(render_error("Not logged in. Please log in first.").into_string());
        }
    };

    let user_id = match auth_state.validate_token(&token).await {
        Some(user_id) => user_id,
        None => {
            return HttpResponse::Unauthorized()
                .body(render_error("Session expired. Please log in again.").into_string());
        }
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
            log::error!("Database error: {}", e);
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
        Ok(_) => {
            info!(
                "Deleted repository '{}/{}' by user: {}",
                form.namespace, form.repo_name, user_id
            );
            HttpResponse::Ok()
                .content_type("text/html")
                .body(render_success("Repository deleted successfully.").into_string())
        }
        Err(e) => {
            log::error!("Failed to delete repository: {}", e);
            HttpResponse::InternalServerError()
                .body(render_error("Failed to delete repository").into_string())
        }
    }
}
