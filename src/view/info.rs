//! The "Docs" page at `/_info`: tabs for bundled documentation and an
//! about page, both sourced from hardcoded data compiled into the binary
//! (see `crate::info`). Registered before `view::namespace::handler`, whose
//! `/{namespace}` pattern would otherwise swallow `/_info`.
//!
//! Every page carries a copy button that puts the page's Markdown source on
//! the clipboard, so a reader can paste a whole doc elsewhere without
//! scraping the rendered prose.
//!
//! There is no editing here — content is fixed at build time. See
//! `crate::info` to change it.

use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use serde::Deserialize;

use super::render_layout;
use super::session_auth::get_username_from_request;
use crate::auth::TwigContext;
use crate::info::{self, DocPage, Page};

#[derive(Deserialize)]
struct Query {
    tab: Option<String>,
    page: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Docs,
    About,
}

impl Tab {
    fn from_query(raw: Option<&str>) -> Self {
        match raw {
            Some("about") => Tab::About,
            _ => Tab::Docs,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Tab::Docs => "docs",
            Tab::About => "about",
        }
    }
}

fn render_tabs(active: Tab) -> maud::Markup {
    let tabs = [(Tab::Docs, "Documentation"), (Tab::About, "About")];
    maud::html! {
        nav class="twig-tabs" aria-label="Docs views" {
            @for (tab, label) in tabs {
                a
                    class="twig-tab"
                    aria-current=[(tab == active).then_some("page")]
                    href=(format!("/_info?tab={}", tab.as_str()))
                {
                    (label)
                }
            }
        }
    }
}

/// Renders the Docs rail, grouping consecutive pages that share a `section`
/// under a small eyebrow label. Pages sort by `order` then title
/// (see [`info::list_doc_pages`]), so a section's pages need not be
/// contiguous in principle, but the built-in content keeps them so; a
/// section is only headed once, at its first appearance.
fn render_docs_menu(docs: &[DocPage], active_slug: &str) -> maud::Markup {
    let mut last_section: Option<&str> = None;
    let show_heading: Vec<bool> = docs
        .iter()
        .map(|doc| {
            let section = doc.page.section;
            let is_new = section.is_some() && section != last_section;
            last_section = section;
            is_new
        })
        .collect();

    maud::html! {
        nav class="twig-rail twig-rail--docs" aria-label="Documentation" {
            @for (doc, is_new_section) in docs.iter().zip(&show_heading) {
                @if *is_new_section {
                    p class="twig-eyebrow" { (doc.page.section.unwrap_or_default()) }
                }
                a
                    class="twig-rail-item"
                    aria-current=[(doc.slug == active_slug).then_some("page")]
                    href=(format!("/_info?tab=docs&page={}", doc.slug))
                {
                    (doc.page.title)
                }
            }
        }
    }
}

/// The toolbar above a page's prose plus the page's Markdown source, held in
/// a screen-reader-only element the copy script reads. Handing over the source
/// rather than the rendered HTML keeps the pasted text portable.
fn render_copy_bar(page: &Page) -> maud::Markup {
    maud::html! {
        header class="twig-docbar" {
            p class="twig-eyebrow" { "MARKDOWN SOURCE" }
            button
                class="twig-btn twig-btn--ghost"
                type="button"
                data-twig-copy-doc
                title="Copy this page as Markdown"
            {
                "Copy page"
            }
        }
        pre class="twig-sr" data-twig-doc-source { (page.content) }
    }
}

/// Copy-to-clipboard wiring for the Docs page, scoped to `#docs-container`:
/// the button in a page's copy bar copies that page's Markdown source and
/// falls back to selecting it so a manual copy still works.
const DOCS_SCRIPT: &str = r"(function(){
var c=document.getElementById('docs-container');
if(!c||c.dataset.twigDocsCopy)return;
c.dataset.twigDocsCopy='1';
c.addEventListener('click',function(e){
var b=e.target.closest?e.target.closest('[data-twig-copy-doc]'):null;
if(!b||!c.contains(b))return;
var src=c.querySelector('[data-twig-doc-source]');
if(!src)return;
var text=src.textContent.replace(/\s+$/,'');
var done=function(ok){
b.textContent=ok?'Copied':'Select manually';
setTimeout(function(){b.textContent='Copy page';},1500);
if(!ok){
try{
var range=document.createRange();range.selectNodeContents(src);
var sel=window.getSelection();sel.removeAllRanges();sel.addRange(range);
}catch(_){}
}
};
if(navigator.clipboard&&navigator.clipboard.writeText){
navigator.clipboard.writeText(text).then(function(){done(true);},function(){done(false);});
}else{done(false);}
});
})();";

