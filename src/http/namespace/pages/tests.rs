use super::*;

use crate::http::view::test_util::{classes_in, index_of};

fn repo(name: &str, days_ago: Option<i64>) -> git::bare::RepoInfo {
    git::bare::RepoInfo {
        name: name.to_string(),
        last_commit_date: days_ago.map(|days| chrono::Utc::now() - chrono::Duration::days(days)),
        is_private: false,
    }
}

#[test]
fn test_namespace_breadcrumb_trail_is_semantic() {
    let html = render_namespace("acme", "", false, &[]).into_string();
    let trail = &html[..index_of(&html, "</nav>")];

    assert!(
        trail.contains("<nav class=\"twig-crumbs twig-crumbs--page\" aria-label=\"Breadcrumb\">"),
        "{html}"
    );
    assert!(trail.contains("<a href=\"/\">Namespaces</a>"), "{html}");
    assert!(
        trail.contains("<span class=\"twig-crumb-sep\" aria-hidden=\"true\">/</span>"),
        "the separator is decorative: {html}"
    );
    assert!(
        trail.contains("aria-current=\"page\">acme</h1>"),
        "the final segment marks the current page: {html}"
    );
    assert_eq!(
        trail.matches("<a ").count(),
        1,
        "the final segment must not be a link: {html}"
    );
}

#[test]
fn test_namespace_is_headed_by_its_identifier_inside_the_trail() {
    let html = render_namespace("Acme-Corp", "", false, &[]).into_string();

    assert!(
        html.contains("<h1 class=\"twig-crumb-current\" aria-current=\"page\">Acme-Corp</h1>"),
        "the trail's final segment is the heading, never uppercased: {html}"
    );
    assert_eq!(html.matches("<h1").count(), 1, "one h1 per page: {html}");
    assert!(
        !html.contains("twig-eyebrow\">NAMESPACE"),
        "the identifier is not restated as a title block: {html}"
    );
    assert!(
        index_of(&html, "class=\"twig-pagehead\"") < index_of(&html, "twig-crumbs"),
        "the trail is the page head: {html}"
    );
}

#[test]
fn test_namespace_does_not_repeat_the_repository_section_heading() {
    let html = render_namespace("acme", "", false, &[]).into_string();

    assert!(!html.contains("repositories-heading"), "{html}");
    assert!(!html.contains(">REPOSITORIES<"), "{html}");
}

#[test]
fn test_namespace_head_action_is_gated_on_namespace_access() {
    let member = render_namespace("acme", "", true, &[]).into_string();
    for wiring in [
        "<div class=\"twig-cluster\">",
        "class=\"twig-btn twig-btn--primary\"",
        "hx-get=\"/acme/create-repo-form\"",
        "hx-target=\"#create-repo-container\"",
        "hx-swap=\"innerHTML\"",
        "<div id=\"create-repo-container\"></div>",
        ">Create repo</button>",
    ] {
        assert!(member.contains(wiring), "missing {wiring}: {member}");
    }

    let visitor = render_namespace("acme", "", false, &[]).into_string();
    for wiring in ["twig-cluster", "create-repo-form", "create-repo-container"] {
        assert!(
            !visitor.contains(wiring),
            "visitors without access get no create affordance, found {wiring}: {visitor}"
        );
    }
}

#[test]
fn test_namespace_search_cluster_uses_the_field_primitives() {
    let unfiltered = render_namespace("acme", "", false, &[]).into_string();
    for part in [
        "<form class=\"twig-search\" role=\"search\" method=\"GET\" action=\"/acme\">",
        "<label class=\"twig-sr\" for=\"repo-search\">Search repositories</label>",
        "class=\"twig-input twig-input--mono\" id=\"repo-search\"",
        "placeholder=\"Search repositories...\"",
        "hx-trigger=\"input changed delay:200ms\"",
    ] {
        assert!(unfiltered.contains(part), "missing {part}: {unfiltered}");
    }
    assert!(
        !unfiltered.contains("twig-btn--quiet"),
        "Clear only appears while a query is active: {unfiltered}"
    );

    let filtered = render_namespace("acme", "twig", false, &[]).into_string();
    assert!(filtered.contains("value=\"twig\""), "{filtered}");
    assert!(
        filtered.contains("<a class=\"twig-btn twig-btn--quiet\" href=\"/acme\">Clear</a>"),
        "{filtered}"
    );
}

