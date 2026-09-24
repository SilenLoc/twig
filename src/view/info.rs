//! The "Information" page at `/_info`: tabs for bundled documentation and an
//! about page, both sourced from hardcoded data compiled into the binary
//! (see `crate::info`). Registered before `view::namespace::handler`, whose
//! `/{namespace}` pattern would otherwise swallow `/_info`.
//!
//! There is no editing here — content is fixed at build time. See
//! `crate::info` to change it.

use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use serde::Deserialize;

use super::render_layout;
use super::session_auth::get_username_from_request;
use crate::auth::FigContext;
use crate::info::{self, DocPage};

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
        nav class="fig-tabs" aria-label="Information views" {
            @for (tab, label) in tabs {
                a
                    class="fig-tab"
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
        nav class="fig-rail fig-rail--docs" aria-label="Documentation" {
            @for (doc, is_new_section) in docs.iter().zip(&show_heading) {
                @if *is_new_section {
                    p class="fig-eyebrow" { (doc.page.section.unwrap_or_default()) }
                }
                a
                    class="fig-rail-item"
                    aria-current=[(doc.slug == active_slug).then_some("page")]
                    href=(format!("/_info?tab=docs&page={}", doc.slug))
                {
                    (doc.page.title)
                }
            }
        }
    }
}

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
        div class="fig-rail-shell" {
            (render_docs_menu(&docs, active_slug))
            div class="fig-rail-body fig-stack" {
                h2 class="fig-title" { (active_page.title) }
                div class="fig-md fig-md--prose" {
                    (maud::PreEscaped(html))
                }
            }
        }
    }
}

fn render_about_tab() -> maud::Markup {
    let html = info::render_page_html(&info::ABOUT_PAGE);

    maud::html! {
        div class="fig-stack" {
            h2 class="fig-title" { (info::ABOUT_PAGE.title) }
            div class="fig-md fig-md--prose" {
                (maud::PreEscaped(html))
            }
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
    auth_state: web::Data<FigContext>,
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
        Some("Information"),
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
            html.starts_with("<nav class=\"fig-tabs\" aria-label=\"Information views\">"),
            "tabs are a labelled navigation landmark: {html}"
        );
        assert_eq!(html.matches("class=\"fig-tab\"").count(), 2, "{html}");
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
                "<nav class=\"fig-rail fig-rail--docs\" aria-label=\"Documentation\">"
            ),
            "the docs menu is a labelled rail landmark: {html}"
        );
        assert!(
            html.contains("<p class=\"fig-eyebrow\">Self-hosting</p>"),
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
            "class=\"fig-rail-shell\"",
            "class=\"fig-rail-body fig-stack\"",
            "<h2 class=\"fig-title\">",
            "class=\"fig-md fig-md--prose\"",
        ] {
            assert!(html.contains(hook), "missing {hook}: {html}");
        }
    }

    #[test]
    fn test_render_about_tab_contains_about_content() {
        let html = render_about_tab().into_string();
        assert!(html.contains("self-hosted Git server"));
        assert!(
            html.contains("<h2 class=\"fig-title\">About</h2>"),
            "{html}"
        );
        assert!(html.contains("class=\"fig-md fig-md--prose\""), "{html}");
    }

    #[test]
    fn test_page_carries_no_outgoing_classes_or_inline_styles() {
        for body in [render_docs_tab(None), render_about_tab()] {
            let html = render_page(Tab::Docs, &body).into_string();
            assert!(!html.contains("style="), "no inline styles: {html}");
            assert!(!html.contains("markdown-body"), "{html}");
            for class in classes_in(&html) {
                assert!(
                    class.starts_with("fig-") || class.starts_with("language-"),
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