/// Renders the Docs tab: the rail of pages (see [`render_docs_menu`]) beside
/// the selected page's rendered content.
fn render_docs_tab(requested_slug: Option<&str>) -> maud::Markup {
    let docs = info::list_doc_pages();

    let active_page = requested_slug
        .and_then(info::load_doc_page)
        .unwrap_or(docs[0].page);
    let active_slug = requested_slug
        .filter(|slug| docs.iter().any(|d| d.slug == *slug))
        .unwrap_or(docs[0].slug);

    let html = info::render_page_html(&active_page);

    maud::html! {
        div class="twig-rail-shell" id="docs-container" {
            (render_docs_menu(&docs, active_slug))
            div class="twig-rail-body twig-stack" {
                (render_copy_bar(&active_page))
                div class="twig-md twig-md--prose" {
                    (maud::PreEscaped(html))
                }
            }
            script { (maud::PreEscaped(DOCS_SCRIPT)) }
        }
    }
}

fn render_about_tab() -> maud::Markup {
    let html = info::render_page_html(&info::ABOUT_PAGE);

    maud::html! {
        div class="twig-stack" id="docs-container" {
            h2 class="twig-title" { (info::ABOUT_PAGE.title) }
            (render_copy_bar(&info::ABOUT_PAGE))
            div class="twig-md twig-md--prose" {
                (maud::PreEscaped(html))
            }
            script { (maud::PreEscaped(DOCS_SCRIPT)) }
        }
    }
}

fn render_page(active: Tab, body: &maud::Markup) -> maud::Markup {
    maud::html! {
        (render_tabs(active))
        (body)
    }
}

