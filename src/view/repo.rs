use std::collections::HashMap;

use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use maud::Markup;
use pulldown_cmark::{Event, Options, Parser, html};
use serde::Deserialize;

use crate::{
    auth::FigContext,
    config,
    git::bare::{
        Commit, Depth, FigConfigWithRaw, PresentConfig, RepoHandle, TreeEntry, is_safe_repo_path,
    },
    md,
};

use super::session_auth::get_username_from_request;

/// Presentation keyboard and fullscreen wiring, scoped to `#present-container`.
/// DESIGN.md 5.17 forbids a global key handler: ArrowLeft/ArrowRight are only
/// claimed while focus is inside the container or the container is fullscreen,
/// and `preventDefault` runs only when a slide actually changes.
const PRESENT_SCRIPT: &str = r"(function(){
var c=document.getElementById('present-container');
if(!c||c.dataset.figPresent)return;
c.dataset.figPresent='1';
var isFull=function(){return document.fullscreenElement===c;};
var sync=function(){
var f=c.querySelector('#fullscreen-toggle');
if(f)f.textContent=isFull()?'Exit fullscreen':'Fullscreen';
};
c.addEventListener('keydown',function(e){
if(e.key!=='ArrowLeft'&&e.key!=='ArrowRight')return;
if(!isFull()&&!c.contains(document.activeElement))return;
var b=c.querySelector(e.key==='ArrowLeft'?'#prev-slide':'#next-slide');
if(!b||b.disabled)return;
e.preventDefault();
b.click();
});
c.addEventListener('click',function(e){
if(!e.target.closest('#fullscreen-toggle'))return;
if(isFull()){document.exitFullscreen();}else{c.requestFullscreen();}
});
c.addEventListener('fullscreenchange',sync);
c.addEventListener('htmx:after:swap',function(){c.focus({preventScroll:true});sync();});
sync();
})();";

/// Presentation text sizes, in percent, from the default through to double
/// size. The A−/A+ buttons step through them one entry at a time; the active
/// size lives in `data-fig-text-size`, which the stylesheet turns into a
/// larger type scale for the slide content only.
const PRESENT_TEXT_SIZES: [u16; 11] = [100, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200];
const PRESENT_TEXT_SIZE_KEY: &str = "fig-present-text-size";

/// Mirrors the active size back to the browser so it survives a reload.
fn present_text_persist() -> String {
    format!(
        "try {{ localStorage.setItem('{PRESENT_TEXT_SIZE_KEY}', String(data.figTextSize)); }} catch (_) {{}}"
    )
}

fn present_size_list() -> String {
    PRESENT_TEXT_SIZES
        .iter()
        .map(|size| format!("'{size}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Restores a previously chosen size on load; expression runs against
/// `#present-container`, where `data.figTextSize` is the shared state.
fn present_size_restore() -> String {
    format!(
        "try {{ let size = localStorage.getItem('{PRESENT_TEXT_SIZE_KEY}'); if ([{}].includes(size)) data.figTextSize = Number(size); }} catch (_) {{}}",
        present_size_list()
    )
}

fn present_size_step(increase: bool) -> String {
    let first = PRESENT_TEXT_SIZES[0];
    let last = PRESENT_TEXT_SIZES[PRESENT_TEXT_SIZES.len() - 1];
    let step = PRESENT_TEXT_SIZES[1] - PRESENT_TEXT_SIZES[0];
    let (bound, operator, clamp) = if increase {
        (last, '+', "Math.min")
    } else {
        (first, '-', "Math.max")
    };
    format!(
        "data.figTextSize = {clamp}({bound}, data.figTextSize {operator} {step}); {}",
        present_text_persist()
    )
}

#[derive(Deserialize)]
struct Params {
    namespace: String,
    repo: String,
}

#[derive(Deserialize)]
struct TabParams {
    namespace: String,
    repo: String,
    tab: String,
}

#[derive(Deserialize)]
struct MarkdownParams {
    namespace: String,
    repo: String,
    file_path: String,
}

#[derive(Deserialize)]
struct SlideParams {
    namespace: String,
    repo: String,
    index: usize,
}

#[derive(Deserialize)]
struct ContentParams {
    namespace: String,
    repo: String,
    path: String,
}

fn render_for_request(
    req: &HttpRequest,
    content: Markup,
    username: Option<&str>,
    page_title: &str,
) -> Markup {
    if req.headers().get("HX-Request").is_some() {
        content
    } else {
        super::render_layout(&content, username, Some(page_title))
    }
}

fn render_repo_auth_error(req: &HttpRequest, page_title: &str) -> Markup {
    let content = super::render_error_with_action(
        "Not logged in. Please log in first.",
        "/auth/login",
        "Log in",
    );
    render_for_request(req, content, None, page_title)
}

/// A rendered slide for the presentation view
struct PresentSlide {
    html: String,
}

/// Context for rendering tab content to reduce parameter count
struct TabContentContext<'a> {
    namespace: &'a str,
    repo: &'a str,
    tab: &'a str,
    commits: &'a [Commit],
    markdown_files: &'a [String],
    selected_md_file: Option<&'a str>,
    selected_content: Option<&'a str>,
    fig_content: Option<&'a str>,
    fig_filename: Option<&'a str>,
    tabs_config: &'a [String],
    present_config: &'a PresentConfig,
    present_slides: &'a [PresentSlide],
    license_content: Option<&'a str>,
    has_license: bool,
    content_path: &'a str,
    content_entries: &'a [TreeEntry],
    content_file_bytes: Option<&'a [u8]>,
}

struct ContentHtmxContext<'a> {
    namespace: &'a str,
    repo: &'a str,
    path: &'a str,
    entries: &'a [TreeEntry],
    file_bytes: Option<&'a [u8]>,
    has_config: bool,
    tabs_config: &'a [String],
    has_present: bool,
}

fn render_tab_response(
    ctx: &TabContentContext<'_>,
    is_htmx: bool,
    has_config: bool,
    has_present: bool,
) -> Markup {
    if is_htmx {
        let inner = render_tab_content_inner(ctx);
        let tabs = render_tabs(
            ctx.namespace,
            ctx.repo,
            ctx.tab,
            has_config,
            ctx.tabs_config,
            has_present,
        );
        maud::html! {
            (tabs)
            (inner)
        }
    } else {
        render_tab_content(ctx)
    }
}

#[get("/{namespace}/{repo}")]
pub async fn handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let page_title = format!("{namespace}/{repo}");
    let username = get_username_from_request(&req, &auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(h) => h,
        Err(e) => {
            let content = render_git_error(&e);
            return Ok(render_for_request(
                &req,
                content,
                username.as_deref(),
                &page_title,
            ));
        }
    };

    let fig_result = handle.load_config_with_raw();
    let fig_config = &fig_result.config;
    let fig_content = fig_result.raw.as_deref();
    let fig_filename = fig_result.filename.as_deref();

    if fig_config.private && username.is_none() {
        return Ok(render_repo_auth_error(&req, &page_title));
    }

    let commits_result = handle.get_commits(&Depth::default());
    let files_result = handle.list_files(Some(fig_config));

    let content = match commits_result {
        Ok(commits) => {
            let files = files_result.unwrap_or_default();
            let markdown_files = files.markdown_files;
            let default_file = get_default_markdown_file(&markdown_files);
            let default_content = if let Some(file) = default_file {
                handle.read_file(file).ok().flatten()
            } else {
                None
            };

            let present_slides = load_present_slides(&handle, &fig_config.present, namespace, repo);
            let license_content = handle.get_license_content();
            let has_license = handle.has_license();
            let content_entries = handle.list_dir("", Some(fig_config)).unwrap_or_default();

            let ctx = TabContentContext {
                namespace,
                repo,
                tab: default_tab(&fig_config.tabs, &markdown_files),
                commits: &commits,
                markdown_files: &markdown_files,
                selected_md_file: default_file,
                selected_content: default_content.as_deref(),
                fig_content,
                fig_filename,
                tabs_config: &fig_config.tabs,
                present_config: &fig_config.present,
                present_slides: &present_slides,
                license_content: Some(&license_content),
                has_license,
                content_path: "",
                content_entries: &content_entries,
                content_file_bytes: None,
            };
            render_repo(&ctx)
        }
        Err(e) => render_git_error(&e),
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(super::render_layout(
            &content,
            username.as_deref(),
            Some(&page_title),
        ))
    }
}

