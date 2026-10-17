use super::tabs::respond_tab;
use super::{
    AwResult, Deserialize, HttpRequest, HttpResponse, Markup, RepoContext, RepoHandle, ScriptEntry,
    ScriptGroupNode, TwigContext, config, get, get_username_from_request, header,
    is_safe_repo_path, render_empty, web,
};

#[derive(Deserialize)]
pub(super) struct ScriptGroupParams {
    namespace: String,
    repo: String,
    /// Slash-joined group key, e.g. `linux/maintenance`.
    group: String,
}

#[derive(Deserialize)]
pub(super) struct RawParams {
    namespace: String,
    repo: String,
    path: String,
}

/// The Scripts tab: the active group's scripts with their copyable `curl`
/// commands. `group` selects a group by its slash-joined key; `None` and
/// unknown keys fall back to the first configured group.
pub(super) fn scripts_body(
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

/// The Scripts tab: each configured group becomes a sub-tab labelled with the
/// group name, and the active group lists its scripts with a copyable
/// download-and-run command and its nested groups as links.
pub(super) fn render_scripts_view(
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
pub(super) fn render_script_group_tabs(ctx: &RepoContext, active: &ScriptGroupNode) -> Markup {
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
pub(super) fn script_command(ctx: &RepoContext, entry: &ScriptEntry) -> String {
    format!(
        "curl -fsSL '{}/{}/{}/raw/{}' | {}",
        ctx.base_url, ctx.namespace, ctx.repo, entry.path, entry.shell
    )
}

/// One script row: its name and path, the run command, and a copy button.
/// A configured path missing from HEAD is marked instead of silently skipped.
pub(super) fn render_script_row(ctx: &RepoContext, entry: &ScriptEntry) -> Markup {
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
pub(super) fn render_script_group_row(ctx: &RepoContext, key: &str) -> Markup {
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
pub(super) const SCRIPTS_SCRIPT: &str = r"(function(){
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
pub(super) fn script_base_url(req: &HttpRequest) -> String {
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
pub(super) fn raw_content_type(path: &str) -> &'static str {
    let extension = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match extension.as_str() {
        "sh" | "bash" | "zsh" | "py" | "rb" | "pl" | "js" | "ts" | "toml" | "yaml" | "yml"
        | "json" | "md" | "txt" | "conf" | "cfg" | "ini" | "env" | "sql" | "rs" | "go" | "c"
        | "h" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
