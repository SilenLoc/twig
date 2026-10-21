use super::*;
use crate::git::bare::{Commit, TreeEntry};
use crate::http::view::test_util::{classes_in, index_of};
use std::collections::HashMap;

fn base_frame() -> TabFrame<'static> {
    TabFrame {
        namespace: "acme",
        repo: "my-project",
        username: None,
        page_title: "acme/my-project",
        can_manage_hooks: false,
        twig_error: None,
        twig_filename: None,
        tabs_config: &[],
        optional_tabs: Vec::new(),
    }
}

/// A frame with the given optional tabs, for tab-bar tests.
fn frame_with(optional_tabs: &[OptionalTab]) -> TabFrame<'static> {
    TabFrame {
        optional_tabs: optional_tabs.to_vec(),
        ..base_frame()
    }
}
fn slides(count: usize) -> Vec<PresentSlide> {
    (0..count)
        .map(|i| PresentSlide {
            html: format!("<h1>Slide {i}</h1>"),
        })
        .collect()
}
fn entry(name: &str, is_dir: bool) -> TreeEntry {
    TreeEntry {
        name: name.to_string(),
        path: name.to_string(),
        is_dir,
    }
}
fn count_of(html: &str, needle: &str) -> usize {
    html.matches(needle).count()
}
fn tab_labels(html: &str) -> Vec<String> {
    html.match_indices("class=\"twig-tab\"")
        .map(|(start, _)| {
            let rest = &html[start..];
            let open = rest.find('>').expect("tab tag must be closed");
            let close = rest.find("</a>").expect("tab must be closed");
            rest[open + 1..close].to_owned()
        })
        .collect()
}
/// Representative markup from every surface this file renders. Each entry is
/// a full page or tab body, so the sweeps below cover the whole view.
fn representative_markup() -> Vec<(&'static str, String)> {
    let deck = slides(2);
    let files = vec!["README.md".to_string(), "docs/guide.md".to_string()];
    let entries = vec![entry("src", true), entry("README.md", false)];
    let commits = [Commit::new(
        "a1b2c3d4".to_string(),
        "silen".to_string(),
        chrono::Utc::now(),
        "Initial commit".to_string(),
    )];

    vec![
        (
            "repo page",
            render_tab_shell(&base_frame(), "commits", &maud::html! {}).into_string(),
        ),
        (
            "markdown tab",
            render_markdown_view(
                "acme",
                "my-project",
                "README.md",
                Some("# Hi\n\nHello."),
                &files,
            )
            .into_string(),
        ),
        ("commits tab", render_commits_view(&commits).into_string()),
        (
            "config tab",
            render_config_view(Some("[present]"), Some(".twig.toml")).into_string(),
        ),
        (
            "license tab",
            render_license_view(Some("<p>MIT</p>")).into_string(),
        ),
        (
            "content directory",
            render_content_dir("acme", "my-project", "src", &entries).into_string(),
        ),
        (
            "content file",
            render_content_file("acme", "my-project", "main.rs", b"fn main() {}").into_string(),
        ),
        (
            "present tab",
            render_present_view("acme", "my-project", &deck).into_string(),
        ),
        (
            "present print",
            render_pdf_document("acme", "my-project", &deck, Some("<p>MIT</p>")).into_string(),
        ),
        (
            "empty present print",
            render_pdf_document("acme", "my-project", &[], None).into_string(),
        ),
        (
            "empty present tab",
            render_present_view("acme", "my-project", &[]).into_string(),
        ),
        (
            "paper tab",
            render_paper_view(
                "acme",
                "my-project",
                "paper",
                &["paper/01.md".to_string(), "paper/02.md".to_string()],
            )
            .into_string(),
        ),
        (
            "empty paper tab",
            render_paper_view("acme", "my-project", "paper", &[]).into_string(),
        ),
        (
            "config error",
            render_config_error(".twig.toml", "TOML parse error at line 1, column 1").into_string(),
        ),
        (
            "git error",
            render_git_error(&git2::Error::from_str("boom")).into_string(),
        ),
        (
            "repo auth error",
            render_repo_auth_error(
                &actix_web::test::TestRequest::default().to_http_request(),
                "acme/secret",
            )
            .into_string(),
        ),
    ]
}

#[path = "tests/content.rs"]
mod content;
#[path = "tests/hooks.rs"]
mod hook_tests;
#[path = "tests/markdown.rs"]
mod markdown;
#[path = "tests/markup.rs"]
mod markup;
#[path = "tests/paper.rs"]
mod paper;
#[path = "tests/presentation.rs"]
mod presentation;
#[path = "tests/scripts.rs"]
mod scripts;
#[path = "tests/tabs.rs"]
mod tabs;
