use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use maud::Markup;
use pulldown_cmark::{Event, Options, Parser, html};
use serde::Deserialize;

use crate::{
    auth::AuthState,
    config,
    git::{
        self,
        bare::{Commit, Depth, FigConfig},
    },
};

/// Helper function to get the username from the session cookie if logged in
async fn get_username_from_request(
    req: &HttpRequest,
    auth_state: &web::Data<AuthState>,
) -> Option<String> {
    let token = req.cookie("session")?;
    let user_id = auth_state.validate_token(token.value()).await?;
    let user = auth_state.db.get_user_by_id(&user_id).await.ok()??;
    Some(user.username)
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
}

#[get("/{namespace}/{repo}")]
pub async fn handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<AuthState>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let username = get_username_from_request(&req, &auth_state).await;

    // Load .fig.toml config
    let fig_config = FigConfig::load(server.project_root(), namespace, repo);

    // Get fig config file content for display
    let fig_content = FigConfig::read_raw(server.project_root(), namespace, repo);

    // Get commits
    let commits_result =
        git::bare::get_commits(server.project_root(), namespace, repo, Depth::default());

    // Get all markdown files (filtered by config)
    let markdown_files_result =
        git::bare::list_markdown_files(server.project_root(), namespace, repo, Some(&fig_config));

    let content = match commits_result {
        Ok(commits) => {
            let markdown_files = markdown_files_result.unwrap_or_default();
            // Get default markdown file content for initial view
            let default_file = get_default_markdown_file(&markdown_files);
            let default_content = if let Some(file) = default_file {
                git::bare::read_file(server.project_root(), namespace, repo, file)
                    .ok()
                    .flatten()
            } else {
                None
            };
            render_repo(
                namespace,
                repo,
                &commits,
                &markdown_files,
                default_file,
                default_content.as_deref(),
                fig_content.as_deref(),
            )
        }
        Err(e) => render_git_error(e),
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
    auth_state: web::Data<AuthState>,
    params: web::Path<TabParams>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let tab = &params.tab;
    let username = get_username_from_request(&req, &auth_state).await;

    // Load .fig.toml config
    let fig_config = FigConfig::load(server.project_root(), namespace, repo);

    // Get fig config file content for display
    let fig_content = FigConfig::read_raw(server.project_root(), namespace, repo);

    // Get commits
    let commits_result =
        git::bare::get_commits(server.project_root(), namespace, repo, Depth::default());

    // Get all markdown files (filtered by config)
    let markdown_files_result =
        git::bare::list_markdown_files(server.project_root(), namespace, repo, Some(&fig_config));

    match commits_result {
        Ok(commits) => {
            let markdown_files = markdown_files_result.unwrap_or_default();

            // Get default markdown file content
            let default_file = get_default_markdown_file(&markdown_files);
            let default_content = if let Some(file) = default_file {
                git::bare::read_file(server.project_root(), namespace, repo, file)
                    .ok()
                    .flatten()
            } else {
                None
            };

            let ctx = TabContentContext {
                namespace,
                repo,
                tab,
                commits: &commits,
                markdown_files: &markdown_files,
                selected_md_file: default_file,
                selected_content: default_content.as_deref(),
                fig_content: fig_content.as_deref(),
            };
            if req.headers().get("HX-Request").is_some() {
                // HTMX request - return only the inner tab content for swapping
                let content = render_tab_content_inner(ctx);
                Ok(content)
            } else {
                // For full page loads, return tabs + content
                let content = render_tab_content(ctx);
                Ok(super::render_layout(&content, username.as_deref()))
            }
        }
        Err(e) => {
            let content = render_git_error(e);
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
    auth_state: web::Data<AuthState>,
    params: web::Path<MarkdownParams>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let file_path = &params.file_path;
    let username = get_username_from_request(&req, &auth_state).await;

    // Load .fig.toml config
    let fig_config = FigConfig::load(server.project_root(), namespace, repo);

    // Get fig config file content for display
    let fig_content = FigConfig::read_raw(server.project_root(), namespace, repo);

    // Get commits
    let commits_result =
        git::bare::get_commits(server.project_root(), namespace, repo, Depth::default());

    // Get all markdown files (filtered by config)
    let markdown_files_result =
        git::bare::list_markdown_files(server.project_root(), namespace, repo, Some(&fig_config));

    // Get the requested markdown file content
    let file_result = git::bare::read_file(server.project_root(), namespace, repo, file_path);

    match commits_result {
        Ok(commits) => {
            let markdown_files = markdown_files_result.unwrap_or_default();

            if req.headers().get("HX-Request").is_some() {
                // For HTMX requests, return just the markdown content (not the full view with sidebar)
                let content = render_markdown_content_only(
                    namespace,
                    repo,
                    file_path,
                    file_result.ok().flatten().as_deref(),
                );
                Ok(content)
            } else {
                // For full page loads, show the markdown tab with the selected file
                let selected_content = file_result.ok().flatten();
                let ctx = TabContentContext {
                    namespace,
                    repo,
                    tab: "markdown",
                    commits: &commits,
                    markdown_files: &markdown_files,
                    selected_md_file: Some(file_path),
                    selected_content: selected_content.as_deref(),
                    fig_content: fig_content.as_deref(),
                };
                let content = render_tab_content(ctx);
                Ok(super::render_layout(&content, username.as_deref()))
            }
        }
        Err(e) => {
            let content = render_git_error(e);
            if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(super::render_layout(&content, username.as_deref()))
            }
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
                        if dest_str.ends_with(".md") || dest_str.ends_with(".markdown") {
                            // Link to the markdown viewer
                            format!("/{}/{}/md/{}", namespace, repo, dest_str).into()
                        } else {
                            // Link to the raw file via repo root
                            format!("/{}/{}/{}", namespace, repo, dest_str).into()
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

fn render_git_error(e: git2::Error) -> Markup {
    let code = e.code();
    let code = format!("{:?}", code);
    let klass = e.class();
    let klass = format!("{:?}", klass);
    let message = e.message();
    maud::html! {
        p { (message) }
        p { (code) }
        p { (klass) }
    }
}

/// A scrollable container for tab content
fn scrollable_container(content: Markup) -> Markup {
    maud::html! {
        div class="overflow-y-auto flex-auto" style="max-height: calc(100vh - 14rem);" {
            (content)
        }
    }
}

/// Renders a tab navigation bar
fn render_tabs(namespace: &str, repo: &str, active_tab: &str, has_config: bool) -> Markup {
    let mut tabs = vec![("markdown", "Markdown"), ("commits", "Commits")];
    if has_config {
        tabs.push(("config", "Config"));
    }

    maud::html! {
        div class="flex bb b--white-20 mb3" {
            @for (tab_id, tab_label) in tabs {
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

fn render_repo(
    namespace: &str,
    repo: &str,
    commits: &[Commit],
    markdown_files: &[String],
    default_file: Option<&str>,
    default_content: Option<&str>,
    fig_content: Option<&str>,
) -> Markup {
    // Default to "markdown" tab if markdown files exist, otherwise "commits"
    let default_tab = if !markdown_files.is_empty() {
        "markdown"
    } else {
        "commits"
    };
    let has_config = fig_content.is_some();

    let ctx = TabContentContext {
        namespace,
        repo,
        tab: default_tab,
        commits,
        markdown_files,
        selected_md_file: default_file,
        selected_content: default_content,
        fig_content,
    };

    maud::html! {
        // Breadcrumb navigation
        div class="mb4 f6 white-70" {
            a href="/" class="link white-70 hover-white no-underline" { "Namespaces" }
            span class="mh2" { "/" }
            a href=(format!("/{}", namespace)) class="link white-70 hover-white no-underline" { (namespace) }
            span class="mh2" { "/" }
            span class="white" { (repo) }
        }

        // Tab navigation
        (render_tabs(namespace, repo, default_tab, has_config))

        // Tab content container (scrollable)
        div id="tab-content" {
            (scrollable_container(render_tab_content_inner(ctx)))
        }
    }
}

fn render_tab_content(ctx: TabContentContext<'_>) -> Markup {
    let has_config = ctx.fig_content.is_some();
    maud::html! {
        // Tab navigation (update active state)
        (render_tabs(ctx.namespace, ctx.repo, ctx.tab, has_config))

        // Tab content container (scrollable)
        div id="tab-content" {
            (scrollable_container(render_tab_content_inner(ctx)))
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
        return readme.map(|s| s.as_str());
    }
    // Then try any README variant
    let readme = markdown_files
        .iter()
        .find(|f| f.to_lowercase().starts_with("readme"));
    if readme.is_some() {
        return readme.map(|s| s.as_str());
    }
    // Fall back to first file
    markdown_files.first().map(|s| s.as_str())
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
        "config" => render_config_view(ctx.namespace, ctx.repo, ctx.fig_content),
        _ => {
            // Unknown tab - show Markdown by default if available
            let new_ctx = TabContentContext {
                namespace: ctx.namespace,
                repo: ctx.repo,
                tab: if !ctx.markdown_files.is_empty() {
                    "markdown"
                } else {
                    "commits"
                },
                commits: ctx.commits,
                markdown_files: ctx.markdown_files,
                selected_md_file: ctx.selected_md_file,
                selected_content: ctx.selected_content,
                fig_content: ctx.fig_content,
            };
            render_tab_content_inner(new_ctx)
        }
    }
}

fn render_config_view(_namespace: &str, _repo: &str, fig_content: Option<&str>) -> Markup {
    let config_filename = FigConfig::config_filename(_namespace, _repo, _repo)
        .unwrap_or_else(|| ".fig.toml".to_string());

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

fn render_markdown_view(
    namespace: &str,
    repo: &str,
    current_file: &str,
    content: Option<&str>,
    markdown_files: &[String],
) -> Markup {
    maud::html! {
        div class="flex" style="height: 100%;" {
            // Left sidebar with markdown files
            div class="w4 w5-ns br b--white-20 pr3 overflow-y-auto" style="max-height: calc(100vh - 14rem); min-width: 200px;" {
                h3 class="f5 fw6 mb2 white" { "Markdown Files" }
                ul class="list pl0" {
                    @for file in markdown_files {
                        @let is_active = file == current_file;
                        li class="mb1" {
                            @if is_active {
                                a
                                    href=(format!("/{}/{}/md/{}", namespace, repo, file))
                                    class="white fw6 no-underline db pa1"
                                    hx-get=(format!("/{}/{}/md/{}", namespace, repo, file))
                                    hx-target="#markdown-view"
                                {
                                    (file)
                                }
                            }
                            @if !is_active {
                                a
                                    href=(format!("/{}/{}/md/{}", namespace, repo, file))
                                    class="white-70 hover-white no-underline db pa1"
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
            div id="markdown-view" class="flex-auto pl3 overflow-y-auto" style="max-height: calc(100vh - 14rem);" {
                (render_markdown_content_only(namespace, repo, current_file, content))
            }
        }
    }
}

/// Renders just the markdown content without the sidebar (for HTMX updates)
fn render_markdown_content_only(
    _namespace: &str,
    _repo: &str,
    _current_file: &str,
    content: Option<&str>,
) -> Markup {
    let html_content = content.map(|md| markdown_to_html(md, _namespace, _repo));

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
            div class="flex items-center mb2" {
                code class="f7 mr2 ph2 pv1 bg-white-10 br1 white" {
                    (hash.chars().take(7).collect::<String>())
                }
                span class="f6 white-70" { (author) }
                span class="f6 white-50 ml2" { (date.format("%Y-%m-%d %H:%M")) }
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