#[get("/_info")]
pub async fn index(
    req: HttpRequest,
    auth_state: web::Data<TwigContext>,
    query: web::Query<Query>,
) -> AwResult<maud::Markup> {
    let username = get_username_from_request(&req, &auth_state).await;
    let active = Tab::from_query(query.tab.as_deref());

    let body = match active {
        Tab::Docs => render_docs_tab(query.page.as_deref()),
        Tab::About => render_about_tab(),
    };

    Ok(render_layout(
        &render_page(active, &body),
        username.as_deref(),
        Some("Docs"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classes_in(html: &str) -> Vec<String> {
        let marker = "class=\"";
        html.match_indices(marker)
            .flat_map(|(start, _)| {
                let rest = &html[start + marker.len()..];
                let end = rest.find('"').expect("class attribute must be closed");
                rest[..end]
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn anchor_containing<'a>(html: &'a str, needle: &str) -> &'a str {
        html.split("<a ")
            .find(|anchor| anchor.contains(needle))
            .unwrap_or_else(|| panic!("expected an anchor for {needle}\n{html}"))
    }

    /// Markdown source as maud escapes it into the page, so a test can assert
    /// the copy source really carries the page's own words.
    fn escaped(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }

    #[test]
    fn test_tab_from_query_defaults_to_docs() {
        assert!(matches!(Tab::from_query(None), Tab::Docs));
        assert!(matches!(Tab::from_query(Some("bogus")), Tab::Docs));
        assert!(matches!(Tab::from_query(Some("docs")), Tab::Docs));
    }

    #[test]
    fn test_tab_from_query_about() {
        assert!(matches!(Tab::from_query(Some("about")), Tab::About));
    }

    #[test]
    fn test_render_tabs_is_a_labelled_navigation_landmark() {
        let html = render_tabs(Tab::About).into_string();
        assert!(
            html.starts_with("<nav class=\"twig-tabs\" aria-label=\"Docs views\">"),
            "tabs are a labelled navigation landmark: {html}"
        );
        assert_eq!(html.matches("class=\"twig-tab\"").count(), 2, "{html}");
    }

    #[test]
    fn test_render_tabs_marks_only_the_active_tab() {
        let html = render_tabs(Tab::About).into_string();
        assert!(
            anchor_containing(&html, "tab=about").contains("aria-current=\"page\""),
            "the active tab carries aria-current: {html}"
        );
        assert!(
            !anchor_containing(&html, "tab=docs").contains("aria-current"),
            "the inactive tab carries no aria-current: {html}"
        );
    }

    #[test]
    fn test_render_docs_menu_is_a_labelled_rail() {
        let docs = info::list_doc_pages();
        let html = render_docs_menu(&docs, docs[0].slug).into_string();
        assert!(
            html.starts_with(
                "<nav class=\"twig-rail twig-rail--docs\" aria-label=\"Documentation\">"
            ),
            "the docs menu is a labelled rail landmark: {html}"
        );
        assert!(
            html.contains("<p class=\"twig-eyebrow\">Self-hosting</p>"),
            "section labels are eyebrow paragraphs, not headings: {html}"
        );
    }

    #[test]
    fn test_render_docs_menu_marks_only_the_active_page() {
        let docs = info::list_doc_pages();
        let html = render_docs_menu(&docs, docs[1].slug).into_string();
        assert!(html.contains(&format!("page={}", docs[0].slug)), "{html}");
        assert!(
            anchor_containing(&html, &format!("page={}", docs[1].slug))
                .contains("aria-current=\"page\""),
            "the active rail item carries aria-current: {html}"
        );
        assert!(
            !anchor_containing(&html, &format!("page={}", docs[0].slug)).contains("aria-current"),
            "inactive rail items carry no aria-current: {html}"
        );
    }

    #[test]
    fn test_render_docs_menu_shows_section_heading_once() {
        let docs = info::list_doc_pages();
        let html = render_docs_menu(&docs, docs[0].slug).into_string();
        assert_eq!(html.matches("Self-hosting").count(), 1);
    }

    #[test]
    fn test_render_docs_tab_falls_back_to_first_page_on_unknown_slug() {
        let docs = info::list_doc_pages();
        let html = render_docs_tab(Some("does-not-exist")).into_string();
        assert!(html.contains(docs[0].page.title));
        assert!(
            anchor_containing(&html, &format!("page={}", docs[0].slug))
                .contains("aria-current=\"page\""),
            "the fallback page is the marked rail item: {html}"
        );
    }

    #[test]
    fn test_render_docs_tab_pairs_the_rail_with_measured_prose() {
        let html = render_docs_tab(None).into_string();
        for hook in [
            "class=\"twig-rail-shell\"",
            "class=\"twig-rail-body twig-stack\"",
            "<h1>API</h1>",
            "class=\"twig-md twig-md--prose\"",
        ] {
            assert!(html.contains(hook), "missing {hook}: {html}");
        }
    }

    #[test]
    fn test_render_about_tab_contains_about_content() {
        let html = render_about_tab().into_string();
        assert!(html.contains("self-hosted Git server"));
        assert!(
            html.contains("<h2 class=\"twig-title\">About</h2>"),
            "{html}"
        );
        assert!(html.contains("class=\"twig-md twig-md--prose\""), "{html}");
    }

    #[test]
    fn test_every_doc_page_offers_a_copy_button_carrying_its_markdown() {
        for doc in info::list_doc_pages() {
            let html = render_docs_tab(Some(doc.slug)).into_string();
            let buttons = html
                .split("<button")
                .filter(|element| element.contains("data-twig-copy-doc"))
                .count();
            assert_eq!(
                buttons, 1,
                "one copy button per doc page, {} has {buttons}: {html}",
                doc.slug
            );
            assert!(
                html.contains(&escaped(doc.page.content)),
                "the copy source must be {}'s own markdown: {html}",
                doc.slug
            );
            assert!(
                html.contains("data-twig-doc-source"),
                "the source must be readable by the copy script: {html}"
            );
        }
    }

    #[test]
    fn test_about_tab_offers_the_same_copy_button() {
        let html = render_about_tab().into_string();
        assert!(html.contains("data-twig-copy-doc"), "{html}");
        assert!(
            html.contains(&escaped(info::ABOUT_PAGE.content)),
            "the copy source must be the about page's markdown: {html}"
        );
    }

    #[test]
    fn test_copy_source_is_hidden_but_not_removed() {
        let html = render_copy_bar(&info::ABOUT_PAGE).into_string();
        assert!(
            html.starts_with("<header class=\"twig-docbar\">"),
            "the copy control heads the page: {html}"
        );
        assert!(
            html.contains("<pre class=\"twig-sr\" data-twig-doc-source>"),
            "the source is off-screen rather than display:none: {html}"
        );
        assert!(html.contains("Copy page"), "{html}");
    }

    #[test]
    fn test_copy_script_uses_the_clipboard_with_a_selection_fallback() {
        assert!(
            DOCS_SCRIPT.contains("navigator.clipboard.writeText"),
            "copying goes through the async clipboard API: {DOCS_SCRIPT}"
        );
        assert!(
            DOCS_SCRIPT.contains("document.createRange"),
            "a rejected copy still selects the source: {DOCS_SCRIPT}"
        );
        for body in [render_docs_tab(None), render_about_tab()] {
            let html = body.into_string();
            assert!(
                html.contains("getElementById('docs-container')"),
                "both tabs are wired by the same script: {html}"
            );
        }
    }

    #[test]
    fn test_page_carries_no_outgoing_classes_or_inline_styles() {
        for body in [render_docs_tab(None), render_about_tab()] {
            let html = render_page(Tab::Docs, &body).into_string();
            assert!(!html.contains("style="), "no inline styles: {html}");
            assert!(!html.contains("markdown-body"), "{html}");
            for class in classes_in(&html) {
                assert!(
                    class.starts_with("twig-") || class.starts_with("language-"),
                    "non design-system class {class:?} on /_info: {html}"
                );
            }
        }
    }

    #[test]
    fn test_query_deserialization() {
        let query: Query = serde_urlencoded::from_str("tab=about&page=x").unwrap();
        assert_eq!(query.tab.as_deref(), Some("about"));
        assert_eq!(query.page.as_deref(), Some("x"));
    }
}
