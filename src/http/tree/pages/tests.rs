use crate::{db::data::TablePage, git};

use super::*;

#[test]
fn tree_hub_only_renders_enabled_sections() {
    let minimal = render_tree_hub(false, false, false, None).into_string();
    assert!(minimal.contains(">Account</a>"));
    assert!(minimal.contains("href=\"/tree/repositories\">Repository</a>"));
    assert!(minimal.contains("href=\"/tree/namespaces\""));
    assert!(!minimal.contains(">Test</a>"));
    assert!(!minimal.contains(">Data</a>"));

    let enabled = render_tree_hub(true, true, true, Some("data")).into_string();
    assert!(enabled.contains("href=\"/_test\""));
    assert!(enabled.contains("href=\"/tree/namespaces\""));
    assert!(!enabled.contains("aria-current=\"page\" href=\"/tree/repositories\">Repository</a>"));
    assert!(enabled.contains("href=\"/tree/data\""));
    assert!(enabled.contains("aria-current=\"page\" href=\"/tree/data\""));
    assert!(enabled.contains("href=\"/tree/users\">User Management</a>"));
    let repository_active =
        render_tree_hub(false, false, false, Some("repositories")).into_string();
    assert!(
        repository_active
            .contains("aria-current=\"page\" href=\"/tree/repositories\">Repository</a>")
    );
}

#[test]
fn namespace_page_lists_repositories_with_confirmed_namespace_buttons() {
    let html = render_namespaces_page(
        &[(
            "acme".to_string(),
            vec![git::bare::RepoInfo {
                name: "twig".to_string(),
                last_commit_date: None,
                is_private: false,
            }],
        )],
        &["acme".to_string(), "solo".to_string()],
        &["solo".to_string()],
        &["solo".to_string()],
    )
    .into_string();

    assert!(html.contains("<span class=\"twig-row-id\">acme/twig</span>"));
    assert!(
        html.contains(
            "hx-confirm=\"Move 'acme/twig' to the 'solo' namespace? Its URL will change.\""
        )
    );
    assert!(
        html.contains("<button class=\"twig-btn twig-btn--ghost\" type=\"submit\">solo</button>")
    );
    assert!(html.contains("name=\"target_namespace\" value=\"solo\""));
    assert!(!html.contains("<select"));
    assert!(html.contains("Delete Namespace"));
    assert!(!html.contains("NO NAMESPACES"));
    assert!(html.contains("hx-post=\"/settings/delete-namespace/\""));
    assert!(html.contains("hx-confirm=\"Are you sure you want to permanently delete the namespace 'solo'? This cannot be undone.\""));
    assert!(html.contains("<span class=\"twig-row-id\">solo</span>"));
}

#[test]
fn namespace_page_explains_when_no_namespace_can_be_deleted() {
    let html = render_namespaces_page(&[], &[], &[], &[]).into_string();

    assert!(html.contains("Delete Namespace"));
    assert!(html.contains(">NO NAMESPACES<"));
    assert!(html.contains(
        "No namespaces available for deletion. You can only delete namespaces you own that have no repositories."
    ));
    assert!(!html.contains("hx-confirm"));
    assert!(!html.contains("twig-btn--danger"));
}

#[test]
fn namespace_page_lists_owned_namespaces_for_rename_with_the_htmx_contract() {
    let html =
        render_namespace_rename_panel(&["acme".to_string(), "solo".to_string()]).into_string();

    assert!(html.contains(
        "<h2 class=\"twig-eyebrow\" id=\"namespace-rename-heading\">Rename Namespace</h2>"
    ));
    assert!(html.contains("hx-post=\"/settings/rename-namespace\""));
    assert!(html.contains("hx-target=\"#rename-namespace-result\""));
    assert!(html.contains(
        "hx-confirm=\"Rename namespace 'acme'? Every repository URL inside it will change.\""
    ));
    assert!(html.contains("<span class=\"twig-row-id\">acme</span>"));
    assert!(html.contains("<span class=\"twig-row-id\">solo</span>"));
    assert!(html.contains("<input type=\"hidden\" name=\"namespace\" value=\"acme\">"));
    assert!(html.contains(
        "<input class=\"twig-input twig-input--mono\" type=\"text\" name=\"new_name\" required minlength=\"2\" autocomplete=\"off\" spellcheck=\"false\" value=\"acme\">"
    ));
    assert!(
        html.contains(
            "<button class=\"twig-btn twig-btn--primary\" type=\"submit\">Rename</button>"
        )
    );
    assert_eq!(
        html.matches("hx-post=\"/settings/rename-namespace\"")
            .count(),
        2
    );
}

#[test]
fn namespace_rename_panel_empty_state_offers_no_targets() {
    let html = render_namespace_rename_panel(&[]).into_string();

    assert!(html.contains("Rename Namespace"));
    assert!(html.contains(">NO NAMESPACES<"));
    assert!(html.contains("You don't have any namespaces to rename."));
    assert!(!html.contains("hx-confirm"));
    assert!(!html.contains("twig-btn--primary"));
}

#[test]
fn data_page_uses_the_infinite_scroll_placeholder() {
    let page = TablePage {
        columns: vec!["id".to_string()],
        rows: (0..DATA_PAGE_SIZE)
            .map(|index| vec![index.to_string()])
            .collect(),
    };
    let html =
        render_data_page(&["users".to_string()], "users", Some(&page), 0, false).into_string();
    assert!(html.contains("hx-trigger=\"revealed\""));
    assert!(html.contains("hx-swap=\"outerHTML\""));
    assert!(html.contains("/tree/data/rows?table=users&amp;offset=50"));
}
