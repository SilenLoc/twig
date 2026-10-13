use super::*;

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
fn test_default_tab_prefers_configuration_then_markdown() {
    let configured = vec!["license".to_string(), "commits".to_string()];
    let files = vec!["README.md".to_string()];
    assert_eq!(default_tab(&configured, &files), "license");
    assert_eq!(default_tab(&[], &files), "markdown");
    assert_eq!(default_tab(&[], &[]), "commits");
}
#[test]
fn test_tabs_are_an_out_of_band_labelled_nav() {
    let html = render_tabs(
        &frame_with(&[
            OptionalTab::Config,
            OptionalTab::Present,
            OptionalTab::Paper,
            OptionalTab::Scripts,
        ]),
        "commits",
    )
    .into_string();

    assert!(
        html.contains(
            r#"<nav id="tab-nav" class="twig-tabs" aria-label="Repository views" hx-swap-oob="true">"#
        ),
        "the tab bar keeps its OOB swap identity: {html}"
    );
    assert_eq!(
        count_of(&html, "hx-swap-oob"),
        1,
        "exactly one OOB target: {html}"
    );
    assert!(!html.contains("<div id=\"tab-nav\""), "{html}");
}
#[test]
fn test_tabs_preserve_every_htmx_navigation_attribute() {
    let html = render_tabs(
        &frame_with(&[
            OptionalTab::Config,
            OptionalTab::Present,
            OptionalTab::Paper,
            OptionalTab::Scripts,
        ]),
        "commits",
    )
    .into_string();
    let tabs = [
        "markdown", "paper", "content", "commits", "config", "present", "scripts", "license",
    ];

    for tab in tabs {
        let href = format!("/acme/my-project/{tab}");
        assert!(
            html.contains(&format!("href=\"{href}\"")),
            "missing href for {tab}: {html}"
        );
        assert!(
            html.contains(&format!("hx-get=\"{href}\"")),
            "missing hx-get for {tab}: {html}"
        );
        assert!(
            html.contains(&format!("hx-push-url=\"{href}\"")),
            "each tab pushes its own address for {tab}: {html}"
        );
    }
    assert_eq!(count_of(&html, "hx-target=\"#tab-content\""), tabs.len());
}
#[test]
fn test_tabs_mark_exactly_one_current_tab() {
    let html = render_tabs(
        &frame_with(&[
            OptionalTab::Config,
            OptionalTab::Present,
            OptionalTab::Paper,
            OptionalTab::Scripts,
        ]),
        "commits",
    )
    .into_string();

    assert_eq!(
        count_of(&html, "aria-current=\"page\""),
        1,
        "one current tab only: {html}"
    );
    let href = index_of(&html, "href=\"/acme/my-project/commits\"");
    let current = index_of(&html, "aria-current=\"page\"");
    let label = index_of(&html, ">Commits<");
    assert!(
        href < current && current < label,
        "aria-current must sit on the active tab: {html}"
    );
}
#[test]
fn test_tabs_render_only_available_views_in_order() {
    let html = render_tabs(&frame_with(&[]), "markdown").into_string();
    assert_eq!(
        tab_labels(&html),
        vec!["Documentation", "Content", "Commits", "License"],
        "unconfigured repositories hide Config, Present, Paper and Scripts: {html}"
    );

    let html = render_tabs(&frame_with(&[OptionalTab::Paper]), "paper").into_string();
    assert_eq!(
        tab_labels(&html),
        vec!["Documentation", "Paper", "Content", "Commits", "License"],
        "a configured paper slots in after Documentation: {html}"
    );

    let configured = [
        "license".to_string(),
        "bogus".to_string(),
        "config".to_string(),
        "present".to_string(),
        "paper".to_string(),
        "commits".to_string(),
    ];
    let html = render_tabs(
        &TabFrame {
            tabs_config: &configured,
            ..frame_with(&[OptionalTab::Present, OptionalTab::Paper])
        },
        "license",
    )
    .into_string();
    assert_eq!(
        tab_labels(&html),
        vec!["License", "Present", "Paper", "Commits"],
        "configured order wins and unknown or unavailable tabs are dropped: {html}"
    );

    let html = render_tabs(
        &TabFrame {
            tabs_config: &configured,
            ..frame_with(&[OptionalTab::Present])
        },
        "paper",
    )
    .into_string();
    assert_eq!(
        tab_labels(&html),
        vec!["License", "Present", "Commits"],
        "a configured paper is hidden when no pages exist: {html}"
    );
}
#[test]
fn test_repo_page_leads_with_a_breadcrumb_head_then_the_tabs() {
    let html = render_tab_shell(&base_frame(), "commits", &maud::html! {}).into_string();

    let crumbs = index_of(
        &html,
        r#"<nav class="twig-crumbs twig-crumbs--page" aria-label="Breadcrumb">"#,
    );
    let tabs = index_of(&html, "id=\"tab-nav\"");
    assert!(crumbs < tabs, "the trail, then the tabs: {html}");
    assert!(
        !html.contains("twig-optic-rule"),
        "the tab bar's own rule closes the head; no second rule: {html}"
    );
    assert!(
        html.contains(r#"<h1 class="twig-crumb-current" aria-current="page">my-project</h1>"#),
        "the trail's final segment is the page heading, rendered verbatim: {html}"
    );
    assert_eq!(count_of(&html, "<h1"), 1, "exactly one h1 per page: {html}");
}
#[test]
fn test_repo_page_offers_workspace_from_the_repository_heading() {
    let html = render_tab_shell(&base_frame(), "commits", &maud::html! {}).into_string();

    assert!(
        html.contains(r#"href="/acme/my-project/edit""#),
        "the overview links to the repository-root workspace: {html}"
    );
    assert!(
        html.contains(">Open workspace</a>"),
        "the workspace entry has a clear accessible label: {html}"
    );
    assert!(
        index_of(&html, ">Open workspace</a>") < index_of(&html, "id=\"tab-nav\""),
        "the entry is in the repository heading before the tabs: {html}"
    );
}
#[test]
fn test_repo_page_announces_its_tab_panel() {
    let html = render_tab_shell(&base_frame(), "commits", &maud::html! {}).into_string();
    assert!(
        html.contains(r#"<div id="tab-content" aria-live="polite">"#),
        "swapped tab content must be announced: {html}"
    );
}
#[test]
fn test_tab_response_differs_for_htmx_and_direct_navigation() {
    let body = render_commits_view(&[]);
    let frame = base_frame();

    let direct_req = actix_web::test::TestRequest::default().to_http_request();
    let direct = render_tab_response(&direct_req, &frame, "commits", &body).into_string();
    assert!(
        direct.contains("<!DOCTYPE html>"),
        "a direct visit is a full document: {direct}"
    );
    assert!(
        direct.contains(r#"<div id="tab-content" aria-live="polite">"#),
        "the full page carries the swap target: {direct}"
    );

    let htmx_req = actix_web::test::TestRequest::default()
        .insert_header(("HX-Request", "true"))
        .to_http_request();
    let fragment = render_tab_response(&htmx_req, &frame, "commits", &body).into_string();
    assert!(
        !fragment.contains("<!DOCTYPE html>"),
        "an htmx call returns the fragment only: {fragment}"
    );
    assert!(
        !fragment.contains(r#"<div id="tab-content""#),
        "the fragment replaces the panel's contents, not the wrapper: {fragment}"
    );
    assert!(
        fragment.contains(">Commits<"),
        "the fragment re-sends the out-of-band tab bar: {fragment}"
    );
    assert!(fragment.contains("NO COMMITS"), "{fragment}");
}
#[test]
fn test_commit_row_is_a_dense_machine_record() {
    let date = chrono::DateTime::from_timestamp(1_756_000_000, 0)
        .expect("valid timestamp")
        .to_utc();
    let commit = Commit::new(
        "a1b2c3d4e5f6".to_string(),
        "silen".to_string(),
        date,
        "Rewrite the theme layer".to_string(),
    );
    let html = render_commit(&commit).into_string();

    assert!(html.starts_with(r#"<li class="twig-commit">"#), "{html}");
    assert!(
        html.contains(r#"<code class="twig-commit-hash">a1b2c3d</code>"#),
        "the hash stays abbreviated to seven characters: {html}"
    );
    assert!(
        html.contains(r#"<span class="twig-commit-author">silen</span>"#),
        "{html}"
    );
    assert!(
        html.contains(&format!(
            r#"<time class="twig-commit-date" datetime="{}">"#,
            date.to_rfc3339()
        )),
        "the machine timestamp travels with the readable one: {html}"
    );
    assert!(
        html.contains(&date.format("%Y-%m-%d %H:%M").to_string()),
        "{html}"
    );
    assert!(
        html.contains(r#"<p class="twig-commit-msg">Rewrite the theme layer</p>"#),
        "{html}"
    );
}
#[test]
fn test_commits_view_falls_back_to_an_empty_state() {
    let html = render_commits_view(&[]).into_string();
    assert!(html.contains("twig-empty"), "{html}");
    assert!(html.contains(">NO COMMITS<"), "{html}");
    assert!(!html.contains("<ol"), "no empty list is rendered: {html}");
}
#[test]
fn test_config_error_is_a_diagnostic_with_a_help_hint() {
    let error = "TOML parse error at line 2, column 1\n  |\n2 | bad = = valid\n  | ^\ninvalid key";
    let html = render_config_error(".twig.toml", error).into_string();

    assert!(html.contains("role=\"alert\""), "{html}");
    assert!(html.contains("twig-notice twig-notice--danger"), "{html}");
    assert!(html.contains(">CONFIG ERROR<"), "{html}");
    assert!(
        html.contains(r#"<code class="twig-mono">.twig.toml</code>"#),
        "the offending file is named: {html}"
    );
    assert!(
        html.contains("error: could not parse configuration"),
        "{html}"
    );
    assert!(
        html.contains("--&gt; .twig.toml"),
        "the pointer is escaped, not interpreted as markup: {html}"
    );
    assert!(html.contains("help: fix the syntax"), "{html}");
    assert!(
        html.contains("invalid key"),
        "the parser's own explanation survives: {html}"
    );
    assert!(
        html.contains(r#"tabindex="0" aria-label=".twig.toml parse error""#),
        "the diagnostic stays keyboard-scrollable: {html}"
    );
}
#[test]
fn test_tab_content_shows_a_config_error_on_every_tab() {
    let frame = TabFrame {
        twig_error: Some("TOML parse error at line 1, column 1"),
        twig_filename: Some(".twig.toml"),
        ..base_frame()
    };
    let html = render_tab_shell(&frame, "commits", &maud::html! {}).into_string();

    let error = index_of(&html, "twig-notice--danger");
    let tabs = index_of(&html, "id=\"tab-nav\"");
    let content = index_of(&html, "id=\"tab-content\"");
    assert!(
        tabs < error && error < content,
        "the notice sits between the tabs and the panel so it survives swaps: {html}"
    );
    assert!(
        html.contains("TOML parse error at line 1, column 1"),
        "{html}"
    );
}
#[test]
fn test_config_and_license_views_render_code_and_empty_states() {
    let html = render_config_view(Some("[present]\nfiles = []"), Some(".twig")).into_string();
    assert!(
        html.contains(r#"<pre class="twig-code" tabindex="0" aria-label=".twig contents">"#),
        "code blocks are keyboard scrollable and named: {html}"
    );

    let html = render_config_view(None, None).into_string();
    assert!(html.contains(">NO CONFIGURATION<"), "{html}");
    assert!(
        html.contains("Create a .twig.toml file in the repository root"),
        "the original guidance survives: {html}"
    );

    let html = render_license_view(Some("<p>MIT</p>")).into_string();
    assert!(
        html.contains(r#"<div class="twig-md twig-md--boxed"><p>MIT</p></div>"#),
        "{html}"
    );

    let html = render_license_view(None).into_string();
    assert!(html.contains(">NO LICENSE<"), "{html}");
    assert!(html.contains("No license information available."), "{html}");
}
#[test]
fn test_git_error_is_an_announced_danger_notice() {
    let error = git2::Error::from_str("could not find repository");
    let html = render_git_error(&error).into_string();
    assert!(html.contains("role=\"alert\""), "{html}");
    assert!(html.contains("twig-notice twig-notice--danger"), "{html}");
    assert!(html.contains(">ERROR<"), "{html}");
    assert!(html.contains("could not find repository"), "{html}");
    assert!(
        html.contains(&format!("{:?}", error.code())),
        "the git error code stays visible: {html}"
    );
    assert!(
        html.contains(&format!("{:?}", error.class())),
        "the git error class stays visible: {html}"
    );
}
