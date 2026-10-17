use super::content::parent_path;
use super::markdown::{markdown_options, render_paper_markdown};
use super::{
    AwResult, Deserialize, Event, HashMap, HashSet, HeadingLevel, HttpRequest, Markup,
    PAPER_TEXT_SIZE_KEY, Parser, RepoContext, RepoHandle, TEXT_SIZES, Tag, TagEnd, TwigConfig,
    TwigContext, config, get, is_safe_repo_path, open_repo, render_empty, render_for_request,
    text_size_restore, text_size_step, web,
};

/// Paper font choices. The active choice lives on `#paper-container` as
/// `data-twig-paper-font`; the stylesheet swaps the reading family from it.
pub(super) const PAPER_FONTS: [(&str, &str); 3] =
    [("sans", "Sans"), ("serif", "Serif"), ("mono", "Mono")];

/// Paper toolbar and reading-position wiring, scoped to `#paper-container`:
/// restores the stored font, keeps the toggle buttons' pressed state in sync,
/// and mirrors the page crossing the viewport's midline into the URL fragment
/// so the current page is always linkable. An `IntersectionObserver` is used
/// instead of a scroll listener, matching the presentation contract of binding
/// nothing at the document level.
pub(super) const PAPER_SCRIPT: &str = r"(function(){
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

#[derive(Deserialize)]
pub(super) struct PaperParams {
    namespace: String,
    repo: String,
    page: String,
}

pub(super) fn paper_body(ctx: &RepoContext) -> Markup {
    let dir = ctx
        .twig_result
        .config
        .paper
        .as_ref()
        .map_or("", |paper| paper.dir.as_str());
    render_paper_view(&ctx.namespace, &ctx.repo, dir, &ctx.paper_pages)
}

/// Whether `page` sits inside `dir` (both repository-relative).
pub(super) fn page_in_dir(page: &str, dir: &str) -> bool {
    !dir.is_empty()
        && page
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The Markdown and parsed headings for one page, or `None` when `page` falls
/// outside the configured paper directory. The headline ids are computed from
/// the page's own anchor.
pub(super) fn open_paper_page(
    ctx: &RepoContext,
    page: &str,
) -> Option<(Option<String>, Vec<PaperHeading>)> {
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

/// The configured paper pages, sorted for reading. Empty when `[paper]` is
/// absent, unconfigured, or its directory is missing from HEAD.
pub(super) fn load_paper_pages(handle: &RepoHandle, config: &TwigConfig) -> Vec<String> {
    let Some(paper) = config.paper.as_ref().filter(|paper| paper.is_configured()) else {
        return Vec::new();
    };
    handle.list_paper_pages(&paper.dir).unwrap_or_default()
}

/// One lazily loaded paper page. htmx swaps in the rendered Markdown when the
/// article scrolls into view, so a long paper only pays for the pages read.
/// `anchor` is the page's stable fragment id, so any page can be linked to.
pub(super) fn render_paper_page(namespace: &str, repo: &str, page: &str, anchor: &str) -> Markup {
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
pub(super) fn paper_relative_path<'a>(dir: &str, page: &'a str) -> &'a str {
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
pub(super) fn paper_path_slug(relative: &str) -> String {
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
pub(super) fn paper_page_anchor(dir: &str, page: &str) -> String {
    format!("paper-{}", paper_path_slug(paper_relative_path(dir, page)))
}

/// Fragment ids for every page, unique even when two paths slug the same. A
/// numeric suffix settles the rare collision.
pub(super) fn paper_page_anchors(dir: &str, pages: &[String]) -> Vec<String> {
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
pub(super) struct PaperHeading {
    pub(super) level: u8,
    pub(super) text: String,
    pub(super) anchor: String,
}

/// The numeric level of a heading, 1 through 6.
pub(super) fn heading_level(level: HeadingLevel) -> u8 {
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
pub(super) fn heading_slug(text: &str) -> String {
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
pub(super) fn paper_headings(page_anchor: &str, markdown: &str) -> Vec<PaperHeading> {
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
pub(super) fn render_paper_toolbar(page_count: usize) -> Markup {
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
pub(super) fn render_paper_view(
    namespace: &str,
    repo: &str,
    dir: &str,
    pages: &[String],
) -> Markup {
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
pub(super) fn render_paper_content_only(
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
