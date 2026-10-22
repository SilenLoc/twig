use super::*;
use crate::git;
use crate::http::view::test_util::classes_in;

fn user_fixture(email: Option<&str>) -> crate::auth::User {
    crate::auth::User {
        id: "11111111-2222-3333-4444-555555555555".to_string(),
        username: "silen".to_string(),
        email: email.map(str::to_owned),
        password_hash: "argon2-hash".to_string(),
        created_at: "2026-08-31T09:30:00Z".to_string(),
    }
}

fn repo_fixture(name: &str) -> git::bare::RepoInfo {
    git::bare::RepoInfo {
        name: name.to_string(),
        last_commit_date: None,
        is_private: false,
    }
}

fn populated_html() -> String {
    render_settings(&user_fixture(Some("silen@example.com"))).into_string()
}

fn empty_html() -> String {
    render_settings(&user_fixture(None)).into_string()
}

fn repo_manage_html(repos: &[(&str, &str, bool)]) -> String {
    let managed: Vec<ManagedRepo> = repos
        .iter()
        .map(|(namespace, name, deletable)| ManagedRepo {
            namespace: namespace.to_string(),
            repo: repo_fixture(name),
            deletable: *deletable,
        })
        .collect();
    render_repo_manage_panel(&managed).into_string()
}

fn repo_panel_html() -> String {
    repo_manage_html(&[("acme", "twig", true)])
}

fn empty_repo_panel_html() -> String {
    repo_manage_html(&[])
}

fn rename_only_html() -> String {
    repo_manage_html(&[("acme", "docs", false)])
}

#[test]
fn test_settings_has_no_redundant_namespace_breadcrumb() {
    let html = populated_html();
    assert!(
        !html.contains("twig-crumbs") && !html.contains("Namespaces"),
        "the Account tab does not repeat a Namespaces breadcrumb: {html}"
    );
    assert!(
        !html.contains("twig-pagehead"),
        "the account page has no empty page head: {html}"
    );
}

#[test]
fn test_account_page_renders_account_panel_only() {
    let html = populated_html();
    assert_eq!(
        html.matches("class=\"twig-bento\"").count(),
        1,
        "one bento container: {html}"
    );
    assert_eq!(
        html.matches("<section class=\"twig-panel").count(),
        1,
        "only the account panel appears here: {html}"
    );
    assert_eq!(
        html.matches("class=\"twig-panel-head\"").count(),
        1,
        "every panel declares a head: {html}"
    );
    assert_eq!(
        html.matches("<h2 class=\"twig-eyebrow\"").count(),
        1,
        "panel headings are h2 eyebrows under the page h1: {html}"
    );
    let heading_id = "settings-profile-heading";
    assert!(
        html.contains(&format!("aria-labelledby=\"{heading_id}\"")),
        "panel {heading_id} must be named by its heading: {html}"
    );
    assert!(
        html.contains(&format!(
            "<h2 class=\"twig-eyebrow\" id=\"{heading_id}\">Account</h2>"
        )),
        "missing heading {heading_id}: {html}"
    );
    assert!(
        !html.contains("<h4"),
        "panels never nest a fourth heading level: {html}"
    );
    assert!(!html.contains("Delete Repository"), "{html}");
}

#[test]
fn test_settings_does_not_render_repository_moves() {
    let html = populated_html();
    assert!(!html.contains("/settings/move-repo"), "{html}");
    assert!(!html.contains("Move Repository"), "{html}");
    assert!(!html.contains("Delete Repository"), "{html}");
}

#[test]
fn test_repo_manage_panel_is_one_searchable_scrollable_list() {
    let html = repo_panel_html();
    assert_eq!(
        html.matches("<section class=\"twig-panel\"").count(),
        1,
        "rename and delete share one panel: {html}"
    );
    assert!(
        !html.contains("twig-panel--danger") && !html.contains("Delete Repository"),
        "{html}"
    );
    assert!(
        html.contains(
            "<input class=\"twig-input twig-input--mono\" type=\"search\" id=\"repo-search\""
        ),
        "the list is searchable: {html}"
    );
    assert!(
        html.contains("<div class=\"twig-list twig-list--scroll\">"),
        "the list scrolls: {html}"
    );
    assert!(html.contains("data-repo-row=\"acme/twig\""), "{html}");
    assert!(html.contains("class=\"twig-empty-body\" hidden"), "{html}");
    assert!(html.contains("getElementById('repo-search')"), "{html}");
}

