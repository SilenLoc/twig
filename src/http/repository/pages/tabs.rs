use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use maud::Markup;

use super::Params;
use super::context::{
    OptionalTab, RepoContext, TabFrame, open_repo, render_empty, render_for_request,
    render_git_error,
};
use crate::auth::TwigContext;
use crate::config;
use crate::git::bare::{Commit, MAX_COMMITS};

pub(super) fn markdown_files(ctx: &RepoContext) -> Vec<String> {
    ctx.handle
        .list_files(Some(&ctx.twig_result.config))
        .unwrap_or_default()
        .markdown_files
}

/// The Documentation tab: the default Markdown file, README preferred.
pub(super) fn markdown_body(ctx: &RepoContext) -> Markup {
    markdown_body_from(ctx, &markdown_files(ctx))
}

pub(super) fn markdown_body_from(ctx: &RepoContext, files: &[String]) -> Markup {
    let file = get_default_markdown_file(files).unwrap_or("README.md");
    let content = ctx.handle.read_file(file).ok().flatten();
    super::markdown::render_markdown_view(
        &ctx.namespace,
        &ctx.repo,
        file,
        content.as_deref(),
        files,
    )
}

/// The root of the Content tab.
pub(super) fn content_body(ctx: &RepoContext) -> Markup {
    let entries = ctx
        .handle
        .list_dir("", Some(&ctx.twig_result.config))
        .unwrap_or_default();
    super::content::render_content_view(&ctx.namespace, &ctx.repo, "", &entries, None)
}

pub(super) fn commits_body(ctx: &RepoContext) -> Result<Markup, git2::Error> {
    let commits = ctx.handle.get_commits(MAX_COMMITS)?;
    Ok(render_commits_view(&commits))
}

pub(super) fn config_body(ctx: &RepoContext) -> Markup {
    render_config_view(
        ctx.twig_result.raw.as_deref(),
        ctx.twig_result.filename.as_deref(),
    )
}

pub(super) fn license_body(ctx: &RepoContext) -> Markup {
    let content = ctx.handle.get_license_content();
    render_license_view(Some(&content))
}

pub(super) fn tab_body(
    ctx: &RepoContext,
    tab: &str,
    group: Option<&str>,
    releases: &[crate::db::binaries::BinaryRelease],
    binaries_unavailable: bool,
) -> Result<Markup, git2::Error> {
    match tab {
        "content" => Ok(content_body(ctx)),
        "config" => Ok(config_body(ctx)),
        "present" => Ok(super::presentation::present_body(ctx)),
        "paper" => Ok(super::paper::paper_body(ctx)),
        "license" => Ok(license_body(ctx)),
        "scripts" => Ok(super::scripts::scripts_body(
            ctx,
            group,
            releases,
            binaries_unavailable,
        )),
        "commits" => commits_body(ctx),
        // Documentation, and the fallback for an unknown tab name.
        _ => Ok(markdown_body(ctx)),
    }
}

/// Shared shape of every per-tab handler: open, build the one body, respond.
/// `group` is the Scripts sub-group key; other tabs pass `None`.
pub(super) async fn respond_tab(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    namespace: &str,
    repo: &str,
    tab: &str,
    group: Option<&str>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, namespace, repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };
    let (releases, binaries_unavailable) = if tab == "scripts" {
        match auth_state.db().list_binary_releases(namespace, repo).await {
            Ok(releases) => (releases, false),
            Err(error) => {
                log::error!("Failed to load binaries for {namespace}/{repo}: {error}");
                (Vec::new(), true)
            }
        }
    } else {
        (Vec::new(), false)
    };
    let body = match tab_body(&ctx, tab, group, &releases, binaries_unavailable) {
        Ok(body) => body,
        Err(e) => {
            let content = render_git_error(&e);
            return Ok(render_for_request(
                &req,
                content,
                ctx.username.as_deref(),
                &ctx.page_title,
            ));
        }
    };
    Ok(render_tab_response(&req, &ctx.frame(), tab, &body))
}

pub(super) fn tab_nav(frame: &TabFrame<'_>, active: &str) -> Markup {
    render_tabs(frame, active)
}

/// The full tab page body, without the document layout.
pub(super) fn render_tab_shell(frame: &TabFrame<'_>, active: &str, body: &Markup) -> Markup {
    maud::html! {
        (render_repo_crumbs(frame.namespace, frame.repo))
        (tab_nav(frame, active))
        // A broken configuration is shown on every tab, outside #tab-content, so
        // it survives htmx swaps until the file is fixed.
        @if let Some(error) = frame.twig_error {
            (render_config_error(frame.twig_filename.unwrap_or(".twig.toml"), error))
        }
        div id="tab-content" aria-live="polite" {
            (body)
        }
    }
}

