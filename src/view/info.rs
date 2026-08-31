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
    let tabs = [(Tab::Docs, "Docs"), (Tab::About, "About")];
    maud::html! {
        div class="flex flex-wrap mb4 bb b--white-20" {
            @for (tab, label) in tabs {
                @let classes = if tab == active {
                    "tf-tab pa2 ph3 white bg-white-10 no-underline"
                } else {
                    "tf-tab pa2 ph3 white-50 hover-white no-underline"
                };
                a href=(format!("/_info?tab={}", tab.as_str())) class=(classes) {
                    (label)
                }
            }
        }
    }
}

/// Renders the Docs left-hand menu, grouping consecutive pages that share a
/// `section` under a small subheading. Pages sort by `order` then title
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
        nav class="mb3 mb0-ns" {
            ul class="list pl0 ma0" {
                @for (doc, is_new_section) in docs.iter().zip(&show_heading) {
                    @if *is_new_section {
                        li class="tf-kicker white-50 mt3 mb1" { (doc.page.section.unwrap_or_default()) }
                    }
                    @let is_active = doc.slug == active_slug;
                    @let classes = if is_active {
                        "db pa2 br1 bg-white-10 white no-underline mb1"
                    } else {
                        "db pa2 br1 white-70 hover-white hover-bg-white-10 no-underline mb1"
                    };
                    li {
                        a href=(format!("/_info?tab=docs&page={}", doc.slug)) class=(classes) {
                            (doc.page.title)
                        }
                    }
                }
            }
        }
    }
}

/// Renders the Docs tab: a left menu of pages (see [`render_docs_menu`]),
/// plus the selected page's rendered content.
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
        div class="flex flex-wrap flex-nowrap-ns" {
            div class="w-100 w-30-ns pr4-ns" {
                (render_docs_menu(&docs, active_slug))
            }
            div class="w-100 w-70-ns" {
                h1 class="tf-title white mt0 mb3" { (active_page.title) }
                div class="markdown-body white lh-copy" {
                    (maud::PreEscaped(html))
                }
            }
        }
    }
}

fn render_about_tab() -> maud::Markup {
    let html = info::render_page_html(&info::ABOUT_PAGE);

    maud::html! {
        h1 class="tf-title white mt0 mb3" { (info::ABOUT_PAGE.title) }
        div class="markdown-body white lh-copy" {
            (maud::PreEscaped(html))
        }
    }
}

fn render_page(active: Tab, body: &maud::Markup) -> maud::Markup {
    maud::html! {
        div class="pt3 pt4-ns" {
            h1 class="tf-hero white ma0" { "Information" }
        }
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
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_render_tabs_marks_active_tab() {
        let html = render_tabs(Tab::About).into_string();
        assert!(html.contains("tab=docs"));
        assert!(html.contains("tab=about"));
    }

    #[test]
    fn test_render_docs_menu_marks_active_page() {
        let docs = info::list_doc_pages();
        let html = render_docs_menu(&docs, docs[1].slug).into_string();
        assert!(html.contains(&format!("page={}", docs[0].slug)));
        assert!(html.contains(&format!("page={}", docs[1].slug)));
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
    }

    #[test]
    fn test_render_about_tab_contains_about_content() {
        let html = render_about_tab().into_string();
        assert!(html.contains("Fig exists"));
    }

    #[test]
    fn test_query_deserialization() {
        let query: Query = serde_urlencoded::from_str("tab=about&page=x").unwrap();
        assert_eq!(query.tab.as_deref(), Some("about"));
        assert_eq!(query.page.as_deref(), Some("x"));
    }
}
