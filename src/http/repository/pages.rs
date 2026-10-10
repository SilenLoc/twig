use std::collections::{HashMap, HashSet};

use actix_web::Result as AwResult;
use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse, get, web};
use maud::{DOCTYPE, Markup};
use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};
use serde::Deserialize;

use crate::{
    auth::TwigContext,
    config,
    git::bare::{
        Commit, MAX_COMMITS, PresentConfig, RepoHandle, ScriptEntry, ScriptGroupNode, TreeEntry,
        TwigConfig, TwigConfigWithRaw, is_safe_repo_path,
    },
    md,
};

use crate::http::auth::session::get_username_from_request;

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

/// Paper font choices. The active choice lives on `#paper-container` as
/// `data-twig-paper-font`; the stylesheet swaps the reading family from it.
const PAPER_FONTS: [(&str, &str); 3] = [("sans", "Sans"), ("serif", "Serif"), ("mono", "Mono")];

/// Paper toolbar and reading-position wiring, scoped to `#paper-container`:
/// restores the stored font, keeps the toggle buttons' pressed state in sync,
/// and mirrors the page crossing the viewport's midline into the URL fragment
/// so the current page is always linkable. An `IntersectionObserver` is used
/// instead of a scroll listener, matching the presentation contract of binding
/// nothing at the document level.
const PAPER_SCRIPT: &str = r"(function(){
var c=document.getElementById('paper-container');
if(!c||c.dataset.twigPaper)return;
c.dataset.twigPaper='1';
var fonts=['sans','serif','mono'];
var apply=function(font){
if(fonts.indexOf(font)<0)font='sans';
c.dataset.twigPaperFont=font;
c.querySelectorAll('[data-twig-font]').forEach(function(b){
b.setAttribute('aria-pressed',String(b.dataset.twigFont===font));
});
};
try{apply(localStorage.getItem('twig-paper-font')||'sans');}catch(_){apply('sans');}
c.addEventListener('click',function(e){
var b=e.target.closest?e.target.closest('[data-twig-font]'):null;
if(!b||!c.contains(b))return;
apply(b.dataset.twigFont);
try{localStorage.setItem('twig-paper-font',b.dataset.twigFont);}catch(_){}
});
var pages=[].slice.call(c.querySelectorAll('.twig-paper-page'));
var track=function(page){
if(!page||!page.id)return;
var hash=location.hash.slice(1);
if(hash&&hash!==page.id){
var el=document.getElementById(hash);
if(el&&el!==page&&el.closest('.twig-paper-page')===page)return;
}
if(location.hash!=='#'+page.id){
try{history.replaceState(null,'','#'+page.id);}catch(_){}
}
};
if(pages.length&&'IntersectionObserver' in window){
var observer=new IntersectionObserver(function(entries){
entries.forEach(function(e){if(e.isIntersecting)track(e.target);});
},{rootMargin:'-50% 0px -50% 0px',threshold:0});
pages.forEach(function(p){observer.observe(p);});
}
})();";

/// The export document's auto-print. Only once every webfont and image has
/// loaded does the binder open the print dialog, and the hint banner flips to
/// its "ready" state so the reader knows what to pick. The `load` event joins
/// the wait so stylesheet-driven fonts cannot race the first pagination pass.
const PDF_PRINT_SCRIPT: &str = r"(function(){
var hint=document.getElementById('twig-pdf-hint');
var ready=function(){
if(hint){hint.setAttribute('data-twig-pdf-ready','true');}
window.print();
};
var fonts='fonts' in document ? document.fonts.ready : Promise.resolve();
var images=Promise.all([].slice.call(document.images).map(function(img){
return img.complete ? null : new Promise(function(resolve){ img.onload=img.onerror=resolve; });
}));
var page=document.readyState==='complete' ? null : new Promise(function(resolve){
window.addEventListener('load',resolve,{once:true});
});
Promise.all([fonts,images,page]).then(ready).catch(ready);
})();";

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

#[derive(Deserialize)]
struct ContentQuery {
    committed: Option<String>,
    conflict_copy: Option<bool>,
}

#[derive(Deserialize)]
struct PaperParams {
    namespace: String,
    repo: String,
    page: String,
}

#[derive(Deserialize)]
struct ScriptGroupParams {
    namespace: String,
    repo: String,
    /// Slash-joined group key, e.g. `linux/maintenance`.
    group: String,
}

#[derive(Deserialize)]
struct RawParams {
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

/// A rendered slide for the presentation view
struct PresentSlide {
    html: String,
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

/// The frame every tab shares: the breadcrumb, the tab bar, and the
/// configuration-error banner. None of it depends on the active tab.
// One flag per optional view; each is a plain yes/no, so four bools are the
// honest shape here.
#[allow(clippy::struct_excessive_bools)]
struct TabFrame<'a> {
    namespace: &'a str,
    repo: &'a str,
    username: Option<&'a str>,
    page_title: &'a str,
    twig_error: Option<&'a str>,
    twig_filename: Option<&'a str>,
    tabs_config: &'a [String],
    has_config: bool,
    has_present: bool,
    has_paper: bool,
    has_scripts: bool,
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
            has_config: self.twig_result.raw.is_some(),
            has_present: !self.twig_result.config.present.files.is_empty(),
            has_paper: !self.paper_pages.is_empty(),
            has_scripts: !self.script_groups.is_empty(),
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

    let paper_pages = load_paper_pages(&handle, &twig_result.config);
    let script_groups = twig_result.config.script_groups();

    Ok(RepoContext {
        handle,
        twig_result,
        namespace: namespace.to_string(),
        repo: repo.to_string(),
        username,
        page_title,
        paper_pages,
        base_url: script_base_url(req),
        script_groups,
    })
}

fn markdown_files(ctx: &RepoContext) -> Vec<String> {
    ctx.handle
        .list_files(Some(&ctx.twig_result.config))
        .unwrap_or_default()
        .markdown_files
}

/// The Documentation tab: the default Markdown file, README preferred.
fn markdown_body(ctx: &RepoContext) -> Markup {
    markdown_body_from(ctx, &markdown_files(ctx))
}

fn markdown_body_from(ctx: &RepoContext, files: &[String]) -> Markup {
    let file = get_default_markdown_file(files).unwrap_or("README.md");
    let content = ctx.handle.read_file(file).ok().flatten();
    render_markdown_view(&ctx.namespace, &ctx.repo, file, content.as_deref(), files)
}

/// The root of the Content tab.
fn content_body(ctx: &RepoContext) -> Markup {
    let entries = ctx
        .handle
        .list_dir("", Some(&ctx.twig_result.config))
        .unwrap_or_default();
    render_content_view(&ctx.namespace, &ctx.repo, "", &entries, None)
}

fn commits_body(ctx: &RepoContext) -> Result<Markup, git2::Error> {
    let commits = ctx.handle.get_commits(MAX_COMMITS)?;
    Ok(render_commits_view(&commits))
}

fn config_body(ctx: &RepoContext) -> Markup {
    render_config_view(
        ctx.twig_result.raw.as_deref(),
        ctx.twig_result.filename.as_deref(),
    )
}

fn license_body(ctx: &RepoContext) -> Markup {
    let content = ctx.handle.get_license_content();
    render_license_view(Some(&content))
}

fn present_body(ctx: &RepoContext) -> Markup {
    let slides = load_present_slides(
        &ctx.handle,
        &ctx.twig_result.config.present,
        &ctx.namespace,
        &ctx.repo,
    );
    render_present_view(&ctx.namespace, &ctx.repo, &slides)
}

fn paper_body(ctx: &RepoContext) -> Markup {
    let dir = ctx
        .twig_result
        .config
        .paper
        .as_ref()
        .map_or("", |paper| paper.dir.as_str());
    render_paper_view(&ctx.namespace, &ctx.repo, dir, &ctx.paper_pages)
}

/// The Scripts tab: the active group's scripts with their copyable `curl`
/// commands. `group` selects a group by its slash-joined key; `None` and
/// unknown keys fall back to the first configured group.
fn scripts_body(
    ctx: &RepoContext,
    group: Option<&str>,
    releases: &[crate::db::binaries::BinaryRelease],
    binaries_unavailable: bool,
) -> Markup {
    let active = ctx
        .script_groups
        .iter()
        .find(|node| Some(node.key.as_str()) == group)
        .or_else(|| ctx.script_groups.first());
    match active {
        Some(node) => render_scripts_view(ctx, node, releases, binaries_unavailable),
        None => render_empty(
            "NO SCRIPTS",
            "No scripts are configured. Add a [scripts.<group>] section with a scripts list to .twig.toml.",
        ),
    }
}

/// Renders a tab by name. Only the requested tab's data is loaded, so a Commits
/// request never reads markdown, slides, or paper pages. `group` selects a
/// Scripts sub-group and is ignored by every other tab.
fn tab_body(
    ctx: &RepoContext,
    tab: &str,
    group: Option<&str>,
    releases: &[crate::db::binaries::BinaryRelease],
    binaries_unavailable: bool,
) -> Result<Markup, git2::Error> {
    match tab {
        "content" => Ok(content_body(ctx)),
        "config" => Ok(config_body(ctx)),
        "present" => Ok(present_body(ctx)),
        "paper" => Ok(paper_body(ctx)),
        "license" => Ok(license_body(ctx)),
        "scripts" => Ok(scripts_body(ctx, group, releases, binaries_unavailable)),
        "commits" => commits_body(ctx),
        // Documentation, and the fallback for an unknown tab name.
        _ => Ok(markdown_body(ctx)),
    }
}

/// Shared shape of every per-tab handler: open, build the one body, respond.
/// `group` is the Scripts sub-group key; other tabs pass `None`.
async fn respond_tab(
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

/// Every tab a repository can show.
const TAB_IDS: [&str; 8] = [
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

#[get("/{namespace}/{repo}/markdown")]
pub async fn markdown_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "markdown",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/content")]
pub async fn content_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "content",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/commits")]
pub async fn commits_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "commits",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/config")]
pub async fn config_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "config",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/present")]
pub async fn present_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "present",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/paper")]
pub async fn paper_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "paper",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/license")]
pub async fn license_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "license",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/scripts")]
pub async fn scripts_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "scripts",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/scripts/{group:.*}")]
pub async fn scripts_group_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<ScriptGroupParams>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "scripts",
        Some(params.group.trim_matches('/')),
    )
    .await
}

/// Serves a repository file as raw bytes so the Scripts tab's `curl` command
/// can fetch it. Paths ignored by `.twig.toml` stay unreachable here, and
/// private repositories need a session like every other read.
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
    if twig_result.config.private && username.is_none() {
        return HttpResponse::Unauthorized().finish();
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
#[get("/{namespace}/{repo}/tab/{tab}")]
pub async fn tab_handler(params: web::Path<TabParams>) -> HttpResponse {
    let target = if TAB_IDS.contains(&params.tab.as_str()) {
        format!("/{}/{}/{}", params.namespace, params.repo, params.tab)
    } else {
        format!("/{}/{}", params.namespace, params.repo)
    };
    HttpResponse::PermanentRedirect()
        .insert_header((header::LOCATION, target))
        .finish()
}

#[get("/{namespace}/{repo}/md/{file_path:.*}")]
pub async fn markdown_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<MarkdownParams>,
) -> AwResult<Markup> {
    let file_path = &params.file_path;
    let ctx = match open_repo(&req, &server, &auth_state, &params.namespace, &params.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };

    if !is_safe_repo_path(file_path) {
        return Ok(render_not_found_for_request(
            &req,
            ctx.username.as_deref(),
            &ctx.page_title,
        ));
    }

    let blob = ctx.handle.read_file(file_path).ok().flatten();

    // The rail swaps only #markdown-view; a direct visit renders the tab page.
    if req.headers().get("HX-Request").is_some() {
        return Ok(render_markdown_content_only(
            &ctx.namespace,
            &ctx.repo,
            file_path,
            blob.as_deref(),
        ));
    }

    let files = markdown_files(&ctx);
    let body = render_markdown_view(
        &ctx.namespace,
        &ctx.repo,
        file_path,
        blob.as_deref(),
        &files,
    );
    Ok(render_tab_response(&req, &ctx.frame(), "markdown", &body))
}

/// Whether `page` sits inside `dir` (both repository-relative).
fn page_in_dir(page: &str, dir: &str) -> bool {
    !dir.is_empty()
        && page
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The Markdown and parsed headings for one page, or `None` when `page` falls
/// outside the configured paper directory. The headline ids are computed from
/// the page's own anchor.
fn open_paper_page(ctx: &RepoContext, page: &str) -> Option<(Option<String>, Vec<PaperHeading>)> {
    let paper = ctx.twig_result.config.paper.as_ref()?;
    if !paper.is_configured()
        || !is_safe_repo_path(page)
        || !page_in_dir(page, paper.dir.trim_matches('/'))
    {
        return None;
    }

    let dir = paper.dir.as_str();
    let anchors = paper_page_anchors(dir, &ctx.paper_pages);
    let anchor = ctx
        .paper_pages
        .iter()
        .position(|candidate| candidate.as_str() == page)
        .map_or_else(
            || paper_page_anchor(dir, page),
            |index| anchors[index].clone(),
        );
    let markdown = ctx.handle.read_file(page).ok().flatten();
    let headings = markdown
        .as_deref()
        .map_or_else(Vec::new, |md| paper_headings(&anchor, md));

    Some((markdown, headings))
}

/// Renders a single paper page. The Paper tab lazy-loads each page through this
/// endpoint as it scrolls into view.
#[get("/{namespace}/{repo}/paper/{page:.*}")]
pub async fn paper_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<PaperParams>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, &params.namespace, &params.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };

    let content = match open_paper_page(&ctx, &params.page) {
        Some((markdown, headings)) => render_paper_content_only(
            &ctx.namespace,
            &ctx.repo,
            &params.page,
            markdown.as_deref(),
            &headings,
        ),
        None => render_empty("NOT FOUND", "Paper page not found."),
    };

    Ok(render_for_request(
        &req,
        content,
        ctx.username.as_deref(),
        &ctx.page_title,
    ))
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

#[get("/{namespace}/{repo}/slide/{index}")]
pub async fn slide_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
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
                Ok(crate::http::view::render_layout(
                    &content,
                    username.as_deref(),
                    Some(&page_title),
                ))
            };
        }
    };

    let twig_result = handle.load_config_with_raw();
    if twig_result.config.private && username.is_none() {
        return Ok(render_repo_auth_error(&req, &page_title));
    }

    let present_slides = load_present_slides(&handle, &twig_result.config.present, namespace, repo);

    if index >= present_slides.len() {
        let content = render_empty("NOT FOUND", "Slide not found");
        if req.headers().get("HX-Request").is_some() {
            Ok(content)
        } else {
            Ok(crate::http::view::render_layout(
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
            Ok(crate::http::view::render_layout(
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

/// Rewrites a markdown link destination into a Twig URL. Returns `None` when the
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

/// Rewrites a link tag's destination into a Twig URL, returning every other tag
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

/// The parser options shared by every Markdown rendering path.
fn markdown_options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options
}

/// Renders markdown to HTML. `base_dir` is the repository-relative directory of
/// the file being rendered and anchors every relative link it contains.
fn markdown_to_html(markdown: &str, namespace: &str, repo: &str, base_dir: &str) -> String {
    let parser = Parser::new_ext(markdown, markdown_options());

    // Process events to fix relative links
    let parser = parser.map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(tag) => Event::Start(fix_link_tag(tag, namespace, repo, base_dir)),
        other => other,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, md::highlight_rust_code_blocks(parser));
    html_output
}

/// Renders a paper page's Markdown with a unique id on every heading. The ids
/// come from `headings`, computed by [`paper_headings`] from the same source.
fn render_paper_markdown(
    markdown: &str,
    namespace: &str,
    repo: &str,
    base_dir: &str,
    headings: &[PaperHeading],
) -> String {
    let parser = Parser::new_ext(markdown, markdown_options());
    let mut index = 0usize;
    let parser = parser.map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(Tag::Heading {
            level,
            classes,
            attrs,
            ..
        }) => {
            let id = headings
                .get(index)
                .map(|heading| CowStr::from(heading.anchor.clone()));
            index += 1;
            Event::Start(Tag::Heading {
                level,
                id,
                classes,
                attrs,
            })
        }
        Event::Start(tag) => Event::Start(fix_link_tag(tag, namespace, repo, base_dir)),
        other => other,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, md::highlight_rust_code_blocks(parser));
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
    let base_dir = parent_path(file);

    let parser = Parser::new_ext(&replaced, markdown_options()).map(|event| match event {
        Event::Start(tag) => Event::Start(fix_link_tag(tag, namespace, repo, base_dir)),
        other => other,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, md::highlight_rust_code_blocks(parser));
    html_output
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

fn render_git_error(e: &git2::Error) -> Markup {
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
        if frame.has_paper {
            all_tabs.push(("paper", "Paper"));
        }
        all_tabs.push(("content", "Content"));

        all_tabs.push(("commits", "Commits"));
        if frame.has_config {
            all_tabs.push(("config", "Config"));
        }
        if frame.has_present {
            all_tabs.push(("present", "Present"));
        }
        if frame.has_scripts {
            all_tabs.push(("scripts", "Scripts"));
        }
        all_tabs.push(("license", "License"));
    } else {
        // Use configured tabs
        for tab in tabs_config {
            match tab.as_str() {
                "markdown" => all_tabs.push(("markdown", "Documentation")),
                "paper" if frame.has_paper => all_tabs.push(("paper", "Paper")),
                "content" => all_tabs.push(("content", "Content")),
                "commits" => all_tabs.push(("commits", "Commits")),
                "config" if frame.has_config => all_tabs.push(("config", "Config")),
                "present" if frame.has_present => all_tabs.push(("present", "Present")),
                "scripts" if frame.has_scripts => all_tabs.push(("scripts", "Scripts")),
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

fn render_commits_view(commits: &[Commit]) -> Markup {
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

fn render_config_view(twig_content: Option<&str>, twig_filename: Option<&str>) -> Markup {
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

fn render_license_view(license_content: Option<&str>) -> Markup {
    maud::html! {
        div class="twig-stack" {
            @if let Some(content) = license_content {
                div class="twig-md twig-md--boxed" {
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
    let href = if path.is_empty() {
        format!("/{namespace}/{repo}/content")
    } else {
        format!("/{namespace}/{repo}/content/{path}")
    };
    (href.clone(), href)
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

fn render_content_dir(namespace: &str, repo: &str, path: &str, entries: &[TreeEntry]) -> Markup {
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

fn render_content_file(namespace: &str, repo: &str, path: &str, bytes: &[u8]) -> Markup {
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
                                super::editor::encode_file_path(path)
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

/// The Scripts tab: each configured group becomes a sub-tab labelled with the
/// group name, and the active group lists its scripts with a copyable
/// download-and-run command and its nested groups as links.
fn render_scripts_view(
    ctx: &RepoContext,
    node: &ScriptGroupNode,
    releases: &[crate::db::binaries::BinaryRelease],
    binaries_unavailable: bool,
) -> Markup {
    let is_private = ctx.twig_result.config.private;
    maud::html! {
        div class="twig-stack" id="scripts-container" {
            (render_script_group_tabs(ctx, node))
            div class="twig-stack twig-stack--tight" {
                header class="twig-cluster" {
                    p class="twig-eyebrow" { "SCRIPT GROUP" }
                    h2 class="twig-section" { (node.label) }
                }
                p class="twig-hint" {
                    "Fetch and run a script in one line: copy its command, or download it from "
                    code class="twig-mono" { (format!("/{}/{}/raw/…", ctx.namespace, ctx.repo)) }
                    "."
                }
                @if is_private {
                    p class="twig-hint" {
                        "This repository is private: add credentials to the command, e.g. "
                        code class="twig-mono" { "curl -u user:password -fsSL …" }
                        "."
                    }
                }

                @if node.scripts.is_empty() && node.children.is_empty() {
                    (render_empty("NO SCRIPTS", "This group has no scripts."))
                } @else {
                    div class="twig-panel twig-panel--flush" {
                        div class="twig-list" {
                            @for entry in &node.scripts {
                                (render_script_row(ctx, entry))
                            }
                            @for child in &node.children {
                                (render_script_group_row(ctx, child))
                            }
                        }
                    }
                }
                h2 class="twig-section" { "Available binaries" }
                @if binaries_unavailable {
                    (render_empty("BINARY REGISTRY ERROR", "Uploaded binaries could not be loaded."))
                } @else if releases.is_empty() {
                    (render_empty("NO BINARIES", "No binaries have been uploaded yet."))
                } @else {
                    p class="twig-hint" {
                        "Download a build directly, or fetch it from a script using "
                        code class="twig-mono" { (format!("/{}/{}/binaries/latest/…", ctx.namespace, ctx.repo)) }
                        "."
                    }
                    @for release in releases {
                        section class="twig-panel twig-stack twig-stack--tight" {
                            h3 class="twig-section" { "v" (release.version) }
                            div class="twig-list" {
                                @for asset in &release.assets {
                                    @let href = format!(
                                        "/{}/{}/binaries/{}/{}",
                                        ctx.namespace, ctx.repo, release.version, asset.filename
                                    );
                                    div class="twig-row" {
                                        span class="twig-row-id" { (asset.filename) }
                                        span class="twig-row-meta" { (asset.size_bytes) " bytes" }
                                        a class="twig-btn twig-btn--quiet" href=(href) { "Download" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            script { (maud::PreEscaped(SCRIPTS_SCRIPT)) }
        }
    }
}

/// One sub-tab per configured group, labelled with the group name. Groups nest
/// in `.twig.toml`; their flattened keys keep the bar flat and readable.
fn render_script_group_tabs(ctx: &RepoContext, active: &ScriptGroupNode) -> Markup {
    maud::html! {
        nav id="script-group-nav" class="twig-tabs twig-tabs--nested" aria-label="Script groups" {
            @for group in &ctx.script_groups {
                @let href = format!("/{}/{}/scripts/{}", ctx.namespace, ctx.repo, group.key);
                a
                    class="twig-tab"
                    href=(href)
                    aria-current=[(group.key == active.key).then_some("page")]
                    hx-get=(href)
                    hx-target="#tab-content"
                    hx-push-url=(href)
                {
                    (group.label)
                }
            }
        }
    }
}

/// The one-liner a reader copies: fetch the file over the raw route and pipe
/// it straight into its interpreter.
fn script_command(ctx: &RepoContext, entry: &ScriptEntry) -> String {
    format!(
        "curl -fsSL '{}/{}/{}/raw/{}' | {}",
        ctx.base_url, ctx.namespace, ctx.repo, entry.path, entry.shell
    )
}

/// One script row: its name and path, the run command, and a copy button.
/// A configured path missing from HEAD is marked instead of silently skipped.
fn render_script_row(ctx: &RepoContext, entry: &ScriptEntry) -> Markup {
    let command = script_command(ctx, entry);
    let view_href = format!("/{}/{}/content/{}", ctx.namespace, ctx.repo, entry.path);
    let missing = !ctx.handle.path_exists(&entry.path);

    maud::html! {
        div class="twig-row twig-row--form twig-script" {
            div class="twig-script-id" {
                span class="twig-row-id" { (entry.label()) }
                span class="twig-row-meta" {
                    (entry.path)
                    @if missing {
                        span class="twig-script-missing" title="Not present in HEAD" { "MISSING" }
                    }
                }
            }
            div class="twig-script-command" {
                code class="twig-mono" data-twig-command { (command) }
                div class="twig-cluster twig-cluster--tight" {
                    a class="twig-btn twig-btn--quiet" href=(view_href) { "View" }
                    button class="twig-btn twig-btn--ghost" type="button" data-twig-copy { "Copy" }
                }
            }
        }
    }
}

/// A nested group rendered as a row, pointing at its own sub-tab.
fn render_script_group_row(ctx: &RepoContext, key: &str) -> Markup {
    let label = ctx
        .script_groups
        .iter()
        .find(|node| node.key == key)
        .map_or(String::from(key), |node| node.label.clone());
    let href = format!("/{}/{}/scripts/{key}", ctx.namespace, ctx.repo);

    maud::html! {
        a
            class="twig-row"
            href=(href)
            hx-get=(href)
            hx-target="#tab-content"
            hx-push-url=(href)
        {
            span class="twig-row-id" { (label) "/" }
            span class="twig-row-meta" { "Group" }
        }
    }
}

/// Copy-to-clipboard wiring for the Scripts tab, scoped to
/// `#scripts-container`: the Copy button next to a command copies it, and
/// falls back to selecting the command so a manual copy still works.
const SCRIPTS_SCRIPT: &str = r"(function(){
var c=document.getElementById('scripts-container');
if(!c||c.dataset.twigScripts)return;
c.dataset.twigScripts='1';
c.addEventListener('click',function(e){
var b=e.target.closest?e.target.closest('[data-twig-copy]'):null;
if(!b||!c.contains(b))return;
var row=b.closest('.twig-script');
var code=row?row.querySelector('[data-twig-command]'):null;
if(!code)return;
var text=code.textContent;
var done=function(ok){
var label=ok?'Copied':'Select manually';
b.textContent=label;
setTimeout(function(){b.textContent='Copy';},1500);
if(!ok){
try{
var range=document.createRange();range.selectNodeContents(code);
var sel=window.getSelection();sel.removeAllRanges();sel.addRange(range);
}catch(_){}
}
};
if(navigator.clipboard&&navigator.clipboard.writeText){
navigator.clipboard.writeText(text).then(function(){done(true);},function(){done(false);});
}else{done(false);}
});
})();";

/// The absolute origin a `curl` command should use: the request's own host,
/// with the scheme a reverse proxy reported via `X-Forwarded-Proto` when
/// present. Local hosts default to `http`, everything else to `https`, so a
/// development server still hands out runnable commands.
fn script_base_url(req: &HttpRequest) -> String {
    let host = req.connection_info().host().to_string();
    let forwarded = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| *value == "http" || *value == "https");
    let scheme = forwarded.unwrap_or_else(|| {
        let hostname = host.split(':').next().unwrap_or(&host);
        if hostname == "localhost" || hostname.starts_with("127.") || hostname == "[::1]" {
            "http"
        } else {
            "https"
        }
    });
    format!("{scheme}://{host}")
}

/// Content type for a raw file: text for the formats worth reading in a
/// terminal, a neutral download type for everything else.
fn raw_content_type(path: &str) -> &'static str {
    let extension = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match extension.as_str() {
        "sh" | "bash" | "zsh" | "py" | "rb" | "pl" | "js" | "ts" | "toml" | "yaml" | "yml"
        | "json" | "md" | "txt" | "conf" | "cfg" | "ini" | "env" | "sql" | "rs" | "go" | "c"
        | "h" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
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

/// The configured paper pages, sorted for reading. Empty when `[paper]` is
/// absent, unconfigured, or its directory is missing from HEAD.
fn load_paper_pages(handle: &RepoHandle, config: &TwigConfig) -> Vec<String> {
    let Some(paper) = config.paper.as_ref().filter(|paper| paper.is_configured()) else {
        return Vec::new();
    };
    handle.list_paper_pages(&paper.dir).unwrap_or_default()
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

/// A Previous/Next slide control. The button swaps `#present-container` via
/// htmx on click, and the matching arrow key drives the same request while
/// focus stays inside the container — a keyboard shortcut without a global
/// handler, since `keyup` only reaches the container from its own subtree.
fn render_slide_nav_button(id: &str, label: &str, target: Option<&str>, key: &str) -> Markup {
    maud::html! {
        @if let Some(href) = target {
            button
                id=(id)
                class="twig-btn twig-btn--quiet"
                type="button"
                hx-get=(href)
                hx-target="#present-container"
                hx-swap="innerHTML"
                hx-trigger=(format!("click, keyup[key=='{key}'] from:#present-container"))
            {
                (label)
            }
        } @else {
            button id=(id) class="twig-btn twig-btn--quiet" type="button" disabled {
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
        header class="twig-present-bar" {
            p class="twig-eyebrow" { "PRESENTATION" }
            div class="twig-present-tools" {
                span class="twig-present-count" aria-live="polite" {
                    (slide_counter(current_index, slide_count))
                }
                button id="decrease-text-size" class="twig-btn twig-btn--quiet" type="button"
                    aria-label="Decrease presentation font size" title="Decrease font size"
                    "hx-on:click"=(text_size_step(false, PRESENT_TEXT_SIZE_KEY))
                    "hx-live:disabled"=(format!("data.twigTextSize === {}", TEXT_SIZES[0]))
                {
                    "A−"
                }
                span id="present-text-size" class="twig-present-count" aria-live="polite"
                    "hx-live:text"="data.twigTextSize + '%'" { "100%" }
                button id="increase-text-size" class="twig-btn twig-btn--quiet" type="button"
                    aria-label="Increase presentation font size" title="Increase font size"
                    "hx-on:click"=(text_size_step(true, PRESENT_TEXT_SIZE_KEY))
                    "hx-live:disabled"=(format!(
                        "data.twigTextSize === {}",
                        TEXT_SIZES[TEXT_SIZES.len() - 1]
                    ))
                {
                    "A+"
                }
                button id="fullscreen-toggle" class="twig-btn twig-btn--quiet" type="button"
                    aria-pressed="false"
                    "hx-on:click"="data.twigFullscreen = !data.twigFullscreen"
                    "hx-live:text"="data.twigFullscreen ? 'Exit fullscreen' : 'Fullscreen'"
                    "hx-live:aria-pressed"="data.twigFullscreen"
                {
                    "Fullscreen"
                }
                a class="twig-btn twig-btn--quiet"
                    href=(format!("/{namespace}/{repo}/present/print"))
                    title="Open the print view with every slide and the license page"
                {
                    "Download PDF"
                }
                (crate::http::view::render_theme_toggle())
            }
        }

        div id="slides-wrapper" class="twig-present-stage" {
            article class="twig-md twig-md--slide" aria-live="polite" {
                (maud::PreEscaped(&slide.html))
            }
        }

        nav class="twig-present-controls" aria-label="Slide navigation" {
            (render_slide_nav_button("prev-slide", "\u{2190} Previous", prev.as_deref(), "ArrowLeft"))
            div class="twig-ticks" role="group" aria-label="Go to slide" {
                @for i in 0..slide_count {
                    button
                        class="twig-tick"
                        type="button"
                        aria-label=(format!("Slide {} of {slide_count}", i + 1))
                        aria-current=[(i == current_index).then_some("true")]
                        hx-get=(format!("/{namespace}/{repo}/slide/{i}"))
                        hx-target="#present-container"
                        hx-swap="innerHTML"
                    {}
                }
            }
            (render_slide_nav_button("next-slide", "Next \u{2192}", next.as_deref(), "ArrowRight"))
        }
    }
}

fn render_present_view(namespace: &str, repo: &str, slides: &[PresentSlide]) -> Markup {
    if slides.is_empty() {
        return render_empty(
            "NO SLIDES",
            "No presentation slides configured. Add a [present] section with files to your .twig.toml.",
        );
    }

    maud::html! {
        section
            id="present-container"
            class="twig-present"
            data-twig-text-size=(TEXT_SIZES[0])
            hx-live=(text_size_restore(PRESENT_TEXT_SIZE_KEY))
            tabindex="-1"
            aria-roledescription="carousel"
            aria-label="Presentation"
            data-twig-fullscreen="false"
            "hx-on:keydown"="if (event.key === 'Escape') data.twigFullscreen = false"
            "hx-on:htmx:after:swap"="this.focus({ preventScroll: true })"
        {
            (render_slide_content(namespace, repo, 0, slides))
        }
    }
}

/// The PDF export route: the entire deck, one A4 page per slide, with the
/// license page last. It serves a standalone document with no application
/// chrome, so the print dialog captures slides only. Rendering reads the slide
/// files again from the repository, so the output never depends on what the
/// live view currently shows — scroll position, text size, and the active
/// slide have no effect on it.
#[get("/{namespace}/{repo}/present/print")]
pub async fn present_print_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let page_title = format!("{namespace}/{repo}");
    let username = get_username_from_request(&req, &auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(handle) => handle,
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

    let twig_result = handle.load_config_with_raw();
    if twig_result.config.private && username.is_none() {
        return Ok(render_repo_auth_error(&req, &page_title));
    }

    let slides = load_present_slides(&handle, &twig_result.config.present, namespace, repo);
    // The license comes from the same repository metadata the License tab reads.
    let license_content = handle.get_license_content();
    let license = if license_content.trim().is_empty() {
        None
    } else {
        Some(license_content.as_str())
    };

    Ok(render_pdf_document(namespace, repo, &slides, license))
}

/// The export binder: a complete HTML document whose head mirrors the app
/// layout (same webfont and stylesheet, so slides look identical), whose body
/// is one page per slide plus the license page, and whose only script waits
/// for fonts and images before opening the print dialog.
fn render_pdf_document(
    namespace: &str,
    repo: &str,
    slides: &[PresentSlide],
    license: Option<&str>,
) -> Markup {
    maud::html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (format!("{namespace}/{repo} slides")) " · Twig" }
                link rel="icon" type="image/svg+xml" href=(crate::http::assets::url("twig.svg"));
                script src=(crate::http::assets::url("theme.js")) {}
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@300..700&display=swap";
                link rel="stylesheet" href=(crate::http::assets::url("twig.css"));
            }
            body class="twig-pdf" {
                main class="twig-pdf-binder" {
                    @if slides.is_empty() {
                        section class="twig-pdf-page" aria-label="No slides" {
                            (render_empty(
                                "NO SLIDES",
                                "No presentation slides configured. Add a [present] section with files to your .twig.toml.",
                            ))
                        }
                    } @else {
                        @for (index, slide) in slides.iter().enumerate() {
                            (render_pdf_slide_page(index, slides.len(), slide))
                        }
                    }
                    (render_pdf_license_page(license))
                }
                @if !slides.is_empty() {
                    div id="twig-pdf-hint" class="twig-pdf-hint" role="status" {
                        p class="twig-eyebrow" { "PRINT" }
                        p class="twig-pdf-hint-body" { "Choose “Save as PDF” in the dialog to download the deck." }
                    }
                    script { (maud::PreEscaped(PDF_PRINT_SCRIPT)) }
                }
            }
        }
    }
}

/// The header strip every export page carries: the section name and, on slide
/// pages, the same zero-padded counter the live deck shows.
fn render_pdf_pagehead(eyebrow: &str, count: Option<&str>) -> Markup {
    maud::html! {
        header class="twig-pdf-pagehead" {
            p class="twig-eyebrow" { (eyebrow) }
            @if let Some(count) = count {
                span class="twig-present-count" { (count) }
            }
        }
    }
}

/// One slide on one A4 page. The slide reuses the live view's `twig-md--slide`
/// article inside a static, print-safe stage, so the export styling matches
/// the deck at its default 100% text size.
fn render_pdf_slide_page(index: usize, slide_count: usize, slide: &PresentSlide) -> Markup {
    maud::html! {
        section
            class="twig-pdf-page"
            aria-label=(format!("Slide {} of {slide_count}", index + 1))
        {
            (render_pdf_pagehead("PRESENTATION", Some(&slide_counter(index, slide_count))))
            div class="twig-pdf-stage" {
                article class="twig-md twig-md--slide" {
                    (maud::PreEscaped(&slide.html))
                }
            }
        }
    }
}

/// The final export page: the repository license, always starting on its own
/// sheet. It reads through the same `get_license_content` fallback chain the
/// License tab uses; a repository without any license text gets the same
/// empty state the License tab shows.
fn render_pdf_license_page(license: Option<&str>) -> Markup {
    maud::html! {
        section class="twig-pdf-page twig-pdf-page--license" aria-label="License" {
            (render_pdf_pagehead("LICENSE", None))
            @if let Some(license) = license {
                div class="twig-md twig-md--boxed" {
                    (maud::PreEscaped(license))
                }
            } @else {
                (render_empty("NO LICENSE", "No license information available."))
            }
        }
    }
}

/// One lazily loaded paper page. htmx swaps in the rendered Markdown when the
/// article scrolls into view, so a long paper only pays for the pages read.
/// `anchor` is the page's stable fragment id, so any page can be linked to.
fn render_paper_page(namespace: &str, repo: &str, page: &str, anchor: &str) -> Markup {
    maud::html! {
        article
            id=(anchor)
            class="twig-paper-page"
            data-twig-paper-page=(page)
            aria-label=(format!("Paper page: {page}"))
            hx-get=(format!("/{namespace}/{repo}/paper/{page}"))
            hx-trigger="revealed"
            hx-swap="innerHTML"
        {
            p class="twig-hint twig-paper-pending" { "Loading…" }
        }
    }
}

/// The path of `page` relative to the paper `dir`.
fn paper_relative_path<'a>(dir: &str, page: &'a str) -> &'a str {
    let dir = dir.trim_matches('/');
    if dir.is_empty() {
        page
    } else {
        page.strip_prefix(dir)
            .map_or(page, |rest| rest.trim_start_matches('/'))
    }
}

/// The slug half of a page anchor: every character that would break a fragment
/// (notably the `/`) becomes `-`, while case, dots, and word separators stay.
fn paper_path_slug(relative: &str) -> String {
    relative
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

/// A stable fragment id for one paper page: `dir/01.md` links as `#paper-01.md`.
fn paper_page_anchor(dir: &str, page: &str) -> String {
    format!("paper-{}", paper_path_slug(paper_relative_path(dir, page)))
}

/// Fragment ids for every page, unique even when two paths slug the same. A
/// numeric suffix settles the rare collision.
fn paper_page_anchors(dir: &str, pages: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut anchors = Vec::with_capacity(pages.len());
    for page in pages {
        let base = paper_page_anchor(dir, page);
        let mut anchor = base.clone();
        let mut suffix = 2;
        while !seen.insert(anchor.clone()) {
            anchor = format!("{base}-{suffix}");
            suffix += 1;
        }
        anchors.push(anchor);
    }
    anchors
}

/// A heading inside a paper page. Its `anchor` is the fragment the rendered
/// heading carries.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PaperHeading {
    level: u8,
    text: String,
    anchor: String,
}

/// The numeric level of a heading, 1 through 6.
fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// A URL-safe slug for heading text: lowercase words joined by single dashes.
fn heading_slug(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    let mut pending = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending && !slug.is_empty() {
                slug.push('-');
            }
            pending = false;
            slug.push(ch.to_ascii_lowercase());
        } else {
            pending = true;
        }
    }
    if slug.is_empty() {
        slug.push_str("section");
    }
    slug
}

/// Every heading in a page, in document order, each with a fragment id unique
/// within the paper (`page_anchor--slug`).
fn paper_headings(page_anchor: &str, markdown: &str) -> Vec<PaperHeading> {
    let mut raw: Vec<(u8, String)> = Vec::new();
    let mut current: Option<(u8, String)> = None;
    for event in Parser::new_ext(markdown, markdown_options()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((heading_level(level), String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = current.take() {
                    raw.push((level, text.trim().to_string()));
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, buffer)) = current.as_mut() {
                    buffer.push_str(&text);
                }
            }
            _ => {}
        }
    }

    let mut seen: HashMap<String, usize> = HashMap::new();
    raw.into_iter()
        .map(|(level, text)| {
            let slug = heading_slug(&text);
            let count = seen.entry(slug.clone()).or_insert(0);
            *count += 1;
            let anchor = if *count == 1 {
                format!("{page_anchor}--{slug}")
            } else {
                format!("{page_anchor}--{slug}-{count}")
            };
            PaperHeading {
                level,
                text,
                anchor,
            }
        })
        .collect()
}

/// The paper toolbar: page count, the shared A−/A+ zoom, and the reading font
/// switch. The zoom state is the same `data-twig-text-size` contract the
/// presentation uses, so both remember their own scale. The toolbar pins below
/// the masthead while reading a long paper.
fn render_paper_toolbar(page_count: usize) -> Markup {
    maud::html! {
        header class="twig-paper-bar" {
            p class="twig-eyebrow" { "PAPER" }
            div class="twig-paper-tools" {
                span class="twig-paper-count" { (format!("{page_count} pages")) }
                button id="paper-decrease-text-size" class="twig-btn twig-btn--quiet" type="button"
                    aria-label="Decrease paper font size" title="Decrease font size"
                    "hx-on:click"=(text_size_step(false, PAPER_TEXT_SIZE_KEY))
                    "hx-live:disabled"=(format!("data.twigTextSize === {}", TEXT_SIZES[0]))
                {
                    "A−"
                }
                span id="paper-text-size" class="twig-paper-count" aria-live="polite"
                    "hx-live:text"="data.twigTextSize + '%'" { "100%" }
                button id="paper-increase-text-size" class="twig-btn twig-btn--quiet" type="button"
                    aria-label="Increase paper font size" title="Increase font size"
                    "hx-on:click"=(text_size_step(true, PAPER_TEXT_SIZE_KEY))
                    "hx-live:disabled"=(format!(
                        "data.twigTextSize === {}",
                        TEXT_SIZES[TEXT_SIZES.len() - 1]
                    ))
                {
                    "A+"
                }
                div role="group" aria-label="Paper font" {
                    @for (value, label) in PAPER_FONTS {
                        button
                            class="twig-btn twig-btn--quiet"
                            type="button"
                            data-twig-font=(value)
                            aria-pressed=(if value == "sans" { "true" } else { "false" })
                        {
                            (label)
                        }
                    }
                }
                (crate::http::view::render_theme_toggle())
            }
        }
    }
}

/// A paper is a directory of Markdown pages read as one continuous,
/// scroll-driven document. Each page is a placeholder until it is revealed,
/// and each carries an anchor so its position is linkable.
fn render_paper_view(namespace: &str, repo: &str, dir: &str, pages: &[String]) -> Markup {
    if pages.is_empty() {
        return render_empty(
            "NO PAGES",
            "No paper pages found. Add a [paper] section with a dir to your .twig.toml and put Markdown files in it.",
        );
    }

    let anchors = paper_page_anchors(dir, pages);

    maud::html! {
        section
            id="paper-container"
            class="twig-paper"
            data-twig-text-size=(TEXT_SIZES[0])
            data-twig-paper-font="sans"
            hx-live=(text_size_restore(PAPER_TEXT_SIZE_KEY))
            tabindex="-1"
            aria-label="Paper"
        {
            (render_paper_toolbar(pages.len()))
            div id="paper-body" class="twig-paper-body" {
                @for (page, anchor) in pages.iter().zip(&anchors) {
                    (render_paper_page(namespace, repo, page, anchor))
                }
            }
        }
        script { (maud::PreEscaped(PAPER_SCRIPT)) }
    }
}

/// Paper page content: like the Documentation view, but every heading carries a
/// paper-scoped id so each section is directly linkable.
fn render_paper_content_only(
    namespace: &str,
    repo: &str,
    page: &str,
    content: Option<&str>,
    headings: &[PaperHeading],
) -> Markup {
    let base_dir = parent_path(page);
    let html_content =
        content.map(|md| render_paper_markdown(md, namespace, repo, base_dir, headings));

    maud::html! {
        @if let Some(html) = html_content {
            div class="twig-md twig-md--prose" {
                (maud::PreEscaped(html))
            }
        } @else {
            (render_empty("NOT FOUND", "File not found or empty."))
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
        div class="twig-rail-shell" {
            // A one-file repository gets no rail at all; its only entry would
            // point at the document already on screen (DESIGN.md 5.10).
            @if markdown_files.len() > 1 {
                nav class="twig-rail twig-rail--files" aria-label="Markdown files" {
                    p class="twig-eyebrow" { "MARKDOWN FILES" }
                    @for file in markdown_files {
                        @let href = format!("/{namespace}/{repo}/md/{file}");
                        a
                            class="twig-rail-item"
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

            div class="twig-rail-body" {
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
            div class="twig-md twig-md--prose" {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::view::test_util::{classes_in, index_of};

    /// A tab frame with nothing configured. Tests override only the fields they
    /// exercise, which keeps each assertion pinned to one input.
    fn base_frame() -> TabFrame<'static> {
        TabFrame {
            namespace: "acme",
            repo: "my-project",
            username: None,
            page_title: "acme/my-project",
            twig_error: None,
            twig_filename: None,
            tabs_config: &[],
            has_config: false,
            has_present: false,
            has_paper: false,
            has_scripts: false,
        }
    }

    /// A frame whose optional tabs are set per flag, for tab-bar tests.
    #[allow(clippy::fn_params_excessive_bools)]
    fn frame_with(config: bool, present: bool, paper: bool, scripts: bool) -> TabFrame<'static> {
        TabFrame {
            has_config: config,
            has_present: present,
            has_paper: paper,
            has_scripts: scripts,
            ..base_frame()
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

    fn count_of(html: &str, needle: &str) -> usize {
        html.matches(needle).count()
    }

    fn tab_labels(html: &str) -> Vec<String> {
        html.match_indices("class=\"twig-tab\"")
            .map(|(start, _)| {
                let rest = &html[start..];
                let open = rest.find('>').expect("tab tag must be closed");
                let close = rest.find("</a>").expect("tab must be closed");
                rest[open + 1..close].to_owned()
            })
            .collect()
    }

    #[test]
    fn test_base_url_schemes_follow_the_request() {
        use actix_web::test as actix_test;

        let local = actix_test::TestRequest::get()
            .uri("http://localhost:8080/ns/repo")
            .to_http_request();
        assert_eq!(
            script_base_url(&local),
            "http://localhost:8080",
            "local hosts stay on http so dev commands run as printed"
        );

        let proxied = actix_test::TestRequest::get()
            .uri("https://git.example.com/ns/repo")
            .insert_header(("X-Forwarded-Proto", "https"))
            .to_http_request();
        assert_eq!(script_base_url(&proxied), "https://git.example.com");

        let plain = actix_test::TestRequest::get()
            .uri("http://git.example.com/ns/repo")
            .to_http_request();
        assert_eq!(
            script_base_url(&plain),
            "https://git.example.com",
            "without a proxy header, remote hosts default to https"
        );
    }

    #[test]
    fn test_raw_content_type_is_text_for_scripts_and_neutral_otherwise() {
        assert_eq!(
            raw_content_type("scripts/install.sh"),
            "text/plain; charset=utf-8"
        );
        assert_eq!(raw_content_type("README.md"), "text/plain; charset=utf-8");
        assert_eq!(
            raw_content_type("assets/logo.bin"),
            "application/octet-stream"
        );
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

        let html = markdown_to_html("```rust\nfn main() {}\n```", "test", "repo", "");
        assert!(
            html.contains("<span class=\"twig-syn-keyword\">fn</span>"),
            "{html}"
        );
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
        let html = render_tabs(&frame_with(true, true, true, true), "commits").into_string();

        assert!(
            html.contains(
                r#"<nav id="tab-nav" class="twig-tabs" aria-label="Repository views" hx-swap-oob="true">"#
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
        let html = render_tabs(&frame_with(true, true, true, true), "commits").into_string();
        let tabs = [
            "markdown", "paper", "content", "commits", "config", "present", "scripts", "license",
        ];

        for tab in tabs {
            let href = format!("/acme/my-project/{tab}");
            assert!(
                html.contains(&format!("href=\"{href}\"")),
                "missing href for {tab}: {html}"
            );
            assert!(
                html.contains(&format!("hx-get=\"{href}\"")),
                "missing hx-get for {tab}: {html}"
            );
            assert!(
                html.contains(&format!("hx-push-url=\"{href}\"")),
                "each tab pushes its own address for {tab}: {html}"
            );
        }
        assert_eq!(count_of(&html, "hx-target=\"#tab-content\""), tabs.len());
    }

    #[test]
    fn test_tabs_mark_exactly_one_current_tab() {
        let html = render_tabs(&frame_with(true, true, true, true), "commits").into_string();

        assert_eq!(
            count_of(&html, "aria-current=\"page\""),
            1,
            "one current tab only: {html}"
        );
        let href = index_of(&html, "href=\"/acme/my-project/commits\"");
        let current = index_of(&html, "aria-current=\"page\"");
        let label = index_of(&html, ">Commits<");
        assert!(
            href < current && current < label,
            "aria-current must sit on the active tab: {html}"
        );
    }

    #[test]
    fn test_tabs_render_only_available_views_in_order() {
        let html = render_tabs(&frame_with(false, false, false, false), "markdown").into_string();
        assert_eq!(
            tab_labels(&html),
            vec!["Documentation", "Content", "Commits", "License"],
            "unconfigured repositories hide Config, Present, Paper and Scripts: {html}"
        );

        let html = render_tabs(&frame_with(false, false, true, false), "paper").into_string();
        assert_eq!(
            tab_labels(&html),
            vec!["Documentation", "Paper", "Content", "Commits", "License"],
            "a configured paper slots in after Documentation: {html}"
        );

        let configured = [
            "license".to_string(),
            "bogus".to_string(),
            "config".to_string(),
            "present".to_string(),
            "paper".to_string(),
            "commits".to_string(),
        ];
        let html = render_tabs(
            &TabFrame {
                tabs_config: &configured,
                ..frame_with(false, true, true, false)
            },
            "license",
        )
        .into_string();
        assert_eq!(
            tab_labels(&html),
            vec!["License", "Present", "Paper", "Commits"],
            "configured order wins and unknown or unavailable tabs are dropped: {html}"
        );

        let html = render_tabs(
            &TabFrame {
                tabs_config: &configured,
                ..frame_with(false, true, false, false)
            },
            "paper",
        )
        .into_string();
        assert_eq!(
            tab_labels(&html),
            vec!["License", "Present", "Commits"],
            "a configured paper is hidden when no pages exist: {html}"
        );
    }

    #[test]
    fn test_repo_page_leads_with_a_breadcrumb_head_then_the_tabs() {
        let html = render_tab_shell(&base_frame(), "commits", &maud::html! {}).into_string();

        let crumbs = index_of(
            &html,
            r#"<nav class="twig-crumbs twig-crumbs--page" aria-label="Breadcrumb">"#,
        );
        let tabs = index_of(&html, "id=\"tab-nav\"");
        assert!(crumbs < tabs, "the trail, then the tabs: {html}");
        assert!(
            !html.contains("twig-optic-rule"),
            "the tab bar's own rule closes the head; no second rule: {html}"
        );
        assert!(
            html.contains(r#"<h1 class="twig-crumb-current" aria-current="page">my-project</h1>"#),
            "the trail's final segment is the page heading, rendered verbatim: {html}"
        );
        assert_eq!(count_of(&html, "<h1"), 1, "exactly one h1 per page: {html}");
    }

    #[test]
    fn test_repo_page_announces_its_tab_panel() {
        let html = render_tab_shell(&base_frame(), "commits", &maud::html! {}).into_string();
        assert!(
            html.contains(r#"<div id="tab-content" aria-live="polite">"#),
            "swapped tab content must be announced: {html}"
        );
    }

    #[test]
    fn test_tab_response_differs_for_htmx_and_direct_navigation() {
        let body = render_commits_view(&[]);
        let frame = base_frame();

        let direct_req = actix_web::test::TestRequest::default().to_http_request();
        let direct = render_tab_response(&direct_req, &frame, "commits", &body).into_string();
        assert!(
            direct.contains("<!DOCTYPE html>"),
            "a direct visit is a full document: {direct}"
        );
        assert!(
            direct.contains(r#"<div id="tab-content" aria-live="polite">"#),
            "the full page carries the swap target: {direct}"
        );

        let htmx_req = actix_web::test::TestRequest::default()
            .insert_header(("HX-Request", "true"))
            .to_http_request();
        let fragment = render_tab_response(&htmx_req, &frame, "commits", &body).into_string();
        assert!(
            !fragment.contains("<!DOCTYPE html>"),
            "an htmx call returns the fragment only: {fragment}"
        );
        assert!(
            !fragment.contains(r#"<div id="tab-content""#),
            "the fragment replaces the panel's contents, not the wrapper: {fragment}"
        );
        assert!(
            fragment.contains(">Commits<"),
            "the fragment re-sends the out-of-band tab bar: {fragment}"
        );
        assert!(fragment.contains("NO COMMITS"), "{fragment}");
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

        assert!(html.starts_with(r#"<li class="twig-commit">"#), "{html}");
        assert!(
            html.contains(r#"<code class="twig-commit-hash">a1b2c3d</code>"#),
            "the hash stays abbreviated to seven characters: {html}"
        );
        assert!(
            html.contains(r#"<span class="twig-commit-author">silen</span>"#),
            "{html}"
        );
        assert!(
            html.contains(&format!(
                r#"<time class="twig-commit-date" datetime="{}">"#,
                date.to_rfc3339()
            )),
            "the machine timestamp travels with the readable one: {html}"
        );
        assert!(
            html.contains(&date.format("%Y-%m-%d %H:%M").to_string()),
            "{html}"
        );
        assert!(
            html.contains(r#"<p class="twig-commit-msg">Rewrite the theme layer</p>"#),
            "{html}"
        );
    }

    #[test]
    fn test_commits_view_falls_back_to_an_empty_state() {
        let html = render_commits_view(&[]).into_string();
        assert!(html.contains("twig-empty"), "{html}");
        assert!(html.contains(">NO COMMITS<"), "{html}");
        assert!(!html.contains("<ol"), "no empty list is rendered: {html}");
    }

    #[test]
    fn test_content_rows_mark_directories_with_a_trailing_slash() {
        let entries = vec![entry("src", true), entry("README.md", false)];
        let html = render_content_dir("acme", "my-project", "", &entries).into_string();

        assert!(
            html.contains(r#"<a class="twig-row" href="/acme/my-project/content/src""#),
            "directories keep the primary identifier treatment: {html}"
        );
        assert!(
            html.contains(r#"<span class="twig-row-id">src/</span>"#),
            "the trailing slash is the whole directory affordance: {html}"
        );
        assert!(
            html.contains(
                r#"<a class="twig-row twig-row--file" href="/acme/my-project/content/README.md""#
            ),
            "files drop to the secondary identifier treatment: {html}"
        );
        assert_eq!(
            count_of(&html, "hx-target=\"#tab-content\""),
            2,
            "one htmx row per entry; the root trail is the current page, not a link: {html}"
        );
        assert!(html.contains("twig-panel twig-panel--flush"), "{html}");
        assert!(html.contains("class=\"twig-list\""), "{html}");
    }

    #[test]
    fn test_content_dir_offers_a_parent_row_below_the_root() {
        let entries = vec![entry("main.rs", false)];
        let html = render_content_dir("acme", "my-project", "src", &entries).into_string();
        assert!(
            html.contains(r##"href="/acme/my-project/content" hx-get="/acme/my-project/content" hx-target="#tab-content" hx-push-url="/acme/my-project/content""##),
            "the parent row climbs back to the tab route: {html}"
        );
        assert!(
            html.contains(r#"<span class="twig-row-id">..</span>"#),
            "{html}"
        );
    }

    #[test]
    fn test_presentation_offers_the_pdf_export() {
        let deck = slides(3);
        let html = render_slide_content("acme", "my-project", 0, &deck).into_string();

        assert!(
            html.contains(r#"href="/acme/my-project/present/print""#),
            "the toolbar links to the print view: {html}"
        );
        assert!(
            html.contains(
                r#"<a class="twig-btn twig-btn--quiet" href="/acme/my-project/present/print""#
            ),
            "Download PDF is a plain navigation link, not an htmx swap: {html}"
        );
    }

    #[test]
    fn test_print_export_renders_one_page_per_slide_with_the_license_last() {
        let deck = slides(3);
        let html =
            render_pdf_document("acme", "my-project", &deck, Some("<p>MIT</p>")).into_string();

        // One page per slide in deck order, each labelled and counted, plus
        // the license page at the end.
        for i in 0..3 {
            assert!(
                html.contains(&format!(r#"aria-label="Slide {} of 3""#, i + 1)),
                "every slide gets its own page: {html}"
            );
            assert!(
                html.contains(&format!("<h1>Slide {i}</h1>")),
                "slide {i} renders from the deck data: {html}"
            );
            assert!(
                html.contains(&format!(
                    r#"<span class="twig-present-count">{} / 3</span>"#,
                    i + 1
                )),
                "the export reuses the live deck's zero-padded counter: {html}"
            );
        }
        assert_eq!(
            count_of(&html, r#"class="twig-pdf-page""#),
            3,
            "exactly one page per slide: {html}"
        );
        assert!(
            index_of(&html, r#"aria-label="Slide 3 of 3""#)
                < index_of(&html, r#"class="twig-pdf-page twig-pdf-page--license""#),
            "the license page follows the last slide: {html}"
        );
        assert!(html.contains("<p>MIT</p>"), "{html}");

        // The license page starts on its own sheet, and the last page never
        // forces a trailing blank sheet.
        assert!(html.contains(r#"class="twig-pdf-page twig-pdf-page--license""#),);
    }

    #[test]
    fn test_print_export_opens_the_dialog_once_loaded() {
        let deck = slides(2);
        let html =
            render_pdf_document("acme", "my-project", &deck, Some("<p>MIT</p>")).into_string();

        assert!(
            html.contains("document.fonts.ready") && html.contains("window.print()"),
            "the export auto-prints after the fonts settle: {html}"
        );
        assert!(
            html.contains("img.complete"),
            "images join the wait before the dialog opens: {html}"
        );
        assert!(
            html.contains(r#"id="twig-pdf-hint""#) && html.contains("data-twig-pdf-ready"),
            "the save hint flips to ready when the dialog opens: {html}"
        );
        assert!(
            html.contains("Save as PDF"),
            "the hint names the dialog option to pick: {html}"
        );
    }

    #[test]
    fn test_print_export_is_a_chrome_free_document() {
        let deck = slides(2);
        let html =
            render_pdf_document("acme", "my-project", &deck, Some("<p>MIT</p>")).into_string();

        assert!(html.starts_with("<!DOCTYPE html>"), "{html}");
        assert!(
            html.contains("<title>acme/my-project slides · Twig</title>"),
            "{html}"
        );
        assert!(
            !html.contains("twig-masthead") && !html.contains("twig-tabs"),
            "the export carries no application chrome: {html}"
        );
        assert!(
            !html.contains("hx-get=") && !html.contains("/assets/h.js"),
            "the export is a static document: {html}"
        );
        assert!(
            html.contains(r#"class="twig-pdf""#) && html.contains(r#"class="twig-pdf-binder""#),
            "{html}"
        );

        let empty = render_pdf_document("acme", "my-project", &[], None).into_string();
        assert!(
            empty.contains("NO SLIDES") && empty.contains("No license information available."),
            "a deck without slides still names both conditions: {empty}"
        );
        assert!(
            !empty.contains("window.print()"),
            "an empty binder has nothing to print: {empty}"
        );
    }

    #[test]
    fn test_content_breadcrumbs_mark_the_final_segment_as_current() {
        let html = render_content_breadcrumbs("acme", "my-project", "src/git").into_string();
        assert!(
            html.starts_with(
                r#"<nav class="twig-crumbs twig-crumbs--path" aria-label="File path">"#
            ),
            "{html}"
        );
        assert!(
            html.contains(r#"<span class="twig-crumb-sep" aria-hidden="true">/</span>"#),
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
            !html.contains("twig-rail-item"),
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
            html.contains(
                r#"<nav class="twig-rail twig-rail--files" aria-label="Markdown files">"#
            ),
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
        assert!(html.contains("twig-empty"), "{html}");
        assert!(html.contains("File not found or empty."), "{html}");

        let html = render_markdown_content_only("acme", "my-project", "README.md", Some("# Hi"))
            .into_string();
        assert!(
            html.contains(r#"<div class="twig-md twig-md--prose">"#),
            "{html}"
        );
    }

    #[test]
    fn test_presentation_targets_its_container_and_scopes_the_keyboard() {
        let deck = slides(3);
        let html = render_present_view("acme", "my-project", &deck).into_string();

        assert!(
            html.contains(r#"<section id="present-container" class="twig-present" "#)
                && html.contains(
                    r#"tabindex="-1" aria-roledescription="carousel" aria-label="Presentation""#
                ),
            "{html}"
        );
        assert!(
            html.contains(r#"data-twig-fullscreen="false""#),
            "fullscreen is a CSS overlay state, not the Fullscreen API: {html}"
        );
        assert_eq!(
            count_of(&html, "hx-target=\"#present-container\""),
            4,
            "next plus three ticks all swap the container: {html}"
        );
        assert_eq!(count_of(&html, "hx-swap=\"innerHTML\""), 4, "{html}");

        // Keyboard navigation is declarative htmx scoped to the container, so a
        // key press only advances the deck while focus is inside it. The first
        // slide disables Previous, so only the right arrow is wired there; the
        // left arrow appears once there is a slide to return to.
        assert!(
            html.contains(r"keyup[key=='ArrowRight'] from:#present-container"),
            "the right arrow advances the next slide via htmx: {html}"
        );
        let middle = render_slide_content("acme", "my-project", 1, &deck).into_string();
        assert!(
            middle.contains(r"keyup[key=='ArrowLeft'] from:#present-container"),
            "the left arrow returns via htmx: {middle}"
        );
        assert!(
            !html.contains("document.addEventListener"),
            "nothing is bound to the document: {html}"
        );
        assert!(
            !html.contains("<script>"),
            "the presentation carries no inline script: {html}"
        );
        assert!(
            !html.contains("requestFullscreen"),
            "fullscreen no longer depends on the Fullscreen API: {html}"
        );
    }

    #[test]
    fn test_presentation_controls_are_labelled_and_bounded() {
        let deck = slides(3);
        let html = render_slide_content("acme", "my-project", 0, &deck).into_string();

        assert!(
            html.contains("twig-theme-toggle"),
            "the theme switch must remain accessible in fullscreen and after slide swaps: {html}"
        );
        for control in [
            r#"id="decrease-text-size" class="twig-btn twig-btn--quiet" type="button" aria-label="Decrease presentation font size" title="Decrease font size""#,
            r#"id="increase-text-size" class="twig-btn twig-btn--quiet" type="button" aria-label="Increase presentation font size" title="Increase font size""#,
            r#"<span id="present-text-size" class="twig-present-count" aria-live="polite" hx-live:text="data.twigTextSize + '%'">100%</span>"#,
        ] {
            assert!(
                html.contains(control),
                "missing presentation font control: {html}"
            );
        }
        assert!(
            html.contains(
                r#"<button id="prev-slide" class="twig-btn twig-btn--quiet" type="button" disabled>"#
            ),
            "the first slide disables Previous rather than swapping in a span: {html}"
        );
        assert!(
            html.contains(r#"<button id="next-slide" class="twig-btn twig-btn--quiet" type="button" hx-get="/acme/my-project/slide/1""#),
            "{html}"
        );
        assert!(
            html.contains(r#"<div class="twig-ticks" role="group" aria-label="Go to slide">"#),
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
            html.contains(
                r#"<button id="fullscreen-toggle" class="twig-btn twig-btn--quiet" type="button" aria-pressed="false" hx-on:click="data.twigFullscreen = !data.twigFullscreen""#
            ),
            "fullscreen is a real toggle button with a visible label: {html}"
        );
        assert!(
            html.contains(
                r#"hx-live:text="data.twigFullscreen ? 'Exit fullscreen' : 'Fullscreen'""#
            ),
            "the fullscreen label reflects the toggle state: {html}"
        );

        let last = render_slide_content("acme", "my-project", 2, &deck).into_string();
        assert!(
            last.contains(
                r#"<button id="next-slide" class="twig-btn twig-btn--quiet" type="button" disabled>"#
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
        assert!(html.contains("twig-empty"), "{html}");
        assert!(html.contains(">NO SLIDES<"), "{html}");
        assert!(
            html.contains(
                "No presentation slides configured. Add a [present] section with files to your .twig.toml."
            ),
            "{html}"
        );
        assert!(!html.contains("present-container"), "{html}");
    }

    #[test]
    fn test_paper_view_lazy_loads_pages_with_zoom_and_font_controls() {
        let pages = vec!["paper/01.md".to_string(), "paper/02.md".to_string()];
        let html = render_paper_view("acme", "my-project", "paper", &pages).into_string();

        assert!(
            html.contains(
                r#"<section id="paper-container" class="twig-paper" data-twig-text-size="100" data-twig-paper-font="sans""#
            ),
            "the paper keeps the shared zoom state: {html}"
        );
        assert!(
            html.contains(r#"aria-label="Paper""#),
            "the reader names itself: {html}"
        );

        for (page, anchor) in pages.iter().zip(["paper-01.md", "paper-02.md"]) {
            let expected = format!(
                r#"<article id="{anchor}" class="twig-paper-page" data-twig-paper-page="{page}""#
            );
            assert!(
                html.contains(&expected),
                "each page carries its anchor: {page}: {html}"
            );
            let wired = format!(
                r#"hx-get="/acme/my-project/paper/{page}" hx-trigger="revealed" hx-swap="innerHTML""#
            );
            assert!(
                html.contains(&wired),
                "each page is revealed on scroll: {page}: {html}"
            );
        }
        assert_eq!(
            count_of(&html, "hx-trigger=\"revealed\""),
            pages.len(),
            "one lazy trigger per page: {html}"
        );

        for control in [
            r#"id="paper-decrease-text-size" class="twig-btn twig-btn--quiet" type="button" aria-label="Decrease paper font size" title="Decrease font size""#,
            r#"id="paper-increase-text-size" class="twig-btn twig-btn--quiet" type="button" aria-label="Increase paper font size" title="Increase font size""#,
        ] {
            assert!(html.contains(control), "missing zoom control: {html}");
        }
        for font in ["sans", "serif", "mono"] {
            assert!(
                html.contains(&format!(r#"data-twig-font="{font}""#)),
                "missing font choice {font}: {html}"
            );
        }
        assert_eq!(
            count_of(&html, r#"aria-pressed="true""#),
            1,
            "exactly one font is pressed by default: {html}"
        );
        assert!(
            html.contains("c.addEventListener('click'"),
            "the font switch is scoped to the container: {html}"
        );
        assert!(
            !html.contains("document.addEventListener"),
            "the paper never binds a document listener: {html}"
        );
        assert!(
            html.contains("IntersectionObserver") && html.contains("history.replaceState"),
            "the page in view is anchored in the URL: {html}"
        );
        assert!(
            !html.contains("addEventListener('scroll'"),
            "the anchor follows an observer, not a scroll listener: {html}"
        );
    }

    #[test]
    fn test_paper_headings_slug_anchor_and_dedupe() {
        let markdown = "# Hello, World!\n\n## Details & More\n\n### Details & More\n";
        assert_eq!(
            paper_headings("paper-01.md", markdown),
            vec![
                PaperHeading {
                    level: 1,
                    text: "Hello, World!".to_string(),
                    anchor: "paper-01.md--hello-world".to_string(),
                },
                PaperHeading {
                    level: 2,
                    text: "Details & More".to_string(),
                    anchor: "paper-01.md--details-more".to_string(),
                },
                PaperHeading {
                    level: 3,
                    text: "Details & More".to_string(),
                    anchor: "paper-01.md--details-more-2".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_render_paper_markdown_puts_heading_ids_on_the_headings() {
        let markdown = "# Hello\n\nBody\n\n## Details\n";
        let headings = paper_headings("paper-01.md", markdown);
        let html = render_paper_markdown(markdown, "acme", "my-project", "paper", &headings);

        assert!(
            html.contains(r#"<h1 id="paper-01.md--hello">Hello</h1>"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<h2 id="paper-01.md--details">Details</h2>"#),
            "{html}"
        );
        assert!(html.contains("<p>Body</p>"), "{html}");
    }

    #[test]
    fn test_render_paper_content_only_anchors_headings() {
        let markdown = "# Hello\n";
        let headings = paper_headings("paper-01.md", markdown);
        let html = render_paper_content_only(
            "acme",
            "my-project",
            "paper/01.md",
            Some(markdown),
            &headings,
        )
        .into_string();

        assert!(
            html.contains(r#"<h1 id="paper-01.md--hello">Hello</h1>"#),
            "{html}"
        );
    }

    #[test]
    fn test_paper_page_anchors_are_path_relative_and_unique() {
        let pages = [
            "paper/01.md".to_string(),
            "paper/extra/02 two.md".to_string(),
            "paper/extra-02 two.md".to_string(),
        ];
        assert_eq!(
            paper_page_anchors("paper", &pages),
            vec![
                "paper-01.md",
                "paper-extra-02-two.md",
                "paper-extra-02-two.md-2",
            ]
        );

        // A different paper directory anchors the same relative page the same.
        assert_eq!(
            paper_page_anchors("manuscript", &["manuscript/intro.md".to_string()]),
            vec!["paper-intro.md"]
        );
    }

    #[test]
    fn test_paper_view_without_pages_states_the_condition() {
        let html = render_paper_view("acme", "my-project", "paper", &[]).into_string();
        assert!(html.contains(">NO PAGES<"), "{html}");
        assert!(
            html.contains("Add a [paper] section with a dir"),
            "the empty state points at the config: {html}"
        );
        assert!(!html.contains("paper-container"), "{html}");
    }

    #[test]
    fn test_page_in_dir_accepts_only_paths_below_the_paper_directory() {
        assert!(page_in_dir("paper/01.md", "paper"));
        assert!(page_in_dir("paper/nested/02.md", "paper"));
        assert!(!page_in_dir("paper", "paper"));
        assert!(!page_in_dir("paperback/01.md", "paper"));
        assert!(!page_in_dir("README.md", "paper"));
        assert!(!page_in_dir("paper/01.md", ""));
    }

    #[test]
    fn test_config_error_is_a_diagnostic_with_a_help_hint() {
        let error =
            "TOML parse error at line 2, column 1\n  |\n2 | bad = = valid\n  | ^\ninvalid key";
        let html = render_config_error(".twig.toml", error).into_string();

        assert!(html.contains("role=\"alert\""), "{html}");
        assert!(html.contains("twig-notice twig-notice--danger"), "{html}");
        assert!(html.contains(">CONFIG ERROR<"), "{html}");
        assert!(
            html.contains(r#"<code class="twig-mono">.twig.toml</code>"#),
            "the offending file is named: {html}"
        );
        assert!(
            html.contains("error: could not parse configuration"),
            "{html}"
        );
        assert!(
            html.contains("--&gt; .twig.toml"),
            "the pointer is escaped, not interpreted as markup: {html}"
        );
        assert!(html.contains("help: fix the syntax"), "{html}");
        assert!(
            html.contains("invalid key"),
            "the parser's own explanation survives: {html}"
        );
        assert!(
            html.contains(r#"tabindex="0" aria-label=".twig.toml parse error""#),
            "the diagnostic stays keyboard-scrollable: {html}"
        );
    }

    #[test]
    fn test_tab_content_shows_a_config_error_on_every_tab() {
        let frame = TabFrame {
            twig_error: Some("TOML parse error at line 1, column 1"),
            twig_filename: Some(".twig.toml"),
            ..base_frame()
        };
        let html = render_tab_shell(&frame, "commits", &maud::html! {}).into_string();

        let error = index_of(&html, "twig-notice--danger");
        let tabs = index_of(&html, "id=\"tab-nav\"");
        let content = index_of(&html, "id=\"tab-content\"");
        assert!(
            tabs < error && error < content,
            "the notice sits between the tabs and the panel so it survives swaps: {html}"
        );
        assert!(
            html.contains("TOML parse error at line 1, column 1"),
            "{html}"
        );
    }

    #[test]
    fn test_config_and_license_views_render_code_and_empty_states() {
        let html = render_config_view(Some("[present]\nfiles = []"), Some(".twig")).into_string();
        assert!(
            html.contains(r#"<pre class="twig-code" tabindex="0" aria-label=".twig contents">"#),
            "code blocks are keyboard scrollable and named: {html}"
        );

        let html = render_config_view(None, None).into_string();
        assert!(html.contains(">NO CONFIGURATION<"), "{html}");
        assert!(
            html.contains("Create a .twig.toml file in the repository root"),
            "the original guidance survives: {html}"
        );

        let html = render_license_view(Some("<p>MIT</p>")).into_string();
        assert!(
            html.contains(r#"<div class="twig-md twig-md--boxed"><p>MIT</p></div>"#),
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
        assert!(html.contains("twig-empty"), "{html}");

        let html = render_content_file("acme", "my-project", "logo.png", &[0, 1, 2]).into_string();
        assert!(html.contains(">BINARY FILE<"), "{html}");
        assert!(!html.contains("Edit file"), "{html}");
        assert!(
            html.contains("Binary file (3 bytes). Not displayed."),
            "{html}"
        );

        let html =
            render_content_file("acme", "my-project", "main.rs", b"fn main() {}").into_string();
        assert!(
            html.contains(r#"<pre class="twig-code" tabindex="0" aria-label="main.rs contents">"#),
            "{html}"
        );
        assert!(html.contains("/acme/my-project/edit/main.rs"), "{html}");
        assert!(html.contains(">Edit file</a>"), "{html}");
        assert!(
            html.contains(r#"<span aria-current="page">main.rs</span>"#),
            "the filename remains in the inner breadcrumb: {html}"
        );
        assert!(
            !html.contains(r#"<h2 class="twig-section">main.rs</h2>"#),
            "the file is not repeated as a title: {html}"
        );
        assert!(
            html.contains("<span class=\"twig-syn-keyword\">fn</span>"),
            "Rust source is tokenized while remaining visible: {html}"
        );

        let html =
            render_content_file("acme", "my-project", "docs/a.md", b"[b](c.md)").into_string();
        assert!(
            html.contains(r#"<div class="twig-md twig-md--boxed">"#),
            "markdown files keep the boxed markdown surface: {html}"
        );
        assert!(
            html.contains("/acme/my-project/md/docs/c.md"),
            "link rewriting still resolves against the containing directory: {html}"
        );
        assert!(html.contains("/acme/my-project/edit/docs/a.md"), "{html}");

        let invalid_utf8 =
            render_content_file("acme", "my-project", "raw.txt", &[0xff]).into_string();
        assert!(!invalid_utf8.contains("Edit file"), "{invalid_utf8}");
        assert!(
            !html.contains("https://"),
            "no remote editor scripts: {html}"
        );
    }

    #[test]
    fn test_git_error_is_an_announced_danger_notice() {
        let error = git2::Error::from_str("could not find repository");
        let html = render_git_error(&error).into_string();
        assert!(html.contains("role=\"alert\""), "{html}");
        assert!(html.contains("twig-notice twig-notice--danger"), "{html}");
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
            (
                "repo page",
                render_tab_shell(&base_frame(), "commits", &maud::html! {}).into_string(),
            ),
            (
                "markdown tab",
                render_markdown_view(
                    "acme",
                    "my-project",
                    "README.md",
                    Some("# Hi\n\nHello."),
                    &files,
                )
                .into_string(),
            ),
            ("commits tab", render_commits_view(&commits).into_string()),
            (
                "config tab",
                render_config_view(Some("[present]"), Some(".twig.toml")).into_string(),
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
                "present print",
                render_pdf_document("acme", "my-project", &deck, Some("<p>MIT</p>")).into_string(),
            ),
            (
                "empty present print",
                render_pdf_document("acme", "my-project", &[], None).into_string(),
            ),
            (
                "empty present tab",
                render_present_view("acme", "my-project", &[]).into_string(),
            ),
            (
                "paper tab",
                render_paper_view(
                    "acme",
                    "my-project",
                    "paper",
                    &["paper/01.md".to_string(), "paper/02.md".to_string()],
                )
                .into_string(),
            ),
            (
                "empty paper tab",
                render_paper_view("acme", "my-project", "paper", &[]).into_string(),
            ),
            (
                "config error",
                render_config_error(".twig.toml", "TOML parse error at line 1, column 1")
                    .into_string(),
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
        assert!(html.contains("<title>acme/secret · Twig</title>"));

        let htmx_req = actix_web::test::TestRequest::default()
            .insert_header(("HX-Request", "true"))
            .to_http_request();
        let htmx_html = render_repo_auth_error(&htmx_req, "acme/secret").into_string();
        assert!(htmx_html.contains("Not logged in. Please log in first."));
        assert!(!htmx_html.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn test_repo_markup_uses_only_twig_design_system_classes() {
        for (surface, html) in representative_markup() {
            let classes = classes_in(&html);
            assert!(
                !classes.is_empty(),
                "{surface} should carry classes: {html}"
            );
            for class in classes {
                assert!(
                    class.starts_with("twig-"),
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
                "presentation CSS lives in twig.css, not in {surface}: {html}"
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
