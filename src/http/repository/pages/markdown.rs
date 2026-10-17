use super::tabs::markdown_files;
use super::{
    AwResult, CowStr, Event, HashMap, HttpRequest, Markup, Options, Parser, RepoContext, Tag,
    TwigContext, config, get, html, is_safe_repo_path, md, open_repo, render_empty,
    render_not_found_for_request, render_tab_response, web,
};
use super::{content::parent_path, paper::PaperHeading};
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct MarkdownParams {
    namespace: String,
    repo: String,
    file_path: String,
}

/// The Documentation tab: the default Markdown file, README preferred.
pub(super) fn markdown_body(ctx: &RepoContext) -> Markup {
    markdown_body_from(ctx, &markdown_files(ctx))
}

pub(super) fn markdown_body_from(ctx: &RepoContext, files: &[String]) -> Markup {
    let file = get_default_markdown_file(files).unwrap_or("README.md");
    let content = ctx.handle.read_file(file).ok().flatten();
    render_markdown_view(&ctx.namespace, &ctx.repo, file, content.as_deref(), files)
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

/// Converts markdown to HTML, fixing relative links to point to repo root
/// A destination is external when it carries a URL scheme such as `https:` or
/// `mailto:`. A bare relative path never does, because a scheme cannot contain
/// a `/`.
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
pub(super) fn fix_link_tag<'a>(
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
        Event::Start(tag) => Event::Start(fix_link_tag(tag, namespace, repo, base_dir)),
        other => other,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, md::highlight_rust_code_blocks(parser));
    html_output
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