#[get("/{namespace}/{repo}/tab/{tab}")]
pub async fn tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<TabParams>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let tab = &params.tab;
    let page_title = format!("{namespace}/{repo}");
    let username = get_username_from_request(&req, &auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(h) => h,
        Err(e) => {
            let content = render_git_error(&e);
            return Ok(render_for_request(
                &req,
                content,
                username.as_deref(),
                &page_title,
            ));
        }
    };

    let fig_result = handle.load_config_with_raw();
    let fig_config = &fig_result.config;
    let fig_content = fig_result.raw.as_deref();
    let fig_filename = fig_result.filename.as_deref();

    if fig_config.private && username.is_none() {
        return Ok(render_repo_auth_error(&req, &page_title));
    }

    let commits_result = handle.get_commits(&Depth::default());
    let files_result = handle.list_files(Some(fig_config));

    match commits_result {
        Ok(commits) => {
            let files = files_result.unwrap_or_default();
            let markdown_files = files.markdown_files;

            let default_file = get_default_markdown_file(&markdown_files);
            let default_content = if let Some(file) = default_file {
                handle.read_file(file).ok().flatten()
            } else {
                None
            };

            let present_slides = load_present_slides(&handle, &fig_config.present, namespace, repo);
            let license_content = handle.get_license_content();
            let has_license = handle.has_license();
            let (content_path, content_entries) = if tab == "content" {
                let entries = handle.list_dir("", Some(fig_config)).unwrap_or_default();
                ("", entries)
            } else {
                ("", Vec::new())
            };

            let ctx = TabContentContext {
                namespace,
                repo,
                tab,
                commits: &commits,
                markdown_files: &markdown_files,
                selected_md_file: default_file,
                selected_content: default_content.as_deref(),
                fig_content,
                fig_filename,
                tabs_config: &fig_config.tabs,
                present_config: &fig_config.present,
                present_slides: &present_slides,
                license_content: Some(&license_content),
                has_license,
                content_path,
                content_entries: &content_entries,
                content_file_bytes: None,
            };
            let is_htmx = req.headers().get("HX-Request").is_some();
            let content = render_tab_response(
                &ctx,
                is_htmx,
                fig_content.is_some(),
                !fig_config.present.files.is_empty(),
            );
            Ok(render_for_request(
                &req,
                content,
                username.as_deref(),
                &page_title,
            ))
        }
        Err(e) => {
            let content = render_git_error(&e);
            Ok(render_for_request(
                &req,
                content,
                username.as_deref(),
                &page_title,
            ))
        }
    }
}

#[get("/{namespace}/{repo}/md/{file_path:.*}")]
pub async fn markdown_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<MarkdownParams>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let file_path = &params.file_path;
    let page_title = format!("{namespace}/{repo}");
    let username = get_username_from_request(&req, &auth_state).await;

    if !is_safe_repo_path(file_path) {
        return Ok(render_not_found_for_request(
            &req,
            username.as_deref(),
            &page_title,
        ));
    }

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(h) => h,
        Err(e) => {
            let content = render_git_error(&e);
            return if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(super::render_layout(
                    &content,
                    username.as_deref(),
                    Some(&page_title),
                ))
            };
        }
    };

    let fig_result = handle.load_config_with_raw();
    let fig_config = &fig_result.config;
    let fig_content = fig_result.raw.as_deref();
    let fig_filename = fig_result.filename.as_deref();

    if fig_config.private && username.is_none() {
        return Ok(render_repo_auth_error(&req, &page_title));
    }

    let commits_result = handle.get_commits(&Depth::default());
    let files_result = handle.list_files(Some(fig_config));
    let blob_result = handle.read_file(file_path);

    match commits_result {
        Ok(commits) => {
            let files = files_result.unwrap_or_default();
            let markdown_files = files.markdown_files;
            let license_content = handle.get_license_content();
            let has_license = handle.has_license();

            let content = if req.headers().get("HX-Request").is_some() {
                render_markdown_content_only(
                    namespace,
                    repo,
                    file_path,
                    blob_result.ok().flatten().as_deref(),
                )
            } else {
                let selected_content = blob_result.ok().flatten();
                let present_slides =
                    load_present_slides(&handle, &fig_config.present, namespace, repo);
                let empty_entries: Vec<TreeEntry> = Vec::new();
                let ctx = TabContentContext {
                    namespace,
                    repo,
                    tab: "markdown",
                    commits: &commits,
                    markdown_files: &markdown_files,
                    selected_md_file: Some(file_path),
                    selected_content: selected_content.as_deref(),
                    fig_content,
                    fig_filename,
                    tabs_config: &fig_config.tabs,
                    present_config: &fig_config.present,
                    present_slides: &present_slides,
                    license_content: Some(&license_content),
                    has_license,
                    content_path: "",
                    content_entries: &empty_entries,
                    content_file_bytes: None,
                };
                render_tab_content(&ctx)
            };
            Ok(render_for_request(
                &req,
                content,
                username.as_deref(),
                &page_title,
            ))
        }
        Err(e) => {
            let content = render_git_error(&e);
            Ok(render_for_request(
                &req,
                content,
                username.as_deref(),
                &page_title,
            ))
        }
    }
}

struct ContentFullContext<'a> {
    handle: &'a RepoHandle,
    fig_result: &'a FigConfigWithRaw,
    namespace: &'a str,
    repo: &'a str,
    path: &'a str,
    commits: &'a [Commit],
    entries: &'a [TreeEntry],
    file_bytes: Option<&'a [u8]>,
}

fn render_content_full(ctx: &ContentFullContext<'_>) -> Markup {
    let files_result = ctx.handle.list_files(Some(&ctx.fig_result.config));
    let markdown_files = files_result.unwrap_or_default().markdown_files;
    let default_file = get_default_markdown_file(&markdown_files);
    let default_content = default_file.and_then(|f| ctx.handle.read_file(f).ok().flatten());
    let present_slides = load_present_slides(
        ctx.handle,
        &ctx.fig_result.config.present,
        ctx.namespace,
        ctx.repo,
    );
    let license_content = ctx.handle.get_license_content();

    let tab_ctx = TabContentContext {
        namespace: ctx.namespace,
        repo: ctx.repo,
        tab: "content",
        commits: ctx.commits,
        markdown_files: &markdown_files,
        selected_md_file: default_file,
        selected_content: default_content.as_deref(),
        fig_content: ctx.fig_result.raw.as_deref(),
        fig_filename: ctx.fig_result.filename.as_deref(),
        tabs_config: &ctx.fig_result.config.tabs,
        present_config: &ctx.fig_result.config.present,
        present_slides: &present_slides,
        license_content: Some(&license_content),
        has_license: ctx.handle.has_license(),
        content_path: ctx.path,
        content_entries: ctx.entries,
        content_file_bytes: ctx.file_bytes,
    };
    render_repo(&tab_ctx)
}

#[get("/{namespace}/{repo}/content/{path:.*}")]
pub async fn content_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<ContentParams>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let path = params.path.trim_matches('/').to_string();
    let page_title = format!("{namespace}/{repo}");
    let username = get_username_from_request(&req, &auth_state).await;

    if !is_safe_repo_path(&path) {
        return Ok(render_not_found_for_request(
            &req,
            username.as_deref(),
            &page_title,
        ));
    }

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(h) => h,
        Err(e) => {
            let content = render_git_error(&e);
            return if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(super::render_layout(
                    &content,
                    username.as_deref(),
                    Some(&page_title),
                ))
            };
        }
    };

    let fig_result = handle.load_config_with_raw();
    let fig_config = &fig_result.config;
    let fig_content = fig_result.raw.as_deref();

    if fig_config.private && username.is_none() {
        return Ok(render_repo_auth_error(&req, &page_title));
    }

    let commits_result = handle.get_commits(&Depth::default());

    match commits_result {
        Ok(commits) => {
            let entries = handle.list_dir(&path, Some(fig_config)).unwrap_or_default();
            let file_bytes = if path.is_empty() || !entries.is_empty() {
                None
            } else {
                handle.read_blob_bytes(&path).ok().flatten()
            };

            let content = if req.headers().get("HX-Request").is_some() {
                render_content_htmx(&ContentHtmxContext {
                    namespace,
                    repo,
                    path: &path,
                    entries: &entries,
                    file_bytes: file_bytes.as_deref(),
                    has_config: fig_content.is_some(),
                    tabs_config: &fig_config.tabs,
                    has_present: !fig_config.present.files.is_empty(),
                })
            } else {
                render_content_full(&ContentFullContext {
                    handle: &handle,
                    fig_result: &fig_result,
                    namespace,
                    repo,
                    path: &path,
                    commits: &commits,
                    entries: &entries,
                    file_bytes: file_bytes.as_deref(),
                })
            };
            Ok(render_for_request(
                &req,
                content,
                username.as_deref(),
                &page_title,
            ))
        }
        Err(e) => {
            let content = render_git_error(&e);
            Ok(render_for_request(
                &req,
                content,
                username.as_deref(),
                &page_title,
            ))
        }
    }
}

