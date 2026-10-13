use super::*;

#[test]
fn test_markdown_controls_are_rendered_only_for_content_files() {
    let source = "# A nested page\n\nIts own source.\n";
    let content = render_content_file("acme", "my-project", "docs/content.md", source.as_bytes())
        .into_string();
    let headings = paper_headings("paper/01.md", source);
    let paper =
        render_paper_content_only("acme", "my-project", "paper/01.md", Some(source), &headings)
            .into_string();

    assert!(
        content.contains("/acme/my-project/raw/docs/content.md")
            && content.contains("data-twig-copy-markdown")
            && content.contains("Its own source."),
        "Content exposes source actions for its Markdown file: {content}"
    );
    assert!(
        !paper.contains("/raw/") && !paper.contains("data-twig-copy-markdown"),
        "Paper pages do not expose Content-only source actions: {paper}"
    );
}
#[test]
fn test_render_repo_auth_error() {
    let req = actix_web::test::TestRequest::default().to_http_request();
    let html = render_repo_auth_error(&req, "acme/secret").into_string();
    assert!(html.contains("Not logged in. Please log in first."));
    assert!(html.contains("href=\"/auth/login\""));
    assert!(html.contains("<title>acme/secret · Twig</title>"));

    let htmx_req = actix_web::test::TestRequest::default()
        .insert_header(("HX-Request", "true"))
        .to_http_request();
    let htmx_html = render_repo_auth_error(&htmx_req, "acme/secret").into_string();
    assert!(htmx_html.contains("Not logged in. Please log in first."));
    assert!(!htmx_html.contains("<!DOCTYPE html>"));
}
#[test]
fn test_repo_markup_uses_only_twig_design_system_classes() {
    for (surface, html) in representative_markup() {
        let classes = classes_in(&html);
        assert!(
            !classes.is_empty(),
            "{surface} should carry classes: {html}"
        );
        for class in classes {
            assert!(
                class.starts_with("twig-"),
                "non design-system class {class:?} in {surface}: {html}"
            );
        }
    }
}
#[test]
fn test_repo_markup_carries_no_inline_presentation() {
    for (surface, html) in representative_markup() {
        assert!(
            !html.contains("style=\""),
            "no inline style attributes in {surface}: {html}"
        );
        assert!(
            !html.contains("<style"),
            "presentation CSS lives in twig.css, not in {surface}: {html}"
        );
        for class in classes_in(&html) {
            for outgoing in ["tf-", "markdown-body", "white-", "lh-copy", "no-underline"] {
                assert!(
                    !class.contains(outgoing),
                    "outgoing class {class:?} still in {surface}: {html}"
                );
            }
        }
    }
}
