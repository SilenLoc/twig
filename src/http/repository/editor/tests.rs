use super::*;

#[test]
fn editor_page_includes_source_and_commit_precondition() {
    let html = render_editor_page(
        "acme",
        "twig",
        "docs/guide.md",
        "# Guide\n",
        git2::Oid::ZERO_SHA1,
        true,
        true,
    )
    .into_string();

    assert!(html.contains("id=\"repository-editor-form\""));
    assert!(
        html.contains("name=\"expected_head\" value=\"0000000000000000000000000000000000000000\"")
    );
    assert!(html.contains("# Guide\n"));
    assert!(html.contains("Markdown source"));
    assert!(html.contains("data-editor-reload"));
    assert!(html.contains("data-editor-conflict-copy"));
    assert!(html.contains(&crate::http::assets::url("quill.js")));
    assert!(html.contains(&crate::http::assets::url("marked.js")));
    assert!(html.contains(&crate::http::assets::url("purify.js")));
    assert!(html.contains(&crate::http::assets::url("turndown.js")));
    assert!(html.contains(&crate::http::assets::url("quill.snow.css")));
    assert!(
        !html.contains("https://"),
        "editor dependencies must be local"
    );
    assert!(html.contains("Commit changes"));
}

#[test]
fn source_page_does_not_offer_rich_controls() {
    let html = render_editor_page(
        "acme",
        "twig",
        "src/main.rs",
        "fn main() {}\n",
        git2::Oid::ZERO_SHA1,
        false,
        true,
    )
    .into_string();

    assert!(html.contains("Source editor"));
    assert!(!html.contains("Markdown source"));
    assert!(html.contains("fn main() {}"));
}

#[test]
fn file_paths_are_encoded_without_losing_directory_separators() {
    assert_eq!(
        encode_file_path("docs/my file #1.md"),
        "docs/my%20file%20%231.md"
    );
    assert_eq!(encode_file_path("src/name?.rs"), "src/name%3F.rs");
}

#[test]
fn conflict_copy_names_include_a_safe_user_timestamp_and_collision_suffix() {
    assert_eq!(
        conflict_copy_candidate("docs/README.md", "alice@example.com", "20261010-123456", 0,)
            .unwrap(),
        "docs/alice-example-com-20261010-123456-README.md"
    );
    assert_eq!(
        conflict_copy_candidate("docs/README.md", "alice@example.com", "20261010-123456", 1,)
            .unwrap(),
        "docs/alice-example-com-20261010-123456-2-README.md"
    );
    assert!(conflict_copy_candidate("../README.md", "alice", "now", 0).is_err());
}

#[test]
fn conflict_copy_filename_truncation_preserves_utf8_and_component_limit() {
    let source = format!("docs/{}.md", "語".repeat(200));
    let candidate = conflict_copy_candidate(&source, "alice", "20261010-123456", 0).unwrap();
    let name = candidate.rsplit('/').next().unwrap();
    assert!(name.len() <= 255);
    assert!(std::str::from_utf8(name.as_bytes()).is_ok());
}
