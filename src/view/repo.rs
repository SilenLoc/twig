use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use maud::Markup;
use pulldown_cmark::{Event, Options, Parser, html};
use serde::Deserialize;

use crate::{
    auth::FigContext,
    config,
    git::bare::{Commit, Depth, PresentConfig, RepoHandle},
    md,
};

use super::session_auth::get_username_from_request;

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
    let username = get_username_from_request(&req, &auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(h) => h,
        Err(e) => {
            let content = render_git_error(&e);
            return if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(super::render_layout(&content, username.as_deref()))
            };
        }
    };

    let fig_result = handle.load_config_with_raw();
    let fig_config = &fig_result.config;
    let fig_content = fig_result.raw.as_deref();
    let fig_filename = fig_result.filename.as_deref();

    let commits_result = handle.get_commits(Depth::default());
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

            let present_slides = load_present_slides(&handle, &fig_config.present);
            let license_content = handle.get_license_content();
            let has_license = handle.has_license();

            render_repo(
                namespace,
                repo,
                &commits,
                &markdown_files,
                default_file,
                default_content.as_deref(),
                fig_content,
                fig_filename,
                &fig_config.tabs,
                &fig_config.present,
                &present_slides,
                Some(&license_content),
                has_license,
            )
        }
        Err(e) => render_git_error(&e),
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(super::render_layout(&content, username.as_deref()))
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
    let username = get_username_from_request(&req, &auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(h) => h,
        Err(e) => {
            let content = render_git_error(&e);
            return if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(super::render_layout(&content, username.as_deref()))
            };
        }
    };

    let fig_result = handle.load_config_with_raw();
    let fig_config = &fig_result.config;
    let fig_content = fig_result.raw.as_deref();
    let fig_filename = fig_result.filename.as_deref();

    let commits_result = handle.get_commits(Depth::default());
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

            let present_slides = load_present_slides(&handle, &fig_config.present);
            let license_content = handle.get_license_content();
            let has_license = handle.has_license();

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
            };
            if req.headers().get("HX-Request").is_some() {
                let has_config = fig_content.is_some();
                let has_present = !fig_config.present.files.is_empty();
                let inner = render_tab_content_inner(ctx);
                let tabs = render_tabs(
                    namespace,
                    repo,
                    tab,
                    has_config,
                    &fig_config.tabs,
                    has_present,
                );
                Ok(maud::html! {
                    (tabs)
                    (inner)
                })
            } else {
                let content = render_tab_content(ctx);
                Ok(super::render_layout(&content, username.as_deref()))
            }
        }
        Err(e) => {
            let content = render_git_error(&e);
            if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(super::render_layout(&content, username.as_deref()))
            }
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
    let username = get_username_from_request(&req, &auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(h) => h,
        Err(e) => {
            let content = render_git_error(&e);
            return if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(super::render_layout(&content, username.as_deref()))
            };
        }
    };

    let fig_result = handle.load_config_with_raw();
    let fig_config = &fig_result.config;
    let fig_content = fig_result.raw.as_deref();
    let fig_filename = fig_result.filename.as_deref();

    let commits_result = handle.get_commits(Depth::default());
    let files_result = handle.list_files(Some(fig_config));
    let file_result = handle.read_file(file_path);

    match commits_result {
        Ok(commits) => {
            let files = files_result.unwrap_or_default();
            let markdown_files = files.markdown_files;
            let license_content = handle.get_license_content();
            let has_license = handle.has_license();

            if req.headers().get("HX-Request").is_some() {
                let content = render_markdown_content_only(
                    namespace,
                    repo,
                    file_path,
                    file_result.ok().flatten().as_deref(),
                );
                Ok(content)
            } else {
                let selected_content = file_result.ok().flatten();
                let present_slides = load_present_slides(&handle, &fig_config.present);
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
                };
                let content = render_tab_content(ctx);
                Ok(super::render_layout(&content, username.as_deref()))
            }
        }
        Err(e) => {
            let content = render_git_error(&e);
            if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(super::render_layout(&content, username.as_deref()))
            }
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
    let username = get_username_from_request(&req, &auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(h) => h,
        Err(e) => {
            let content = render_git_error(&e);
            return if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(super::render_layout(&content, username.as_deref()))
            };
        }
    };

    let fig_result = handle.load_config_with_raw();
    let present_slides = load_present_slides(&handle, &fig_result.config.present);

    if index >= present_slides.len() {
        let content = maud::html! {
            div class="pa3 white-50" { "Slide not found" }
        };
        if req.headers().get("HX-Request").is_some() {
            Ok(content)
        } else {
            Ok(super::render_layout(&content, username.as_deref()))
        }
    } else {
        let content = render_slide_content(namespace, repo, index, &present_slides);
        if req.headers().get("HX-Request").is_some() {
            Ok(content)
        } else {
            Ok(super::render_layout(&content, username.as_deref()))
        }
    }
}

