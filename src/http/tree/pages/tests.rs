use crate::{db::data::TablePage, git};

use super::*;

#[test]
fn tree_hub_only_renders_enabled_sections() {
    let minimal = render_tree_hub(false, false, false, None).into_string();
    assert!(minimal.contains("aria-label=\"Manage sections\""));
    assert!(minimal.contains(">Account</a>"));
    assert!(minimal.contains("href=\"/settings/tokens\">Access tokens</a>"));
    assert!(minimal.contains("href=\"/manage/repositories\">Repositories</a>"));
    assert!(minimal.contains("href=\"/manage/namespaces\">Namespaces</a>"));
    assert!(!minimal.contains(">Users</a>"));
    assert!(!minimal.contains(">Test</a>"));
    assert!(!minimal.contains(">Data</a>"));

    let enabled = render_tree_hub(true, true, true, Some("data")).into_string();
    assert!(enabled.contains("href=\"/_test\">Test</a>"));
    assert!(
        !enabled.contains("aria-current=\"page\" href=\"/manage/repositories\">Repositories</a>")
    );
    assert!(enabled.contains("aria-current=\"page\" href=\"/manage/data\">Data</a>"));
    assert!(enabled.contains("href=\"/manage/users\">Users</a>"));
    let repository_active =
        render_tree_hub(false, false, false, Some("repositories")).into_string();
    assert!(
        repository_active
            .contains("aria-current=\"page\" href=\"/manage/repositories\">Repositories</a>")
    );
    let tokens_active = render_tree_hub(false, false, false, Some("tokens")).into_string();
    assert!(
        tokens_active.contains("aria-current=\"page\" href=\"/settings/tokens\">Access tokens</a>")
    );
}

#[test]
fn namespace_list_is_one_searchable_scrollable_panel() {
    let html = render_namespaces_page(&["acme".to_string(), "solo".to_string()]).into_string();

    assert_eq!(
        html.matches("<section class=\"twig-panel\"").count(),
        1,
        "{html}"
    );
    assert!(!html.contains(">Namespaces</h2>"), "{html}");
    assert!(
        html.contains(
            "<input class=\"twig-input twig-input--mono\" type=\"search\" id=\"namespace-search\""
        ),
        "the list is searchable: {html}"
    );
    assert!(
        html.contains("<div class=\"twig-list twig-list--scroll\">"),
        "the list scrolls: {html}"
    );
    assert!(html.contains("data-namespace-row=\"acme\""), "{html}");
    assert!(html.contains("data-namespace-row=\"solo\""), "{html}");
    assert!(
        html.contains("getElementById('namespace-search')"),
        "{html}"
    );
    assert!(html.contains("class=\"twig-empty-body\" hidden"), "{html}");
}

#[test]
fn namespace_list_rows_link_to_their_settings_page_only() {
    let html = render_namespaces_page(&["acme".to_string()]).into_string();

    assert!(
        html.contains(
            "<a class=\"twig-btn\" href=\"/manage/namespaces/acme/settings\">Settings</a>"
        ),
        "{html}"
    );
    assert!(!html.contains("hx-post"), "{html}");
    assert!(!html.contains("Delete"), "{html}");
    assert!(!html.contains("Rename"), "{html}");
}

#[test]
fn namespace_list_empty_state_offers_no_targets() {
    let html = render_namespaces_page(&[]).into_string();

    assert_eq!(
        html.matches("<div class=\"twig-empty\">").count(),
        1,
        "{html}"
    );
    assert!(html.contains(">NO NAMESPACES<"), "{html}");
    assert!(
        html.contains("You don't have any namespaces to manage."),
        "{html}"
    );
    assert!(!html.contains("<input"), "{html}");
    assert!(!html.contains("hx-post"), "{html}");
}

