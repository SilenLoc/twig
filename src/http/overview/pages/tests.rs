use super::*;
use crate::http::view::test_util::{classes_in, index_of};

fn namespace(name: &str, owner: &str) -> (crate::auth::Namespace, String) {
    let namespace = crate::auth::Namespace {
        id: format!("id-{name}"),
        name: name.to_owned(),
        owner_id: format!("owner-{owner}"),
        created_at: "2026-01-01T00:00:00Z".to_owned(),
    };
    (namespace, owner.to_owned())
}

fn view(search_query: &str) -> ListView<'_> {
    ListView {
        search_query,
        more: false,
        pinned: None,
    }
}

fn render(
    search_query: &str,
    username: Option<&str>,
    namespaces: &[(crate::auth::Namespace, String)],
) -> String {
    render_index(&view(search_query), username, namespaces, &[]).into_string()
}

fn render_with_repo_hits(
    search_query: &str,
    username: Option<&str>,
    namespaces: &[(crate::auth::Namespace, String)],
    repo_hits: &[(String, String)],
) -> String {
    render_index(&view(search_query), username, namespaces, repo_hits).into_string()
}

#[test]
fn test_search_query_deserialization() {
    let query: SearchQuery = serde_urlencoded::from_str("q=rust").unwrap();
    assert_eq!(query.q, Some("rust".to_string()));
}

#[test]
fn test_search_query_empty() {
    let query: SearchQuery = serde_urlencoded::from_str("").unwrap();
    assert_eq!(query.q, None);
}

#[test]
fn test_head_carries_the_single_create_action_for_signed_in_users() {
    let html = render("", Some("silen"), &[]);
    assert!(
        html.contains(
            "<div class=\"twig-cluster\"><a class=\"twig-btn twig-btn--primary\" \
                 href=\"/auth/namespace\">Create namespace</a></div>"
        ),
        "one primary action, inside the head, pointing at the unchanged URL: {html}"
    );
    assert_eq!(
        html.matches("twig-btn--primary").count(),
        1,
        "a page head offers at most one main action: {html}"
    );
}

#[test]
fn test_create_action_stays_hidden_without_a_session() {
    let html = render("", None, &[]);
    assert!(
        !html.contains("/auth/namespace"),
        "anonymous visitors are not offered namespace creation: {html}"
    );
    assert!(
        !html.contains("twig-cluster"),
        "an empty action row is not rendered: {html}"
    );
}

#[test]
fn test_search_cluster_preserves_the_get_form_contract() {
    let html = render("ac me", None, &[]);
    for attribute in [
        "class=\"twig-search\"",
        "role=\"search\"",
        "method=\"GET\"",
        "action=\"/\"",
    ] {
        assert!(
            html.contains(attribute),
            "search stays a plain GET form on the index route ({attribute}): {html}"
        );
    }
    assert!(
        html.contains("name=\"q\"") && html.contains("value=\"ac me\""),
        "the active query is echoed back into the field: {html}"
    );
    assert!(
        html.contains("<label class=\"twig-sr\" for=\"index-search\">")
            && html.contains("id=\"index-search\""),
        "the input needs a real label, not just a placeholder: {html}"
    );
    assert!(
        html.contains("class=\"twig-input twig-input--mono\""),
        "namespace names are identifiers, so the field is mono: {html}"
    );
    assert!(
        html.contains("class=\"twig-btn twig-btn--ghost\"") && html.contains(">Search</button>"),
        "search is the secondary action: {html}"
    );
}

#[test]
fn test_clear_returns_to_the_unfiltered_index_only_while_filtering() {
    let filtered = render("acme", None, &[]);
    assert!(
        filtered.contains("<a class=\"twig-btn twig-btn--quiet\" href=\"/\">Clear</a>"),
        "clearing navigates back to the unfiltered index: {filtered}"
    );
    let unfiltered = render("", None, &[]);
    assert!(
        !unfiltered.contains("Clear"),
        "nothing to clear when no query is active: {unfiltered}"
    );
}

