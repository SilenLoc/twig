use actix_web::{HttpRequest, HttpResponse, get, web};
use serde::Deserialize;

use crate::{auth::FigContext, config, db::data::TablePage, git};

#[derive(Debug, Deserialize)]
pub struct DataQuery {
    table: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RowsQuery {
    table: String,
    offset: usize,
}

const DATA_PAGE_SIZE: usize = 50;

pub(crate) fn render_tree_hub(
    test_enabled: bool,
    data_enabled: bool,
    active: Option<&str>,
) -> maud::Markup {
    maud::html! {
        nav class="fig-tabs" aria-label="Tree sections" {
            a class="fig-tab" aria-current=[(active == Some("account")).then_some("page")] href="/settings" { "Account" }
            a class="fig-tab" aria-current=[(active == Some("repositories")).then_some("page")] href="/tree/repositories" { "Repository" }
            a class="fig-tab" aria-current=[(active == Some("namespaces")).then_some("page")] href="/tree/namespaces" { "Namespace" }
            @if test_enabled {
                a class="fig-tab" aria-current=[(active == Some("test")).then_some("page")] href="/_test" { "Test" }
            }
            @if data_enabled {
                a class="fig-tab" aria-current=[(active == Some("data")).then_some("page")] href="/tree/data" { "Data" }
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
        section class="fig-panel" aria-labelledby="namespace-repos-heading" {
            header class="fig-panel-head" {
                h1 class="fig-eyebrow" id="namespace-repos-heading" { "Repositories" }
                p class="fig-row-meta" { "Choose a namespace to move each repository." }
            }
            @if !has_repositories {
                div class="fig-empty" {
                    p class="fig-eyebrow" { "NO REPOSITORIES" }
                    p class="fig-empty-body" { "You don't have any repositories in your namespaces." }
                }
            } @else {
                div class="fig-panel-body fig-stack fig-stack--tight" {
                    p class="fig-body-sm fig-ink-secondary" {
                        "Moving a repository changes its web and Git URLs. Its history and configuration are preserved."
                    }
                    div id="move-repo-result" aria-live="polite" {}
                }
                @if !repos_by_namespace.iter().any(|(source, repos)| {
                    !repos.is_empty() && owned_namespaces.iter().any(|target| target != source)
                }) {
                    p class="fig-empty-body" { "Create another namespace you own to move repositories." }
                }
                div class="fig-list" {
                    @for (source, repos) in repos_by_namespace {
                        @for repo in repos {
                            div class="fig-row" {
                                div class="fig-stack fig-stack--tight" {
                                    span class="fig-row-id" { (source) "/" (repo.name) }
                                    span class="fig-label" { "Move to namespace" }
                                }
                                div class="fig-form-actions" aria-label=(format!("Move {}/{} to namespace", source, repo.name)) {
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
                                                button class="fig-btn fig-btn--ghost" type="submit" { (target) }
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
        (render_namespace_deletion_panel(deletable_namespaces))
    }
}

/// Namespaces the user owns that hold no repositories, and so can be deleted.
fn render_namespace_deletion_panel(deletable_namespaces: &[String]) -> maud::Markup {
    maud::html! {
        section
            class="fig-panel fig-panel--danger"
            aria-labelledby="namespace-delete-heading"
        {
            header class="fig-panel-head" {
                h2 class="fig-eyebrow" id="namespace-delete-heading" { "Delete Namespace" }
            }

            @if deletable_namespaces.is_empty() {
                div class="fig-empty" {
                    p class="fig-eyebrow" { "NO NAMESPACES" }
                    p class="fig-empty-body" { "No namespaces available for deletion. You can only delete namespaces you own that have no repositories." }
                }
            } @else {
                div class="fig-panel-body" {
                    div class="fig-notice fig-notice--warning" role="alert" {
                        p class="fig-eyebrow" { "CAUTION" }
                        p class="fig-notice-body" { "Select a namespace to permanently delete it. This action cannot be undone." }
                    }
                    div id="delete-namespace-result" aria-live="polite" {}
                }

                div class="fig-list" {
                    @for ns_name in deletable_namespaces {
                        form
                            class="fig-row fig-row--form"
                            hx-post="/settings/delete-namespace/"
                            hx-target="#delete-namespace-result"
                            hx-swap="innerHTML"
                            hx-confirm=(format!("Are you sure you want to permanently delete the namespace '{}'? This cannot be undone.", ns_name))
                        {
                            input type="hidden" name="namespace" value=(ns_name);
                            span class="fig-row-id" { (ns_name) }
                            button class="fig-btn fig-btn--danger" type="submit" { "Delete" }
                        }
                    }
                }
            }
        }
    }
}

fn render_data_page(
    tables: &[String],
    selected: &str,
    page: Option<&TablePage>,
    offset: usize,
    test_enabled: bool,
) -> maud::Markup {
    maud::html! {
        (render_tree_hub(test_enabled, true, Some("data")))
        section class="fig-panel" {
            header class="fig-panel-head" {
                h1 class="fig-eyebrow" { "Database tables" }
                p class="fig-row-meta" { "Read-only table data" }
            }
            div class="fig-panel-body" {
                nav class="fig-tabs" aria-label="Database tables" {
                    @for table in tables {
                        a class="fig-tab" aria-current=[(table == selected).then_some("page")] href=(format!("/tree/data?table={table}")) { (table) }
                    }
                }
                @if let Some(page) = page {
                    @if page.rows.is_empty() && offset == 0 {
                        p class="fig-empty-body" { "This table has no rows." }
                    } @else {
                        div class="fig-table-scroll" {
                            table class="fig-table" {
                                thead {
                                    tr {
                                        @for column in &page.columns {
                                            th scope="col" { (column) }
                                        }
                                    }
                                }
                                tbody id="database-rows" {
                                    @for row in &page.rows {
                                        (render_data_row(row))
                                    }
                                    @if page.rows.len() == DATA_PAGE_SIZE {
                                        (render_load_more(selected, offset + page.rows.len()))
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

fn render_data_row(row: &[String]) -> maud::Markup {
    maud::html! {
        tr {
            @for value in row {
                td class="fig-data" { (value) }
            }
        }
    }
}

fn render_load_more(table: &str, offset: usize) -> maud::Markup {
    maud::html! {
        tr
            hx-get=(format!("/tree/data/rows?table={table}&offset={offset}"))
            hx-trigger="revealed"
            hx-swap="outerHTML"
        {
            td colspan="100" class="fig-row-meta" { "Loading more rows…" }
        }
    }
}

fn render_more_rows(table: &str, offset: usize, page: &TablePage) -> maud::Markup {
    maud::html! {
        @for row in &page.rows {
            (render_data_row(row))
        }
        @if page.rows.len() == DATA_PAGE_SIZE {
            (render_load_more(table, offset + page.rows.len()))
        }
    }
}

async fn require_admin(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<FigContext>,
) -> Result<String, HttpResponse> {
    let Some(admin_user) = server.admin_user() else {
        return Err(HttpResponse::NotFound().finish());
    };
    let username = super::session_auth::get_username_from_request(req, auth_state).await;
    if !username
        .as_deref()
        .is_some_and(|username| server.is_configured_admin(username))
    {
        return Err(HttpResponse::Forbidden().body("Forbidden"));
    }
    Ok(admin_user.to_string())
}

#[get("/tree")]
pub async fn tree_page(req: HttpRequest, auth_state: web::Data<FigContext>) -> HttpResponse {
    if super::session_auth::get_username_from_request(&req, &auth_state)
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
    auth_state: web::Data<FigContext>,
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
    let namespaces = match db.get_namespaces_for_user(&user_id).await {
        Ok(namespaces) => namespaces,
        Err(error) => {
            log::error!("Failed to load namespaces for namespace tree: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let owned_namespaces: Vec<String> = namespaces
        .iter()
        .filter(|namespace| namespace.owner_id == user_id)
        .map(|namespace| namespace.name.clone())
        .collect();
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
        super::render_layout(&content, Some(&user.username), Some("Namespaces"))
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}

#[get("/tree/repositories")]
pub async fn repositories_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
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
    let namespaces = match db.get_namespaces_for_user(&user_id).await {
        Ok(namespaces) => namespaces,
        Err(error) => {
            log::error!("Failed to load namespaces for repository tree: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let mut repos_by_namespace: Vec<(String, Vec<git::bare::RepoInfo>)> = Vec::new();
    for namespace in namespaces {
        let deletable_repos: Vec<_> =
            git::bare::get_repos_with_info(server.project_root(), &namespace.name)
                .into_iter()
                .filter(|repo| {
                    git::bare::FigConfig::load(server.project_root(), &namespace.name, &repo.name)
                        .deleteable
                })
                .collect();
        if !deletable_repos.is_empty() {
            repos_by_namespace.push((namespace.name, deletable_repos));
        }
    }

    let content = maud::html! {
        (render_tree_hub(
            server.is_test_user_enabled(),
            server.is_configured_admin(&user.username),
            Some("repositories"),
        ))
        div class="fig-bento" {
            (super::settings::render_repo_deletion_panel(&repos_by_namespace))
        }
    };
    let content = if req.headers().get("HX-Request").is_some() {
        content
    } else {
        super::render_layout(&content, Some(&user.username), Some("Repository"))
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}

#[get("/tree/data")]
pub async fn data_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    query: web::Query<DataQuery>,
) -> HttpResponse {
    let username = match require_admin(&req, &server, &auth_state).await {
        Ok(username) => username,
        Err(response) => return response,
    };
    let db = auth_state.db();
    let tables = match db.list_table_names().await {
        Ok(tables) => tables,
        Err(error) => {
            log::error!("Failed to list database tables: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let selected = query
        .table
        .as_deref()
        .filter(|table| tables.iter().any(|name| name == table))
        .or_else(|| tables.first().map(String::as_str));
    let Some(selected) = selected else {
        return HttpResponse::Ok().body("No database tables found");
    };
    let page = match db.read_table_page(selected, DATA_PAGE_SIZE, 0).await {
        Ok(page) => page,
        Err(error) => {
            log::error!("Failed to read database table '{selected}': {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let content = render_data_page(
        &tables,
        selected,
        page.as_ref(),
        0,
        server.is_test_user_enabled(),
    );
    let content = if req.headers().get("HX-Request").is_some() {
        content
    } else {
        super::render_layout(&content, Some(&username), Some("Database Data"))
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}

#[get("/tree/data/rows")]
pub async fn data_rows(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    query: web::Query<RowsQuery>,
) -> HttpResponse {
    if let Err(response) = require_admin(&req, &server, &auth_state).await {
        return response;
    }
    let page = match auth_state
        .db()
        .read_table_page(&query.table, DATA_PAGE_SIZE, query.offset)
        .await
    {
        Ok(Some(page)) => page,
        Ok(None) => return HttpResponse::NotFound().finish(),
        Err(error) => {
            log::error!("Failed to read database table '{}': {error}", query.table);
            return HttpResponse::InternalServerError().finish();
        }
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_more_rows(&query.table, query.offset, &page).into_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_hub_only_renders_enabled_sections() {
        let minimal = render_tree_hub(false, false, None).into_string();
        assert!(minimal.contains(">Account</a>"));
        assert!(minimal.contains("href=\"/tree/repositories\">Repository</a>"));
        assert!(minimal.contains("href=\"/tree/namespaces\""));
        assert!(!minimal.contains(">Test</a>"));
        assert!(!minimal.contains(">Data</a>"));

        let enabled = render_tree_hub(true, true, Some("data")).into_string();
        assert!(enabled.contains("href=\"/_test\""));
        assert!(enabled.contains("href=\"/tree/namespaces\""));
        assert!(
            !enabled.contains("aria-current=\"page\" href=\"/tree/repositories\">Repository</a>")
        );
        assert!(enabled.contains("href=\"/tree/data\""));
        assert!(enabled.contains("aria-current=\"page\" href=\"/tree/data\""));
        let repository_active = render_tree_hub(false, false, Some("repositories")).into_string();
        assert!(
            repository_active
                .contains("aria-current=\"page\" href=\"/tree/repositories\">Repository</a>")
        );
    }

    #[test]
    fn namespace_page_lists_repositories_with_confirmed_namespace_buttons() {
        let html = render_namespaces_page(
            &[(
                "acme".to_string(),
                vec![git::bare::RepoInfo {
                    name: "fig".to_string(),
                    last_commit_date: None,
                    is_private: false,
                }],
            )],
            &["acme".to_string(), "solo".to_string()],
            &["solo".to_string()],
        )
        .into_string();

        assert!(html.contains("<span class=\"fig-row-id\">acme/fig</span>"));
        assert!(html.contains(
            "hx-confirm=\"Move 'acme/fig' to the 'solo' namespace? Its URL will change.\""
        ));
        assert!(
            html.contains("<button class=\"fig-btn fig-btn--ghost\" type=\"submit\">solo</button>")
        );
        assert!(html.contains("name=\"target_namespace\" value=\"solo\""));
        assert!(!html.contains("<select"));
        assert!(html.contains("Delete Namespace"));
        assert!(!html.contains("NO NAMESPACES"));
        assert!(html.contains("hx-post=\"/settings/delete-namespace/\""));
        assert!(html.contains("hx-confirm=\"Are you sure you want to permanently delete the namespace 'solo'? This cannot be undone.\""));
        assert!(html.contains("<span class=\"fig-row-id\">solo</span>"));
    }

    #[test]
    fn namespace_page_explains_when_no_namespace_can_be_deleted() {
        let html = render_namespaces_page(&[], &[], &[]).into_string();

        assert!(html.contains("Delete Namespace"));
        assert!(html.contains(">NO NAMESPACES<"));
        assert!(html.contains(
            "No namespaces available for deletion. You can only delete namespaces you own that have no repositories."
        ));
        assert!(!html.contains("hx-confirm"));
        assert!(!html.contains("fig-btn--danger"));
    }

    #[test]
    fn data_page_uses_the_infinite_scroll_placeholder() {
        let page = TablePage {
            columns: vec!["id".to_string()],
            rows: (0..DATA_PAGE_SIZE)
                .map(|index| vec![index.to_string()])
                .collect(),
        };
        let html =
            render_data_page(&["users".to_string()], "users", Some(&page), 0, false).into_string();
        assert!(html.contains("hx-trigger=\"revealed\""));
        assert!(html.contains("hx-swap=\"outerHTML\""));
        assert!(html.contains("/tree/data/rows?table=users&amp;offset=50"));
    }
}
