use actix_web::Result as AwResult;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use log::info;
use serde::Deserialize;

use super::session_auth::get_username_from_request;
use super::{render_error, render_success};
use crate::{
    auth::FigContext,
    config,
    git::{self, repo::bare_init},
};

async fn user_has_namespace_access(
    req: &HttpRequest,
    auth_state: &web::Data<FigContext>,
    namespace: &str,
) -> bool {
    let Some(cookie) = req.cookie("session") else {
        return false;
    };
    let token = cookie.value().to_string();

    let Some(user_id) = auth_state.validate_token(&token).await else {
        return false;
    };

    let db = auth_state.db();

    match db.user_has_namespace_access(&user_id, namespace).await {
        Ok(access) => access,
        Err(e) => {
            log::error!("Failed to check namespace access: {e}");
            false
        }
    }
}

#[derive(Deserialize)]
struct Params {
    namespace: String,
}

#[derive(Deserialize)]
struct CreateRepoForm {
    repo_name: String,
    #[serde(default = "crate::git::repo::default_branch")]
    branch: String,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

fn format_date(date: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(*date);

    if duration.num_days() > 365 {
        format!("{} years ago", duration.num_days() / 365)
    } else if duration.num_days() > 30 {
        format!("{} months ago", duration.num_days() / 30)
    } else if duration.num_days() > 0 {
        format!("{} days ago", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{} hours ago", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{} minutes ago", duration.num_minutes())
    } else {
        "just now".to_string()
    }
}

#[get("/{namespace}")]
pub async fn handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<Params>,
    query: web::Query<SearchQuery>,
) -> AwResult<maud::Markup> {
    let namespace = &params.namespace;
    let search_query = query.q.as_deref().unwrap_or("");
    let username = get_username_from_request(&req, &auth_state).await;
    let has_access = user_has_namespace_access(&req, &auth_state, namespace).await;

    // Get repos with info (last commit date)
    let repos = if search_query.is_empty() {
        git::bare::get_repos_with_info(server.project_root(), namespace).unwrap_or_else(|e| {
            log::error!("Failed to get repos: {e}");
            Vec::new()
        })
    } else {
        git::bare::search_repos_with_info(server.project_root(), namespace, search_query)
            .unwrap_or_else(|e| {
                log::error!("Failed to search repos: {e}");
                Vec::new()
            })
    };

    let content = maud::html! {
        div class="mb3 mb4-ns f6 white-70" {
            a href="/" class="link white-70 hover-white no-underline" { "Namespaces" }
            span class="mh2" { "/" }
            span class="white" { (namespace) }
        }

        div class="flex flex-wrap justify-between items-center mb3 mb4-ns" {
            h1 class="f3 fw6 white ma0 mb2 mb0-ns" {
                "Namespace: " (namespace)
            }
            @if has_access {
                button
                    hx-get=(format!("/{}/create-repo-form", namespace))
                    hx-target="#create-repo-container"
                    hx-swap="innerHTML"
                    class="pa2 bg-white black bn br1 pointer hover-bg-white-90 f6"
                {
                    "Create repo"
                }
            }
        }

        div id="create-repo-container" class="mb3 mb4-ns" {}

        div class="mb3 mb4-ns" {
            form
                method="GET"
                action=(format!("/{}", namespace))
                class="flex items-center"
            {
                input
                    type="text"
                    name="q"
                    value=(search_query)
                    placeholder="Search repositories..."
                    class="flex-auto pa2 bg-black white ba b--white-30 br1 mr2"
                    style="outline: none; min-width: 0;"
                    onfocus="this.style.borderColor='white'"
                    onblur="this.style.borderColor='rgba(255,255,255,0.3)'";
                button
                    type="submit"
                    class="pa2 bg-white-10 white bn br1 pointer hover-bg-white-20"
                {
                    "Search"
                }
                @if !search_query.is_empty() {
                    a
                        href=(format!("/{}", namespace))
                        class="ml2 pa2 link white-70 hover-white no-underline"
                    {
                        "Clear"
                    }
                }
            }
        }

        div class="ba b--white-20 br2 bg-black-20 overflow-hidden overflow-x-auto" {
            div class="flex pa3 bb b--white-20 bg-black-30 white-70 f6 fw6" {
                div class="flex-auto" { "Repository" }
                div class="tr dn db-ns" style="min-width: 150px;" { "Last Commit" }
            }

            @if repos.is_empty() {
                div class="pa4 tc" {
                    @if search_query.is_empty() {
                        p class="f6 white-70" { "No repositories yet. Click 'Create repo' to add one!" }
                    } @else {
                        p class="f6 white-70" { "No repositories found matching your search." }
                    }
                }
            } @else {
                div class="flex flex-column" {
                    @for repo in repos {
                        a
                            href=(format!("{}/{}", namespace, repo.name))
                            class="flex pa3 bb b--white-10 link white-90 hover-white hover-bg-white-10 no-underline items-center"
                        {
                            div class="flex-auto" {
                                span class="f5" { (repo.name) }
                            }
                            div class="tr white-60 f6 dn db-ns" style="min-width: 150px;" {
                                @match repo.last_commit_date {
                                    Some(date) => { (format_date(&date)) }
                                    None => { "No commits" }
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
        Ok(super::render_layout(&content, username.as_deref()))
    }
}

#[get("/{namespace}/create-repo-form")]
pub async fn create_repo_form_handler(
    _req: HttpRequest,
    params: web::Path<Params>,
) -> AwResult<maud::Markup> {
    let namespace = &params.namespace;

    let form = maud::html! {
        div class="ba b--white-20 br2 pa3 bg-black-20" {
            div class="flex justify-between items-center mb3" {
                h2 class="f4 fw6 white ma0" { "Create Repository" }
                button
                    onclick="document.getElementById('create-repo-container').innerHTML = ''"
                    class="pa1 bg-transparent white bn pointer hover-white-70 f6"
                {
                    "✕ Cancel"
                }
            }
            form
                hx-post=(format!("/{}/create-repo", namespace))
                hx-target="#create-repo-result"
                hx-target-error="#create-repo-result"
                hx-swap="innerHTML"
                hx-on::after-request="if(event.detail.successful) { setTimeout(() => { document.getElementById('create-repo-container').innerHTML = ''; window.location.reload(); }, 1500); }"
            {
                div class="mb3" {
                    label class="db f6 white-70 mb2" for="repo_name" { "Repository Name" }
                    input
                        type="text"
                        name="repo_name"
                        id="repo_name"
                        required
                        minlength="1"
                        class="db w-100 pa2 bg-black white ba b--white-30 br1"
                        placeholder="Enter repository name (e.g., my-project)";
                }
                div class="mb3" {
                    label class="db f6 white-70 mb2" for="branch" { "Default Branch" }
                    input
                        type="text"
                        name="branch"
                        id="branch"
                        value="main"
                        class="db w-100 pa2 bg-black white ba b--white-30 br1"
                        placeholder="main";
                }
                button
                    type="submit"
                    class="w-100 pa2 bg-white black bn br1 pointer hover-bg-white-90"
                {
                    "Create Repository"
                }
            }
            div id="create-repo-result" class="mt3" {}
        }
    };

    Ok(form)
}

#[post("/{namespace}/create-repo")]
pub async fn create_repo_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<Params>,
    form: web::Form<CreateRepoForm>,
) -> impl Responder {
    let namespace = &params.namespace;

    // Authenticate user via session cookie
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

    // Check if user has access to namespace
    match db.user_has_namespace_access(&user_id, namespace).await {
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

    // Validate repository name
    if form.repo_name.is_empty() {
        return HttpResponse::BadRequest()
            .body(render_error("Repository name must be at least 1 character").into_string());
    }

    // Create repository path
    let repo_path = std::path::Path::new(server.project_root())
        .join(namespace)
        .join(&form.repo_name);

    // Check if repository already exists
    if repo_path.exists() {
        return HttpResponse::Conflict()
            .body(render_error("Repository already exists").into_string());
    }

    // Create the repository directory
    if let Err(e) = std::fs::create_dir_all(&repo_path) {
        log::error!("Failed to create repository directory: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to create repository directory").into_string());
    }

    // Get user details for git author info
    let Ok(Some(user)) = db.get_user_by_id(&user_id).await else {
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to load user").into_string());
    };

    // Require user to have set an email before creating repositories
    let Some(email) = &user.email else {
        let _ = std::fs::remove_dir_all(&repo_path);
        return HttpResponse::BadRequest()
            .body(render_error("Please set your email in settings before creating a repository. <a href='/settings' class='link white underline'>Go to Settings</a>").into_string());
    };
    let author_email = email.as_str();

    // Initialize bare repository with user info
    match bare_init(&repo_path, &form.branch, &user.username, author_email) {
        Ok(_) => {
            info!(
                "Created repository '{}/{}' via UI for user: {}",
                namespace, form.repo_name, user_id
            );
            let success_msg = format!(
                "Repository '{}' created successfully! You can now push to it using:\n\ngit remote add origin <url>\ngit push -u origin {}",
                form.repo_name, form.branch
            );
            HttpResponse::Ok()
                .content_type("text/html")
                .body(render_success(&success_msg).into_string())
        }
        Err(e) => {
            log::error!("Failed to initialize bare repository: {e}");
            // Clean up the created directory
            let _ = std::fs::remove_dir_all(&repo_path);
            HttpResponse::InternalServerError()
                .body(render_error("Failed to initialize repository").into_string())
        }
    }
}
