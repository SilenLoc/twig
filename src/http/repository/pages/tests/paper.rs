use super::*;

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
