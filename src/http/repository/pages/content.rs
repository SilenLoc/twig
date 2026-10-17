use actix_web::Result as AwResult;
use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse, get, web};
use maud::Markup;

use crate::auth::TwigContext;
use crate::config;
use crate::git::bare::{RepoHandle, TreeEntry, is_safe_repo_path};
use crate::http::auth::session::get_username_from_request;

use super::context::{can_view_namespace, open_repo, render_empty, render_not_found_for_request};
use super::markdown::markdown_to_html;
use super::tabs::render_tab_response;
use super::{ContentParams, ContentQuery, RawParams};

#[get("/{namespace}/{repo}/raw/{path:.*}")]
pub async fn raw_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<RawParams>,
) -> HttpResponse {
    let username = get_username_from_request(&req, &auth_state).await;

    let Ok(handle) = RepoHandle::open(server.project_root(), &params.namespace, &params.repo)
    else {
        return HttpResponse::NotFound().finish();
    };

    let twig_result = handle.load_config_with_raw();
    if twig_result.config.private {
        if username.is_none() {
            return HttpResponse::Unauthorized().finish();
        }
        match can_view_namespace(&req, &server, &auth_state, &params.namespace).await {
            Ok(true) => {}
            Ok(false) => return HttpResponse::Forbidden().finish(),
            Err(error) => {
                log::error!("Failed to authorize private raw repository read: {error}");
                return HttpResponse::InternalServerError().finish();
            }
        }
    }

    let path = params.path.trim_matches('/');
    if !is_safe_repo_path(path) || path.is_empty() || twig_result.config.should_ignore(path) {
        return HttpResponse::NotFound().finish();
    }

    match handle.read_blob_bytes(path) {
        Ok(Some(bytes)) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, raw_content_type(path)))
            .insert_header(("X-Content-Type-Options", "nosniff"))
            .body(bytes),
        _ => HttpResponse::NotFound().finish(),
    }
}

/// The URL tabs used before each got its own route. A permanent redirect keeps
/// old links and refreshes landing on the tab's current address.
#[get("/{namespace}/{repo}/content/{path:.*}")]
pub async fn content_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<ContentParams>,
    query: web::Query<ContentQuery>,
) -> AwResult<Markup> {
    let path = params.path.trim_matches('/').to_string();
    let ctx = match open_repo(&req, &server, &auth_state, &params.namespace, &params.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };

    if !is_safe_repo_path(&path) {
        return Ok(render_not_found_for_request(
            &req,
            ctx.username.as_deref(),
            &ctx.page_title,
        ));
    }

    let entries = ctx
        .handle
        .list_dir(&path, Some(&ctx.twig_result.config))
        .unwrap_or_default();
    let file_bytes = if path.is_empty() || !entries.is_empty() {
        None
    } else {
        ctx.handle.read_blob_bytes(&path).ok().flatten()
    };

    let file_view = render_content_view(
        &ctx.namespace,
        &ctx.repo,
        &path,
        &entries,
        file_bytes.as_deref(),
    );
    let saved_commit = query
        .committed
        .as_deref()
        .and_then(|oid| git2::Oid::from_str(oid).ok());
    let is_saved_commit =
        saved_commit.is_some_and(|oid| ctx.handle.head_oid().ok().flatten() == Some(oid));
    let body = if is_saved_commit {
        let message = if query.conflict_copy.unwrap_or(false) {
            "Draft saved as a conflict copy."
        } else {
            "Changes committed successfully."
        };
        maud::html! {
            div class="twig-stack" {
                (crate::http::view::render_success(message))
                (file_view)
            }
        }
    } else {
        file_view
    };
    Ok(render_tab_response(&req, &ctx.frame(), "content", &body))
}

pub(super) fn content_urls(namespace: &str, repo: &str, path: &str) -> (String, String) {
    let href = if path.is_empty() {
        format!("/{namespace}/{repo}/content")
    } else {
        format!("/{namespace}/{repo}/content/{path}")
    };
    (href.clone(), href)
}

pub(super) fn parent_path(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[..i],
        None => "",
    }
}

/// Precomputed breadcrumb segments: (label, accumulated path, `is_last`)
pub(super) fn breadcrumb_segments(path: &str) -> Vec<(String, String, bool)> {
    if path.is_empty() {
        return Vec::new();
    }
    let mut segments = Vec::new();
    let mut acc = String::new();
    for segment in path.split('/') {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(segment);
        let is_last = acc == path;
        segments.push((segment.to_string(), acc.clone(), is_last));
    }
    segments
}