#[test]
fn test_namespace_list_is_a_flush_technical_panel() {
    let html = render("", None, &[namespace("acme", "silen")]);
    assert!(
        html.contains("<section class=\"twig-panel twig-panel--flush\">"),
        "rows supply their own padding, so the panel body is flush: {html}"
    );
    assert!(
        html.contains("<div class=\"twig-colhead\"><span>Namespace</span><span>Owner</span></div>"),
        "the column header names both columns: {html}"
    );
    assert!(
        html.contains("<nav class=\"twig-list\" aria-label=\"Namespaces\">"),
        "the list of namespace links is a labelled navigation region: {html}"
    );
    assert!(
        index_of(&html, "aria-live=\"polite\"") < index_of(&html, "twig-list"),
        "the swappable list region announces its changes: {html}"
    );
}

#[test]
fn test_rows_keep_owner_metadata_visible_and_labelled_at_every_width() {
    let html = render(
        "",
        None,
        &[namespace("acme", "silen"), namespace("hooli", "gavin")],
    );
    assert_eq!(
        html.matches("class=\"twig-row\"").count(),
        2,
        "one row per namespace: {html}"
    );
    assert!(
        html.contains(
            "<span class=\"twig-row-meta\"><span class=\"twig-sr\">Owner: </span>silen</span>"
        ),
        "the stacked metadata line names the value it carries: {html}"
    );
    assert_eq!(
        html.matches("<span class=\"twig-sr\">Owner: </span>")
            .count(),
        2,
        "every metadata cell is labelled, not just the first: {html}"
    );
    for outgoing in ["dn db-ns", "db-ns", "white-50"] {
        assert!(
            !html.contains(outgoing),
            "owner metadata is never hidden or dimmed away on mobile ({outgoing}): {html}"
        );
    }
}

#[test]
fn test_rows_keep_their_namespace_hrefs() {
    let html = render("", None, &[namespace("acme", "silen")]);
    assert!(
        html.contains(
            "<a class=\"twig-row\" href=\"acme\"><span class=\"twig-row-id\">acme</span>"
        ),
        "the row is a real link to the namespace, identifier rendered verbatim: {html}"
    );
}

#[test]
fn test_repo_hits_render_the_namespace_beneath_the_repository() {
    let html = render_with_repo_hits(
        "api",
        None,
        &[],
        &[("acme".to_owned(), "twig-api".to_owned())],
    );
    assert!(
        html.contains(
            "<div class=\"twig-colhead\"><span>Repository</span><span>Namespace</span></div>"
        ),
        "repository matches are listed under their own column header: {html}"
    );
    assert!(
        html.contains(
            "<a class=\"twig-row\" href=\"acme/twig-api\">\
<span class=\"twig-row-id\">twig-api</span>"
        ),
        "a repository hit links straight to the repository: {html}"
    );
    assert!(
        html.contains(
            "<span class=\"twig-row-meta\"><span class=\"twig-sr\">Namespace: </span>acme</span>"
        ),
        "the hit row names the namespace it lives in: {html}"
    );
    assert!(
        html.contains("<nav class=\"twig-list\" aria-label=\"Repository matches\">"),
        "repository hits get their own labelled list: {html}"
    );
}

#[test]
fn test_repo_hits_respect_the_private_filter_and_missing_roots() {
    let hits = search_repo_hits(
        "/nonexistent-twig-root",
        &[namespace("acme", "silen")],
        "x",
        true,
    );
    assert!(hits.is_empty(), "a missing root yields no repository hits");
}

