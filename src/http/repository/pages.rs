use std::collections::{HashMap, HashSet};

use actix_web::Result as AwResult;
use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse, get, web};
use maud::{DOCTYPE, Markup};
use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};
use serde::Deserialize;

use crate::{auth::TwigContext, config, md};
use twig_git::bare::{
    Commit, MAX_COMMITS, PresentConfig, RepoHandle, ScriptEntry, ScriptGroupNode, TreeEntry,
    TwigConfig, TwigConfigWithRaw, is_safe_repo_path,
};

use crate::http::auth::session::get_username_from_request;

#[derive(Deserialize)]
pub(super) struct Params {
    pub(super) namespace: String,
    pub(super) repo: String,
}

mod content;
mod markdown;
mod paper;
mod presentation;
mod scripts;
mod tabs;
pub use content::content_handler;
#[cfg(test)]
use content::*;
pub use markdown::markdown_handler;
#[cfg(test)]
use markdown::*;
pub use paper::paper_handler;
#[cfg(test)]
use paper::*;
#[cfg(test)]
use presentation::*;
pub use presentation::{present_print_handler, slide_handler};
#[cfg(test)]
use scripts::*;
pub use scripts::{raw_handler, scripts_group_handler};
#[cfg(test)]
use tabs::*;
pub use tabs::{
    commits_tab_handler, config_tab_handler, content_tab_handler, handler, license_tab_handler,
    markdown_tab_handler, paper_tab_handler, present_tab_handler, scripts_tab_handler, tab_handler,
};

/// Reading text sizes, in percent, from the default through to double size.
/// The A−/A+ buttons step through them one entry at a time; the active size
/// lives in `data-twig-text-size` on the owning container, which the stylesheet
/// turns into a larger type scale for the framed content only. Shared by the
/// presentation deck and the paper reader.
const TEXT_SIZES: [u16; 11] = [100, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200];
const PRESENT_TEXT_SIZE_KEY: &str = "twig-present-text-size";
const PAPER_TEXT_SIZE_KEY: &str = "twig-paper-text-size";

/// Mirrors the active size back to the browser so it survives a reload.
fn text_size_persist(key: &str) -> String {
    format!("try {{ localStorage.setItem('{key}', String(data.twigTextSize)); }} catch (_) {{}}")
}

fn text_size_list() -> String {
    TEXT_SIZES
        .iter()
        .map(|size| format!("'{size}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Restores a previously chosen size on load; the expression runs against the
/// owning container, where `data.twigTextSize` is the shared state.
fn text_size_restore(key: &str) -> String {
    format!(
        "try {{ let size = localStorage.getItem('{key}'); if ([{}].includes(size)) data.twigTextSize = Number(size); }} catch (_) {{}}",
        text_size_list()
    )
}

fn text_size_step(increase: bool, key: &str) -> String {
    let first = TEXT_SIZES[0];
    let last = TEXT_SIZES[TEXT_SIZES.len() - 1];
    let step = TEXT_SIZES[1] - TEXT_SIZES[0];
    let (bound, operator, clamp) = if increase {
        (last, '+', "Math.min")
    } else {
        (first, '-', "Math.max")
    };
    format!(
        "data.twigTextSize = {clamp}({bound}, data.twigTextSize {operator} {step}); {}",
        text_size_persist(key)
    )
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
        crate::http::view::render_layout(&content, username, Some(page_title))
    }
}

fn render_repo_auth_error(req: &HttpRequest, page_title: &str) -> Markup {
    let content = crate::http::view::render_error_with_action(
        "Not logged in. Please log in first.",
        "/auth/login",
        "Log in",
    );
    render_for_request(req, content, None, page_title)
}

/// The opened repository plus everything a tab handler needs before loading its
/// own data: the parsed configuration, the signed-in username, and the page
/// title. Each handler borrows this to build only its own tab's body.
struct RepoContext {
    handle: RepoHandle,
    twig_result: TwigConfigWithRaw,
    namespace: String,
    repo: String,
    username: Option<String>,
    page_title: String,
    paper_pages: Vec<String>,
    /// Absolute origin (`https://git.example.com`) the `curl` commands in the
    /// Scripts tab are built from.
    base_url: String,
    /// The configured script groups, flattened and pruned. Empty when the
    /// repository has no `[scripts]` section, which hides the tab.
    script_groups: Vec<ScriptGroupNode>,
}

/// Optional views that can be exposed in the repository tab bar.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OptionalTab {
    Config,
    Present,
    Paper,
    Scripts,
}

/// The frame every tab shares: the breadcrumb, the tab bar, and the
/// configuration-error banner. None of it depends on the active tab.
struct TabFrame<'a> {
    namespace: &'a str,
    repo: &'a str,
    username: Option<&'a str>,
    page_title: &'a str,
    twig_error: Option<&'a str>,
    twig_filename: Option<&'a str>,
    tabs_config: &'a [String],
    available_tabs: Vec<OptionalTab>,
}

impl RepoContext {
    fn frame(&self) -> TabFrame<'_> {
        TabFrame {
            namespace: &self.namespace,
            repo: &self.repo,
            username: self.username.as_deref(),
            page_title: &self.page_title,
            twig_error: self.twig_result.error.as_deref(),
            twig_filename: self.twig_result.filename.as_deref(),
            tabs_config: &self.twig_result.config.tabs,
            available_tabs: [
                self.twig_result
                    .raw
                    .is_some()
                    .then_some(OptionalTab::Config),
                (!self.twig_result.config.present.files.is_empty()).then_some(OptionalTab::Present),
                (!self.paper_pages.is_empty()).then_some(OptionalTab::Paper),
                (!self.script_groups.is_empty()).then_some(OptionalTab::Scripts),
            ]
            .into_iter()
            .flatten()
            .collect(),
        }
    }
}