pub(super) fn render_content_breadcrumbs(namespace: &str, repo: &str, path: &str) -> Markup {
    let segments = breadcrumb_segments(path);
    maud::html! {
        nav class="twig-crumbs twig-crumbs--path" aria-label="File path" {
            @if segments.is_empty() {
                span aria-current="page" { (repo) }
            } @else {
                @let (root_href, root_push) = content_urls(namespace, repo, "");
                a
                    href=(root_href)
                    hx-get=(root_href)
                    hx-target="#tab-content"
                    hx-push-url=(root_push)
                {
                    (repo)
                }
            }
            @for (segment, acc_path, is_last) in segments {
                span class="twig-crumb-sep" aria-hidden="true" { "/" }
                @if is_last {
                    span aria-current="page" { (segment) }
                } @else {
                    @let (href, push) = content_urls(namespace, repo, &acc_path);
                    a
                        href=(href)
                        hx-get=(href)
                        hx-target="#tab-content"
                        hx-push-url=(push)
                    {
                        (segment)
                    }
                }
            }
        }
    }
}

pub(super) fn render_content_view(
    namespace: &str,
    repo: &str,
    path: &str,
    entries: &[TreeEntry],
    file_bytes: Option<&[u8]>,
) -> Markup {
    if !entries.is_empty() || path.is_empty() {
        return render_content_dir(namespace, repo, path, entries);
    }
    if let Some(bytes) = file_bytes {
        render_content_file(namespace, repo, path, bytes)
    } else {
        render_empty("NOT FOUND", "Path not found.")
    }
}

/// One Technical Row per tree entry. A directory is marked by its trailing
/// slash and the primary-ink identifier; there is no icon (DESIGN.md 5.6).
pub(super) fn render_content_row(href: &str, push: &str, entry: &TreeEntry) -> Markup {
    maud::html! {
        a
            class=(if entry.is_dir { "twig-row" } else { "twig-row twig-row--file" })
            href=(href)
            hx-get=(href)
            hx-target="#tab-content"
            hx-push-url=(push)
        {
            span class="twig-row-id" {
                (entry.name)
                @if entry.is_dir { "/" }
            }
        }
    }
}

pub(super) fn render_content_dir(
    namespace: &str,
    repo: &str,
    path: &str,
    entries: &[TreeEntry],
) -> Markup {
    maud::html! {
        div class="twig-stack" {
            (render_content_breadcrumbs(namespace, repo, path))

            @if entries.is_empty() && path.is_empty() {
                (render_empty("NO FILES", "No files in this repository."))
            } @else {
                div class="twig-panel twig-panel--flush" {
                    div class="twig-list" {
                        @if !path.is_empty() {
                            @let (href, push) = content_urls(namespace, repo, parent_path(path));
                            a
                                class="twig-row"
                                href=(href)
                                hx-get=(href)
                                hx-target="#tab-content"
                                hx-push-url=(push)
                            {
                                span class="twig-row-id" { ".." }
                            }
                        }
                        @for entry in entries {
                            @let (href, push) = content_urls(namespace, repo, &entry.path);
                            (render_content_row(&href, &push, entry))
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn render_content_file(namespace: &str, repo: &str, path: &str, bytes: &[u8]) -> Markup {
    let is_binary = bytes.contains(&0);

    maud::html! {
        div class="twig-stack" {
            (render_content_breadcrumbs(namespace, repo, path))

            @if is_binary {
                (render_empty(
                    "BINARY FILE",
                    &format!("Binary file ({} bytes). Not displayed.", bytes.len())
                ))
            } @else {
                @let text = String::from_utf8_lossy(bytes).into_owned();
                @if crate::md::is_markdown(path) {
                    @if let Ok(source) = std::str::from_utf8(bytes) {
                        (super::markdown::render_markdown_controls(namespace, repo, path, source))
                    }
                    div class="twig-md twig-md--boxed" {
                        (maud::PreEscaped(markdown_to_html(&text, namespace, repo, parent_path(path))))
                    }
                } @else {
                    pre class="twig-code" tabindex="0" aria-label=(format!("{path} contents")) {
                        @if let Some(highlighted) = twig_highlight::highlight_path(path, &text) {
                            code { (maud::PreEscaped(highlighted)) }
                        } @else {
                            code { (text) }
                        }
                    }
                }
            }
        }
    }
}

/// The Scripts tab: each configured group becomes a sub-tab labelled with the
/// group name, and the active group lists its scripts with a copyable
/// download-and-run command and its nested groups as links.
/// Content type for a raw file: text for the formats worth reading in a
/// terminal, a neutral download type for everything else.
pub(super) fn raw_content_type(path: &str) -> &'static str {
    let extension = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match extension.as_str() {
        "sh" | "bash" | "zsh" | "py" | "rb" | "pl" | "js" | "ts" | "toml" | "yaml" | "yml"
        | "json" | "md" | "markdown" | "txt" | "conf" | "cfg" | "ini" | "env" | "sql" | "rs"
        | "go" | "c" | "h" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
