use super::*;

#[test]
fn test_parent_path() {
    assert_eq!(parent_path(""), "");
    assert_eq!(parent_path("README.md"), "");
    assert_eq!(parent_path("src/main.rs"), "src");
    assert_eq!(parent_path("a/b/c.txt"), "a/b");
}

#[test]
fn test_breadcrumb_segments() {
    let segs = breadcrumb_segments("src/git");
    assert_eq!(
        segs,
        vec![
            ("src".to_string(), "src".to_string(), false),
            ("git".to_string(), "src/git".to_string(), true),
        ]
    );

    let segs = breadcrumb_segments("");
    assert!(segs.is_empty());
}

#[test]
fn test_content_rows_mark_directories_with_a_trailing_slash() {
    let entries = vec![entry("src", true), entry("README.md", false)];
    let html = render_content_dir("acme", "my-project", "", &entries).into_string();

    assert!(
        html.contains(r#"<a class="twig-row" href="/acme/my-project/content/src""#),
        "directories keep the primary identifier treatment: {html}"
    );
    assert!(
        html.contains(r#"<span class="twig-row-id">src/</span>"#),
        "the trailing slash is the whole directory affordance: {html}"
    );
    assert!(
        html.contains(
            r#"<a class="twig-row twig-row--file" href="/acme/my-project/content/README.md""#
        ),
        "files drop to the secondary identifier treatment: {html}"
    );
    assert_eq!(
        count_of(&html, "hx-target=\"#tab-content\""),
        2,
        "one htmx row per entry; the root trail is the current page, not a link: {html}"
    );
    assert!(html.contains("twig-panel twig-panel--flush"), "{html}");
    assert!(html.contains("class=\"twig-list\""), "{html}");
}

#[test]
fn test_content_dir_offers_a_parent_row_below_the_root() {
    let entries = vec![entry("main.rs", false)];
    let html = render_content_dir("acme", "my-project", "src", &entries).into_string();
    assert!(
            html.contains(r##"href="/acme/my-project/content" hx-get="/acme/my-project/content" hx-target="#tab-content" hx-push-url="/acme/my-project/content""##),
            "the parent row climbs back to the tab route: {html}"
        );
    assert!(
        html.contains(r#"<span class="twig-row-id">..</span>"#),
        "{html}"
    );
}

#[test]
fn test_content_breadcrumbs_mark_the_final_segment_as_current() {
    let html = render_content_breadcrumbs("acme", "my-project", "src/git").into_string();
    assert!(
        html.starts_with(r#"<nav class="twig-crumbs twig-crumbs--path" aria-label="File path">"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<span class="twig-crumb-sep" aria-hidden="true">/</span>"#),
        "separators are decorative: {html}"
    );
    assert!(
        html.contains(r#"<span aria-current="page">git</span>"#),
        "{html}"
    );
    assert_eq!(count_of(&html, "aria-current=\"page\""), 1, "{html}");
    assert!(
            html.contains(r##"hx-get="/acme/my-project/content/src" hx-target="#tab-content" hx-push-url="/acme/my-project/content/src""##),
            "intermediate segments stay htmx links: {html}"
        );

    let root = render_content_breadcrumbs("acme", "my-project", "").into_string();
    assert!(
        root.contains(r#"<span aria-current="page">my-project</span>"#),
        "at the root the repository itself is the current segment: {root}"
    );
}

#[test]
fn test_content_view_reports_missing_paths_and_binary_files() {
    let html = render_content_view("acme", "my-project", "nope.txt", &[], None).into_string();
    assert!(
        html.contains("Path not found."),
        "the rejection wording is preserved verbatim: {html}"
    );
    assert!(html.contains("twig-empty"), "{html}");

    let html = render_content_file("acme", "my-project", "logo.png", &[0, 1, 2]).into_string();
    assert!(html.contains(">BINARY FILE<"), "{html}");
    assert!(!html.contains("Edit file"), "{html}");
    assert!(
        html.contains("Binary file (3 bytes). Not displayed."),
        "{html}"
    );

    let html = render_content_file("acme", "my-project", "main.rs", b"fn main() {}").into_string();
    assert!(
        html.contains(r#"<pre class="twig-code" tabindex="0" aria-label="main.rs contents">"#),
        "{html}"
    );
    assert!(html.contains("/acme/my-project/edit/main.rs"), "{html}");
    assert!(html.contains(">Edit file</a>"), "{html}");
    assert!(
        html.contains(r#"<span aria-current="page">main.rs</span>"#),
        "the filename remains in the inner breadcrumb: {html}"
    );
    assert!(
        !html.contains(r#"<h2 class="twig-section">main.rs</h2>"#),
        "the file is not repeated as a title: {html}"
    );
    assert!(
        html.contains("<span class=\"twig-syn-keyword\">fn</span>"),
        "Rust source is tokenized while remaining visible: {html}"
    );

    let html = render_content_file("acme", "my-project", "docs/a.md", b"[b](c.md)").into_string();
    assert!(
        html.contains(r#"<div class="twig-md twig-md--boxed">"#),
        "markdown files keep the boxed markdown surface: {html}"
    );
    assert!(
        html.contains("/acme/my-project/md/docs/c.md"),
        "link rewriting still resolves against the containing directory: {html}"
    );
    assert!(html.contains("/acme/my-project/edit/docs/a.md"), "{html}");

    let invalid_utf8 = render_content_file("acme", "my-project", "raw.txt", &[0xff]).into_string();
    assert!(!invalid_utf8.contains("Edit file"), "{invalid_utf8}");
    assert!(
        !html.contains("https://"),
        "no remote editor scripts: {html}"
    );
}
