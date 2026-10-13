use actix_web::HttpRequest;
use maud::Markup;

use crate::db::binaries::BinaryRelease;
use crate::git::bare::{ScriptEntry, ScriptGroupNode};

use super::context::{RepoContext, render_empty};

/// Renders a tab by name. Only the requested tab's data is loaded, so a Commits
/// request never reads markdown, slides, or paper pages. `group` selects a
/// Scripts sub-group and is ignored by every other tab.
pub(super) fn scripts_body(
    ctx: &RepoContext,
    group: Option<&str>,
    releases: &[BinaryRelease],
    binaries_unavailable: bool,
) -> Markup {
    let active = ctx
        .script_groups
        .iter()
        .find(|node| Some(node.key.as_str()) == group)
        .or_else(|| ctx.script_groups.first());
    match active {
        Some(node) => {
            super::scripts::render_scripts_view(ctx, node, releases, binaries_unavailable)
        }
        None => render_empty(
            "NO SCRIPTS",
            "No scripts are configured. Add a [scripts.<group>] section with a scripts list to .twig.toml.",
        ),
    }
}

/// Renders a tab by name. Only the requested tab's data is loaded, so a Commits
/// request never reads markdown, slides, or paper pages. `group` selects a
/// Scripts sub-group and is ignored by every other tab.
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

/// Each file-based Markdown view carries its own copy control and source. The
/// script is emitted after the hidden source so it binds to exactly that page,
/// including when htmx replaces the current document.
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
