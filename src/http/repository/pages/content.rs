use super::markdown::markdown_to_html;
use super::{
    AwResult, Deserialize, HttpRequest, Markup, RepoContext, TreeEntry, TwigContext, config, get,
    is_safe_repo_path, open_repo, render_empty, render_not_found_for_request, render_tab_response,
    web,
};

#[derive(Deserialize)]
pub(super) struct ContentParams {
    namespace: String,
    repo: String,
    path: String,
}

#[derive(Deserialize)]
pub(super) struct ContentQuery {
    committed: Option<String>,
    conflict_copy: Option<bool>,
}

/// The root of the Content tab.
pub(super) fn content_body(ctx: &RepoContext) -> Markup {
    let entries = ctx
        .handle
        .list_dir("", Some(&ctx.twig_result.config))
        .unwrap_or_default();
    render_content_view(&ctx.namespace, &ctx.repo, "", &entries, None)
}

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
        .and_then(|oid| twig_git::Oid::from_str(oid).ok());
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

/// Returns (href, push-url) pair for a path inside the content tab
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

/// One Technical Row per tree entry, with type and path exposed as separate
/// columns for the compact repository listing.
pub(super) fn render_content_row(href: &str, push: &str, entry: &TreeEntry) -> Markup {
    maud::html! {
        a
            class=(if entry.is_dir { "twig-row" } else { "twig-row twig-row--file" })
            href=(href)
            hx-get=(href)
            hx-target="#tab-content"
            hx-push-url=(push)
        {
            span class="twig-row-kind" {
                @if entry.is_dir { "DIR" } @else { "FILE" }
            }
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
                    div class="twig-colhead twig-colhead--content" {
                        span { "TYPE" }
                        span { "PATH" }
                    }
                    nav class="twig-list twig-list--content" aria-label="Repository entries" {
                        @if !path.is_empty() {
                            @let (href, push) = content_urls(namespace, repo, parent_path(path));
                            a
                                class="twig-row"
                                href=(href)
                                hx-get=(href)
                                hx-target="#tab-content"
                                hx-push-url=(push)
                            {
                                span class="twig-row-kind" { "UP" }
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
                @if std::str::from_utf8(bytes).is_ok() {
                    div class="twig-editor-file-actions" {
                        a
                            class="twig-btn twig-btn--quiet"
                            href=(format!(
                                "/{namespace}/{repo}/edit/{}",
                                crate::http::repository::editor::encode_file_path(path)
                            ))
                        { "Edit file" }
                    }
                }
                @if crate::md::is_markdown(path) {
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
