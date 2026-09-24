//! The "Information" section: hardcoded documentation and about content,
//! shown at `/_info`.
//!
//! Everything here is compiled into the binary — plain Rust data plus
//! `include_str!` for the markdown bodies under `docs/`. There is no
//! runtime editing, no database row, and no git repository backing this;
//! changing it means changing this file (or the `docs/*.md` files it
//! embeds) and rebuilding.
//!
//! Doc pages carry an `order` used to sort the left-hand menu, and an
//! optional `section` used to group related pages under a subheading within
//! that menu (for example, "Self-hosting").

/// A single content page: a doc page or the about page. `content` is the
/// markdown source itself (already resolved at compile time via
/// `include_str!` where a page's body lives in its own file).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Page {
    pub title: &'static str,
    pub order: i64,
    pub section: Option<&'static str>,
    pub content: &'static str,
}

/// A doc page paired with the URL-safe slug used to link to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocPage {
    pub slug: &'static str,
    pub page: Page,
}

/// Renders a page's markdown content to HTML.
pub fn render_page_html(page: &Page) -> String {
    crate::md::process_markdown(page.content, &std::collections::HashMap::new())
}

/// The about page.
pub const ABOUT_PAGE: Page = Page {
    title: "About",
    order: 0,
    section: None,
    content: r"Fig is a self-hosted Git server and web interface for browsing and
managing repositories. It combines Git smart HTTP, account and namespace
management, and repository browsing in a single small application.

The web interface includes Markdown documentation, a file browser, commit
history, and optional repository slide presentations. See the Docs tab for
Git usage, web UI details, and server configuration.",
};

/// All doc pages, in a fixed order (already sorted by `order`, but see
/// [`list_doc_pages`] which sorts anyway so this list doesn't have to stay
/// perfectly ordered by hand).
const DOCS_PAGES: &[DocPage] = &[
    DocPage {
        slug: "git-backend",
        page: Page {
            title: "Git Backend",
            order: 20,
            section: None,
            content: include_str!("../../docs/git-backend.md"),
        },
    },
    DocPage {
        slug: "ui",
        page: Page {
            title: "Web UI",
            order: 30,
            section: None,
            content: include_str!("../../docs/ui.md"),
        },
    },
    DocPage {
        slug: "environment-variables",
        page: Page {
            title: "Environment Variables",
            order: 40,
            section: Some("Self-hosting"),
            content: include_str!("../../docs/environment-variables.md"),
        },
    },
];

/// All doc pages, sorted by `order` then title.
pub fn list_doc_pages() -> Vec<DocPage> {
    let mut pages = DOCS_PAGES.to_vec();
    pages.sort_by(|a, b| {
        a.page
            .order
            .cmp(&b.page.order)
            .then_with(|| a.page.title.cmp(b.page.title))
    });
    pages
}

/// Loads a single doc page by slug.
pub fn load_doc_page(slug: &str) -> Option<Page> {
    DOCS_PAGES
        .iter()
        .find(|doc| doc.slug == slug)
        .map(|doc| doc.page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_about_page_has_content() {
        assert_eq!(ABOUT_PAGE.title, "About");
        assert!(ABOUT_PAGE.content.contains("self-hosted Git server"));
    }

    #[test]
    fn test_list_doc_pages_sorted_by_order() {
        let docs = list_doc_pages();
        assert!(!docs.is_empty());
        let orders: Vec<i64> = docs.iter().map(|d| d.page.order).collect();
        let mut sorted = orders.clone();
        sorted.sort_unstable();
        assert_eq!(orders, sorted);
    }

    #[test]
    fn test_environment_variables_page_is_grouped_under_self_hosting() {
        let page = load_doc_page("environment-variables").expect("page should exist");
        assert_eq!(page.section, Some("Self-hosting"));
        assert!(page.content.contains("PROJECT_ROOT"));
    }

    #[test]
    fn test_load_doc_page_unknown_slug_is_none() {
        assert!(load_doc_page("does-not-exist").is_none());
    }

    #[test]
    fn test_render_page_html_produces_html() {
        let page = load_doc_page("git-backend").unwrap();
        let html = render_page_html(&page);
        assert!(html.contains("<h1"));
    }
}