#[test]
fn test_empty_states_name_their_condition_and_keep_their_copy() {
    let void = render("", None, &[]);
    assert!(
        void.contains("<div class=\"twig-empty\">"),
        "an empty list renders an empty state, not zero rows: {void}"
    );
    assert!(
        void.contains("<p class=\"twig-eyebrow\">NO NAMESPACES</p>"),
        "the eyebrow states the condition in words: {void}"
    );
    assert!(
        void.contains("No namespaces yet. Create one to get started!"),
        "existing copy is preserved verbatim: {void}"
    );
    assert!(
        index_of(&void, "aria-live=\"polite\"") < index_of(&void, "twig-empty"),
        "the empty state sits inside the live region: {void}"
    );
    assert!(
        !void.contains("twig-colhead"),
        "column headers describe columns that an empty state does not have: {void}"
    );

    let filtered = render("zzz", None, &[]);
    assert!(
        filtered.contains("<div class=\"twig-empty\">")
            && filtered.contains("<p class=\"twig-eyebrow\">NO MATCHES</p>"),
        "a filtered miss is a different condition than an empty server: {filtered}"
    );
    assert!(
        filtered.contains("No namespaces or repositories found matching your search."),
        "existing copy is preserved verbatim: {filtered}"
    );
    assert!(
        !filtered.contains("Create one to get started"),
        "creating is not what a searching user asked for: {filtered}"
    );
}

#[test]
fn test_active_search_swaps_the_results_region_as_the_user_types() {
    let html = render("acme", None, &[namespace("acme", "silen")]);
    for attribute in [
        "hx-get=\"/\"",
        "hx-target=\"#index-results\"",
        "hx-swap=\"innerHTML\"",
        "hx-trigger=\"input changed delay:200ms\"",
        "hx-indicator=\"#index-search-status\"",
    ] {
        assert!(
            html.contains(attribute),
            "active search wiring is missing ({attribute}): {html}"
        );
    }
    assert!(
        html.contains("<span id=\"index-search-status\" class=\"twig-search-status\""),
        "the indicator element that hx-indicator points at is rendered: {html}"
    );
    assert!(
        html.contains("placeholder=\"Search namespaces and repositories...\" hx-get=\"/\""),
        "the input itself issues the request: htmx does not inherit hx-get from the form: {html}"
    );
    assert!(
        html.contains("<div id=\"index-results\" class=\"twig-panel-body\" aria-live=\"polite\">"),
        "the swap target is the live results region: {html}"
    );
}

#[test]
fn test_htmx_fragment_is_only_the_results_and_the_out_of_band_clear() {
    let fragment = render_results(
        &view("acme"),
        &[namespace("acme", "silen")],
        &[("acme".to_owned(), "twig-api".to_owned())],
    )
    .into_string();
    assert!(
        !fragment.contains("<form") && !fragment.contains("index-search"),
        "the input keeps focus, so the form is never part of a swap: {fragment}"
    );
    assert!(
        !fragment.contains("twig-search-status"),
        "the indicator stays outside the swap so htmx can clear its request class: {fragment}"
    );
    assert!(
        fragment.contains("twig-row-id"),
        "the fragment carries the matching rows: {fragment}"
    );

    let clear = render_clear("acme", true).into_string();
    assert!(
        clear.contains("id=\"index-clear\"") && clear.contains("hx-swap-oob=\"outerHTML\""),
        "clear is replaced out-of-band: {clear}"
    );
    assert!(
        clear.contains("<a class=\"twig-btn twig-btn--quiet\" href=\"/\">Clear</a>"),
        "clear is offered while a query is active: {clear}"
    );

    let cleared = render_clear("", true).into_string();
    assert!(
        cleared.contains("hidden") && !cleared.contains("<a"),
        "clear disappears out-of-band once the query is empty: {cleared}"
    );
}

#[test]
fn test_index_uses_only_design_system_classes_and_no_inline_style() {
    let cases = [
        ("", None, Vec::new()),
        ("acme", Some("silen"), vec![namespace("acme", "silen")]),
    ];
    for (query, username, namespaces) in &cases {
        let html = render(query, *username, namespaces);
        assert!(
            !html.contains("style=\""),
            "every visual decision lives in twig.css: {html}"
        );
        let classes = classes_in(&html);
        assert!(!classes.is_empty(), "markup should carry classes: {html}");
        for class in classes {
            assert!(
                class.starts_with("twig-"),
                "non design-system class {class:?}: {html}"
            );
        }
    }
}

