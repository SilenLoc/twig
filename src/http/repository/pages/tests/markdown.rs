use super::*;

#[test]
fn test_markdown_to_html() {
    let md = "# Hello\n\nThis is **bold** text.";
    let html = markdown_to_html(md, "test", "repo", "");
    assert!(html.contains("<h1>Hello</h1>"));
    assert!(html.contains("<strong>bold</strong>"));

    let html = markdown_to_html("```rust\nfn main() {}\n```", "test", "repo", "");
    assert!(
        html.contains("<span class=\"twig-syn-keyword\">fn</span>"),
        "{html}"
    );
}

#[test]
fn test_markdown_link_fixing() {
    let md = "[Link](./other.md) and [External](https://example.com)";
    let html = markdown_to_html(md, "ns", "repo", "");
    // Internal markdown links should be fixed and normalized
    assert!(html.contains("/ns/repo/md/other.md"), "{html}");
    // External links should remain unchanged
    assert!(html.contains("https://example.com"));
}

#[test]
fn test_markdown_links_resolve_against_containing_directory() {
    let md = "[Sibling](other.md) [Nested](sub/deep.md) [Up](../top.md)";
    let html = markdown_to_html(md, "ns", "repo", "docs/guide");
    assert!(html.contains("/ns/repo/md/docs/guide/other.md"), "{html}");
    assert!(
        html.contains("/ns/repo/md/docs/guide/sub/deep.md"),
        "{html}"
    );
    assert!(html.contains("/ns/repo/md/docs/top.md"), "{html}");
}

#[test]
fn test_markdown_non_markdown_links_target_content_route() {
    let md = "[Image](logo.png) [Dir](sub/)";
    let html = markdown_to_html(md, "ns", "repo", "foo.bar");
    assert!(html.contains("/ns/repo/content/foo.bar/logo.png"), "{html}");
    assert!(html.contains("/ns/repo/content/foo.bar/sub"), "{html}");
}

#[test]
fn test_markdown_link_preserves_fragment_and_query() {
    let md = "[Anchor](other.md#section) [Query](file.txt?raw=1)";
    let html = markdown_to_html(md, "ns", "repo", "docs");
    assert!(html.contains("/ns/repo/md/docs/other.md#section"), "{html}");
    assert!(
        html.contains("/ns/repo/content/docs/file.txt?raw=1"),
        "{html}"
    );
}

#[test]
fn test_markdown_link_leaves_absolute_and_scheme_links_alone() {
    let md = "[Root](/other) [Mail](mailto:a@b.com) [Frag](#here)";
    let html = markdown_to_html(md, "ns", "repo", "docs");
    assert!(html.contains("href=\"/other\""), "{html}");
    assert!(html.contains("href=\"mailto:a@b.com\""), "{html}");
    assert!(html.contains("href=\"#here\""), "{html}");
}

#[test]
fn test_slide_markdown_anchors_relative_links_to_the_slide_directory() {
    let md = "See [next](second.md), [chart](img/chart.png) and [site](https://example.com).";
    let html = render_slide_markdown(md, "ns", "repo", "slides/first.md", &HashMap::new());
    assert!(html.contains("/ns/repo/md/slides/second.md"), "{html}");
    assert!(
        html.contains("/ns/repo/content/slides/img/chart.png"),
        "{html}"
    );
    assert!(html.contains("href=\"https://example.com\""), "{html}");
}

#[test]
fn test_slide_markdown_substitutes_template_variables() {
    let mut vars = HashMap::new();
    vars.insert("author".to_string(), "Jane".to_string());
    let html = render_slide_markdown(
        "# Talk by {{author}}",
        "ns",
        "repo",
        "slides/intro.md",
        &vars,
    );
    assert!(html.contains("Talk by Jane"), "{html}");
}

#[test]
fn test_resolve_relative_path() {
    assert_eq!(resolve_relative_path("", "a.md"), "a.md");
    assert_eq!(resolve_relative_path("docs", "./a.md"), "docs/a.md");
    assert_eq!(resolve_relative_path("docs/sub", "../a.md"), "docs/a.md");
    assert_eq!(resolve_relative_path("docs", "../../../a.md"), "a.md");
    assert_eq!(resolve_relative_path("docs", "sub/"), "docs/sub");
    assert_eq!(resolve_relative_path("docs", "."), "docs");
}

#[test]
fn test_has_url_scheme() {
    assert!(has_url_scheme("https://example.com"));
    assert!(has_url_scheme("mailto:a@b.com"));
    assert!(!has_url_scheme("docs/a.md"));
    assert!(!has_url_scheme("docs/a:b.md"));
    assert!(!has_url_scheme(":leading"));
    assert!(!has_url_scheme("./a.md"));
}

#[test]
fn test_get_default_markdown_file() {
    let files = vec![
        "docs/guide.md".to_string(),
        "README.md".to_string(),
        "CHANGELOG.md".to_string(),
    ];
    assert_eq!(get_default_markdown_file(&files), Some("README.md"));

    let files = vec!["docs/guide.md".to_string(), "readme.md".to_string()];
    assert_eq!(get_default_markdown_file(&files), Some("readme.md"));

    let files = vec!["guide.md".to_string(), "docs/help.md".to_string()];
    assert_eq!(get_default_markdown_file(&files), Some("guide.md"));

    let files: Vec<String> = vec![];
    assert_eq!(get_default_markdown_file(&files), None);
}

#[test]
fn test_markdown_tables_rendering() {
    let md = "| Header 1 | Header 2 |\n|----------|----------|\n| Cell 1   | Cell 2   |";
    let html = markdown_to_html(md, "test", "repo", "");
    // Tables should be rendered as HTML table elements
    assert!(
        html.contains("<table>"),
        "Expected <table> tag in output: {html}"
    );
    assert!(html.contains("<th>"), "Expected <th> tag in output: {html}");
    assert!(html.contains("<td>"), "Expected <td> tag in output: {html}");
}

#[test]
fn test_markdown_rail_appears_only_for_multiple_files() {
    let one = vec!["README.md".to_string()];
    let html =
        render_markdown_view("acme", "my-project", "README.md", Some("# Hi"), &one).into_string();
    assert!(
        !html.contains("twig-rail-item"),
        "a single markdown file gets no rail: {html}"
    );
    assert!(
        html.contains(r#"<div id="markdown-view" aria-live="polite">"#),
        "{html}"
    );

    let many = vec!["README.md".to_string(), "docs/guide.md".to_string()];
    let html =
        render_markdown_view("acme", "my-project", "README.md", Some("# Hi"), &many).into_string();
    assert!(
        html.contains(r#"<nav class="twig-rail twig-rail--files" aria-label="Markdown files">"#),
        "{html}"
    );
    assert!(
            html.contains(r##"aria-current="page" hx-get="/acme/my-project/md/README.md" hx-target="#markdown-view""##),
            "the open file is the current rail item and keeps its htmx wiring: {html}"
        );
    assert_eq!(count_of(&html, "aria-current=\"page\""), 1, "{html}");
    assert_eq!(count_of(&html, "hx-target=\"#markdown-view\""), 2, "{html}");
}

#[test]
fn test_markdown_content_reports_a_missing_file() {
    let html = render_markdown_content_only("acme", "my-project", "README.md", None).into_string();
    assert!(html.contains("twig-empty"), "{html}");
    assert!(html.contains("File not found or empty."), "{html}");

    let html =
        render_markdown_content_only("acme", "my-project", "README.md", Some("# Hi")).into_string();
    assert!(
        html.contains(r#"<div class="twig-md twig-md--prose">"#),
        "{html}"
    );
}