/// Opens the repository, loads its configuration, and gates private reads on a
/// session. On failure it returns the already-rendered response, so every tab
/// handler shares one prologue.
async fn open_repo(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
    repo: &str,
) -> Result<RepoContext, Markup> {
    let page_title = format!("{namespace}/{repo}");
    let username = get_username_from_request(req, auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(handle) => handle,
        Err(e) => {
            let content = render_git_error(&e);
            return Err(render_for_request(
                req,
                content,
                username.as_deref(),
                &page_title,
            ));
        }
    };

    let twig_result = handle.load_config_with_raw();
    if twig_result.config.private && username.is_none() {
        return Err(render_repo_auth_error(req, &page_title));
    }

    let paper_pages = paper::load_paper_pages(&handle, &twig_result.config);
    let script_groups = twig_result.config.script_groups();

    Ok(RepoContext {
        handle,
        twig_result,
        namespace: namespace.to_string(),
        repo: repo.to_string(),
        username,
        page_title,
        paper_pages,
        base_url: scripts::script_base_url(req),
        script_groups,
    })
}

fn tab_nav(frame: &TabFrame<'_>, active: &str) -> Markup {
    render_tabs(frame, active)
}

/// The full tab page body, without the document layout.
fn render_tab_shell(frame: &TabFrame<'_>, active: &str, body: &Markup) -> Markup {
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
fn render_tab_response(
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

/// An Empty State (DESIGN.md 5.13). The eyebrow names the condition in words so
/// the meaning never rests on colour alone.
fn render_empty(eyebrow: &str, body: &str) -> Markup {
    maud::html! {
        div class="twig-empty" {
            p class="twig-eyebrow" { (eyebrow) }
            p class="twig-empty-body" { (body) }
        }
    }
}

fn render_git_error(e: &twig_git::GitError) -> Markup {
    let code = e.code();
    let code = format!("{code:?}");
    let klass = e.class();
    let klass = format!("{klass:?}");
    let message = e.message();
    maud::html! {
        div class="twig-notice twig-notice--danger" role="alert" {
            p class="twig-eyebrow" { "ERROR" }
            p class="twig-notice-body" { (message) }
            p class="twig-notice-body twig-mono twig-ink-tertiary" { (code) " / " (klass) }
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
        crate::http::view::render_layout(&content, username, Some(page_title))
    }
}

/// The page head: the trail is the whole heading zone, with the repository
/// name as its highlighted final segment, so the tabs follow it directly.
fn render_repo_crumbs(namespace: &str, repo: &str) -> Markup {
    maud::html! {
        div class="twig-pagehead" {
            nav class="twig-crumbs twig-crumbs--page" aria-label="Breadcrumb" {
                a href="/" { "Namespaces" }
                span class="twig-crumb-sep" aria-hidden="true" { "/" }
                a href=(format!("/{namespace}")) { (namespace) }
                span class="twig-crumb-sep" aria-hidden="true" { "/" }
                h1 class="twig-crumb-current" aria-current="page" { (repo) }
            }
        }
    }
}

/// Renders a tab navigation bar. `tabs_config` lists the only tabs to show
/// when set; every tab additionally requires its content to exist.
fn render_tabs(frame: &TabFrame<'_>, active_tab: &str) -> Markup {
    let namespace = frame.namespace;
    let repo = frame.repo;
    let tabs_config = frame.tabs_config;

    // Build the list of available tabs
    let mut all_tabs: Vec<(&str, &str)> = vec![];

    // Only add markdown tab if there are markdown files
    if tabs_config.is_empty() {
        // Show all available tabs
        all_tabs.push(("markdown", "Documentation"));
        if frame.available_tabs.contains(&OptionalTab::Paper) {
            all_tabs.push(("paper", "Paper"));
        }
        all_tabs.push(("content", "Content"));

        all_tabs.push(("commits", "Commits"));
        if frame.available_tabs.contains(&OptionalTab::Config) {
            all_tabs.push(("config", "Config"));
        }
        if frame.available_tabs.contains(&OptionalTab::Present) {
            all_tabs.push(("present", "Present"));
        }
        if frame.available_tabs.contains(&OptionalTab::Scripts) {
            all_tabs.push(("scripts", "Scripts"));
        }
        all_tabs.push(("license", "License"));
    } else {
        // Use configured tabs
        for tab in tabs_config {
            match tab.as_str() {
                "markdown" => all_tabs.push(("markdown", "Documentation")),
                "paper" if frame.available_tabs.contains(&OptionalTab::Paper) => {
                    all_tabs.push(("paper", "Paper"));
                }
                "content" => all_tabs.push(("content", "Content")),
                "commits" => all_tabs.push(("commits", "Commits")),
                "config" if frame.available_tabs.contains(&OptionalTab::Config) => {
                    all_tabs.push(("config", "Config"));
                }
                "present" if frame.available_tabs.contains(&OptionalTab::Present) => {
                    all_tabs.push(("present", "Present"));
                }
                "scripts" if frame.available_tabs.contains(&OptionalTab::Scripts) => {
                    all_tabs.push(("scripts", "Scripts"));
                }
                "license" => all_tabs.push(("license", "License")),
                _ => {}
            }
        }
    }

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

/// A parse failure in `.twig.toml`, presented like a compiler diagnostic: the
/// offending file, the TOML error with its line context, and a help hint.
/// Reaching this is the exceptional path, so the extra formatting costs
/// nothing on a healthy repository.
fn render_config_error(filename: &str, error: &str) -> Markup {
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

#[cfg(test)]
#[path = "pages/tests/mod.rs"]
mod tests;
