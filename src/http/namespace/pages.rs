use actix_web::Result as AwResult;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use log::info;
use serde::Deserialize;

use crate::http::auth::session::get_username_from_request;
use crate::http::view::{render_error, render_error_with_action};
use crate::{
    auth::{
        TwigContext,
        permissions::{self, NamespacePermission},
    },
    config,
    git::{self, repo::bare_init},
};

async fn user_can_create_repositories(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
) -> bool {
    user_has_namespace_permission_for_request(
        req,
        auth_state,
        server,
        namespace,
        NamespacePermission::CreateRepositories,
    )
    .await
}

async fn user_has_namespace_permission_for_request(
    req: &HttpRequest,
    auth_state: &web::Data<TwigContext>,
    server: &config::Server,
    namespace: &str,
    permission: NamespacePermission,
) -> bool {
    let Some(user_id) = auth_state.user_id_from_request(req).await else {
        return false;
    };
    user_has_namespace_permission(auth_state, server, &user_id, namespace, permission).await
}

async fn user_has_namespace_permission(
    auth_state: &TwigContext,
    server: &config::Server,
    user_id: &str,
    namespace: &str,
    permission: NamespacePermission,
) -> bool {
    let db = auth_state.db();
    let user = match db.get_user_by_id(user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return false,
        Err(e) => {
            log::error!("Failed to load user for namespace permission check: {e}");
            return false;
        }
    };
    match permissions::has(
        db,
        user_id,
        namespace,
        server.is_configured_admin(&user.username),
        permission,
    )
    .await
    {
        Ok(allowed) => allowed,
        Err(error) => {
            log::error!("Failed to check namespace permission: {error}");
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

/// The exact UTC time behind a relative date, shown on hover.
pub(crate) fn absolute_date(date: &chrono::DateTime<chrono::Utc>) -> String {
    format!("{} UTC", date.format("%Y-%m-%d %H:%M"))
}

fn format_date(date: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(*date);

    if duration.num_days() >= 365 {
        let years = duration.num_days() / 365;
        format!("{years} year{} ago", if years == 1 { "" } else { "s" })
    } else if duration.num_days() >= 30 {
        let months = duration.num_days() / 30;
        format!("{months} month{} ago", if months == 1 { "" } else { "s" })
    } else if duration.num_days() > 0 {
        let days = duration.num_days();
        format!("{days} day{} ago", if days == 1 { "" } else { "s" })
    } else if duration.num_hours() > 0 {
        let hours = duration.num_hours();
        format!("{hours} hour{} ago", if hours == 1 { "" } else { "s" })
    } else if duration.num_minutes() > 0 {
        let minutes = duration.num_minutes();
        format!(
            "{minutes} minute{} ago",
            if minutes == 1 { "" } else { "s" }
        )
    } else {
        "just now".to_string()
    }
}

/// Renders the repository listing shared by the full-page and HTMX responses.
fn render_namespace(
    namespace: &str,
    search_query: &str,
    can_create_repositories: bool,
    repos: &[git::bare::RepoInfo],
) -> maud::Markup {
    let namespace_href = format!("/{namespace}");

    maud::html! {
        div class="twig-pagehead" {
            nav class="twig-crumbs twig-crumbs--page" aria-label="Breadcrumb" {
                a href="/" { "Namespaces" }
                span class="twig-crumb-sep" aria-hidden="true" { "/" }
                h1 class="twig-crumb-current" aria-current="page" { (namespace) }
            }
            @if can_create_repositories {
                div class="twig-cluster" {
                    button
                        type="button"
                        class="twig-btn twig-btn--primary"
                        hx-get=(format!("/{namespace}/create-repo-form"))
                        hx-target="#create-repo-container"
                        hx-swap="innerHTML"
                    {
                        "Create repo"
                    }
                }
            }
        }


        div class="twig-stack" {
            form class="twig-search" role="search" method="GET" action=(namespace_href) {
                label class="twig-sr" for="repo-search" { "Search repositories" }
                input
                    class="twig-input twig-input--mono"
                    id="repo-search"
                    type="search"
                    name="q"
                    value=(search_query)
                    placeholder="Search repositories..."
                    hx-get=(namespace_href)
                    hx-target="#repo-results"
                    hx-select="#repo-results"
                    hx-swap="outerHTML"
                    hx-push-url="true"
                    hx-trigger="input changed delay:200ms";
                @if !search_query.is_empty() {
                    a class="twig-btn twig-btn--quiet" href=(namespace_href) { "Clear" }
                }
            }

            @if can_create_repositories {
                div id="create-repo-container" {}
            }

            section id="repo-results" class="twig-panel twig-panel--flush" {
                div class="twig-panel-body" {
                    @if repos.is_empty() {
                        @if search_query.is_empty() {
                            div class="twig-empty" {
                                p class="twig-eyebrow" { "NO REPOSITORIES" }
                                p class="twig-empty-body" {
                                    @if can_create_repositories {
                                        "No repositories yet. Click 'Create repo' to add one!"
                                    } @else {
                                        "There are no repositories in this namespace yet."
                                    }
                                }
                            }
                        } @else {
                            div class="twig-empty" {
                                p class="twig-eyebrow" { "NO MATCHES" }
                                p class="twig-empty-body" { "No repositories found matching your search." }
                                div class="twig-empty-actions" {
                                    a class="twig-btn twig-btn--ghost" href=(namespace_href) { "Clear search" }
                                }
                            }
                        }
                    } @else {
                        div class="twig-colhead" {
                            span { "Repository" }
                            span { "Last commit" }
                        }
                        div class="twig-list" {
                            @for repo in repos {
                                a class="twig-row" href=(format!("{namespace}/{}", repo.name)) {
                                    span class="twig-row-id" { (repo.name) }
                                    span class="twig-row-meta" {
                                        @if repo.is_private {
                                            span class="twig-tag" { "Private" }
                                        }
                                        span class="twig-sr" { "Last commit: " }
                                        @match repo.last_commit_date {
                                            Some(date) => {
                                                time datetime=(date.to_rfc3339()) title=(absolute_date(&date)) {
                                                    (format_date(&date))
                                                }
                                            }
                                            None => { "No commits" }
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
}

#[get("/{namespace}")]
pub async fn handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
    query: web::Query<SearchQuery>,
) -> AwResult<maud::Markup> {
    let namespace = &params.namespace;
    if let Err(error) = crate::git::reserved::validate_namespace_name(namespace) {
        return Err(actix_web::error::ErrorBadRequest(error));
    }
    let search_query = query.q.as_deref().unwrap_or("");
    let username = get_username_from_request(&req, &auth_state).await;
    let can_view_private_repositories = user_has_namespace_permission_for_request(
        &req,
        &auth_state,
        &server,
        namespace,
        NamespacePermission::ViewNamespace,
    )
    .await;
    let can_create_repositories =
        user_can_create_repositories(&req, &server, &auth_state, namespace).await;

    let repos = if search_query.is_empty() {
        git::bare::get_repos_with_info(server.project_root(), namespace)
    } else {
        git::bare::search_repos_with_info(server.project_root(), namespace, search_query)
    };

    let repos: Vec<_> = if can_view_private_repositories {
        repos
    } else {
        repos.into_iter().filter(|r| !r.is_private).collect()
    };

    let content = render_namespace(namespace, search_query, can_create_repositories, &repos);

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(crate::http::view::render_layout(
            &content,
            username.as_deref(),
            Some(namespace),
        ))
    }
}

/// The origin shown in clone and push instructions: `PUBLIC_BASE_URL` when the
/// server is configured with one, otherwise the scheme and host of this request.
fn clone_base_url(req: &HttpRequest, server: &config::Server) -> String {
    if let Some(base) = server.public_base_url() {
        return base.trim_end_matches('/').to_string();
    }
    let info = req.connection_info();
    format!("{}://{}", info.scheme(), info.host())
}

/// The result of a successful create, kept on screen so the push instructions
/// can be read and copied.
fn render_repo_created(namespace: &str, repo: &str, branch: &str, base: &str) -> maud::Markup {
    let remote = format!("{base}/{namespace}/{repo}");
    let commands = format!("git remote add origin {remote}\ngit push -u origin {branch}");
    maud::html! {
        div class="twig-notice twig-notice--success" role="status" {
            p class="twig-eyebrow" { "DONE" }
            p class="twig-notice-body" {
                "Created " (namespace) "/" (repo) ". Push an existing project to it with:"
            }
            pre class="twig-code" tabindex="0" aria-label="Push commands" {
                code { (commands) }
            }
            div class="twig-notice-actions" {
                a class="twig-btn twig-btn--primary" href=(format!("/{namespace}/{repo}")) { "Open repository" }
            }
        }
    }
}

/// Renders the create-repository panel swapped into `#create-repo-container`.
fn render_create_repo_form(namespace: &str) -> maud::Markup {
    maud::html! {
        section class="twig-panel" aria-labelledby="create-repo-heading" {
            header class="twig-panel-head" {
                h2 id="create-repo-heading" class="twig-eyebrow" { "Create Repository" }
                div class="twig-panel-action" {
                    button
                        type="button"
                        class="twig-btn twig-btn--quiet"
                        onclick="document.getElementById('create-repo-container').innerHTML = ''"
                    {
                        "Cancel"
                    }
                }
            }
            div class="twig-panel-body" {
                form
                    class="twig-form twig-form--narrow"
                    hx-post=(format!("/{namespace}/create-repo"))
                    hx-target="#create-repo-result"
                    "hx-status:4xx"="swap:innerHTML target:#create-repo-result"
                    "hx-status:5xx"="swap:innerHTML target:#create-repo-result"
                    hx-swap="innerHTML"
                {
                    div class="twig-field" {
                        label class="twig-label" for="repo_name" { "Repository Name" }
                        input
                            class="twig-input twig-input--mono"
                            type="text"
                            name="repo_name"
                            id="repo_name"
                            required
                            minlength="1"
                            placeholder="Enter repository name (e.g., my-project)";
                    }
                    div class="twig-field" {
                        label class="twig-label" for="branch" { "Default Branch" }
                        input
                            class="twig-input twig-input--mono"
                            type="text"
                            name="branch"
                            id="branch"
                            value="main"
                            placeholder="main";
                    }
                    div class="twig-form-actions" {
                        button type="submit" class="twig-btn twig-btn--primary" { "Create Repository" }
                    }
                }
                div id="create-repo-result" aria-live="polite" {}
            }
        }
    }
}

#[get("/{namespace}/create-repo-form")]
pub async fn create_repo_form_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> impl Responder {
    if let Err(message) = crate::git::reserved::validate_namespace_name(&params.namespace) {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
    }
    if !user_can_create_repositories(&req, &server, &auth_state, &params.namespace).await {
        return HttpResponse::Forbidden()
            .body(render_error("Only namespace owners can create repositories").into_string());
    }
    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_create_repo_form(&params.namespace).into_string())
}

#[post("/{namespace}/create-repo")]
pub async fn create_repo_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
    form: web::Form<CreateRepoForm>,
) -> impl Responder {
    let namespace = &params.namespace;
    if let Err(message) = crate::git::reserved::validate_namespace_name(namespace) {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
    }

    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        let message = if req.cookie("session").is_some() {
            "Session expired. Please log in again."
        } else {
            "Not logged in. Please log in first."
        };
        return HttpResponse::Unauthorized().body(render_error(message).into_string());
    };

    let db = auth_state.db();

    if !user_has_namespace_permission(
        &auth_state,
        &server,
        &user_id,
        namespace,
        NamespacePermission::CreateRepositories,
    )
    .await
    {
        return HttpResponse::Forbidden()
            .body(render_error("Only namespace owners can create repositories").into_string());
    }

    if let Err(message) = crate::git::reserved::validate_repo_name(&form.repo_name) {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
    }

    let namespace_path = match crate::git::repo::ensure_namespace_directory(
        std::path::Path::new(server.project_root()),
        namespace,
    ) {
        Ok(path) => path,
        Err(error) => {
            log::error!("Failed to prepare namespace directory: {error}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to prepare namespace directory").into_string());
        }
    };
    let repo_path = namespace_path.join(&form.repo_name);

    if repo_path.exists() {
        return HttpResponse::Conflict()
            .body(render_error("Repository already exists").into_string());
    }

    if let Err(e) = std::fs::create_dir_all(&repo_path) {
        log::error!("Failed to create repository directory: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to create repository directory").into_string());
    }

    let Ok(Some(user)) = db.get_user_by_id(&user_id).await else {
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to load user").into_string());
    };

    // Require user to have set an email before creating repositories
    let Some(email) = &user.email else {
        let _ = std::fs::remove_dir_all(&repo_path);
        return missing_email_response();
    };
    let author_email = email.as_str();

    // Initialize bare repository with user info
    match bare_init(&repo_path, &form.branch, &user.username, author_email) {
        Ok(_) => {
            info!(
                "Created repository '{}/{}' via UI for user: {}",
                namespace, form.repo_name, user_id
            );
            let base = clone_base_url(&req, &server);
            HttpResponse::Ok()
                .content_type("text/html")
                .body(render_repo_created(namespace, &form.repo_name, &form.branch, &base).into_string())
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

fn missing_email_response() -> HttpResponse {
    HttpResponse::BadRequest().body(
        render_error_with_action(
            "Please set your email in your account before creating a repository.",
            "/settings",
            "Go to Account",
        )
        .into_string(),
    )
}

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