#[get("/{namespace}/{repo}/slide/{index}")]
pub async fn slide_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    params: web::Path<SlideParams>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let index = params.index;
    let page_title = format!("{namespace}/{repo}");
    let username = get_username_from_request(&req, &auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(h) => h,
        Err(e) => {
            let content = render_git_error(&e);
            return if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(super::render_layout(
                    &content,
                    username.as_deref(),
                    Some(&page_title),
                ))
            };
        }
    };

    let fig_result = handle.load_config_with_raw();
    if fig_result.config.private && username.is_none() {
        return Ok(render_repo_auth_error(&req, &page_title));
    }

    let present_slides = load_present_slides(&handle, &fig_result.config.present, namespace, repo);

    if index >= present_slides.len() {
        let content = render_empty("NOT FOUND", "Slide not found");
        if req.headers().get("HX-Request").is_some() {
            Ok(content)
        } else {
            Ok(super::render_layout(
                &content,
                username.as_deref(),
                Some(&page_title),
            ))
        }
    } else {
        let content = render_slide_content(namespace, repo, index, &present_slides);
        if req.headers().get("HX-Request").is_some() {
            Ok(content)
        } else {
            Ok(super::render_layout(
                &content,
                username.as_deref(),
                Some(&page_title),
            ))
        }
    }
}

/// Converts markdown to HTML, fixing relative links to point to repo root
/// A destination is external when it carries a URL scheme such as `https:` or
/// `mailto:`. A bare relative path never does, because a scheme cannot contain
/// a `/`.
fn has_url_scheme(dest: &str) -> bool {
    match dest.find(':') {
        Some(0) | None => false,
        Some(i) => {
            let scheme = &dest[..i];
            scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
    }
}

/// Splits a link destination into its path part and the trailing `#fragment`
/// or `?query` suffix, which must survive path resolution untouched.
fn split_link_suffix(dest: &str) -> (&str, &str) {
    match dest.find(['#', '?']) {
        Some(i) => dest.split_at(i),
        None => (dest, ""),
    }
}

/// Resolves `link` against `base_dir` (a repository-relative directory, empty
/// for the repository root) into a repository-relative path. `.` segments are
/// dropped and `..` segments pop a parent, clamped at the repository root.
fn resolve_relative_path(base_dir: &str, link: &str) -> String {
    let mut segments: Vec<&str> = if base_dir.is_empty() {
        Vec::new()
    } else {
        base_dir.split('/').collect()
    };

    for segment in link.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }

    segments.join("/")
}

/// Rewrites a markdown link destination into a Fig URL. Returns `None` when the
/// destination is absolute, external or a bare fragment and must be left alone.
fn rewrite_markdown_link(
    namespace: &str,
    repo: &str,
    base_dir: &str,
    dest: &str,
) -> Option<String> {
    if dest.starts_with('/') || dest.starts_with('#') || has_url_scheme(dest) {
        return None;
    }

    let (path_part, suffix) = split_link_suffix(dest);
    let resolved = resolve_relative_path(base_dir, path_part);
    if resolved.is_empty() {
        return None;
    }

    if crate::md::is_markdown(&resolved) {
        Some(format!("/{namespace}/{repo}/md/{resolved}{suffix}"))
    } else {
        Some(format!("/{namespace}/{repo}/content/{resolved}{suffix}"))
    }
}

/// Rewrites a link tag's destination into a Fig URL, returning every other tag
/// unchanged. Shared by the Documentation view and presentations so relative
/// repository links resolve the same way in both.
fn fix_link_tag<'a>(
    tag: pulldown_cmark::Tag<'a>,
    namespace: &str,
    repo: &str,
    base_dir: &str,
) -> pulldown_cmark::Tag<'a> {
    match tag {
        pulldown_cmark::Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        } => {
            let dest_url = match rewrite_markdown_link(namespace, repo, base_dir, &dest_url) {
                Some(url) => url.into(),
                None => dest_url,
            };
            pulldown_cmark::Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }
        }
        other => other,
    }
}

