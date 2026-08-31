//! Per-namespace issue tracker web UI.
//!
//! Tickets live as TOML files in a per-namespace ticket repository (see
//! `crate::ticket`) and are edited both from this UI and by `git push`. Every
//! mutation goes through the ingest lock in `crate::ticket::ops`, which is why
//! every store/ops call below runs inside `web::block`.

use std::collections::BTreeSet;

use actix_web::Result as AwResult;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use serde::Deserialize;

use super::session_auth::get_username_from_request;
use super::{render_error, render_success};
use crate::{
    auth::FigContext,
    config,
    db::Database,
    ticket::{
        markdown,
        model::{Body, Comment, Link, Status, Ticket},
        ops::{self, Actor},
        repo::PUSH_CONFIG_LINE,
        store::TicketStore,
    },
};

#[derive(Deserialize)]
struct Params {
    namespace: String,
}

#[derive(Deserialize)]
struct TicketParams {
    namespace: String,
    number: u64,
}

#[derive(Deserialize)]
struct ListQuery {
    status: Option<String>,
}

#[derive(Deserialize)]
struct CreateTicketForm {
    title: String,
    #[serde(default)]
    markdown: String,
    #[serde(default)]
    labels: Option<String>,
}

#[derive(Deserialize)]
struct CommentForm {
    markdown: String,
}

#[derive(Deserialize)]
struct StatusForm {
    status: String,
}

/// Formats an RFC3339 timestamp as a short relative time. Falls back to the
/// raw string when it cannot be parsed, so a malformed value is still shown
/// rather than hidden.
fn format_date(date_str: &str) -> String {
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(date_str) else {
        return date_str.to_string();
    };
    let date = parsed.with_timezone(&chrono::Utc);
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(date);

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

/// Tachyons colour classes layered on top of `.tf-badge` for each status.
fn status_badge_classes(status: Status) -> &'static str {
    match status {
        Status::Open => "bg-dark-green white",
        Status::InProgress => "bg-dark-blue white",
        Status::Blocked => "bg-dark-red white",
        Status::Closed => "bg-white-20 white-70",
    }
}

fn status_badge(status: Status) -> maud::Markup {
    maud::html! {
        span class=(format!("tf-badge {}", status_badge_classes(status))) {
            (status.label())
        }
    }
}

fn label_pill(label: &str) -> maud::Markup {
    maud::html! {
        span class="dib mr1 mb1 ph2 pv1 f7 ba b--white-20 white-70 br1" { (label) }
    }
}

/// Whether the filter tab for `candidate` (`None` is the "All" tab) is the
/// active one given the `status` query parameter currently in effect.
fn is_active_filter(current: Option<&str>, candidate: Option<Status>) -> bool {
    match candidate {
        None => current.is_none(),
        Some(status) => current == Some(status.as_str()),
    }
}

/// Tickets matching `status`, or all of them when `status` is `None`.
fn filtered_tickets(tickets: &[Ticket], status: Option<Status>) -> Vec<&Ticket> {
    match status {
        None => tickets.iter().collect(),
        Some(status) => tickets.iter().filter(|t| t.status == status).collect(),
    }
}