/// Converts markdown to HTML, fixing relative links to point to repo root
fn markdown_to_html(markdown: &str, namespace: &str, repo: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(markdown, options);

    // Process events to fix relative links
    let parser = parser.map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(tag) => {
            // Fix relative links in markdown
            let fixed_tag = match tag {
                pulldown_cmark::Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                } => {
                    let dest_str = dest_url.to_string();
                    // If it's a relative link (doesn't start with http/https or /)
                    let fixed_dest = if !dest_str.starts_with("http://")
                        && !dest_str.starts_with("https://")
                        && !dest_str.starts_with('/')
                        && !dest_str.starts_with('#')
                    {
                        // Check if it's a markdown file
                        let lower = dest_str.to_ascii_lowercase();
                        if lower.ends_with(".md") || lower.ends_with(".markdown") {
                            // Link to the markdown viewer
                            format!("/{namespace}/{repo}/md/{dest_str}").into()
                        } else {
                            // Link to the raw file via repo root
                            format!("/{namespace}/{repo}/{dest_str}").into()
                        }
                    } else {
                        dest_url
                    };
                    pulldown_cmark::Tag::Link {
                        link_type,
                        dest_url: fixed_dest,
                        title,
                        id,
                    }
                }
                _ => tag,
            };
            Event::Start(fixed_tag)
        }
        _ => event,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

fn render_git_error(e: &git2::Error) -> Markup {
    let code = e.code();
    let code = format!("{code:?}");
    let klass = e.class();
    let klass = format!("{klass:?}");
    let message = e.message();
    maud::html! {
        p { (message) }
        p { (code) }
        p { (klass) }
    }
}

/// A scrollable container for tab content
fn scrollable_container(content: &Markup) -> Markup {
    maud::html! {
        div class="overflow-y-auto flex-auto" {
            (content)
        }
    }
}

