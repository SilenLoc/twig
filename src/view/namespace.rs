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

/// Renders the repository listing shared by the full-page and HTMX responses.
fn render_namespace(
    namespace: &str,
    search_query: &str,
    has_access: bool,
    repos: &[git::bare::RepoInfo],
) -> maud::Markup {
    maud::html! {
        div class="mb3 mb4-ns tf-kicker white-50" {
            a href="/" class="link white-50 hover-white no-underline" { "Namespaces" }
            span class="mh2" { "/" }
            span class="white" { (namespace) }
        }

        h1 class="tf-hero white ma0 mb3 mb4-ns word-wrap" style="overflow-wrap: anywhere;" {
            (namespace)
        }

        div class="flex flex-wrap justify-between items-end mb3 mb4-ns" {
            @if has_access {
                button
                    hx-get=(format!("/{}/create-repo-form", namespace))
                    hx-target="#create-repo-container"
                    hx-swap="innerHTML"
                    class="tf-btn"
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
                    class="tf-input flex-auto mr2"
                    style="min-width: 0;";
                button
                    type="submit"
                    class="tf-btn tf-btn-ghost"
                {
                    "Search"
                }
                @if !search_query.is_empty() {
                    a
                        href=(format!("/{}", namespace))
                        class="ml2 pa2 link white-70 hover-white no-underline tf-kicker"
                    {
                        "Clear"
                    }
                }
            }
        }

        div class="ba b--white-20 bg-black-20 overflow-hidden overflow-x-auto" {
            div class="flex pa3 bb b--white-20 white-50 f6 fw6 tf-kicker" {
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
                            class="flex pa3 bb b--white-10 link white hover-white hover-bg-white-10 no-underline items-baseline"
                        {
                            div class="flex-auto" {
                                span class="f4 fw6" style="letter-spacing: -0.01em;" { (repo.name) }
                            }
                            div class="tr white-50 f6 dn db-ns" style="min-width: 150px;" {
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
        git::bare::get_repos_with_info(server.project_root(), namespace)
    } else {
        git::bare::search_repos_with_info(server.project_root(), namespace, search_query)
    };

    let content = render_namespace(namespace, search_query, has_access, &repos);

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
        div class="ba b--white-20 pa3 bg-black-20" {
            div class="flex justify-between items-center mb3" {
                h2 class="tf-section white ma0" { "Create Repository" }
                button
                    onclick="document.getElementById('create-repo-container').innerHTML = ''"
                    class="pa1 bg-transparent white bn pointer hover-white-70 tf-kicker"
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
                    label class="db tf-kicker white-50 mb2" for="repo_name" { "Repository Name" }
                    input
                        type="text"
                        name="repo_name"
                        id="repo_name"
                        required
                        minlength="1"
                        class="tf-input db w-100"
                        placeholder="Enter repository name (e.g., my-project)";
                }
                div class="mb3" {
                    label class="db tf-kicker white-50 mb2" for="branch" { "Default Branch" }
                    input
                        type="text"
                        name="branch"
                        id="branch"
                        value="main"
                        class="tf-input db w-100"
                        placeholder="main";
                }
                button
                    type="submit"
                    class="tf-btn tf-btn-block"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_date_years() {
        let date = chrono::Utc::now() - chrono::Duration::days(800);
        let result = format_date(&date);
        assert!(
            result.contains("years ago"),
            "expected years ago, got {result}"
        );
    }

    #[test]
    fn test_format_date_months() {
        let date = chrono::Utc::now() - chrono::Duration::days(100);
        let result = format_date(&date);
        assert!(
            result.contains("months ago"),
            "expected months ago, got {result}"
        );
    }

    #[test]
    fn test_format_date_days() {
        let date = chrono::Utc::now() - chrono::Duration::days(5);
        let result = format_date(&date);
        assert!(
            result.contains("days ago"),
            "expected days ago, got {result}"
        );
    }

    #[test]
    fn test_format_date_hours() {
        let date = chrono::Utc::now() - chrono::Duration::hours(3);
        let result = format_date(&date);
        assert!(
            result.contains("hours ago"),
            "expected hours ago, got {result}"
        );
    }

    #[test]
    fn test_format_date_minutes() {
        let date = chrono::Utc::now() - chrono::Duration::minutes(10);
        let result = format_date(&date);
        assert!(
            result.contains("minutes ago"),
            "expected minutes ago, got {result}"
        );
    }

    #[test]
    fn test_format_date_just_now() {
        let date = chrono::Utc::now();
        assert_eq!(format_date(&date), "just now");
    }

    #[test]
    fn test_params_deserialization() {
        let params: Params = serde_urlencoded::from_str("namespace=my-ns").unwrap();
        assert_eq!(params.namespace, "my-ns");
    }

    #[test]
    fn test_create_repo_form_deserialization() {
        let form: CreateRepoForm =
            serde_urlencoded::from_str("repo_name=my-repo&branch=main").unwrap();
        assert_eq!(form.repo_name, "my-repo");
        assert_eq!(form.branch, "main");
    }

    #[test]
    fn test_create_repo_form_default_branch() {
        let form: CreateRepoForm = serde_urlencoded::from_str("repo_name=my-repo").unwrap();
        assert_eq!(form.repo_name, "my-repo");
        assert_eq!(form.branch, "main");
    }

    #[test]
    fn test_search_query_deserialization() {
        let query: SearchQuery = serde_urlencoded::from_str("q=search-term").unwrap();
        assert_eq!(query.q, Some("search-term".to_string()));
    }

    #[test]
    fn test_search_query_empty() {
        let query: SearchQuery = serde_urlencoded::from_str("").unwrap();
        assert_eq!(query.q, None);
    }
}