/// An htmx call gets the swapped tab bar plus the body; a direct navigation
/// gets the full page. This is what keeps every tab URL refreshable.
pub(super) fn render_tab_response(
    req: &HttpRequest,
    frame: &TabFrame<'_>,
    active: &str,
    body: &Markup,
) -> Markup {
    if req.headers().get("HX-Request").is_some() {
        maud::html! {
            (tab_nav(frame, active))
            (body)
        }
    } else {
        let content = render_tab_shell(frame, active, body);
        crate::http::view::render_layout(&content, frame.username, Some(frame.page_title))
    }
}

/// Every tab a repository can show.
pub(super) const TAB_IDS: [&str; 8] = [
    "markdown", "paper", "content", "commits", "config", "present", "scripts", "license",
];

/// The tab the repository home shows: the first configured tab, else Markdown
/// when the repository has markdown, else Commits. A configured name that is
/// not a real tab falls back the same way.
fn resolved_default_tab<'a>(tabs_config: &'a [String], markdown_files: &[String]) -> &'a str {
    let tab = default_tab(tabs_config, markdown_files);
    if TAB_IDS.contains(&tab) {
        tab
    } else if markdown_files.is_empty() {
        "commits"
    } else {
        "markdown"
    }
}

#[get("/{namespace}/{repo}")]
pub async fn handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, &params.namespace, &params.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };

    // The default tab needs the markdown list either way; reuse it when the
    // default turns out to be Documentation.
    let files = markdown_files(&ctx);
    let tab = resolved_default_tab(&ctx.twig_result.config.tabs, &files);
    let body = if tab == "markdown" {
        markdown_body_from(&ctx, &files)
    } else {
        let (releases, binaries_unavailable) = if tab == "scripts" {
            match auth_state
                .db()
                .list_binary_releases(&params.namespace, &params.repo)
                .await
            {
                Ok(releases) => (releases, false),
                Err(error) => {
                    log::error!(
                        "Failed to load binaries for {}/{}: {error}",
                        params.namespace,
                        params.repo
                    );
                    (Vec::new(), true)
                }
            }
        } else {
            (Vec::new(), false)
        };
        match tab_body(&ctx, tab, None, &releases, binaries_unavailable) {
            Ok(body) => body,
            Err(e) => {
                let content = render_git_error(&e);
                return Ok(render_for_request(
                    &req,
                    content,
                    ctx.username.as_deref(),
                    &ctx.page_title,
                ));
            }
        }
    };
    Ok(render_tab_response(&req, &ctx.frame(), tab, &body))
}

/// The page head: the trail is the whole heading zone, with the repository
/// name as its highlighted final segment, so the tabs follow it directly.
pub(super) fn render_repo_crumbs(namespace: &str, repo: &str) -> Markup {
    maud::html! {
        div class="twig-pagehead" {
            nav class="twig-crumbs twig-crumbs--page" aria-label="Breadcrumb" {
                a href="/" { "Namespaces" }
                span class="twig-crumb-sep" aria-hidden="true" { "/" }
                a href=(format!("/{namespace}")) { (namespace) }
                span class="twig-crumb-sep" aria-hidden="true" { "/" }
                h1 class="twig-crumb-current" aria-current="page" { (repo) }
            }
            a class="twig-btn twig-btn--primary" href=(format!("/{namespace}/{repo}/edit")) {
                "Open workspace"
            }
        }
    }
}

/// Renders a tab navigation bar. `tabs_config` lists the only tabs to show
/// when set; every tab additionally requires its content to exist.
pub(super) const DEFAULT_TABS: [(&str, &str, Option<OptionalTab>); 8] = [
    ("markdown", "Documentation", None),
    ("paper", "Paper", Some(OptionalTab::Paper)),
    ("content", "Content", None),
    ("commits", "Commits", None),
    ("config", "Config", Some(OptionalTab::Config)),
    ("present", "Present", Some(OptionalTab::Present)),
    ("scripts", "Scripts", Some(OptionalTab::Scripts)),
    ("license", "License", None),
];

pub(super) fn available_tabs(frame: &TabFrame<'_>) -> Vec<(&'static str, &'static str)> {
    if frame.tabs_config.is_empty() {
        DEFAULT_TABS
            .into_iter()
            .filter_map(|(id, label, requirement)| {
                requirement
                    .is_none_or(|tab| frame.has_optional_tab(tab))
                    .then_some((id, label))
            })
            .collect()
    } else {
        frame
            .tabs_config
            .iter()
            .filter_map(|tab| configured_tab(tab, frame))
            .collect()
    }
}