/// Renders markdown to HTML. `base_dir` is the repository-relative directory of
/// the file being rendered and anchors every relative link it contains.
fn markdown_to_html(markdown: &str, namespace: &str, repo: &str, base_dir: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(markdown, options);

    // Process events to fix relative links
    let parser = parser.map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(tag) => Event::Start(fix_link_tag(tag, namespace, repo, base_dir)),
        other => other,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Renders a presentation slide. Mustache template variables are substituted
/// first; relative links are then anchored to the directory that holds the
/// slide's source file, so they open the repository document instead of
/// resolving beneath the `/slide/` URL and 404ing.
fn render_slide_markdown(
    content: &str,
    namespace: &str,
    repo: &str,
    file: &str,
    vars: &HashMap<String, String>,
) -> String {
    let replaced = md::replace_mustache(content, vars);
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let base_dir = parent_path(file);

    let parser = Parser::new_ext(&replaced, options).map(|event| match event {
        Event::Start(tag) => Event::Start(fix_link_tag(tag, namespace, repo, base_dir)),
        other => other,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// An Empty State (DESIGN.md 5.13). The eyebrow names the condition in words so
/// the meaning never rests on colour alone.
fn render_empty(eyebrow: &str, body: &str) -> Markup {
    maud::html! {
        div class="fig-empty" {
            p class="fig-eyebrow" { (eyebrow) }
            p class="fig-empty-body" { (body) }
        }
    }
}

fn render_git_error(e: &git2::Error) -> Markup {
    let code = e.code();
    let code = format!("{code:?}");
    let klass = e.class();
    let klass = format!("{klass:?}");
    let message = e.message();
    maud::html! {
        div class="fig-notice fig-notice--danger" role="alert" {
            p class="fig-eyebrow" { "ERROR" }
            p class="fig-notice-body" { (message) }
            p class="fig-notice-body fig-mono fig-ink-tertiary" { (code) " / " (klass) }
        }
    }
}

/// Renders a "Path not found" response, wrapping it in the full layout for
/// non-HTMX requests.
fn render_not_found_for_request(
    req: &HttpRequest,
    username: Option<&str>,
    page_title: &str,
) -> Markup {
    let content = render_empty("NOT FOUND", "Path not found.");
    if req.headers().get("HX-Request").is_some() {
        content
    } else {
        super::render_layout(&content, username, Some(page_title))
    }
}

fn render_content_htmx(ctx: &ContentHtmxContext<'_>) -> Markup {
    let inner = render_content_view(
        ctx.namespace,
        ctx.repo,
        ctx.path,
        ctx.entries,
        ctx.file_bytes,
    );
    let tabs = render_tabs(
        ctx.namespace,
        ctx.repo,
        "content",
        ctx.has_config,
        ctx.tabs_config,
        ctx.has_present,
    );
    maud::html! {
        (tabs)
        (inner)
    }
}

/// The page head: the trail is the whole heading zone, with the repository
/// name as its highlighted final segment, so the tabs follow it directly.
fn render_repo_crumbs(namespace: &str, repo: &str) -> Markup {
    maud::html! {
        div class="fig-pagehead" {
            nav class="fig-crumbs fig-crumbs--page" aria-label="Breadcrumb" {
                a href="/" { "Namespaces" }
                span class="fig-crumb-sep" aria-hidden="true" { "/" }
                a href=(format!("/{namespace}")) { (namespace) }
                span class="fig-crumb-sep" aria-hidden="true" { "/" }
                h1 class="fig-crumb-current" aria-current="page" { (repo) }
            }
        }
    }
}

/// Renders a tab navigation bar
/// If `tabs_config` is not empty, only those tabs are shown
fn render_tabs(
    namespace: &str,
    repo: &str,
    active_tab: &str,
    has_config: bool,
    tabs_config: &[String],
    has_present: bool,
) -> Markup {
    // Build the list of available tabs
    let mut all_tabs: Vec<(&str, &str)> = vec![];

    // Only add markdown tab if there are markdown files
    if tabs_config.is_empty() {
        // Show all available tabs
        all_tabs.push(("markdown", "Documentation"));
        all_tabs.push(("content", "Content"));

        all_tabs.push(("commits", "Commits"));
        if has_config {
            all_tabs.push(("config", "Config"));
        }
        if has_present {
            all_tabs.push(("present", "Present"));
        }
        all_tabs.push(("license", "License"));
    } else {
        // Use configured tabs
        for tab in tabs_config {
            match tab.as_str() {
                "markdown" => all_tabs.push(("markdown", "Documentation")),
                "content" => all_tabs.push(("content", "Content")),
                "commits" => all_tabs.push(("commits", "Commits")),
                "config" if has_config => all_tabs.push(("config", "Config")),
                "present" if has_present => all_tabs.push(("present", "Present")),
                "license" => all_tabs.push(("license", "License")),
                _ => {}
            }
        }
    }

    maud::html! {
        nav id="tab-nav" class="fig-tabs" aria-label="Repository views" hx-swap-oob="true" {
            @for (tab_id, tab_label) in all_tabs {
                @let href = format!("/{namespace}/{repo}/tab/{tab_id}");
                a
                    class="fig-tab"
                    href=(href)
                    aria-current=[(tab_id == active_tab).then_some("page")]
                    hx-get=(href)
                    hx-target="#tab-content"
                    hx-push-url=(format!("/{namespace}/{repo}"))
                {
                    (tab_label)
                }
            }
        }
    }
}

/// The tab shown when no tab is named: the first configured one, else Markdown
/// when the repository has markdown, else Commits.
fn default_tab<'a>(tabs_config: &'a [String], markdown_files: &[String]) -> &'a str {
    if let Some(first) = tabs_config.first() {
        first.as_str()
    } else if markdown_files.is_empty() {
        "commits"
    } else {
        "markdown"
    }
}

fn render_repo(ctx: &TabContentContext<'_>) -> Markup {
    maud::html! {
        (render_repo_crumbs(ctx.namespace, ctx.repo))
        (render_tab_content(ctx))
    }
}

fn render_tab_content(ctx: &TabContentContext<'_>) -> Markup {
    let has_config = ctx.fig_content.is_some();
    let has_present = !ctx.present_config.files.is_empty();
    maud::html! {
        // Tab navigation (update active state)
        (render_tabs(ctx.namespace, ctx.repo, ctx.tab, has_config, ctx.tabs_config, has_present))

        // Tab content container, announced on swap
        div id="tab-content" aria-live="polite" {
            (render_tab_content_inner(ctx))
        }
    }
}

/// Get the default markdown file to show - prefers README.md if it exists
fn get_default_markdown_file(markdown_files: &[String]) -> Option<&str> {
    // First try to find README.md (case-insensitive)
    let readme = markdown_files
        .iter()
        .find(|f| f.eq_ignore_ascii_case("README.md"));
    if readme.is_some() {
        return readme.map(String::as_str);
    }
    // Then try any README variant
    let readme = markdown_files
        .iter()
        .find(|f| f.to_lowercase().starts_with("readme"));
    if readme.is_some() {
        return readme.map(String::as_str);
    }
    // Fall back to first file
    markdown_files.first().map(String::as_str)
}

fn render_tab_content_inner(ctx: &TabContentContext<'_>) -> Markup {
    match ctx.tab {
        "markdown" => {
            // Use selected file or find default (README.md preferred)
            let file_to_show = ctx
                .selected_md_file
                .or_else(|| get_default_markdown_file(ctx.markdown_files))
                .unwrap_or("README.md");

            render_markdown_view(
                ctx.namespace,
                ctx.repo,
                file_to_show,
                ctx.selected_content,
                ctx.markdown_files,
            )
        }
        "commits" => render_commits_view(ctx.commits),
        "config" => render_config_view(ctx.fig_content, ctx.fig_filename),
        "content" => render_content_view(
            ctx.namespace,
            ctx.repo,
            ctx.content_path,
            ctx.content_entries,
            ctx.content_file_bytes,
        ),
        "present" => render_present_view(ctx.namespace, ctx.repo, ctx.present_slides),
        "license" => render_license_view(ctx.license_content),
        _ => {
            // Unknown tab - show Markdown by default if available, then Commits
            let new_tab = if ctx.markdown_files.is_empty() {
                "commits"
            } else {
                "markdown"
            };
            let new_ctx = TabContentContext {
                namespace: ctx.namespace,
                repo: ctx.repo,
                tab: new_tab,
                commits: ctx.commits,
                markdown_files: ctx.markdown_files,
                selected_md_file: ctx.selected_md_file,
                selected_content: ctx.selected_content,
                fig_content: ctx.fig_content,
                fig_filename: ctx.fig_filename,
                tabs_config: ctx.tabs_config,
                present_config: ctx.present_config,
                present_slides: ctx.present_slides,
                license_content: ctx.license_content,
                has_license: ctx.has_license,
                content_path: ctx.content_path,
                content_entries: ctx.content_entries,
                content_file_bytes: ctx.content_file_bytes,
            };
            render_tab_content_inner(&new_ctx)
        }
    }
}

fn render_commits_view(commits: &[Commit]) -> Markup {
    maud::html! {
        div class="fig-stack" {
            @if commits.is_empty() {
                (render_empty("NO COMMITS", "This repository has no commits yet."))
            } @else {
                ol class="fig-commits" {
                    @for commit in commits {
                        (render_commit(commit))
                    }
                }
            }
        }
    }
}

fn render_config_view(fig_content: Option<&str>, fig_filename: Option<&str>) -> Markup {
    let config_filename = fig_filename.unwrap_or(".fig.toml");

    maud::html! {
        div class="fig-stack" {
            h2 class="fig-section" { "Configuration" }
            p class="fig-hint" {
                "Repository configuration from " code class="fig-mono" { (config_filename) }
            }
            @if let Some(content) = fig_content {
                pre class="fig-code" tabindex="0" aria-label=(format!("{config_filename} contents")) {
                    code { (content) }
                }
            } @else {
                (render_empty(
                    "NO CONFIGURATION",
                    "No configuration file found. Create a .fig.toml file in the repository root to configure ignore patterns."
                ))
            }
        }
    }
}

fn render_license_view(license_content: Option<&str>) -> Markup {
    maud::html! {
        div class="fig-stack" {
            @if let Some(content) = license_content {
                div class="fig-md fig-md--boxed" {
                    (maud::PreEscaped(content))
                }
            } @else {
                (render_empty("NO LICENSE", "No license information available."))
            }
        }
    }
}

/// Returns (href, push-url) pair for a path inside the content tab
fn content_urls(namespace: &str, repo: &str, path: &str) -> (String, String) {
    if path.is_empty() {
        let href = format!("/{namespace}/{repo}/tab/content");
        (href, format!("/{namespace}/{repo}"))
    } else {
        let href = format!("/{namespace}/{repo}/content/{path}");
        (href.clone(), href)
    }
}

fn parent_path(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[..i],
        None => "",
    }
}

/// Precomputed breadcrumb segments: (label, accumulated path, `is_last`)
fn breadcrumb_segments(path: &str) -> Vec<(String, String, bool)> {
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

fn render_content_breadcrumbs(namespace: &str, repo: &str, path: &str) -> Markup {
    let segments = breadcrumb_segments(path);
    maud::html! {
        nav class="fig-crumbs fig-crumbs--path" aria-label="File path" {
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
                span class="fig-crumb-sep" aria-hidden="true" { "/" }
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

fn render_content_view(
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
fn render_content_row(href: &str, push: &str, entry: &TreeEntry) -> Markup {
    maud::html! {
        a
            class=(if entry.is_dir { "fig-row" } else { "fig-row fig-row--file" })
            href=(href)
            hx-get=(href)
            hx-target="#tab-content"
            hx-push-url=(push)
        {
            span class="fig-row-id" {
                (entry.name)
                @if entry.is_dir { "/" }
            }
        }
    }
}

fn render_content_dir(namespace: &str, repo: &str, path: &str, entries: &[TreeEntry]) -> Markup {
    maud::html! {
        div class="fig-stack" {
            (render_content_breadcrumbs(namespace, repo, path))

            @if entries.is_empty() && path.is_empty() {
                (render_empty("NO FILES", "No files in this repository."))
            } @else {
                div class="fig-panel fig-panel--flush" {
                    div class="fig-list" {
                        @if !path.is_empty() {
                            @let (href, push) = content_urls(namespace, repo, parent_path(path));
                            a
                                class="fig-row"
                                href=(href)
                                hx-get=(href)
                                hx-target="#tab-content"
                                hx-push-url=(push)
                            {
                                span class="fig-row-id" { ".." }
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

fn render_content_file(namespace: &str, repo: &str, path: &str, bytes: &[u8]) -> Markup {
    let is_binary = bytes.contains(&0);

    maud::html! {
        div class="fig-stack" {
            (render_content_breadcrumbs(namespace, repo, path))

            @if is_binary {
                (render_empty(
                    "BINARY FILE",
                    &format!("Binary file ({} bytes). Not displayed.", bytes.len())
                ))
            } @else {
                @let text = String::from_utf8_lossy(bytes).into_owned();
                @if crate::md::is_markdown(path) {
                    div class="fig-md fig-md--boxed" {
                        (maud::PreEscaped(markdown_to_html(&text, namespace, repo, parent_path(path))))
                    }
                } @else {
                    pre class="fig-code" tabindex="0" aria-label=(format!("{path} contents")) {
                        code { (text) }
                    }
                }
            }
        }
    }
}

fn load_present_slides(
    handle: &RepoHandle,
    present_config: &PresentConfig,
    namespace: &str,
    repo: &str,
) -> Vec<PresentSlide> {
    present_config
        .files
        .iter()
        .filter_map(|file| {
            let content = handle.read_file(file).ok()??;
            let html = render_slide_markdown(
                &content,
                namespace,
                repo,
                file,
                &present_config.template_vars,
            );
            Some(PresentSlide { html })
        })
        .collect()
}

/// The slide counter, zero-padded to the width of the total so the mono column
/// never reflows as the deck advances (DESIGN.md 5.17).
fn slide_counter(current_index: usize, slide_count: usize) -> String {
    let width = slide_count.to_string().len();
    format!(
        "{:0width$} / {slide_count}",
        current_index + 1,
        width = width
    )
}

fn render_slide_nav_button(id: &str, label: &str, target: Option<&str>) -> Markup {
    maud::html! {
        @if let Some(href) = target {
            button
                id=(id)
                class="fig-btn fig-btn--quiet"
                type="button"
                hx-get=(href)
                hx-target="#present-container"
                hx-swap="innerHTML"
            {
                (label)
            }
        } @else {
            button id=(id) class="fig-btn fig-btn--quiet" type="button" disabled {
                (label)
            }
        }
    }
}

fn render_slide_content(
    namespace: &str,
    repo: &str,
    current_index: usize,
    slides: &[PresentSlide],
) -> Markup {
    let slide_count = slides.len();
    let slide = &slides[current_index];
    let prev =
        (current_index > 0).then(|| format!("/{namespace}/{repo}/slide/{}", current_index - 1));
    let next = (current_index + 1 < slide_count)
        .then(|| format!("/{namespace}/{repo}/slide/{}", current_index + 1));

    maud::html! {
        header class="fig-present-bar" {
            p class="fig-eyebrow" { "PRESENTATION" }
            div class="fig-present-tools" {
                span class="fig-present-count" aria-live="polite" {
                    (slide_counter(current_index, slide_count))
                }
                button id="decrease-text-size" class="fig-btn fig-btn--quiet" type="button"
                    aria-label="Decrease presentation font size" title="Decrease font size"
                    "hx-on:click"=(present_size_step(false))
                    "hx-live:disabled"=(format!("data.figTextSize === {}", PRESENT_TEXT_SIZES[0]))
                {
                    "A−"
                }
                span id="present-text-size" class="fig-present-count" aria-live="polite"
                    "hx-live:text"="data.figTextSize + '%'" { "100%" }
                button id="increase-text-size" class="fig-btn fig-btn--quiet" type="button"
                    aria-label="Increase presentation font size" title="Increase font size"
                    "hx-on:click"=(present_size_step(true))
                    "hx-live:disabled"=(format!(
                        "data.figTextSize === {}",
                        PRESENT_TEXT_SIZES[PRESENT_TEXT_SIZES.len() - 1]
                    ))
                {
                    "A+"
                }
                button id="fullscreen-toggle" class="fig-btn fig-btn--quiet" type="button" {
                    "Fullscreen"
                }
                (super::render_theme_toggle())
            }
        }

        div id="slides-wrapper" class="fig-present-stage" {
            article class="fig-md fig-md--slide" aria-live="polite" {
                (maud::PreEscaped(&slide.html))
            }
        }

        nav class="fig-present-controls" aria-label="Slide navigation" {
            (render_slide_nav_button("prev-slide", "\u{2190} Previous", prev.as_deref()))
            div class="fig-ticks" role="group" aria-label="Go to slide" {
                @for i in 0..slide_count {
                    button
                        class="fig-tick"
                        type="button"
                        aria-label=(format!("Slide {} of {slide_count}", i + 1))
                        aria-current=[(i == current_index).then_some("true")]
                        hx-get=(format!("/{namespace}/{repo}/slide/{i}"))
                        hx-target="#present-container"
                        hx-swap="innerHTML"
                    {}
                }
            }
            (render_slide_nav_button("next-slide", "Next \u{2192}", next.as_deref()))
        }
    }
}

fn render_present_view(namespace: &str, repo: &str, slides: &[PresentSlide]) -> Markup {
    if slides.is_empty() {
        return render_empty(
            "NO SLIDES",
            "No presentation slides configured. Add a [present] section with files to your .fig.toml.",
        );
    }

    maud::html! {
        section
            id="present-container"
            class="fig-present"
            data-fig-text-size=(PRESENT_TEXT_SIZES[0])
            hx-live=(present_size_restore())
            tabindex="-1"
            aria-roledescription="carousel"
            aria-label="Presentation"
        {
            (render_slide_content(namespace, repo, 0, slides))
        }
        script { (maud::PreEscaped(PRESENT_SCRIPT)) }
    }
}

fn render_markdown_view(
    namespace: &str,
    repo: &str,
    current_file: &str,
    content: Option<&str>,
    markdown_files: &[String],
) -> Markup {
    maud::html! {
        div class="fig-rail-shell" {
            // A one-file repository gets no rail at all; its only entry would
            // point at the document already on screen (DESIGN.md 5.10).
            @if markdown_files.len() > 1 {
                nav class="fig-rail fig-rail--files" aria-label="Markdown files" {
                    p class="fig-eyebrow" { "MARKDOWN FILES" }
                    @for file in markdown_files {
                        @let href = format!("/{namespace}/{repo}/md/{file}");
                        a
                            class="fig-rail-item"
                            href=(href)
                            aria-current=[(file == current_file).then_some("page")]
                            hx-get=(href)
                            hx-target="#markdown-view"
                        {
                            (file)
                        }
                    }
                }
            }

            div class="fig-rail-body" {
                div id="markdown-view" aria-live="polite" {
                    (render_markdown_content_only(namespace, repo, current_file, content))
                }
            }
        }
    }
}

/// Renders just the markdown content without the sidebar (for HTMX updates)
fn render_markdown_content_only(
    namespace: &str,
    repo: &str,
    current_file: &str,
    content: Option<&str>,
) -> Markup {
    let base_dir = parent_path(current_file);
    let html_content = content.map(|md| markdown_to_html(md, namespace, repo, base_dir));

    maud::html! {
        @if let Some(html) = html_content {
            div class="fig-md fig-md--prose" {
                (maud::PreEscaped(html))
            }
        } @else {
            (render_empty("NOT FOUND", "File not found or empty."))
        }
    }
}

fn render_commit(commit: &Commit) -> Markup {
    let hash = commit.hash();
    let author = commit.author();
    let date = commit.date();
    let commit_message = commit.message();
    maud::html! {
        li class="fig-commit" {
            div class="fig-commit-meta" {
                code class="fig-commit-hash" {
                    (hash.chars().take(7).collect::<String>())
                }
                span class="fig-commit-author" { (author) }
                time class="fig-commit-date" datetime=(date.to_rfc3339()) {
                    (date.format("%Y-%m-%d %H:%M"))
                }
            }
            p class="fig-commit-msg" { (commit_message) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_present_config() -> &'static PresentConfig {
        static EMPTY: std::sync::OnceLock<PresentConfig> = std::sync::OnceLock::new();
        EMPTY.get_or_init(PresentConfig::default)
    }

    /// A context with nothing configured. Tests override only the fields they
    /// exercise, which keeps each assertion pinned to one input.
    fn base_ctx() -> TabContentContext<'static> {
        TabContentContext {
            namespace: "acme",
            repo: "my-project",
            tab: "commits",
            commits: &[],
            markdown_files: &[],
            selected_md_file: None,
            selected_content: None,
            fig_content: None,
            fig_filename: None,
            tabs_config: &[],
            present_config: empty_present_config(),
            present_slides: &[],
            license_content: None,
            has_license: false,
            content_path: "",
            content_entries: &[],
            content_file_bytes: None,
        }
    }

    fn slides(count: usize) -> Vec<PresentSlide> {
        (0..count)
            .map(|i| PresentSlide {
                html: format!("<h1>Slide {i}</h1>"),
            })
            .collect()
    }

    fn entry(name: &str, is_dir: bool) -> TreeEntry {
        TreeEntry {
            name: name.to_string(),
            path: name.to_string(),
            is_dir,
        }
    }

    fn classes_in(html: &str) -> Vec<String> {
        let marker = "class=\"";
        html.match_indices(marker)
            .flat_map(|(start, _)| {
                let rest = &html[start + marker.len()..];
                let end = rest.find('"').expect("class attribute must be closed");
                rest[..end]
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn index_of(html: &str, needle: &str) -> usize {
        html.find(needle)
            .unwrap_or_else(|| panic!("expected markup to contain {needle}\n{html}"))
    }

    fn count_of(html: &str, needle: &str) -> usize {
        html.matches(needle).count()
    }

    fn tab_labels(html: &str) -> Vec<String> {
        html.match_indices("class=\"fig-tab\"")
            .map(|(start, _)| {
                let rest = &html[start..];
                let open = rest.find('>').expect("tab tag must be closed");
                let close = rest.find("</a>").expect("tab must be closed");
                rest[open + 1..close].to_owned()
            })
            .collect()
    }

    #[test]
    fn test_parent_path() {
        assert_eq!(parent_path(""), "");
        assert_eq!(parent_path("README.md"), "");
        assert_eq!(parent_path("src/main.rs"), "src");
        assert_eq!(parent_path("a/b/c.txt"), "a/b");
    }

    #[test]
    fn test_breadcrumb_segments() {
        let segs = breadcrumb_segments("src/git");
        assert_eq!(
            segs,
            vec![
                ("src".to_string(), "src".to_string(), false),
                ("git".to_string(), "src/git".to_string(), true),
            ]
        );

        let segs = breadcrumb_segments("");
        assert!(segs.is_empty());
    }

    #[test]
    fn test_markdown_to_html() {
        let md = "# Hello\n\nThis is **bold** text.";
        let html = markdown_to_html(md, "test", "repo", "");
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_markdown_link_fixing() {
        let md = "[Link](./other.md) and [External](https://example.com)";
        let html = markdown_to_html(md, "ns", "repo", "");
        // Internal markdown links should be fixed and normalized
        assert!(html.contains("/ns/repo/md/other.md"), "{html}");
        // External links should remain unchanged
        assert!(html.contains("https://example.com"));
    }

    #[test]
    fn test_markdown_links_resolve_against_containing_directory() {
        let md = "[Sibling](other.md) [Nested](sub/deep.md) [Up](../top.md)";
        let html = markdown_to_html(md, "ns", "repo", "docs/guide");
        assert!(html.contains("/ns/repo/md/docs/guide/other.md"), "{html}");
        assert!(
            html.contains("/ns/repo/md/docs/guide/sub/deep.md"),
            "{html}"
        );
        assert!(html.contains("/ns/repo/md/docs/top.md"), "{html}");
    }

    #[test]
    fn test_markdown_non_markdown_links_target_content_route() {
        let md = "[Image](logo.png) [Dir](sub/)";
        let html = markdown_to_html(md, "ns", "repo", "foo.bar");
        assert!(html.contains("/ns/repo/content/foo.bar/logo.png"), "{html}");
        assert!(html.contains("/ns/repo/content/foo.bar/sub"), "{html}");
    }

    #[test]
    fn test_markdown_link_preserves_fragment_and_query() {
        let md = "[Anchor](other.md#section) [Query](file.txt?raw=1)";
        let html = markdown_to_html(md, "ns", "repo", "docs");
        assert!(html.contains("/ns/repo/md/docs/other.md#section"), "{html}");
        assert!(
            html.contains("/ns/repo/content/docs/file.txt?raw=1"),
            "{html}"
        );
    }

    #[test]
    fn test_markdown_link_leaves_absolute_and_scheme_links_alone() {
        let md = "[Root](/other) [Mail](mailto:a@b.com) [Frag](#here)";
        let html = markdown_to_html(md, "ns", "repo", "docs");
        assert!(html.contains("href=\"/other\""), "{html}");
        assert!(html.contains("href=\"mailto:a@b.com\""), "{html}");
        assert!(html.contains("href=\"#here\""), "{html}");
    }

    #[test]
    fn test_slide_markdown_anchors_relative_links_to_the_slide_directory() {
        let md = "See [next](second.md), [chart](img/chart.png) and [site](https://example.com).";
        let html = render_slide_markdown(md, "ns", "repo", "slides/first.md", &HashMap::new());
        assert!(html.contains("/ns/repo/md/slides/second.md"), "{html}");
        assert!(
            html.contains("/ns/repo/content/slides/img/chart.png"),
            "{html}"
        );
        assert!(html.contains("href=\"https://example.com\""), "{html}");
    }

    #[test]
    fn test_slide_markdown_substitutes_template_variables() {
        let mut vars = HashMap::new();
        vars.insert("author".to_string(), "Jane".to_string());
        let html = render_slide_markdown(
            "# Talk by {{author}}",
            "ns",
            "repo",
            "slides/intro.md",
            &vars,
        );
        assert!(html.contains("Talk by Jane"), "{html}");
    }

    #[test]
    fn test_resolve_relative_path() {
        assert_eq!(resolve_relative_path("", "a.md"), "a.md");
        assert_eq!(resolve_relative_path("docs", "./a.md"), "docs/a.md");
        assert_eq!(resolve_relative_path("docs/sub", "../a.md"), "docs/a.md");
        assert_eq!(resolve_relative_path("docs", "../../../a.md"), "a.md");
        assert_eq!(resolve_relative_path("docs", "sub/"), "docs/sub");
        assert_eq!(resolve_relative_path("docs", "."), "docs");
    }

    #[test]
    fn test_has_url_scheme() {
        assert!(has_url_scheme("https://example.com"));
        assert!(has_url_scheme("mailto:a@b.com"));
        assert!(!has_url_scheme("docs/a.md"));
        assert!(!has_url_scheme("docs/a:b.md"));
        assert!(!has_url_scheme(":leading"));
        assert!(!has_url_scheme("./a.md"));
    }

    #[test]
    fn test_get_default_markdown_file() {
        let files = vec![
            "docs/guide.md".to_string(),
            "README.md".to_string(),
            "CHANGELOG.md".to_string(),
        ];
        assert_eq!(get_default_markdown_file(&files), Some("README.md"));

        let files = vec!["docs/guide.md".to_string(), "readme.md".to_string()];
        assert_eq!(get_default_markdown_file(&files), Some("readme.md"));

        let files = vec!["guide.md".to_string(), "docs/help.md".to_string()];
        assert_eq!(get_default_markdown_file(&files), Some("guide.md"));

        let files: Vec<String> = vec![];
        assert_eq!(get_default_markdown_file(&files), None);
    }

    #[test]
    fn test_markdown_tables_rendering() {
        let md = "| Header 1 | Header 2 |\n|----------|----------|\n| Cell 1   | Cell 2   |";
        let html = markdown_to_html(md, "test", "repo", "");
        // Tables should be rendered as HTML table elements
        assert!(
            html.contains("<table>"),
            "Expected <table> tag in output: {html}"
        );
        assert!(html.contains("<th>"), "Expected <th> tag in output: {html}");
        assert!(html.contains("<td>"), "Expected <td> tag in output: {html}");
    }

    #[test]
    fn test_default_tab_prefers_configuration_then_markdown() {
        let configured = vec!["license".to_string(), "commits".to_string()];
        let files = vec!["README.md".to_string()];
        assert_eq!(default_tab(&configured, &files), "license");
        assert_eq!(default_tab(&[], &files), "markdown");
        assert_eq!(default_tab(&[], &[]), "commits");
    }

    #[test]
    fn test_tabs_are_an_out_of_band_labelled_nav() {
        let html = render_tabs("acme", "my-project", "commits", true, &[], true).into_string();

        assert!(
            html.contains(
                r#"<nav id="tab-nav" class="fig-tabs" aria-label="Repository views" hx-swap-oob="true">"#
            ),
            "the tab bar keeps its OOB swap identity: {html}"
        );
        assert_eq!(
            count_of(&html, "hx-swap-oob"),
            1,
            "exactly one OOB target: {html}"
        );
        assert!(!html.contains("<div id=\"tab-nav\""), "{html}");
    }

    #[test]
    fn test_tabs_preserve_every_htmx_navigation_attribute() {
        let html = render_tabs("acme", "my-project", "commits", true, &[], true).into_string();
        let tabs = [
            "markdown", "content", "commits", "config", "present", "license",
        ];

        for tab in tabs {
            let href = format!("/acme/my-project/tab/{tab}");
            assert!(
                html.contains(&format!("href=\"{href}\"")),
                "missing href for {tab}: {html}"
            );
            assert!(
                html.contains(&format!("hx-get=\"{href}\"")),
                "missing hx-get for {tab}: {html}"
            );
        }
        assert_eq!(count_of(&html, "hx-target=\"#tab-content\""), tabs.len());
        assert_eq!(
            count_of(&html, "hx-push-url=\"/acme/my-project\""),
            tabs.len()
        );
    }

    #[test]
    fn test_tabs_mark_exactly_one_current_tab() {
        let html = render_tabs("acme", "my-project", "commits", true, &[], true).into_string();

        assert_eq!(
            count_of(&html, "aria-current=\"page\""),
            1,
            "one current tab only: {html}"
        );
        let href = index_of(&html, "href=\"/acme/my-project/tab/commits\"");
        let current = index_of(&html, "aria-current=\"page\"");
        let label = index_of(&html, ">Commits<");
        assert!(
            href < current && current < label,
            "aria-current must sit on the active tab: {html}"
        );
    }

    #[test]
    fn test_tabs_render_only_available_views_in_order() {
        let html = render_tabs("acme", "my-project", "markdown", false, &[], false).into_string();
        assert_eq!(
            tab_labels(&html),
            vec!["Documentation", "Content", "Commits", "License"],
            "unconfigured repositories hide Config and Present: {html}"
        );

        let configured = [
            "license".to_string(),
            "bogus".to_string(),
            "config".to_string(),
            "present".to_string(),
            "commits".to_string(),
        ];
        let html =
            render_tabs("acme", "my-project", "license", false, &configured, true).into_string();
        assert_eq!(
            tab_labels(&html),
            vec!["License", "Present", "Commits"],
            "configured order wins and unknown or unavailable tabs are dropped: {html}"
        );
    }

    #[test]
    fn test_repo_page_leads_with_a_breadcrumb_head_then_the_tabs() {
        let html = render_repo(&base_ctx()).into_string();

        let crumbs = index_of(
            &html,
            r#"<nav class="fig-crumbs fig-crumbs--page" aria-label="Breadcrumb">"#,
        );
        let tabs = index_of(&html, "id=\"tab-nav\"");
        assert!(crumbs < tabs, "the trail, then the tabs: {html}");
        assert!(
            !html.contains("fig-optic-rule"),
            "the tab bar's own rule closes the head; no second rule: {html}"
        );
        assert!(
            html.contains(r#"<h1 class="fig-crumb-current" aria-current="page">my-project</h1>"#),
            "the trail's final segment is the page heading, rendered verbatim: {html}"
        );
        assert_eq!(count_of(&html, "<h1"), 1, "exactly one h1 per page: {html}");
    }

    #[test]
    fn test_repo_page_announces_its_tab_panel() {
        let html = render_repo(&base_ctx()).into_string();
        assert!(
            html.contains(r#"<div id="tab-content" aria-live="polite">"#),
            "swapped tab content must be announced: {html}"
        );
    }

    #[test]
    fn test_commit_row_is_a_dense_machine_record() {
        let date = chrono::DateTime::from_timestamp(1_756_000_000, 0)
            .expect("valid timestamp")
            .to_utc();
        let commit = Commit::new(
            "a1b2c3d4e5f6".to_string(),
            "silen".to_string(),
            date,
            "Rewrite the theme layer".to_string(),
        );
        let html = render_commit(&commit).into_string();

        assert!(html.starts_with(r#"<li class="fig-commit">"#), "{html}");
        assert!(
            html.contains(r#"<code class="fig-commit-hash">a1b2c3d</code>"#),
            "the hash stays abbreviated to seven characters: {html}"
        );
        assert!(
            html.contains(r#"<span class="fig-commit-author">silen</span>"#),
            "{html}"
        );
        assert!(
            html.contains(&format!(
                r#"<time class="fig-commit-date" datetime="{}">"#,
                date.to_rfc3339()
            )),
            "the machine timestamp travels with the readable one: {html}"
        );
        assert!(
            html.contains(&date.format("%Y-%m-%d %H:%M").to_string()),
            "{html}"
        );
        assert!(
            html.contains(r#"<p class="fig-commit-msg">Rewrite the theme layer</p>"#),
            "{html}"
        );
    }

    #[test]
    fn test_commits_view_falls_back_to_an_empty_state() {
        let html = render_commits_view(&[]).into_string();
        assert!(html.contains("fig-empty"), "{html}");
        assert!(html.contains(">NO COMMITS<"), "{html}");
        assert!(!html.contains("<ol"), "no empty list is rendered: {html}");
    }

    #[test]
    fn test_content_rows_mark_directories_with_a_trailing_slash() {
        let entries = vec![entry("src", true), entry("README.md", false)];
        let html = render_content_dir("acme", "my-project", "", &entries).into_string();

        assert!(
            html.contains(r#"<a class="fig-row" href="/acme/my-project/content/src""#),
            "directories keep the primary identifier treatment: {html}"
        );
        assert!(
            html.contains(r#"<span class="fig-row-id">src/</span>"#),
            "the trailing slash is the whole directory affordance: {html}"
        );
        assert!(
            html.contains(
                r#"<a class="fig-row fig-row--file" href="/acme/my-project/content/README.md""#
            ),
            "files drop to the secondary identifier treatment: {html}"
        );
        assert_eq!(
            count_of(&html, "hx-target=\"#tab-content\""),
            2,
            "one htmx row per entry; the root trail is the current page, not a link: {html}"
        );
        assert!(html.contains("fig-panel fig-panel--flush"), "{html}");
        assert!(html.contains("class=\"fig-list\""), "{html}");
    }

    #[test]
    fn test_content_dir_offers_a_parent_row_below_the_root() {
        let entries = vec![entry("main.rs", false)];
        let html = render_content_dir("acme", "my-project", "src", &entries).into_string();
        assert!(
            html.contains(r##"href="/acme/my-project/tab/content" hx-get="/acme/my-project/tab/content" hx-target="#tab-content" hx-push-url="/acme/my-project""##),
            "the parent row climbs back to the tab route: {html}"
        );
        assert!(
            html.contains(r#"<span class="fig-row-id">..</span>"#),
            "{html}"
        );
    }

    #[test]
    fn test_content_breadcrumbs_mark_the_final_segment_as_current() {
        let html = render_content_breadcrumbs("acme", "my-project", "src/git").into_string();
        assert!(
            html.starts_with(r#"<nav class="fig-crumbs fig-crumbs--path" aria-label="File path">"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<span class="fig-crumb-sep" aria-hidden="true">/</span>"#),
            "separators are decorative: {html}"
        );
        assert!(
            html.contains(r#"<span aria-current="page">git</span>"#),
            "{html}"
        );
        assert_eq!(count_of(&html, "aria-current=\"page\""), 1, "{html}");
        assert!(
            html.contains(r##"hx-get="/acme/my-project/content/src" hx-target="#tab-content" hx-push-url="/acme/my-project/content/src""##),
            "intermediate segments stay htmx links: {html}"
        );

        let root = render_content_breadcrumbs("acme", "my-project", "").into_string();
        assert!(
            root.contains(r#"<span aria-current="page">my-project</span>"#),
            "at the root the repository itself is the current segment: {root}"
        );
    }

    #[test]
    fn test_markdown_rail_appears_only_for_multiple_files() {
        let one = vec!["README.md".to_string()];
        let html = render_markdown_view("acme", "my-project", "README.md", Some("# Hi"), &one)
            .into_string();
        assert!(
            !html.contains("fig-rail-item"),
            "a single markdown file gets no rail: {html}"
        );
        assert!(
            html.contains(r#"<div id="markdown-view" aria-live="polite">"#),
            "{html}"
        );

        let many = vec!["README.md".to_string(), "docs/guide.md".to_string()];
        let html = render_markdown_view("acme", "my-project", "README.md", Some("# Hi"), &many)
            .into_string();
        assert!(
            html.contains(r#"<nav class="fig-rail fig-rail--files" aria-label="Markdown files">"#),
            "{html}"
        );
        assert!(
            html.contains(r##"aria-current="page" hx-get="/acme/my-project/md/README.md" hx-target="#markdown-view""##),
            "the open file is the current rail item and keeps its htmx wiring: {html}"
        );
        assert_eq!(count_of(&html, "aria-current=\"page\""), 1, "{html}");
        assert_eq!(count_of(&html, "hx-target=\"#markdown-view\""), 2, "{html}");
    }

    #[test]
    fn test_markdown_content_reports_a_missing_file() {
        let html =
            render_markdown_content_only("acme", "my-project", "README.md", None).into_string();
        assert!(html.contains("fig-empty"), "{html}");
        assert!(html.contains("File not found or empty."), "{html}");

        let html = render_markdown_content_only("acme", "my-project", "README.md", Some("# Hi"))
            .into_string();
        assert!(
            html.contains(r#"<div class="fig-md fig-md--prose">"#),
            "{html}"
        );
    }

    #[test]
    fn test_presentation_targets_its_container_and_scopes_the_keyboard() {
        let deck = slides(3);
        let html = render_present_view("acme", "my-project", &deck).into_string();

        assert!(
            html.contains(r#"<section id="present-container" class="fig-present" "#)
                && html.contains(
                    r#"tabindex="-1" aria-roledescription="carousel" aria-label="Presentation">"#
                ),
            "{html}"
        );
        assert_eq!(
            count_of(&html, "hx-target=\"#present-container\""),
            4,
            "next plus three ticks all swap the container: {html}"
        );
        assert_eq!(count_of(&html, "hx-swap=\"innerHTML\""), 4, "{html}");

        assert!(
            !html.contains("document.onkeydown"),
            "the global key hijack is gone: {html}"
        );
        assert!(
            !html.contains("document.addEventListener"),
            "nothing is bound to the document: {html}"
        );
        assert!(
            html.contains("c.addEventListener('keydown'"),
            "the key listener is bound to the container: {html}"
        );
        assert!(
            html.contains("c.contains(document.activeElement)"),
            "arrow keys act only while the container owns focus: {html}"
        );
        assert!(
            html.contains("document.fullscreenElement===c"),
            "fullscreen counts as container focus: {html}"
        );
    }

    #[test]
    fn test_presentation_controls_are_labelled_and_bounded() {
        let deck = slides(3);
        let html = render_slide_content("acme", "my-project", 0, &deck).into_string();

        assert!(
            html.contains("fig-theme-toggle"),
            "the theme switch must remain accessible in fullscreen and after slide swaps: {html}"
        );
        for control in [
            r#"id="decrease-text-size" class="fig-btn fig-btn--quiet" type="button" aria-label="Decrease presentation font size" title="Decrease font size""#,
            r#"id="increase-text-size" class="fig-btn fig-btn--quiet" type="button" aria-label="Increase presentation font size" title="Increase font size""#,
            r#"<span id="present-text-size" class="fig-present-count" aria-live="polite" hx-live:text="data.figTextSize + '%'">100%</span>"#,
        ] {
            assert!(
                html.contains(control),
                "missing presentation font control: {html}"
            );
        }
        assert!(
            html.contains(
                r#"<button id="prev-slide" class="fig-btn fig-btn--quiet" type="button" disabled>"#
            ),
            "the first slide disables Previous rather than swapping in a span: {html}"
        );
        assert!(
            html.contains(r#"<button id="next-slide" class="fig-btn fig-btn--quiet" type="button" hx-get="/acme/my-project/slide/1""#),
            "{html}"
        );
        assert!(
            html.contains(r#"<div class="fig-ticks" role="group" aria-label="Go to slide">"#),
            "{html}"
        );
        for i in 1..=3 {
            assert!(
                html.contains(&format!(r#"aria-label="Slide {i} of 3""#)),
                "every tick names its slide: {html}"
            );
        }
        assert_eq!(
            count_of(&html, "aria-current=\"true\""),
            1,
            "one tick is current: {html}"
        );
        assert!(
            html.contains(r#"<button id="fullscreen-toggle" class="fig-btn fig-btn--quiet" type="button">Fullscreen</button>"#),
            "fullscreen is a real button with a visible label: {html}"
        );

        let last = render_slide_content("acme", "my-project", 2, &deck).into_string();
        assert!(
            last.contains(
                r#"<button id="next-slide" class="fig-btn fig-btn--quiet" type="button" disabled>"#
            ),
            "the last slide disables Next: {last}"
        );
    }

    #[test]
    fn test_slide_counter_pads_to_the_width_of_the_deck() {
        assert_eq!(slide_counter(0, 3), "1 / 3");
        assert_eq!(slide_counter(2, 12), "03 / 12");
        assert_eq!(slide_counter(11, 12), "12 / 12");
    }

    #[test]
    fn test_present_view_without_slides_states_the_condition() {
        let html = render_present_view("acme", "my-project", &[]).into_string();
        assert!(html.contains("fig-empty"), "{html}");
        assert!(html.contains(">NO SLIDES<"), "{html}");
        assert!(
            html.contains(
                "No presentation slides configured. Add a [present] section with files to your .fig.toml."
            ),
            "{html}"
        );
        assert!(!html.contains("present-container"), "{html}");
    }

    #[test]
    fn test_config_and_license_views_render_code_and_empty_states() {
        let html = render_config_view(Some("[present]\nfiles = []"), Some(".fig")).into_string();
        assert!(
            html.contains(r#"<pre class="fig-code" tabindex="0" aria-label=".fig contents">"#),
            "code blocks are keyboard scrollable and named: {html}"
        );

        let html = render_config_view(None, None).into_string();
        assert!(html.contains(">NO CONFIGURATION<"), "{html}");
        assert!(
            html.contains("Create a .fig.toml file in the repository root"),
            "the original guidance survives: {html}"
        );

        let html = render_license_view(Some("<p>MIT</p>")).into_string();
        assert!(
            html.contains(r#"<div class="fig-md fig-md--boxed"><p>MIT</p></div>"#),
            "{html}"
        );

        let html = render_license_view(None).into_string();
        assert!(html.contains(">NO LICENSE<"), "{html}");
        assert!(html.contains("No license information available."), "{html}");
    }

    #[test]
    fn test_content_view_reports_missing_paths_and_binary_files() {
        let html = render_content_view("acme", "my-project", "nope.txt", &[], None).into_string();
        assert!(
            html.contains("Path not found."),
            "the rejection wording is preserved verbatim: {html}"
        );
        assert!(html.contains("fig-empty"), "{html}");

        let html = render_content_file("acme", "my-project", "logo.png", &[0, 1, 2]).into_string();
        assert!(html.contains(">BINARY FILE<"), "{html}");
        assert!(
            html.contains("Binary file (3 bytes). Not displayed."),
            "{html}"
        );

        let html =
            render_content_file("acme", "my-project", "main.rs", b"fn main() {}").into_string();
        assert!(
            html.contains(r#"<pre class="fig-code" tabindex="0" aria-label="main.rs contents">"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<span aria-current="page">main.rs</span>"#),
            "the filename remains in the inner breadcrumb: {html}"
        );
        assert!(
            !html.contains(r#"<h2 class="fig-section">main.rs</h2>"#),
            "the file is not repeated as a title: {html}"
        );
        assert!(html.contains("fn main() {}"), "{html}");

        let html =
            render_content_file("acme", "my-project", "docs/a.md", b"[b](c.md)").into_string();
        assert!(
            html.contains(r#"<div class="fig-md fig-md--boxed">"#),
            "markdown files keep the boxed markdown surface: {html}"
        );
        assert!(
            html.contains("/acme/my-project/md/docs/c.md"),
            "link rewriting still resolves against the containing directory: {html}"
        );
    }

    #[test]
    fn test_git_error_is_an_announced_danger_notice() {
        let error = git2::Error::from_str("could not find repository");
        let html = render_git_error(&error).into_string();
        assert!(html.contains("role=\"alert\""), "{html}");
        assert!(html.contains("fig-notice fig-notice--danger"), "{html}");
        assert!(html.contains(">ERROR<"), "{html}");
        assert!(html.contains("could not find repository"), "{html}");
        assert!(
            html.contains(&format!("{:?}", error.code())),
            "the git error code stays visible: {html}"
        );
        assert!(
            html.contains(&format!("{:?}", error.class())),
            "the git error class stays visible: {html}"
        );
    }

    /// Representative markup from every surface this file renders. Each entry is
    /// a full page or tab body, so the sweeps below cover the whole view.
    fn representative_markup() -> Vec<(&'static str, String)> {
        let deck = slides(2);
        let files = vec!["README.md".to_string(), "docs/guide.md".to_string()];
        let entries = vec![entry("src", true), entry("README.md", false)];
        let commits = [Commit::new(
            "a1b2c3d4".to_string(),
            "silen".to_string(),
            chrono::Utc::now(),
            "Initial commit".to_string(),
        )];

        vec![
            ("repo page", render_repo(&base_ctx()).into_string()),
            (
                "markdown tab",
                render_tab_content(&TabContentContext {
                    tab: "markdown",
                    markdown_files: &files,
                    selected_content: Some("# Hi\n\nHello."),
                    ..base_ctx()
                })
                .into_string(),
            ),
            ("commits tab", render_commits_view(&commits).into_string()),
            (
                "config tab",
                render_config_view(Some("[present]"), Some(".fig.toml")).into_string(),
            ),
            (
                "license tab",
                render_license_view(Some("<p>MIT</p>")).into_string(),
            ),
            (
                "content directory",
                render_content_dir("acme", "my-project", "src", &entries).into_string(),
            ),
            (
                "content file",
                render_content_file("acme", "my-project", "main.rs", b"fn main() {}").into_string(),
            ),
            (
                "present tab",
                render_present_view("acme", "my-project", &deck).into_string(),
            ),
            (
                "empty present tab",
                render_present_view("acme", "my-project", &[]).into_string(),
            ),
            (
                "git error",
                render_git_error(&git2::Error::from_str("boom")).into_string(),
            ),
            (
                "repo auth error",
                render_repo_auth_error(
                    &actix_web::test::TestRequest::default().to_http_request(),
                    "acme/secret",
                )
                .into_string(),
            ),
        ]
    }

    #[test]
    fn test_render_repo_auth_error() {
        let req = actix_web::test::TestRequest::default().to_http_request();
        let html = render_repo_auth_error(&req, "acme/secret").into_string();
        assert!(html.contains("Not logged in. Please log in first."));
        assert!(html.contains("href=\"/auth/login\""));
        assert!(html.contains("<title>acme/secret · Fig</title>"));

        let htmx_req = actix_web::test::TestRequest::default()
            .insert_header(("HX-Request", "true"))
            .to_http_request();
        let htmx_html = render_repo_auth_error(&htmx_req, "acme/secret").into_string();
        assert!(htmx_html.contains("Not logged in. Please log in first."));
        assert!(!htmx_html.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn test_repo_markup_uses_only_fig_design_system_classes() {
        for (surface, html) in representative_markup() {
            let classes = classes_in(&html);
            assert!(
                !classes.is_empty(),
                "{surface} should carry classes: {html}"
            );
            for class in classes {
                assert!(
                    class.starts_with("fig-"),
                    "non design-system class {class:?} in {surface}: {html}"
                );
            }
        }
    }

    #[test]
    fn test_repo_markup_carries_no_inline_presentation() {
        for (surface, html) in representative_markup() {
            assert!(
                !html.contains("style=\""),
                "no inline style attributes in {surface}: {html}"
            );
            assert!(
                !html.contains("<style"),
                "presentation CSS lives in fig.css, not in {surface}: {html}"
            );
            for class in classes_in(&html) {
                for outgoing in ["tf-", "markdown-body", "white-", "lh-copy", "no-underline"] {
                    assert!(
                        !class.contains(outgoing),
                        "outgoing class {class:?} still in {surface}: {html}"
                    );
                }
            }
        }
    }
}