#[test]
fn namespace_settings_offers_the_opposite_pin_action() {
    let unpinned = render_namespace_settings("acme", false, &[], true, &[], false).into_string();
    assert!(unpinned.contains("Pin to home"), "{unpinned}");
    assert!(
        unpinned.contains("name=\"pinned\" value=\"true\""),
        "{unpinned}"
    );

    let pinned = render_namespace_settings("acme", true, &[], true, &[], false).into_string();
    assert!(pinned.contains("Unpin from home"), "{pinned}");
    assert!(
        pinned.contains("name=\"pinned\" value=\"false\""),
        "{pinned}"
    );
    assert!(
        pinned.contains("hx-post=\"/settings/pin-namespace\""),
        "{pinned}"
    );
}

#[test]
fn namespace_settings_has_rename_move_and_delete_panels() {
    let html = render_namespace_settings(
        "acme",
        false,
        &[git::bare::RepoInfo {
            name: "twig".to_string(),
            last_commit_date: None,
            is_private: false,
        }],
        true,
        &["solo".to_string()],
        true,
    )
    .into_string();

    assert!(
        html.contains("href=\"/manage/namespaces\">Namespaces</a>"),
        "{html}"
    );
    assert!(
        html.contains("<span class=\"twig-row-id\">acme/twig</span>"),
        "{html}"
    );
    assert!(
        html.contains("<button class=\"twig-btn twig-btn--ghost\" type=\"submit\">solo</button>"),
        "{html}"
    );
    assert!(
        html.contains("hx-post=\"/settings/rename-namespace\""),
        "{html}"
    );
    assert!(
        html.contains("hx-target=\"#rename-namespace-result\""),
        "{html}"
    );
    assert!(
        html.contains(
            "hx-confirm=\"Rename namespace 'acme'? Every repository URL inside it will change.\""
        ),
        "{html}"
    );
    assert!(
        html.contains("<input type=\"hidden\" name=\"namespace\" value=\"acme\">"),
        "{html}"
    );
    assert!(html.contains("hx-post=\"/settings/move-repo\""), "{html}");
    assert!(
        html.contains(
            "hx-confirm=\"Move 'acme/twig' to the 'solo' namespace? Its URL will change.\""
        ),
        "{html}"
    );
    assert!(
        html.contains("hx-post=\"/settings/delete-namespace/\""),
        "{html}"
    );
    assert!(
        html.contains("hx-confirm=\"Are you sure you want to permanently delete the namespace 'acme'? This cannot be undone.\""),
        "{html}"
    );
    assert!(!html.contains("<select"), "{html}");
}

#[test]
fn namespace_settings_hides_moves_without_repository_rights() {
    let html = render_namespace_settings("acme", false, &[], false, &["solo".to_string()], true)
        .into_string();

    assert!(!html.contains("Move Repositories"), "{html}");
    assert!(!html.contains("/settings/move-repo"), "{html}");
    assert!(
        html.contains("hx-post=\"/settings/rename-namespace\""),
        "{html}"
    );
    assert!(
        html.contains("hx-post=\"/settings/delete-namespace/\""),
        "{html}"
    );
}

#[test]
fn namespace_settings_explains_move_empty_states() {
    let no_repos = render_namespace_settings("acme", false, &[], true, &["solo".to_string()], true)
        .into_string();
    assert!(
        no_repos.contains("This namespace has no repositories to move."),
        "{no_repos}"
    );
    assert!(!no_repos.contains("/settings/move-repo"), "{no_repos}");

    let no_targets = render_namespace_settings(
        "acme",
        false,
        &[git::bare::RepoInfo {
            name: "twig".to_string(),
            last_commit_date: None,
            is_private: false,
        }],
        true,
        &[],
        true,
    )
    .into_string();
    assert!(
        no_targets.contains("There is no other namespace you can move repositories to."),
        "{no_targets}"
    );
}

#[test]
fn namespace_settings_offers_delete_only_when_empty() {
    let html = render_namespace_settings("acme", false, &[], true, &[], false).into_string();

    assert!(html.contains("Delete Namespace"), "{html}");
    assert!(
        html.contains("Delete every repository in this namespace before deleting it."),
        "{html}"
    );
    assert!(!html.contains("delete-namespace"), "{html}");
    assert!(!html.contains("twig-btn--danger"), "{html}");
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
    assert!(html.contains("/manage/data/rows?table=users&amp;offset=50"));
}
