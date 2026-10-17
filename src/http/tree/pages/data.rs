use crate::auth::TwigContext;
use crate::config;
use crate::db::data::TablePage;
use actix_web::{HttpRequest, HttpResponse, get, web};

use super::{DataQuery, RowsQuery, render_tree_hub};

pub(super) const DATA_PAGE_SIZE: usize = 50;

/// Upper bound on the OFFSET the rows route accepts. The infinite-scroll chain
/// only ever requests page-aligned offsets counting up from 0, so anything
/// larger is a hand-crafted URL; reject it instead of making `SQLite` walk that
/// many rows (OFFSET scans are O(offset)). Offsets past the end of a table
/// within this bound are fine — they yield an empty page and the chain stops.
const MAX_DATA_OFFSET: usize = 1_000_000;

pub(super) fn render_data_page(
    tables: &[String],
    selected: &str,
    page: Option<&TablePage>,
    offset: usize,
    test_enabled: bool,
) -> maud::Markup {
    maud::html! {
        (render_tree_hub(test_enabled, true, true, Some("data")))
        section class="twig-panel" {
            header class="twig-panel-head" {
                h1 class="twig-eyebrow" { "Database tables" }
                p class="twig-row-meta" { "Read-only table data" }
            }
            div class="twig-panel-body" {
                nav class="twig-tabs" aria-label="Database tables" {
                    @for table in tables {
                        a class="twig-tab" aria-current=[(table == selected).then_some("page")] href=(format!("/tree/data?table={table}")) { (table) }
                    }
                }
                @if let Some(page) = page {
                    @if page.rows.is_empty() && offset == 0 {
                        p class="twig-empty-body" { "This table has no rows." }
                    } @else {
                        div class="twig-table-scroll" {
                            table class="twig-table" {
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
                td class="twig-data" { (value) }
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
            td colspan="100" class="twig-row-meta" { "Loading more rows…" }
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
    auth_state: &web::Data<TwigContext>,
) -> Result<String, HttpResponse> {
    let Some(admin_user) = server.admin_user() else {
        return Err(HttpResponse::NotFound().finish());
    };
    let username = crate::http::auth::session::get_username_from_request(req, auth_state).await;
    if !username
        .as_deref()
        .is_some_and(|username| server.is_configured_admin(username))
    {
        return Err(HttpResponse::Forbidden().body("Forbidden"));
    }
    Ok(admin_user.to_string())
}

#[get("/tree/data")]
pub async fn data_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
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
        crate::http::view::render_layout(&content, Some(&username), Some("Database Data"))
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}

#[get("/tree/data/rows")]
pub async fn data_rows(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<RowsQuery>,
) -> HttpResponse {
    if let Err(response) = require_admin(&req, &server, &auth_state).await {
        return response;
    }
    if query.offset > MAX_DATA_OFFSET {
        return HttpResponse::BadRequest().body("Offset out of range");
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
