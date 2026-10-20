use std::collections::HashMap;

use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use maud::Markup;
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};

use crate::auth::TwigContext;
use crate::config;
use crate::git::bare::is_safe_repo_path;
use crate::md;

use super::MarkdownParams;
use super::content::parent_path;
use super::context::{open_repo, render_empty, render_not_found_for_request};
use super::paper::PaperHeading;
use super::tabs::{markdown_files, render_tab_response};

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
pub(super) fn has_url_scheme(dest: &str) -> bool {
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
pub(super) fn split_link_suffix(dest: &str) -> (&str, &str) {
    match dest.find(['#', '?']) {
        Some(i) => dest.split_at(i),
        None => (dest, ""),
    }
}

/// Resolves `link` against `base_dir` (a repository-relative directory, empty
/// for the repository root) into a repository-relative path. `.` segments are
/// dropped and `..` segments pop a parent, clamped at the repository root.
pub(super) fn resolve_relative_path(base_dir: &str, link: &str) -> String {
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
pub(super) fn rewrite_markdown_link(
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
pub(super) fn markdown_options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options
}

/// Renders markdown to HTML. `base_dir` is the repository-relative directory of
/// the file being rendered and anchors every relative link it contains.
pub(super) fn markdown_to_html(
    markdown: &str,
    namespace: &str,
    repo: &str,
    base_dir: &str,
) -> String {
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
pub(super) fn render_paper_markdown(
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
pub(super) fn render_slide_markdown(
    content: &str,
    namespace: &str,
    repo: &str,
    file: &str,
    vars: &HashMap<String, String>,
) -> String {
    let replaced = md::replace_mustache(content, vars);
    let base_dir = parent_path(file);

    let parser = Parser::new_ext(&replaced, markdown_options()).map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(tag) => Event::Start(fix_link_tag(tag, namespace, repo, base_dir)),
        other => other,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, md::highlight_rust_code_blocks(parser));
    html_output
}

/// An Empty State (DESIGN.md 5.13). The eyebrow names the condition in words so
/// the meaning never rests on colour alone.
pub(super) const MARKDOWN_COPY_SCRIPT: &str = r"(function(){
var s=document.currentScript;
var src=s&&s.previousElementSibling;
var bar=src&&src.previousElementSibling;
var b=bar&&bar.querySelector('[data-twig-copy-markdown]');
if(!src||!b)return;
var done=function(ok){
b.textContent=ok?'Copied':'Select manually';
setTimeout(function(){b.textContent='Copy source';},1500);
if(!ok){
try{var range=document.createRange();range.selectNodeContents(src);var sel=window.getSelection();sel.removeAllRanges();sel.addRange(range);}catch(_){}
}
};
b.addEventListener('click',function(){
var text=src.textContent;
if(navigator.clipboard&&navigator.clipboard.writeText){
navigator.clipboard.writeText(text).then(function(){done(true);},function(){done(false);});
}else{done(false);}
});
})();";

/// The source actions shown only for Markdown files in the Content tab. The Raw
/// link reuses the existing guarded `/raw/` route; the hidden source lets Copy
/// preserve Markdown rather than rendered HTML.
pub(super) fn render_markdown_controls(
    namespace: &str,
    repo: &str,
    file_path: &str,
    source: &str,
) -> Markup {
    let raw_url = format!("/{namespace}/{repo}/raw/{file_path}");
    maud::html! {
        header class="twig-docbar" {
            p class="twig-eyebrow" { "MARKDOWN SOURCE" }
            div class="twig-cluster twig-cluster--tight" {
                a class="twig-btn twig-btn--quiet" href=(raw_url) { "Raw" }
                button
                    class="twig-btn twig-btn--ghost"
                    type="button"
                    data-twig-copy-markdown
                    title="Copy this document as Markdown"
                {
                    "Copy source"
                }
            }
        }
        // A leading comment prevents the HTML parser's special <pre> newline
        // handling from changing a source that begins with a newline.
        pre class="twig-sr" data-twig-markdown-source {
            (maud::PreEscaped("<!-- -->"))
            (source)
        }
        script { (maud::PreEscaped(MARKDOWN_COPY_SCRIPT)) }
    }
}

pub(super) fn render_markdown_view(
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
pub(super) fn render_markdown_content_only(
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