/// Splits a comma-separated label list into trimmed, non-empty labels.
fn parse_labels(raw: Option<&str>) -> Vec<String> {
    raw.map(|s| {
        s.split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(ToString::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// Mentioned usernames (from the body and every comment) that resolve to a
/// real account, for highlighting in the rendered markdown.
async fn known_users_for_ticket(db: &Database, ticket: &Ticket) -> BTreeSet<String> {
    let mut mentions = markdown::extract_mentions(&ticket.body.markdown);
    for comment in &ticket.comments {
        mentions.extend(markdown::extract_mentions(&comment.markdown));
    }

    let mut known = BTreeSet::new();
    for name in mentions {
        if let Ok(Some(_user)) = db.get_user_by_username(&name).await {
            known.insert(name);
        }
    }
    known
}

fn render_status_tabs(namespace: &str, current: Option<&str>) -> maud::Markup {
    let tabs: [(Option<Status>, &str); 5] = [
        (None, "All"),
        (Some(Status::Open), "Open"),
        (Some(Status::InProgress), "In progress"),
        (Some(Status::Blocked), "Blocked"),
        (Some(Status::Closed), "Closed"),
    ];

    maud::html! {
        div class="flex flex-wrap mb3 bb b--white-20" {
            @for (status, label) in tabs {
                @let is_active = is_active_filter(current, status);
                @let href = match status {
                    Some(status) => format!("/{namespace}/tickets?status={}", status.as_str()),
                    None => format!("/{namespace}/tickets"),
                };
                @let classes = if is_active {
                    "tf-tab pa2 ph3 white bg-white-10 no-underline"
                } else {
                    "tf-tab pa2 ph3 white-50 hover-white no-underline"
                };
                a
                    href=(href)
                    class=(classes)
                    hx-get=(href)
                    hx-target="#ticket-list"
                    hx-swap="innerHTML"
                    hx-push-url="true"
                {
                    (label)
                }
            }
        }
    }
}

fn render_ticket_row(namespace: &str, ticket: &Ticket) -> maud::Markup {
    maud::html! {
        a
            href=(format!("/{}/tickets/{}", namespace, ticket.number()))
            class="flex flex-wrap pa3 bb b--white-10 link white hover-white hover-bg-white-10 no-underline items-baseline"
        {
            div class="flex-auto" style="min-width: 0;" {
                div class="flex items-center flex-wrap" {
                    span class="white-50 f6 mr2" { "#" (ticket.number()) }
                    span class="f5 fw6 mr2" style="letter-spacing: -0.01em;" { (ticket.title) }
                    (status_badge(ticket.status))
                }
                div class="mt1 f6 white-50" {
                    "opened by " (ticket.author())
                    @if !ticket.comments.is_empty() {
                        span class="ml2" { (ticket.comments.len()) " comments" }
                    }
                }
                @if !ticket.labels.is_empty() {
                    div class="mt1" {
                        @for label in &ticket.labels {
                            (label_pill(label))
                        }
                    }
                }
            }
        }
    }
}

/// Bordered panel shown when a namespace has no tickets yet, explaining both
/// ways a ticket can be created.
fn render_empty_state(namespace: &str) -> maud::Markup {
    maud::html! {
        div class="ba b--white-20 bg-black-20 pa4 tc" {
            p class="f5 white mb2" { "No tickets yet." }
            p class="f6 white-70 mb3" {
                "Create one above, or clone the ticket repository and push a new file:"
            }
            pre class="pa3 bg-black-50 overflow-x-auto tl" {
                code class="f6 white lh-copy" {
                    "git clone <origin>/" (namespace) "/ticket\n"
                    "cd ticket\n"
                    (PUSH_CONFIG_LINE) "\n"
                }
            }
        }
    }
}

/// Renders the `#ticket-list` contents shared by the full-page and HTMX
/// filter responses.
fn render_ticket_list(namespace: &str, tickets: &[&Ticket]) -> maud::Markup {
    maud::html! {
        @if tickets.is_empty() {
            (render_empty_state(namespace))
        } @else {
            div class="ba b--white-20 bg-black-20 overflow-hidden" {
                div class="flex flex-column" {
                    @for ticket in tickets {
                        (render_ticket_row(namespace, ticket))
                    }
                }
            }
        }
    }
}

fn render_new_ticket_form(namespace: &str) -> maud::Markup {
    maud::html! {
        div class="ba b--white-20 pa3 bg-black-20" {
            div class="flex justify-between items-center mb3" {
                h2 class="tf-section white ma0" { "New ticket" }
                button
                    onclick="document.getElementById('new-ticket-form').innerHTML = ''"
                    class="pa1 bg-transparent white bn pointer hover-white-70 tf-kicker"
                {
                    "✕ Cancel"
                }
            }
            form
                hx-post=(format!("/{}/tickets", namespace))
                hx-target="#new-ticket-form"
                hx-swap="innerHTML"
                hx-target-error="#new-ticket-form"
            {
                div class="mb3" {
                    label class="db tf-kicker white-50 mb2" for="title" { "Title" }
                    input
                        type="text"
                        name="title"
                        id="title"
                        required
                        class="tf-input db w-100"
                        placeholder="Short summary of the issue";
                }
                div class="mb3" {
                    label class="db tf-kicker white-50 mb2" for="markdown" { "Description" }
                    textarea
                        name="markdown"
                        id="markdown"
                        rows="6"
                        class="tf-input db w-100"
                        placeholder="Describe the issue. Use @username to mention someone." {}
                }
                div class="mb3" {
                    label class="db tf-kicker white-50 mb2" for="labels" { "Labels" }
                    input
                        type="text"
                        name="labels"
                        id="labels"
                        class="tf-input db w-100"
                        placeholder="bug, ui (comma-separated, optional)";
                }
                button type="submit" class="tf-btn tf-btn-block" { "Create ticket" }
            }
        }
    }
}

/// Renders the ticket list page body shared by the full-page and HTMX
/// responses.
fn render_tickets_page(
    namespace: &str,
    status_filter: Option<&str>,
    tickets: &[Ticket],
) -> maud::Markup {
    let filtered = filtered_tickets(tickets, status_filter.and_then(Status::parse));

    maud::html! {
        div class="mb3 mb4-ns tf-kicker white-50" {
            a href="/" class="link white-50 hover-white no-underline" { "Namespaces" }
            span class="mh2" { "/" }
            a href=(format!("/{}", namespace)) class="link white-50 hover-white no-underline" { (namespace) }
            span class="mh2" { "/" }
            span class="white" { "Tickets" }
        }

        div class="flex flex-wrap justify-between items-end mb3 mb4-ns" {
            h1 class="tf-title white ma0" { "Tickets" }
            button
                hx-get=(format!("/{}/tickets/new", namespace))
                hx-target="#new-ticket-form"
                hx-swap="innerHTML"
                class="tf-btn"
            {
                "New ticket"
            }
        }

        div id="new-ticket-form" class="mb3 mb4-ns" {}

        (render_status_tabs(namespace, status_filter))

        div id="ticket-list" {
            (render_ticket_list(namespace, &filtered))
        }
    }
}

fn render_ticket_link(namespace: &str, link: &Link) -> maud::Markup {
    maud::html! {
        li class="mb1" {
            a
                href=(format!("/{}/{}", namespace, link.repo))
                class="link white hover-white-70 no-underline"
            {
                (link.repo)
                @if let Some(branch) = &link.branch {
                    " @ " (branch)
                }
            }
        }
    }
}

fn render_comment(comment: &Comment, known_users: &BTreeSet<String>) -> maud::Markup {
    maud::html! {
        div class="ba b--white-10 pa3 mb3 bg-black-20" {
            div class="tf-kicker white-50 mb2" {
                span class="white" { (comment.author.as_deref().unwrap_or("unknown")) }
                @if let Some(created) = &comment.created_at {
                    span class="ml2" { (format_date(created)) }
                }
            }
            div class="markdown-body white lh-copy" {
                (maud::PreEscaped(markdown::render(&comment.markdown, known_users)))
            }
        }
    }
}

fn render_comment_form(namespace: &str, number: u64) -> maud::Markup {
    maud::html! {
        form
            hx-post=(format!("/{}/tickets/{}/comment", namespace, number))
            hx-target="#ticket-detail"
            hx-swap="innerHTML"
            hx-target-error="#comment-error"
            class="mt3"
        {
            div class="mb2" {
                textarea
                    name="markdown"
                    required
                    rows="4"
                    class="tf-input db w-100"
                    placeholder="Write a comment... use @username to mention" {}
            }
            button type="submit" class="tf-btn" { "Comment" }
        }
        div id="comment-error" {}
    }
}

fn render_status_form(namespace: &str, number: u64, current: Status) -> maud::Markup {
    maud::html! {
        form
            hx-post=(format!("/{}/tickets/{}/status", namespace, number))
            hx-target="#ticket-detail"
            hx-swap="innerHTML"
            class="flex items-center flex-wrap mb4"
        {
            label class="tf-kicker white-50 mr2" for="status" { "Status" }
            select name="status" id="status" class="tf-input mr2" {
                @for status in Status::all() {
                    option value=(status.as_str()) selected[status == current] { (status.label()) }
                }
            }
            button type="submit" class="tf-btn tf-btn-ghost" { "Update" }
        }
    }
}

/// Renders the ticket detail content, without the `#ticket-detail` wrapper.
/// Used both by the initial page load (wrapped once by
/// [`render_ticket_detail_page`]) and by the comment/status HTMX endpoints,
/// which swap the wrapper's `innerHTML` directly.
fn render_ticket_detail_body(
    namespace: &str,
    ticket: &Ticket,
    known_users: &BTreeSet<String>,
) -> maud::Markup {
    maud::html! {
        div class="flex flex-wrap items-center mb3" {
            h1 class="tf-title white ma0 mr3" style="overflow-wrap: anywhere;" {
                "#" (ticket.number()) " " (ticket.title)
            }
            (status_badge(ticket.status))
        }

        div class="mb3 f6 white-50" {
            "Opened by " span class="white" { (ticket.author()) }
            @if let Some(created) = &ticket.created_at {
                span class="mh2" { "\u{00b7}" }
                "created " (format_date(created))
            }
            @if let Some(updated) = &ticket.updated_at {
                span class="mh2" { "\u{00b7}" }
                "updated " (format_date(updated))
            }
        }

        @if !ticket.labels.is_empty() {
            div class="mb3" {
                @for label in &ticket.labels {
                    (label_pill(label))
                }
            }
        }

        @if !ticket.assignees.is_empty() {
            div class="mb3 f6 white-50" {
                "Assigned to " (ticket.assignees.join(", "))
            }
        }

        @if !ticket.links.is_empty() {
            div class="mb3" {
                h3 class="tf-kicker white-50 mb2" { "Linked repositories" }
                ul class="list pl0" {
                    @for link in &ticket.links {
                        (render_ticket_link(namespace, link))
                    }
                }
            }
        }

        div class="markdown-body white lh-copy mb4" {
            (maud::PreEscaped(markdown::render(&ticket.body.markdown, known_users)))
        }

        (render_status_form(namespace, ticket.number(), ticket.status))

        div class="mt4" {
            h3 class="tf-section white mb3" { "Comments" }
            @if ticket.comments.is_empty() {
                p class="f6 white-50" { "No comments yet." }
            } @else {
                @for comment in &ticket.comments {
                    (render_comment(comment, known_users))
                }
            }
            (render_comment_form(namespace, ticket.number()))
        }
    }
}

fn render_ticket_detail_page(
    namespace: &str,
    ticket: &Ticket,
    known_users: &BTreeSet<String>,
) -> maud::Markup {
    maud::html! {
        div id="ticket-detail" {
            (render_ticket_detail_body(namespace, ticket, known_users))
        }
    }
}

#[get("/{namespace}/tickets")]
pub async fn list_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<Params>,
    query: web::Query<ListQuery>,
) -> AwResult<maud::Markup> {
    let namespace = params.namespace.clone();
    let username = get_username_from_request(&req, &auth_state).await;
    let status_filter = query.status.as_deref();

    let project_root = server.project_root().to_string();
    let namespace_for_block = namespace.clone();
    let result = web::block(move || {
        let store = TicketStore::open_or_create(&project_root, &namespace_for_block)?;
        store.read_tickets()
    })
    .await;

    let tickets = match result {
        Ok(Ok(tickets)) => tickets,
        Ok(Err(e)) => {
            log::error!("Failed to read tickets for '{namespace}': {e}");
            Vec::new()
        }
        Err(e) => {
            log::error!("Blocking task failed reading tickets for '{namespace}': {e:?}");
            Vec::new()
        }
    };

    let is_hx = req.headers().get("HX-Request").is_some();

    if is_hx && status_filter.is_some() {
        let filtered = filtered_tickets(&tickets, status_filter.and_then(Status::parse));
        return Ok(render_ticket_list(&namespace, &filtered));
    }

    let content = render_tickets_page(&namespace, status_filter, &tickets);

    if is_hx {
        Ok(content)
    } else {
        Ok(super::render_layout(&content, username.as_deref()))
    }
}

#[get("/{namespace}/tickets/new")]
pub async fn new_form_handler(
    _req: HttpRequest,
    params: web::Path<Params>,
) -> AwResult<maud::Markup> {
    Ok(render_new_ticket_form(&params.namespace))
}

#[post("/{namespace}/tickets")]
pub async fn create_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<Params>,
    form: web::Form<CreateTicketForm>,
) -> impl Responder {
    let namespace = params.namespace.clone();

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

    match db.user_has_namespace_access(&user_id, &namespace).await {
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

    let Ok(Some(user)) = db.get_user_by_id(&user_id).await else {
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to load user").into_string());
    };

    if form.title.trim().is_empty() {
        return HttpResponse::BadRequest()
            .body(render_error("Ticket title cannot be empty").into_string());
    }

    let project_root = server.project_root().to_string();
    let namespace_for_block = namespace.clone();
    let title = form.title.clone();
    let markdown_body = form.markdown.clone();
    let labels = parse_labels(form.labels.as_deref());
    let username = user.username.clone();
    let email = user
        .email
        .clone()
        .unwrap_or_else(|| "fig@localhost".to_string());

    let result = web::block(move || {
        let store = TicketStore::open_or_create(&project_root, &namespace_for_block)?;
        let actor = Actor::new(&username, &email);
        let draft = Ticket {
            title,
            labels,
            body: Body {
                markdown: markdown_body,
            },
            ..Ticket::default()
        };
        ops::create_ticket(&store, &actor, draft)
    })
    .await;

    match result {
        Ok(Ok(ticket)) => {
            let redirect = format!("/{namespace}/tickets/{}", ticket.number());
            HttpResponse::Ok()
                .content_type("text/html")
                .insert_header(("HX-Redirect", redirect))
                .body(render_success("Ticket created.").into_string())
        }
        Ok(Err(e)) => HttpResponse::BadRequest().body(render_error(&e).into_string()),
        Err(e) => {
            log::error!("Blocking task failed creating ticket: {e:?}");
            HttpResponse::InternalServerError()
                .body(render_error("Failed to create ticket").into_string())
        }
    }
}

#[get("/{namespace}/tickets/{number}")]
pub async fn detail_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<TicketParams>,
) -> impl Responder {
    let namespace = params.namespace.clone();
    let number = params.number;
    let username = get_username_from_request(&req, &auth_state).await;

    let project_root = server.project_root().to_string();
    let namespace_for_block = namespace.clone();
    let result = web::block(move || {
        let store = TicketStore::open_or_create(&project_root, &namespace_for_block)?;
        store.read_ticket(number)
    })
    .await;

    let ticket = match result {
        Ok(Ok(Some(ticket))) => ticket,
        Ok(Ok(None)) => {
            return HttpResponse::NotFound().body(render_error("Ticket not found").into_string());
        }
        Ok(Err(e)) => {
            log::error!("Failed to read ticket #{number} in '{namespace}': {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to load ticket").into_string());
        }
        Err(e) => {
            log::error!("Blocking task failed reading ticket #{number}: {e:?}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to load ticket").into_string());
        }
    };

    let known_users = known_users_for_ticket(auth_state.db(), &ticket).await;
    let content = render_ticket_detail_page(&namespace, &ticket, &known_users);

    if req.headers().get("HX-Request").is_some() {
        HttpResponse::Ok()
            .content_type("text/html")
            .body(content.into_string())
    } else {
        HttpResponse::Ok()
            .content_type("text/html")
            .body(super::render_layout(&content, username.as_deref()).into_string())
    }
}