/// Renders a tab navigation bar
/// If tabs_config is not empty, only those tabs are shown
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
    if !tabs_config.is_empty() {
        // Use configured tabs
        for tab in tabs_config {
            match tab.as_str() {
                "markdown" => all_tabs.push(("markdown", "Markdown")),
                "commits" => all_tabs.push(("commits", "Commits")),
                "config" if has_config => all_tabs.push(("config", "Config")),
                "present" if has_present => all_tabs.push(("present", "Present")),
                "license" => all_tabs.push(("license", "License")),
                _ => {}
            }
        }
    } else {
        // Show all available tabs
        all_tabs.push(("markdown", "Markdown"));

        all_tabs.push(("commits", "Commits"));
        if has_config {
            all_tabs.push(("config", "Config"));
        }
        if has_present {
            all_tabs.push(("present", "Present"));
        }
        all_tabs.push(("license", "License"));
    }

    maud::html! {
        div id="tab-nav" hx-swap-oob="true" class="flex flex-wrap bb b--white-20 mb3" {
            @for (tab_id, tab_label) in all_tabs {
                @let is_active = tab_id == active_tab;
                @let active_classes = if is_active { "white fw6 bg-white-10" } else { "white-70 hover-white" };
                a
                    href=(format!("/{}/{}/tab/{}", namespace, repo, tab_id))
                    class=(format!("pa2 ph3 {} no-underline pointer hover-bg-white-10", active_classes))
                    hx-get=(format!("/{}/{}/tab/{}", namespace, repo, tab_id))
                    hx-target="#tab-content"
                    hx-push-url=(format!("/{}/{}", namespace, repo))
                {
                    (tab_label)
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_repo(
    namespace: &str,
    repo: &str,
    commits: &[Commit],
    markdown_files: &[String],
    default_file: Option<&str>,
    default_content: Option<&str>,
    fig_content: Option<&str>,
    fig_filename: Option<&str>,
    tabs_config: &[String],
    present_config: &PresentConfig,
    present_slides: &[PresentSlide],
    license_content: Option<&str>,
    has_license: bool,
) -> Markup {
    // Determine default tab based on configuration and available files
    let default_tab = if !tabs_config.is_empty() {
        // Use first configured tab
        tabs_config[0].as_str()
    } else if !markdown_files.is_empty() {
        "markdown"
    } else {
        "commits"
    };
    let has_config = fig_content.is_some();
    let has_present = !present_config.files.is_empty();

    let ctx = TabContentContext {
        namespace,
        repo,
        tab: default_tab,
        commits,
        markdown_files,
        selected_md_file: default_file,
        selected_content: default_content,
        fig_content,
        fig_filename,
        tabs_config,
        present_config,
        present_slides,
        license_content,
        has_license,
    };

    maud::html! {
        div class="mb3 mb4-ns f6 white-70" {
            a href="/" class="link white-70 hover-white no-underline" { "Namespaces" }
            span class="mh2" { "/" }
            a href=(format!("/{}", namespace)) class="link white-70 hover-white no-underline" { (namespace) }
            span class="mh2" { "/" }
            span class="white" { (repo) }
        }

        // Tab navigation
        (render_tabs(namespace, repo, default_tab, has_config, tabs_config, has_present))

        // Tab content container (scrollable)
        div id="tab-content" {
            (scrollable_container(&render_tab_content_inner(ctx)))
        }
    }
}

fn render_tab_content(ctx: TabContentContext<'_>) -> Markup {
    let has_config = ctx.fig_content.is_some();
    let has_present = !ctx.present_config.files.is_empty();
    maud::html! {
        // Tab navigation (update active state)
        (render_tabs(ctx.namespace, ctx.repo, ctx.tab, has_config, ctx.tabs_config, has_present))

        // Tab content container (scrollable)
        div id="tab-content" {
            (scrollable_container(&render_tab_content_inner(ctx)))
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

fn render_tab_content_inner(ctx: TabContentContext<'_>) -> Markup {
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
        "commits" => {
            maud::html! {
                div {
                    h2 class="f4 fw6 mb3 white" { "Commits" }
                    ol class="list pl0" {
                        @for commit in ctx.commits {
                            li class="mb3" {
                                (render_commit(commit))
                            }
                        }
                    }
                }
            }
        }
        "config" => render_config_view(ctx.fig_content, ctx.fig_filename),
        "present" => render_present_view(
            ctx.namespace,
            ctx.repo,
            ctx.present_config,
            ctx.present_slides,
        ),
        "license" => render_license_view(ctx.license_content),
        _ => {
            // Unknown tab - show Markdown by default if available, then Commits
            let new_tab = if !ctx.markdown_files.is_empty() {
                "markdown"
            } else {
                "commits"
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
            };
            render_tab_content_inner(new_ctx)
        }
    }
}

fn render_config_view(fig_content: Option<&str>, fig_filename: Option<&str>) -> Markup {
    let config_filename = fig_filename.unwrap_or(".fig.toml");

    maud::html! {
        div {
            h2 class="f4 fw6 mb3 white" { "Configuration" }
            p class="f6 white-70 mb3" {
                "Repository configuration from " (config_filename)
            }
            @if let Some(content) = fig_content {
                pre class="pa3 bg-black-20 br2 overflow-x-auto" {
                    code class="f6 white lh-copy" { (content) }
                }
            } @else {
                div class="pa3 white-50 bg-black-20 br2" {
                    "No configuration file found. Create a .fig.toml file in the repository root to configure ignore patterns."
                }
            }
        }
    }
}

fn render_license_view(license_content: Option<&str>) -> Markup {
    maud::html! {
        div {
            h2 class="f4 fw6 mb3 white" { "License" }
            @if let Some(content) = license_content {
                div class="markdown-body white lh-copy pa3 bg-black-20 br2 overflow-x-auto" {
                    (maud::PreEscaped(content))
                }
            } @else {
                div class="pa3 white-50 bg-black-20 br2" {
                    "No license information available."
                }
            }
        }
    }
}

fn load_present_slides(handle: &RepoHandle, present_config: &PresentConfig) -> Vec<PresentSlide> {
    present_config
        .files
        .iter()
        .filter_map(|file| {
            let content = handle.read_file(file).ok()??;
            let html = md::process_markdown(&content, &present_config.template_vars);
            Some(PresentSlide { html })
        })
        .collect()
}

fn render_slide_content(
    namespace: &str,
    repo: &str,
    current_index: usize,
    slides: &[PresentSlide],
) -> Markup {
    let slide_count = slides.len();
    let slide = &slides[current_index];
    let has_prev = current_index > 0;
    let has_next = current_index < slide_count - 1;

    maud::html! {
        div class="flex items-center justify-between mb3" {
            div class="f4 fw6 white" { "Presentation" }
            div class="flex items-center" {
                span class="f6 white-70 mr3" { (current_index + 1) " / " (slide_count) }
                button
                    class="f6 link white-70 hover-white bg-transparent bn pointer pa1 mr2"
                    onclick="document.getElementById('present-container').requestFullscreen()"
                {
                    "Fullscreen"
                }
            }
        }

        div id="slides-wrapper" style="display: flex; flex-direction: column; flex-grow: 1; min-height: 0;" {
            div
                class="present-slide"
                style="display: flex; align-items: center; justify-content: center; flex-grow: 1; padding: 2rem;"
            {
                div class="markdown-body white lh-copy" style="max-width: 800px; width: 100%;" {
                    (maud::PreEscaped(&slide.html))
                }
            }
        }

        div class="flex items-center justify-between mt3" {
            @if has_prev {
                button
                    id="prev-slide"
                    class="f6 link white-70 hover-white bg-transparent bn pointer pa2 ph3"
                    hx-get=(format!("/{}/{}/slide/{}", namespace, repo, current_index - 1))
                    hx-target="#present-container"
                    hx-swap="innerHTML"
                {
                    "\u{2190} Previous"
                }
            } @else {
                span id="prev-slide" class="f6 white-30 pa2 ph3" { "\u{2190} Previous" }
            }
            div class="flex" {
                @for i in 0..slide_count {
                    @let is_active = i == current_index;
                    @let dot_classes = if is_active { "present-dot bg-white" } else { "present-dot bg-white-30" };
                    button
                        class=(dot_classes)
                        hx-get=(format!("/{}/{}/slide/{}", namespace, repo, i))
                        hx-target="#present-container"
                        hx-swap="innerHTML"
                        style="width: 10px; height: 10px; border-radius: 50%; margin: 0 4px; border: none; cursor: pointer;"
                    {}
                }
            }
            @if has_next {
                button
                    id="next-slide"
                    class="f6 link white-70 hover-white bg-transparent bn pointer pa2 ph3"
                    hx-get=(format!("/{}/{}/slide/{}", namespace, repo, current_index + 1))
                    hx-target="#present-container"
                    hx-swap="innerHTML"
                {
                    "Next \u{2192}"
                }
            } @else {
                span id="next-slide" class="f6 white-30 pa2 ph3" { "Next \u{2192}" }
            }
        }

        script {
            (maud::PreEscaped("document.onkeydown=function(e){if(e.key==='ArrowLeft'){e.preventDefault();var p=document.getElementById('prev-slide');if(p&&p.tagName==='BUTTON')p.click();}else if(e.key==='ArrowRight'){e.preventDefault();var n=document.getElementById('next-slide');if(n&&n.tagName==='BUTTON')n.click();}};"))
        }
    }
}

fn render_present_view(
    namespace: &str,
    repo: &str,
    _present_config: &PresentConfig,
    slides: &[PresentSlide],
) -> Markup {
    if slides.is_empty() {
        return maud::html! {
            div class="pa3 white-50" {
                "No presentation slides configured. Add a [present] section with files to your .fig.toml."
            }
        };
    }

    maud::html! {
        div id="present-container" style="display: flex; flex-direction: column; min-height: 50vh;" {
            (render_slide_content(namespace, repo, 0, slides))
        }
        style {
            "#present-container:fullscreen { background: black; min-height: 100vh; }"
            "#present-container:fullscreen #slides-wrapper { flex-grow: 1; }"
            "#present-container:fullscreen .present-slide { flex-grow: 1; }"
            "#present-container:fullscreen .markdown-body { font-size: 1.5rem; max-width: 1200px; }"
        }
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
        div class="flex flex-column flex-row-ns" style="height: 100%;" {
            // Left sidebar with markdown files
            div class="w-100 w4-ns w5-l br-ns b--white-20 pr3-ns mb3 mb0-ns overflow-x-auto overflow-y-auto-ns" style="max-height: 70vh; min-width: 0;" {
                @if markdown_files.len() > 1 {
                    h3 class="f5 fw6 mb2 white" { "Markdown Files" }
                }
                ul class="list pl0 flex flex-row flex-column-ns overflow-x-auto overflow-y-auto-ns mb0" {
                    @for file in markdown_files {
                        @let is_active = file == current_file;
                        li class="mb1 mr2 mr0-ns flex-shrink-0 flex-shrink-0-ns" {
                            @if is_active {
                                a
                                    href=(format!("/{}/{}/md/{}", namespace, repo, file))
                                    class="white fw6 no-underline db pa1 nowrap"
                                    hx-get=(format!("/{}/{}/md/{}", namespace, repo, file))
                                    hx-target="#markdown-view"
                                {
                                    (file)
                                }
                            }
                            @if !is_active {
                                a
                                    href=(format!("/{}/{}/md/{}", namespace, repo, file))
                                    class="white-70 hover-white no-underline db pa1 nowrap"
                                    hx-get=(format!("/{}/{}/md/{}", namespace, repo, file))
                                    hx-target="#markdown-view"
                                {
                                    (file)
                                }
                            }
                        }
                    }
                }
            }

            // Right content area
            div id="markdown-view" class="flex-auto pl0 pl3-ns overflow-y-auto" {
                (render_markdown_content_only(namespace, repo, current_file, content))
            }
        }
    }
}

/// Renders just the markdown content without the sidebar (for HTMX updates)
fn render_markdown_content_only(
    namespace: &str,
    repo: &str,
    _current_file: &str,
    content: Option<&str>,
) -> Markup {
    let html_content = content.map(|md| markdown_to_html(md, namespace, repo));

    maud::html! {
        @if let Some(ref html) = html_content {
            div class="markdown-body white lh-copy" {
                (maud::PreEscaped(html))
            }
        }
        @if html_content.is_none() {
            div class="pa3 white-50" {
                "File not found or empty."
            }
        }
    }
}

fn render_commit(commit: &Commit) -> Markup {
    let hash = commit.hash();
    let author = commit.author();
    let date = commit.date();
    let commit_message = commit.commit_message();
    maud::html! {
        div class="commit ba b--white-20 br2 pa3 bg-black-20" {
            div class="flex flex-wrap items-center mb2" {
                code class="f7 mr2 ph2 pv1 bg-white-10 br1 white" {
                    (hash.chars().take(7).collect::<String>())
                }
                span class="f6 white-70 mr2" { (author) }
                span class="f6 white-50" { (date.format("%Y-%m-%d %H:%M")) }
            }
            p class="f5 white ma0" { (commit_message) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_html() {
        let md = "# Hello\n\nThis is **bold** text.";
        let html = markdown_to_html(md, "test", "repo");
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_markdown_link_fixing() {
        let md = "[Link](./other.md) and [External](https://example.com)";
        let html = markdown_to_html(md, "ns", "repo");
        // Internal markdown links should be fixed
        assert!(html.contains("/ns/repo/md/./other.md"));
        // External links should remain unchanged
        assert!(html.contains("https://example.com"));
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
        let html = markdown_to_html(md, "test", "repo");
        // Tables should be rendered as HTML table elements
        assert!(
            html.contains("<table>"),
            "Expected <table> tag in output: {}",
            html
        );
        assert!(
            html.contains("<th>"),
            "Expected <th> tag in output: {}",
            html
        );
        assert!(
            html.contains("<td>"),
            "Expected <td> tag in output: {}",
            html
        );
    }
}