fn pinned_ids(ids: &[&str]) -> std::collections::HashSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn test_signed_in_home_leads_with_pinned_and_hides_the_rest_behind_more() {
    let namespaces = [namespace("acme", "silen"), namespace("hooli", "gavin")];
    let pinned = pinned_ids(&["id-hooli"]);
    let view = ListView {
        search_query: "",
        more: false,
        pinned: Some(&pinned),
    };
    let html = render_results(&view, &namespaces, &[]).into_string();

    assert!(
        html.contains("<nav class=\"twig-list\" aria-label=\"Pinned namespaces\">")
            && html.contains("href=\"hooli\""),
        "pinned namespaces get their own list at the top: {html}"
    );
    assert!(
        !html.contains("href=\"acme\""),
        "unpinned namespaces stay hidden until More is pressed: {html}"
    );
    assert!(
        index_of(&html, "Pinned namespaces") < index_of(&html, "Show 1 more"),
        "the toggle follows the pinned section: {html}"
    );
}

#[test]
fn test_expanded_home_lists_the_others_and_offers_to_collapse() {
    let namespaces = [namespace("acme", "silen"), namespace("hooli", "gavin")];
    let pinned = pinned_ids(&["id-hooli"]);
    let view = ListView {
        search_query: "",
        more: true,
        pinned: Some(&pinned),
    };
    let html = render_results(&view, &namespaces, &[]).into_string();

    assert!(
        html.contains("<nav class=\"twig-list\" aria-label=\"Other namespaces\">")
            && html.contains("href=\"acme\""),
        "expanding shows the unpinned namespaces: {html}"
    );
    assert!(
        html.contains(
            "hx-get=\"/\" hx-target=\"#index-results\" hx-swap=\"innerHTML\" hx-push-url=\"true\">Show fewer</button>"
        ),
        "the expanded list collapses back to the plain index: {html}"
    );
}

#[test]
fn test_home_rows_have_no_pin_controls() {
    let namespaces = [namespace("acme", "silen"), namespace("hooli", "gavin")];
    let pinned = pinned_ids(&["id-hooli"]);
    let view = ListView {
        search_query: "",
        more: true,
        pinned: Some(&pinned),
    };
    let html = render_results(&view, &namespaces, &[]).into_string();

    assert!(
        !html.contains("pin-namespace") && !html.contains(">Pin</button>"),
        "pinning lives in namespace settings, not on the home list: {html}"
    );
}

#[test]
fn test_search_shows_one_flat_list_without_the_home_grouping() {
    let namespaces = [namespace("acme", "silen")];
    let pinned = pinned_ids(&[]);
    let view = ListView {
        search_query: "ac",
        more: false,
        pinned: Some(&pinned),
    };
    let html = render_results(&view, &namespaces, &[]).into_string();

    assert!(
        !html.contains("PINNED") && !html.contains("Show "),
        "searching shows one flat list, not the home grouping: {html}"
    );
    assert!(
        html.contains("aria-label=\"Namespaces\""),
        "the search result list keeps its plain label: {html}"
    );
}

#[test]
fn test_signed_in_home_with_no_pins_explains_how_to_pin() {
    let namespaces = [namespace("acme", "silen"), namespace("hooli", "gavin")];
    let pinned = pinned_ids(&[]);
    let view = ListView {
        search_query: "",
        more: false,
        pinned: Some(&pinned),
    };
    let html = render_results(&view, &namespaces, &[]).into_string();

    assert!(
        html.contains("Pin a namespace from its settings to keep it at the top."),
        "without pins the home page says how pinning works: {html}"
    );
    assert!(
        html.contains("Show 2 more"),
        "every namespace waits behind More when nothing is pinned: {html}"
    );
}

#[test]
fn test_anonymous_visitors_see_the_full_list_without_pin_controls() {
    let html = render_results(
        &ListView {
            search_query: "",
            more: false,
            pinned: None,
        },
        &[namespace("acme", "silen"), namespace("hooli", "gavin")],
        &[],
    )
    .into_string();

    assert!(
        html.contains("href=\"acme\"") && html.contains("href=\"hooli\""),
        "anonymous visitors are not held back behind More: {html}"
    );
    assert!(
        !html.contains("pin-namespace") && !html.contains("Show "),
        "pinning and its toggle need a session: {html}"
    );
}