#[test]
fn test_namespace_rows_keep_last_commit_labelled_when_stacked() {
    let repos = [repo("twig", Some(5)), repo("fresh", None)];
    let html = render_namespace("acme", "", true, &repos).into_string();

    assert!(
        html.contains(
            "<div class=\"twig-colhead\"><span>Repository</span><span>Last commit</span></div>"
        ),
        "{html}"
    );
    assert!(html.contains("<div class=\"twig-list\">"), "{html}");
    assert!(
        html.contains(
            "<a class=\"twig-row\" href=\"acme/twig\"><span class=\"twig-row-id\">twig</span>"
        ),
        "{html}"
    );
    assert_eq!(
        html.matches("<span class=\"twig-sr\">Last commit: </span>")
            .count(),
        repos.len(),
        "every stacked row labels its metadata: {html}"
    );
    assert!(html.contains("5 days ago"), "{html}");
    assert!(
        html.contains("No commits"),
        "repos without commits keep their value: {html}"
    );
    assert!(!html.contains("twig-empty"), "{html}");
}

#[test]
fn test_namespace_empty_states_preserve_their_messages() {
    let void = render_namespace("acme", "", true, &[]).into_string();
    assert!(void.contains("class=\"twig-empty\""), "{void}");
    assert!(
        void.contains("<p class=\"twig-eyebrow\">NO REPOSITORIES</p>"),
        "{void}"
    );
    assert!(
        void.contains("No repositories yet. Click 'Create repo' to add one!"),
        "{void}"
    );

    let filtered = render_namespace("acme", "zzz", true, &[]).into_string();
    assert!(
        filtered.contains("class=\"twig-empty\"")
            && filtered.contains("<p class=\"twig-eyebrow\">NO MATCHES</p>"),
        "{filtered}"
    );
    assert!(
        filtered.contains("No repositories found matching your search."),
        "{filtered}"
    );
    assert!(
        filtered.contains("<a class=\"twig-btn twig-btn--ghost\" href=\"/acme\">Clear search</a>"),
        "a filtered empty state offers Clear, never a creation prompt: {filtered}"
    );
    assert!(
        !filtered.contains("twig-colhead"),
        "no column header without rows: {filtered}"
    );
}

#[test]
fn test_namespace_uses_only_twig_design_system_classes() {
    let repos = [repo("twig", Some(2))];
    for (query, has_access, listed) in [
        ("", true, &repos[..]),
        ("", false, &[][..]),
        ("twig", true, &repos[..]),
        ("zzz", false, &[][..]),
    ] {
        let html = render_namespace("acme", query, has_access, listed).into_string();
        let classes = classes_in(&html);
        assert!(!classes.is_empty(), "{html}");
        for class in classes {
            assert!(
                class.starts_with("twig-"),
                "non design-system class {class:?}: {html}"
            );
        }
        assert!(!html.contains("style=\""), "no inline styles: {html}");
        assert!(!html.contains('\u{2715}'), "no glyph icons: {html}");
    }
}

#[test]
fn test_create_repo_form_preserves_the_htmx_contract() {
    let html = render_create_repo_form("acme").into_string();
    for attribute in [
        "hx-post=\"/acme/create-repo\"",
        "hx-target=\"#create-repo-result\"",
        "hx-status:4xx=\"swap:innerHTML target:#create-repo-result\"",
        "hx-status:5xx=\"swap:innerHTML target:#create-repo-result\"",
        "hx-swap=\"innerHTML\"",
        "hx-swap=\"innerHTML\"",
    ] {
        assert!(html.contains(attribute), "missing {attribute}: {html}");
    }
    assert!(
        html.contains("<div id=\"create-repo-result\" aria-live=\"polite\"></div>"),
        "the result target announces its swaps: {html}"
    );
}

#[test]
fn test_create_repo_form_cancel_is_plain_text_with_its_dismiss_behaviour() {
    let html = render_create_repo_form("acme").into_string();
    assert!(
        html.contains(
            "<button type=\"button\" class=\"twig-btn twig-btn--quiet\" onclick=\"document.getElementById('create-repo-container').innerHTML = ''\">Cancel</button>"
        ),
        "{html}"
    );
    assert!(
        !html.contains('\u{2715}'),
        "the cross glyph is removed: {html}"
    );
}