pub(super) fn configured_tab(
    tab: &str,
    frame: &TabFrame<'_>,
) -> Option<(&'static str, &'static str)> {
    let (id, label, requirement) = match tab {
        "markdown" => ("markdown", "Documentation", None),
        "paper" => ("paper", "Paper", Some(OptionalTab::Paper)),
        "content" => ("content", "Content", None),
        "commits" => ("commits", "Commits", None),
        "config" => ("config", "Config", Some(OptionalTab::Config)),
        "present" => ("present", "Present", Some(OptionalTab::Present)),
        "scripts" => ("scripts", "Scripts", Some(OptionalTab::Scripts)),
        "license" => ("license", "License", None),
        _ => return None,
    };
    requirement
        .is_none_or(|tab| frame.has_optional_tab(tab))
        .then_some((id, label))
}

pub(super) fn render_tabs(frame: &TabFrame<'_>, active_tab: &str) -> Markup {
    let namespace = frame.namespace;
    let repo = frame.repo;
    let all_tabs = available_tabs(frame);

    maud::html! {
        nav id="tab-nav" class="twig-tabs" aria-label="Repository views" hx-swap-oob="true" {
            @for (tab_id, tab_label) in all_tabs {
                @let href = format!("/{namespace}/{repo}/{tab_id}");
                a
                    class="twig-tab"
                    href=(href)
                    aria-current=[(tab_id == active_tab).then_some("page")]
                    hx-get=(href)
                    hx-target="#tab-content"
                    hx-push-url=(href)
                {
                    (tab_label)
                }
            }
        }
    }
}

/// The tab shown when no tab is named: the first configured one, else Markdown
/// when the repository has markdown, else Commits.
pub(super) fn default_tab<'a>(tabs_config: &'a [String], markdown_files: &[String]) -> &'a str {
    if let Some(first) = tabs_config.first() {
        first.as_str()
    } else if markdown_files.is_empty() {
        "commits"
    } else {
        "markdown"
    }
}

/// Get the default markdown file to show - prefers README.md if it exists
pub(super) fn get_default_markdown_file(markdown_files: &[String]) -> Option<&str> {
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

pub(super) fn render_commits_view(commits: &[Commit]) -> Markup {
    maud::html! {
        div class="twig-stack" {
            @if commits.is_empty() {
                (render_empty("NO COMMITS", "This repository has no commits yet."))
            } @else {
                ol class="twig-commits" {
                    @for commit in commits {
                        (render_commit(commit))
                    }
                }
            }
        }
    }
}

pub(super) fn render_config_view(
    twig_content: Option<&str>,
    twig_filename: Option<&str>,
) -> Markup {
    let config_filename = twig_filename.unwrap_or(".twig.toml");

    maud::html! {
        div class="twig-stack" {
            h2 class="twig-section" { "Configuration" }
            p class="twig-hint" {
                "Repository configuration from " code class="twig-mono" { (config_filename) }
            }
            @if let Some(content) = twig_content {
                pre class="twig-code" tabindex="0" aria-label=(format!("{config_filename} contents")) {
                    code { (content) }
                }
            } @else {
                (render_empty(
                    "NO CONFIGURATION",
                    "No configuration file found. Create a .twig.toml file in the repository root to configure ignore patterns."
                ))
            }
        }
    }
}

/// A parse failure in `.twig.toml`, presented like a compiler diagnostic: the
/// offending file, the TOML error with its line context, and a help hint.
/// Reaching this is the exceptional path, so the extra formatting costs
/// nothing on a healthy repository.
pub(super) fn render_config_error(filename: &str, error: &str) -> Markup {
    let diagnostic = format!(
        "error: could not parse configuration\n --> {filename}\n\n{error}\n\nhelp: fix the syntax shown above; default settings apply until it is valid"
    );

    maud::html! {
        div class="twig-notice twig-notice--danger" role="alert" {
            p class="twig-eyebrow" { "CONFIG ERROR" }
            p class="twig-notice-body" {
                code class="twig-mono" { (filename) }
                " could not be parsed. Continuing with default settings."
            }
            pre class="twig-code" tabindex="0" aria-label=(format!("{filename} parse error")) {
                code { (diagnostic) }
            }
        }
    }
}

pub(super) fn render_license_view(license_content: Option<&str>) -> Markup {
    maud::html! {
        div class="twig-stack" {
            @if let Some(content) = license_content {
                div class="twig-md twig-md--boxed" {
                    (content)
                }
            } @else {
                (render_empty("NO LICENSE", "No license information available."))
            }
        }
    }
}

/// Returns (href, push-url) pair for a path inside the content tab
pub(super) fn render_commit(commit: &Commit) -> Markup {
    let hash = commit.hash();
    let author = commit.author();
    let date = commit.date();
    let commit_message = commit.message();
    maud::html! {
        li class="twig-commit" {
            div class="twig-commit-meta" {
                code class="twig-commit-hash" {
                    (hash.chars().take(7).collect::<String>())
                }
                span class="twig-commit-author" { (author) }
                time class="twig-commit-date" datetime=(date.to_rfc3339()) {
                    (date.format("%Y-%m-%d %H:%M"))
                }
            }
            p class="twig-commit-msg" { (commit_message) }
        }
    }
}
