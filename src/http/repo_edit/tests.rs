use super::{WorkspaceDraft, ZERO_OID, encode_file_path, pages};

#[test]
fn workspace_shell_loads_the_file_tree_through_htmx() {
    let draft = WorkspaceDraft {
        base_head: ZERO_OID.to_string(),
        active_path: None,
        tree_path: String::new(),
        changes: Vec::new(),
    };
    let html = pages::workspace_shell("acme", "notes", None, &draft, true).into_string();

    assert!(html.contains("Repository workspace"), "{html}");
    assert!(html.contains("hx-get=\"/acme/notes/edit/tree/\""), "{html}");
    assert!(html.contains("hx-trigger=\"load\""), "{html}");
    assert!(html.contains("hx-target=\"#repo-edit-tree\""), "{html}");
    assert!(
        html.contains("aria-label=\"New file\""),
        "the compact create-file control retains its accessible name: {html}"
    );
    assert!(html.contains("twig-repo-new-file-symbol"), "{html}");
    assert!(
        html.contains("aria-label=\"Repository navigation\""),
        "the file tree is in the persistent navigation rail: {html}"
    );
    assert!(
        html.contains("aria-label=\"Editor\""),
        "the editor canvas is separately labelled: {html}"
    );
    assert!(
        html.contains("id=\"repo-edit-changes-panel\""),
        "pending changes remain available in the workspace: {html}"
    );
    let navigation = html.find("aria-label=\"Repository navigation\"").unwrap();
    let changes = html.find("id=\"repo-edit-changes-panel\"").unwrap();
    let editor = html.find("aria-label=\"Editor\"").unwrap();
    assert!(
        navigation < editor && editor < changes,
        "file navigation, editor, and pending changes occupy left-to-right workspace columns: {html}"
    );
    assert!(
        html.contains("hx-action=\"/acme/notes/edit/commit\""),
        "{html}"
    );
    assert!(html.contains("hx-method=\"POST\""), "{html}");
    assert!(html.contains("hx-trigger=\"submit\""), "{html}");
    assert!(
        !html.contains("Run"),
        "repository code must not be run: {html}"
    );
}

#[test]
fn workspace_paths_are_encoded_for_query_and_route_components() {
    assert_eq!(
        encode_file_path("docs/my file #1.md"),
        "docs/my%20file%20%231.md"
    );
    assert_eq!(encode_file_path("src/name?.rs"), "src/name%3F.rs");
}

#[test]
fn editor_operation_forms_override_implicit_get_inheritance() {
    let html =
        pages::file_editor("acme", "notes", "README.md", "# Notes\n", true, true).into_string();

    assert!(
        html.contains("hx-action=\"/acme/notes/edit/operation\""),
        "{html}"
    );
    assert!(html.contains("hx-method=\"POST\""), "{html}");
    assert!(html.contains("hx-trigger=\"submit\""), "{html}");
}