#[test]
fn test_create_repo_form_is_a_panel_that_keeps_its_field_contract() {
    let html = render_create_repo_form("acme").into_string();
    for part in [
        "<section class=\"twig-panel\" aria-labelledby=\"create-repo-heading\">",
        "<h2 id=\"create-repo-heading\" class=\"twig-eyebrow\">Create Repository</h2>",
        "class=\"twig-form twig-form--narrow\"",
        "<label class=\"twig-label\" for=\"repo_name\">Repository Name</label>",
        "name=\"repo_name\" id=\"repo_name\" required minlength=\"1\"",
        "placeholder=\"Enter repository name (e.g., my-project)\"",
        "<label class=\"twig-label\" for=\"branch\">Default Branch</label>",
        "name=\"branch\" id=\"branch\" value=\"main\"",
        "<button type=\"submit\" class=\"twig-btn twig-btn--primary\">Create Repository</button>",
    ] {
        assert!(html.contains(part), "missing {part}: {html}");
    }

    for class in classes_in(&html) {
        assert!(
            class.starts_with("twig-"),
            "non design-system class {class:?}: {html}"
        );
    }
    assert!(!html.contains("style=\""), "no inline styles: {html}");
}

#[test]
fn test_format_date_years() {
    let date = chrono::Utc::now() - chrono::Duration::days(800);
    let result = format_date(&date);
    assert!(
        result.contains("years ago"),
        "expected years ago, got {result}"
    );
}

#[test]
fn test_format_date_months() {
    let date = chrono::Utc::now() - chrono::Duration::days(100);
    let result = format_date(&date);
    assert!(
        result.contains("months ago"),
        "expected months ago, got {result}"
    );
}

#[test]
fn test_format_date_days() {
    let date = chrono::Utc::now() - chrono::Duration::days(5);
    let result = format_date(&date);
    assert!(
        result.contains("days ago"),
        "expected days ago, got {result}"
    );
}

#[test]
fn test_format_date_hours() {
    let date = chrono::Utc::now() - chrono::Duration::hours(3);
    let result = format_date(&date);
    assert!(
        result.contains("hours ago"),
        "expected hours ago, got {result}"
    );
}

#[test]
fn test_format_date_minutes() {
    let date = chrono::Utc::now() - chrono::Duration::minutes(10);
    let result = format_date(&date);
    assert!(
        result.contains("minutes ago"),
        "expected minutes ago, got {result}"
    );
}

#[test]
fn test_format_date_just_now() {
    let date = chrono::Utc::now();
    assert_eq!(format_date(&date), "just now");
}

#[test]
fn test_format_date_singular_units() {
    for (date, expected) in [
        (
            chrono::Utc::now() - chrono::Duration::days(365),
            "1 year ago",
        ),
        (
            chrono::Utc::now() - chrono::Duration::days(30),
            "1 month ago",
        ),
        (chrono::Utc::now() - chrono::Duration::days(1), "1 day ago"),
        (
            chrono::Utc::now() - chrono::Duration::hours(1),
            "1 hour ago",
        ),
        (
            chrono::Utc::now() - chrono::Duration::minutes(1),
            "1 minute ago",
        ),
    ] {
        assert_eq!(format_date(&date), expected);
    }
}

#[actix_web::test]
async fn test_missing_email_response_is_a_bad_request_fragment_with_account_link() {
    let response = missing_email_response();
    assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);

    let body = actix_web::body::to_bytes(response.into_body())
        .await
        .expect("response body");
    let html = String::from_utf8(body.to_vec()).expect("HTML response");
    assert!(!html.contains("<!DOCTYPE html>"), "{html}");
    assert!(
        html.contains("<a class=\"twig-btn twig-btn--ghost\" href=\"/settings\">Go to Account</a>"),
        "{html}"
    );
    assert!(!html.contains("&lt;a "), "{html}");
}

#[test]
fn test_params_deserialization() {
    let params: Params = serde_urlencoded::from_str("namespace=my-ns").unwrap();
    assert_eq!(params.namespace, "my-ns");
}

#[test]
fn test_create_repo_form_deserialization() {
    let form: CreateRepoForm = serde_urlencoded::from_str("repo_name=my-repo&branch=main").unwrap();
    assert_eq!(form.repo_name, "my-repo");
    assert_eq!(form.branch, "main");
}

#[test]
fn test_create_repo_form_default_branch() {
    let form: CreateRepoForm = serde_urlencoded::from_str("repo_name=my-repo").unwrap();
    assert_eq!(form.repo_name, "my-repo");
    assert_eq!(form.branch, "main");
}

#[test]
fn test_search_query_deserialization() {
    let query: SearchQuery = serde_urlencoded::from_str("q=search-term").unwrap();
    assert_eq!(query.q, Some("search-term".to_string()));
}

#[test]
fn test_search_query_empty() {
    let query: SearchQuery = serde_urlencoded::from_str("").unwrap();
    assert_eq!(query.q, None);
}