#[post("/{namespace}/tickets/{number}/comment")]
pub async fn comment_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<TicketParams>,
    form: web::Form<CommentForm>,
) -> impl Responder {
    let namespace = params.namespace.clone();
    let number = params.number;

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

    match db.user_has_namespace_access(&user_id, &namespace).await {
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

    let Ok(Some(user)) = db.get_user_by_id(&user_id).await else {
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to load user").into_string());
    };

    if form.markdown.trim().is_empty() {
        return HttpResponse::BadRequest()
            .body(render_error("Comment cannot be empty").into_string());
    }

    let project_root = server.project_root().to_string();
    let namespace_for_block = namespace.clone();
    let markdown_body = form.markdown.clone();
    let username = user.username.clone();
    let email = user
        .email
        .clone()
        .unwrap_or_else(|| "fig@localhost".to_string());

    let result = web::block(move || {
        let store = TicketStore::open_or_create(&project_root, &namespace_for_block)?;
        let actor = Actor::new(&username, &email);
        ops::add_comment(&store, &actor, number, &markdown_body)
    })
    .await;

    match result {
        Ok(Ok(ticket)) => {
            let known_users = known_users_for_ticket(db, &ticket).await;
            let content = render_ticket_detail_body(&namespace, &ticket, &known_users);
            HttpResponse::Ok()
                .content_type("text/html")
                .body(content.into_string())
        }
        Ok(Err(e)) => HttpResponse::BadRequest().body(render_error(&e).into_string()),
        Err(e) => {
            log::error!("Blocking task failed adding comment: {e:?}");
            HttpResponse::InternalServerError()
                .body(render_error("Failed to add comment").into_string())
        }
    }
}

#[post("/{namespace}/tickets/{number}/status")]
pub async fn status_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<TicketParams>,
    form: web::Form<StatusForm>,
) -> impl Responder {
    let namespace = params.namespace.clone();
    let number = params.number;

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

    match db.user_has_namespace_access(&user_id, &namespace).await {
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

    let Some(status) = Status::parse(&form.status) else {
        return HttpResponse::BadRequest().body(render_error("Invalid status").into_string());
    };

    let Ok(Some(user)) = db.get_user_by_id(&user_id).await else {
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to load user").into_string());
    };

    let project_root = server.project_root().to_string();
    let namespace_for_block = namespace.clone();
    let username = user.username.clone();
    let email = user
        .email
        .clone()
        .unwrap_or_else(|| "fig@localhost".to_string());

    let result = web::block(move || {
        let store = TicketStore::open_or_create(&project_root, &namespace_for_block)?;
        let actor = Actor::new(&username, &email);
        ops::set_status(&store, &actor, number, status)
    })
    .await;

    match result {
        Ok(Ok(ticket)) => {
            let known_users = known_users_for_ticket(db, &ticket).await;
            let content = render_ticket_detail_body(&namespace, &ticket, &known_users);
            HttpResponse::Ok()
                .content_type("text/html")
                .body(content.into_string())
        }
        Ok(Err(e)) => HttpResponse::BadRequest().body(render_error(&e).into_string()),
        Err(e) => {
            log::error!("Blocking task failed setting status: {e:?}");
            HttpResponse::InternalServerError()
                .body(render_error("Failed to update status").into_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ticket(number: u64, status: Status) -> Ticket {
        Ticket {
            uuid: Some("test-uuid".to_string()),
            number: Some(number),
            namespace: Some("acme".to_string()),
            title: "Sample ticket".to_string(),
            status,
            author: Some("alice".to_string()),
            created_at: Some("2026-08-31T10:00:00Z".to_string()),
            updated_at: Some("2026-08-31T12:00:00Z".to_string()),
            labels: vec!["bug".to_string()],
            assignees: vec![],
            body: Body {
                markdown: "Body text".to_string(),
            },
            links: vec![],
            comments: vec![],
        }
    }

    #[test]
    fn test_status_badge_classes_cover_every_status() {
        assert_eq!(status_badge_classes(Status::Open), "bg-dark-green white");
        assert_eq!(
            status_badge_classes(Status::InProgress),
            "bg-dark-blue white"
        );
        assert_eq!(status_badge_classes(Status::Blocked), "bg-dark-red white");
        assert_eq!(status_badge_classes(Status::Closed), "bg-white-20 white-70");
    }

    #[test]
    fn test_status_badge_renders_label_and_classes() {
        let html = status_badge(Status::Open).into_string();
        assert!(html.contains("tf-badge"));
        assert!(html.contains("bg-dark-green"));
        assert!(html.contains("Open"));
    }

    #[test]
    fn test_is_active_filter_all_tab() {
        assert!(is_active_filter(None, None));
        assert!(!is_active_filter(Some("open"), None));
    }

    #[test]
    fn test_is_active_filter_status_tab() {
        assert!(is_active_filter(Some("open"), Some(Status::Open)));
        assert!(!is_active_filter(Some("closed"), Some(Status::Open)));
        assert!(!is_active_filter(None, Some(Status::Open)));
    }

    #[test]
    fn test_filtered_tickets_returns_all_when_no_status() {
        let tickets = vec![
            sample_ticket(1, Status::Open),
            sample_ticket(2, Status::Closed),
        ];
        assert_eq!(filtered_tickets(&tickets, None).len(), 2);
    }

    #[test]
    fn test_filtered_tickets_by_status() {
        let tickets = vec![
            sample_ticket(1, Status::Open),
            sample_ticket(2, Status::Closed),
            sample_ticket(3, Status::Open),
        ];
        let open = filtered_tickets(&tickets, Some(Status::Open));
        assert_eq!(open.len(), 2);
        assert!(open.iter().all(|t| t.status == Status::Open));
    }

    #[test]
    fn test_parse_labels_splits_and_trims() {
        let labels = parse_labels(Some(" bug, ui ,, urgent "));
        assert_eq!(labels, vec!["bug", "ui", "urgent"]);
    }

    #[test]
    fn test_parse_labels_none_is_empty() {
        assert!(parse_labels(None).is_empty());
    }

    #[test]
    fn test_empty_state_contains_clone_instructions() {
        let html = render_empty_state("acme").into_string();
        assert!(html.contains("No tickets yet"));
        assert!(html.contains("git clone"));
        assert!(html.contains("acme/ticket"));
        assert!(html.contains(PUSH_CONFIG_LINE));
    }

    #[test]
    fn test_render_ticket_list_shows_empty_state_when_no_tickets() {
        let html = render_ticket_list("acme", &[]).into_string();
        assert!(html.contains("No tickets yet"));
    }

    #[test]
    fn test_render_ticket_list_shows_rows_when_tickets_present() {
        let ticket = sample_ticket(7, Status::Open);
        let html = render_ticket_list("acme", &[&ticket]).into_string();
        assert!(html.contains("#7"));
        assert!(html.contains("Sample ticket"));
        assert!(!html.contains("No tickets yet"));
    }

    #[test]
    fn test_detail_view_escapes_script_body() {
        let mut ticket = sample_ticket(1, Status::Open);
        ticket.body = Body {
            markdown: "<script>alert(1)</script>".to_string(),
        };
        let known_users = BTreeSet::new();
        let html = render_ticket_detail_body("acme", &ticket, &known_users).into_string();
        assert!(
            !html.contains("<script>"),
            "raw script tag must not survive: {html}"
        );
        assert!(html.contains("&lt;script&gt;"), "{html}");
    }

    #[test]
    fn test_format_date_just_now() {
        let now = chrono::Utc::now().to_rfc3339();
        assert_eq!(format_date(&now), "just now");
    }

    #[test]
    fn test_format_date_invalid_falls_back_to_raw_string() {
        assert_eq!(format_date("not-a-date"), "not-a-date");
    }

    #[test]
    fn test_params_deserialization() {
        let params: Params = serde_urlencoded::from_str("namespace=acme").unwrap();
        assert_eq!(params.namespace, "acme");
    }

    #[test]
    fn test_create_ticket_form_labels_optional() {
        let form: CreateTicketForm = serde_urlencoded::from_str("title=Bug&markdown=oops").unwrap();
        assert_eq!(form.title, "Bug");
        assert_eq!(form.markdown, "oops");
        assert_eq!(form.labels, None);
    }

    #[test]
    fn test_status_form_deserialization() {
        let form: StatusForm = serde_urlencoded::from_str("status=in_progress").unwrap();
        assert_eq!(form.status, "in_progress");
    }
}