#[test]
fn test_repo_manage_rows_offer_delete_only_when_deletable() {
    let html = repo_panel_html();
    for attribute in [
        "hx-post=\"/settings/delete-repo\"",
        "hx-target=\"#repo-manage-result\"",
        "hx-confirm=\"Are you sure you want to permanently delete 'acme/twig'?\"",
        "<input type=\"hidden\" name=\"namespace\" value=\"acme\">",
        "<input type=\"hidden\" name=\"repo_name\" value=\"twig\">",
        "<button class=\"twig-btn twig-btn--danger\" type=\"submit\">Delete</button>",
    ] {
        assert!(html.contains(attribute), "missing {attribute}: {html}");
    }

    let rename_only = rename_only_html();
    assert!(
        !rename_only.contains("hx-post=\"/settings/delete-repo\""),
        "non-deletable rows get no delete control: {rename_only}"
    );
    assert!(!rename_only.contains("twig-btn--danger"), "{rename_only}");
}

#[test]
fn test_repo_manage_rows_preserve_the_rename_htmx_contract() {
    let html = repo_panel_html();
    for attribute in [
        "<details class=\"twig-repo-rename\">",
        "<summary class=\"twig-btn\">Rename</summary>",
        "hx-post=\"/settings/rename-repo\"",
        "hx-confirm=\"Rename 'acme/twig'? Its URL will change.\"",
        "<input class=\"twig-input twig-input--mono\" type=\"text\" name=\"new_name\" required autocomplete=\"off\" spellcheck=\"false\" aria-label=\"New name\" value=\"twig\">",
        "<button class=\"twig-btn twig-btn--primary\" type=\"submit\">Rename</button>",
        "<span class=\"twig-row-id\">acme/twig</span>",
    ] {
        assert!(html.contains(attribute), "missing {attribute}: {html}");
    }
}

#[test]
fn test_repo_manage_empty_state_offers_no_targets() {
    let html = empty_repo_panel_html();
    assert_eq!(
        html.matches("<div class=\"twig-empty\">").count(),
        1,
        "{html}"
    );
    assert!(html.contains(">NO REPOSITORIES<"), "{html}");
    assert!(
        html.contains("You don't have any repositories to manage."),
        "{html}"
    );
    assert!(!html.contains("hx-post"), "{html}");
    assert!(!html.contains("<input"), "{html}");
}

#[test]
fn test_settings_does_not_render_namespace_deletion_controls() {
    let html = populated_html();
    assert!(!html.contains("Delete Namespace"), "{html}");
    assert!(!html.contains("delete-namespace"), "{html}");
}

#[test]
fn test_settings_email_form_preserves_the_htmx_contract() {
    let html = populated_html();
    for attribute in [
        "hx-post=\"/settings/email\"",
        "hx-target=\"#settings-result\"",
        "hx-swap=\"innerHTML\"",
    ] {
        assert!(
            html.contains(attribute),
            "missing exact attribute {attribute}: {html}"
        );
    }
    assert!(
        html.contains("<label class=\"twig-label\" for=\"email\">Email Address</label>"),
        "every input keeps a real label: {html}"
    );
    assert!(
        html.contains(
            "<input class=\"twig-input\" type=\"email\" name=\"email\" id=\"email\" required value=\"silen@example.com\""
        ),
        "the email input keeps its type, requirement and current value: {html}"
    );
    assert!(
        html.contains(
            "<button class=\"twig-btn twig-btn--primary\" type=\"submit\">Save Email</button>"
        ),
        "{html}"
    );
}

