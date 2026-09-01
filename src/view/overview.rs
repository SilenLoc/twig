use actix_web::HttpRequest;
use actix_web::Result as AwResult;
use actix_web::get;
use actix_web::web;
use serde::Deserialize;

use crate::auth::FigContext;
use crate::config;
use crate::view::session_auth::get_username_from_request;

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

/// Renders the namespace listing shared by the full-page and HTMX responses.
fn render_index(
    search_query: &str,
    username: Option<&str>,
    namespaces: &[(crate::auth::Namespace, String)],
) -> maud::Markup {
    maud::html! {
        section class="fig-pagehead" {
            h1 class="fig-display-xl" { "Namespaces" }
            @if username.is_some() {
                div class="fig-cluster" {
                    a class="fig-btn fig-btn--primary" href="/auth/namespace" { "Create namespace" }
                }
            }
        }

        div class="fig-optic-rule fig-optic-rule--column" aria-hidden="true" {}

        div class="fig-stack" {
            form class="fig-search" role="search" method="GET" action="/" {
                label class="fig-sr" for="namespace-search" { "Search namespaces" }
                input
                    class="fig-input fig-input--mono"
                    id="namespace-search"
                    type="search"
                    name="q"
                    value=(search_query)
                    placeholder="Search namespaces...";
                button type="submit" class="fig-btn fig-btn--ghost" { "Search" }
                @if !search_query.is_empty() {
                    a class="fig-btn fig-btn--quiet" href="/" { "Clear" }
                }
            }

            section class="fig-panel fig-panel--flush" {
                div class="fig-panel-body" aria-live="polite" {
                    @if namespaces.is_empty() {
                        @if search_query.is_empty() {
                            div class="fig-empty fig-empty--void" {
                                p class="fig-eyebrow" { "NO NAMESPACES" }
                                p class="fig-empty-body" { "No namespaces yet. Create one to get started!" }
                            }
                        } @else {
                            div class="fig-empty fig-empty--filtered" {
                                p class="fig-eyebrow" { "NO MATCHES" }
                                p class="fig-empty-body" { "No namespaces found matching your search." }
                            }
                        }
                    } @else {
                        div class="fig-colhead" {
                            span { "Namespace" }
                            span { "Owner" }
                        }
                        nav class="fig-list" aria-label="Namespaces" {
                            @for (namespace, owner) in namespaces {
                                a class="fig-row" href=(namespace.name) {
                                    span class="fig-row-id" { (namespace.name) }
                                    span class="fig-row-meta" {
                                        span class="fig-sr" { "Owner: " }
                                        (owner)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[get("/")]
pub async fn index(
    req: HttpRequest,
    _server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    query: web::Query<SearchQuery>,
) -> AwResult<maud::Markup> {
    let search_query = query.q.as_deref().unwrap_or("");
    let username = get_username_from_request(&req, &auth_state).await;

    let db = auth_state.db();

    // Get namespaces with owners from database
    let namespaces = if search_query.is_empty() {
        match db.get_all_namespaces_with_owners().await {
            Ok(n) => n,
            Err(e) => {
                log::error!("Failed to get namespaces: {e}");
                Vec::new()
            }
        }
    } else {
        match db.search_namespaces_with_owners(search_query).await {
            Ok(n) => n,
            Err(e) => {
                log::error!("Failed to search namespaces: {e}");
                Vec::new()
            }
        }
    };

    let content = render_index(search_query, username.as_deref(), &namespaces);

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(super::render_layout(
            &content,
            username.as_deref(),
            Some("Namespaces"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn namespace(name: &str, owner: &str) -> (crate::auth::Namespace, String) {
        let namespace = crate::auth::Namespace {
            id: format!("id-{name}"),
            name: name.to_owned(),
            owner_id: format!("owner-{owner}"),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        };
        (namespace, owner.to_owned())
    }

    fn render(
        search_query: &str,
        username: Option<&str>,
        namespaces: &[(crate::auth::Namespace, String)],
    ) -> String {
        render_index(search_query, username, namespaces).into_string()
    }

    fn classes_in(html: &str) -> Vec<String> {
        let marker = "class=\"";
        html.match_indices(marker)
            .flat_map(|(start, _)| {
                let rest = &html[start + marker.len()..];
                let end = rest.find('"').expect("class attribute must be closed");
                rest[..end]
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn index_of(html: &str, needle: &str) -> usize {
        html.find(needle)
            .unwrap_or_else(|| panic!("expected markup to contain {needle}\n{html}"))
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
    fn test_page_head_is_a_single_quiet_word() {
        let html = render("", None, &[]);
        assert!(
            html.contains("<section class=\"fig-pagehead\">"),
            "the page opens on a quiet head: {html}"
        );
        assert!(
            html.contains("<h1 class=\"fig-display-xl\">Namespaces</h1>"),
            "the head is one fixed word, set quiet: {html}"
        );
        assert_eq!(
            html.matches("<h1").count(),
            1,
            "exactly one h1 per page, and it is the head: {html}"
        );
        let title = &html[index_of(&html, "<h1")..index_of(&html, "</h1>")];
        assert!(
            !title.contains("NAMESPACES"),
            "display uppercasing comes from CSS, never from a Rust literal: {html}"
        );
    }

    #[test]
    fn test_optic_rule_terminates_the_page_head() {
        let html = render("", Some("silen"), &[namespace("acme", "silen")]);
        assert!(
            html.contains(
                "<div class=\"fig-optic-rule fig-optic-rule--column\" aria-hidden=\"true\">"
            ),
            "the head rule is decorative and page-column wide: {html}"
        );
        let rule = index_of(&html, "class=\"fig-optic-rule");
        assert!(
            index_of(&html, "class=\"fig-pagehead\"") < rule,
            "the rule closes the head: {html}"
        );
        assert!(
            rule < index_of(&html, "fig-panel"),
            "the rule is the whole transition into the data zone: {html}"
        );
        assert_eq!(
            html.matches("class=\"fig-optic-rule").count(),
            1,
            "this page owns exactly one optic rule; the masthead owns the other: {html}"
        );
    }

    #[test]
    fn test_head_carries_the_single_create_action_for_signed_in_users() {
        let html = render("", Some("silen"), &[]);
        assert!(
            html.contains(
                "<div class=\"fig-cluster\"><a class=\"fig-btn fig-btn--primary\" \
                 href=\"/auth/namespace\">Create namespace</a></div>"
            ),
            "one primary action, inside the head, pointing at the unchanged URL: {html}"
        );
        assert!(
            index_of(&html, "fig-cluster") < index_of(&html, "class=\"fig-optic-rule"),
            "the action belongs to the head, not the data zone: {html}"
        );
        assert_eq!(
            html.matches("fig-btn--primary").count(),
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
            !html.contains("fig-cluster"),
            "an empty action row is not rendered: {html}"
        );
    }

    #[test]
    fn test_search_cluster_preserves_the_get_form_contract() {
        let html = render("ac me", None, &[]);
        for attribute in [
            "class=\"fig-search\"",
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
            html.contains("<label class=\"fig-sr\" for=\"namespace-search\">")
                && html.contains("id=\"namespace-search\""),
            "the input needs a real label, not just a placeholder: {html}"
        );
        assert!(
            html.contains("class=\"fig-input fig-input--mono\""),
            "namespace names are identifiers, so the field is mono: {html}"
        );
        assert!(
            html.contains("class=\"fig-btn fig-btn--ghost\"") && html.contains(">Search</button>"),
            "search is the secondary action: {html}"
        );
    }

    #[test]
    fn test_clear_returns_to_the_unfiltered_index_only_while_filtering() {
        let filtered = render("acme", None, &[]);
        assert!(
            filtered.contains("<a class=\"fig-btn fig-btn--quiet\" href=\"/\">Clear</a>"),
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
            html.contains("<section class=\"fig-panel fig-panel--flush\">"),
            "rows supply their own padding, so the panel body is flush: {html}"
        );
        assert!(
            html.contains(
                "<div class=\"fig-colhead\"><span>Namespace</span><span>Owner</span></div>"
            ),
            "the column header names both columns: {html}"
        );
        assert!(
            html.contains("<nav class=\"fig-list\" aria-label=\"Namespaces\">"),
            "the list of namespace links is a labelled navigation region: {html}"
        );
        assert!(
            index_of(&html, "aria-live=\"polite\"") < index_of(&html, "fig-list"),
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
            html.matches("class=\"fig-row\"").count(),
            2,
            "one row per namespace: {html}"
        );
        assert!(
            html.contains(
                "<span class=\"fig-row-meta\"><span class=\"fig-sr\">Owner: </span>silen</span>"
            ),
            "the stacked metadata line names the value it carries: {html}"
        );
        assert_eq!(
            html.matches("<span class=\"fig-sr\">Owner: </span>")
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
                "<a class=\"fig-row\" href=\"acme\"><span class=\"fig-row-id\">acme</span>"
            ),
            "the row is a real link to the namespace, identifier rendered verbatim: {html}"
        );
    }

    #[test]
    fn test_empty_states_name_their_condition_and_keep_their_copy() {
        let void = render("", None, &[]);
        assert!(
            void.contains("<div class=\"fig-empty fig-empty--void\">"),
            "an empty list renders an empty state, not zero rows: {void}"
        );
        assert!(
            void.contains("<p class=\"fig-eyebrow\">NO NAMESPACES</p>"),
            "the eyebrow states the condition in words: {void}"
        );
        assert!(
            void.contains("No namespaces yet. Create one to get started!"),
            "existing copy is preserved verbatim: {void}"
        );
        assert!(
            index_of(&void, "aria-live=\"polite\"") < index_of(&void, "fig-empty"),
            "the empty state sits inside the live region: {void}"
        );
        assert!(
            !void.contains("fig-colhead"),
            "column headers describe columns that an empty state does not have: {void}"
        );

        let filtered = render("zzz", None, &[]);
        assert!(
            filtered.contains("<div class=\"fig-empty fig-empty--filtered\">")
                && filtered.contains("<p class=\"fig-eyebrow\">NO MATCHES</p>"),
            "a filtered miss is a different condition than an empty server: {filtered}"
        );
        assert!(
            filtered.contains("No namespaces found matching your search."),
            "existing copy is preserved verbatim: {filtered}"
        );
        assert!(
            !filtered.contains("Create one to get started"),
            "creating is not what a searching user asked for: {filtered}"
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
                "every visual decision lives in fig.css: {html}"
            );
            let classes = classes_in(&html);
            assert!(!classes.is_empty(), "markup should carry classes: {html}");
            for class in classes {
                assert!(
                    class.starts_with("fig-"),
                    "non design-system class {class:?}: {html}"
                );
            }
        }
    }
}