#[test]
fn test_settings_result_targets_announce_swaps() {
    let html = populated_html();
    assert!(
        html.contains("<div id=\"settings-result\" aria-live=\"polite\">"),
        "the email swap target must be a polite live region: {html}"
    );
}

#[test]
fn test_settings_keeps_identity_visible_and_email_break_safe() {
    let html = populated_html();
    assert!(
        html.contains("<span class=\"twig-label\">Username</span>"),
        "{html}"
    );
    assert!(
        html.contains("<p class=\"twig-body twig-break\">silen</p>"),
        "{html}"
    );
    assert!(
        html.contains("<p class=\"twig-body twig-break\">silen@example.com</p>"),
        "long addresses must wrap rather than overflow: {html}"
    );

    let unset = empty_html();
    assert!(
        unset.contains("<p class=\"twig-body twig-ink-tertiary\">Not set</p>"),
        "a missing email reads as tertiary ink, not as an error: {unset}"
    );
    assert!(unset.contains("silen"), "username stays visible: {unset}");
}

#[test]
fn test_settings_empty_states_name_their_condition_and_offer_no_targets() {
    let html = empty_repo_panel_html();
    assert_eq!(
        html.matches("<div class=\"twig-empty\">").count(),
        1,
        "repository tab degrades to an empty state: {html}"
    );
    assert!(html.contains(">NO REPOSITORIES<"), "{html}");
    assert!(
        html.contains("You don't have any repositories to manage."),
        "{html}"
    );
    assert!(
        !html.contains("hx-confirm"),
        "no deletion affordance without a deletable object: {html}"
    );
    assert!(
        !html.contains("twig-btn--danger"),
        "no danger control without a deletable object: {html}"
    );
}

#[test]
fn test_settings_uses_only_twig_design_system_classes() {
    for html in [
        populated_html(),
        empty_html(),
        repo_panel_html(),
        empty_repo_panel_html(),
        rename_only_html(),
    ] {
        let classes = classes_in(&html);
        assert!(!classes.is_empty(), "settings should carry classes: {html}");
        for class in classes {
            assert!(
                class.starts_with("twig-"),
                "non design-system class {class:?} in settings: {html}"
            );
        }
        assert!(
            !html.contains("style=\""),
            "no inline style attributes: {html}"
        );
    }
}

#[test]
fn test_namespace_form_deserialization() {
    let form: NamespaceForm = serde_urlencoded::from_str("namespace=my-namespace").unwrap();
    assert_eq!(form.namespace, "my-namespace");
}

#[test]
fn test_update_email_form_deserialization() {
    let form: UpdateEmailForm = serde_urlencoded::from_str("email=test@example.com").unwrap();
    assert_eq!(form.email, "test@example.com");
}

#[test]
fn test_delete_repo_form_deserialization() {
    let form: DeleteRepoForm = serde_urlencoded::from_str("namespace=ns&repo_name=repo").unwrap();
    assert_eq!(form.namespace, "ns");
    assert_eq!(form.repo_name, "repo");
}

#[test]
fn test_move_repo_form_deserialization() {
    let form: MoveRepoForm = serde_urlencoded::from_str(
        "source_namespace=source&repo_name=repo&target_namespace=target",
    )
    .unwrap();
    assert_eq!(form.source_namespace, "source");
    assert_eq!(form.repo_name, "repo");
    assert_eq!(form.target_namespace, "target");
}

#[test]
fn test_rename_repo_form_deserialization() {
    let form: RenameRepoForm =
        serde_urlencoded::from_str("namespace=ns&repo_name=old&new_name=new").unwrap();
    assert_eq!(form.namespace, "ns");
    assert_eq!(form.repo_name, "old");
    assert_eq!(form.new_name, "new");
}

#[test]
fn test_rename_namespace_form_deserialization() {
    let form: RenameNamespaceForm =
        serde_urlencoded::from_str("namespace=oldns&new_name=newns").unwrap();
    assert_eq!(form.namespace, "oldns");
    assert_eq!(form.new_name, "newns");
}
