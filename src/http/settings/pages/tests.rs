use super::*;
use crate::http::view::test_util::{classes_in, index_of};

pub(super) fn user_fixture(email: Option<&str>) -> crate::auth::User {
    crate::auth::User {
        id: "11111111-2222-3333-4444-555555555555".to_string(),
        username: "silen".to_string(),
        email: email.map(str::to_owned),
        password_hash: "argon2-hash".to_string(),
        created_at: "2026-08-31T09:30:00Z".to_string(),
    }
}

pub(super) fn repo_fixture(name: &str) -> git::bare::RepoInfo {
    git::bare::RepoInfo {
        name: name.to_string(),
        last_commit_date: None,
        is_private: false,
    }
}

pub(super) fn populated_html() -> String {
    render_settings(&user_fixture(Some("silen@example.com"))).into_string()
}

pub(super) fn empty_html() -> String {
    render_settings(&user_fixture(None)).into_string()
}

pub(super) fn repo_panel_html() -> String {
    render_repo_deletion_panel(&[("acme".to_string(), vec![repo_fixture("twig")])]).into_string()
}

pub(super) fn empty_repo_panel_html() -> String {
    render_repo_deletion_panel(&[]).into_string()
}

pub(super) fn rename_panel_html() -> String {
    render_repo_rename_panel(&[("acme".to_string(), vec![repo_fixture("twig")])]).into_string()
}

pub(super) fn empty_rename_panel_html() -> String {
    render_repo_rename_panel(&[]).into_string()
}

#[test]
pub(super) fn test_settings_leads_with_crumbs_and_page_head() {
    let html = populated_html();
    assert!(
        html.contains("<nav class=\"twig-crumbs twig-crumbs--page\" aria-label=\"Breadcrumb\">"),
        "breadcrumb must be a labelled nav landmark: {html}"
    );
    assert!(
        html.contains("<span class=\"twig-crumb-sep\" aria-hidden=\"true\">/</span>"),
        "separators are decorative: {html}"
    );
    assert!(
        html.contains("<span aria-current=\"page\">Account</span>"),
        "final crumb names the account page: {html}"
    );
    let crumbs = index_of(&html, "twig-crumbs");
    let head = index_of(&html, "<section class=\"twig-pagehead\">");
    assert!(
        crumbs < head && head < index_of(&html, "twig-bento"),
        "crumbs sit above the page head and bento: {html}"
    );
}

#[test]
pub(super) fn test_account_page_renders_account_panel_only() {
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
pub(super) fn test_settings_does_not_render_repository_moves() {
    let html = populated_html();
    assert!(!html.contains("/settings/move-repo"), "{html}");
    assert!(!html.contains("Move Repository"), "{html}");
    assert!(!html.contains("Delete Repository"), "{html}");
}

#[test]
pub(super) fn test_settings_deletion_panels_carry_danger_and_caution_affordances() {
    let html = repo_panel_html();
    assert_eq!(
        html.matches("twig-panel twig-panel--danger").count(),
        1,
        "the repository panel takes the danger variant: {html}"
    );
    assert_eq!(
        html.matches("<div class=\"twig-notice twig-notice--warning\" role=\"alert\">")
            .count(),
        1,
        "the deletion panel warns before it lists targets: {html}"
    );
    assert_eq!(
        html.matches(">CAUTION<").count(),
        1,
        "the signal word carries the meaning, not the colour: {html}"
    );
    assert!(
        html.contains(
            "Select a repository to permanently delete it. This action cannot be undone."
        ),
        "{html}"
    );
    assert_eq!(
        html.matches("<button class=\"twig-btn twig-btn--danger\" type=\"submit\">Delete</button>")
            .count(),
        1,
        "repository deletion uses a full-height danger button: {html}"
    );
}

#[test]
pub(super) fn test_settings_delete_repo_rows_preserve_the_htmx_contract() {
    let html = repo_panel_html();
    for attribute in [
        "hx-post=\"/settings/delete-repo\"",
        "hx-target=\"#delete-repo-result\"",
        "hx-swap=\"innerHTML\"",
        "hx-confirm=\"Are you sure you want to permanently delete 'acme/twig'?\"",
    ] {
        assert!(
            html.contains(attribute),
            "missing exact attribute {attribute}: {html}"
        );
    }
    assert!(
        html.contains("<form class=\"twig-row twig-row--form\""),
        "deletion targets are technical form rows: {html}"
    );
    assert!(
        html.contains("<span class=\"twig-row-id\">acme/twig</span>"),
        "the row identifier names the same object as the confirm text: {html}"
    );
    for hidden in [
        "<input type=\"hidden\" name=\"namespace\" value=\"acme\">",
        "<input type=\"hidden\" name=\"repo_name\" value=\"twig\">",
    ] {
        assert!(html.contains(hidden), "missing payload {hidden}: {html}");
    }
}

#[test]
pub(super) fn test_settings_rename_repo_rows_preserve_the_htmx_contract() {
    let html = rename_panel_html();
    for attribute in [
        "hx-post=\"/settings/rename-repo\"",
        "hx-target=\"#rename-repo-result\"",
        "hx-swap=\"innerHTML\"",
        "hx-confirm=\"Rename 'acme/twig'? Its URL will change.\"",
    ] {
        assert!(
            html.contains(attribute),
            "missing exact attribute {attribute}: {html}"
        );
    }
    assert!(
        html.contains("<form class=\"twig-row twig-row--form\""),
        "rename targets are technical form rows: {html}"
    );
    assert!(
        html.contains("<span class=\"twig-row-id\">acme/twig</span>"),
        "the row identifier names the repository being renamed: {html}"
    );
    for hidden in [
        "<input type=\"hidden\" name=\"namespace\" value=\"acme\">",
        "<input type=\"hidden\" name=\"repo_name\" value=\"twig\">",
    ] {
        assert!(html.contains(hidden), "missing payload {hidden}: {html}");
    }
    assert!(
            html.contains(
                "<input class=\"twig-input twig-input--mono\" type=\"text\" name=\"new_name\" required autocomplete=\"off\" spellcheck=\"false\" value=\"twig\">"
            ),
            "the new-name input is labelled, required and prefilled: {html}"
        );
    assert!(
        html.contains(
            "<button class=\"twig-btn twig-btn--primary\" type=\"submit\">Rename</button>"
        ),
        "{html}"
    );
    assert!(
        html.contains("Renaming a repository changes its web and Git URLs."),
        "{html}"
    );
}

#[test]
pub(super) fn test_settings_rename_panel_empty_state_offers_no_targets() {
    let html = empty_rename_panel_html();
    assert!(html.contains("Rename Repository"), "{html}");
    assert!(html.contains(">NO REPOSITORIES<"), "{html}");
    assert!(
        html.contains("You don't have any repositories to rename."),
        "{html}"
    );
    assert!(!html.contains("hx-confirm"), "{html}");
    assert!(!html.contains("twig-btn--primary"), "{html}");
}

#[test]
pub(super) fn test_settings_does_not_render_namespace_deletion_controls() {
    let html = populated_html();
    assert!(!html.contains("Delete Namespace"), "{html}");
    assert!(!html.contains("delete-namespace"), "{html}");
}

#[test]
pub(super) fn test_settings_email_form_preserves_the_htmx_contract() {
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
pub(super) fn test_settings_result_targets_announce_swaps() {
    let html = populated_html();
    assert!(
        html.contains("<div id=\"settings-result\" aria-live=\"polite\">"),
        "the email swap target must be a polite live region: {html}"
    );
}

#[test]
pub(super) fn test_settings_keeps_identity_visible_and_email_break_safe() {
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
pub(super) fn test_settings_empty_states_name_their_condition_and_offer_no_targets() {
    let html = empty_repo_panel_html();
    assert_eq!(
        html.matches("<div class=\"twig-empty\">").count(),
        1,
        "repository tab degrades to an empty state: {html}"
    );
    assert!(html.contains(">NO REPOSITORIES<"), "{html}");
    assert!(
        html.contains("You don't have any repositories to delete."),
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
pub(super) fn test_settings_uses_only_twig_design_system_classes() {
    for html in [
        populated_html(),
        empty_html(),
        repo_panel_html(),
        empty_repo_panel_html(),
        rename_panel_html(),
        empty_rename_panel_html(),
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
pub(super) fn test_namespace_form_deserialization() {
    let form: NamespaceForm = serde_urlencoded::from_str("namespace=my-namespace").unwrap();
    assert_eq!(form.namespace, "my-namespace");
}

#[test]
pub(super) fn test_update_email_form_deserialization() {
    let form: UpdateEmailForm = serde_urlencoded::from_str("email=test@example.com").unwrap();
    assert_eq!(form.email, "test@example.com");
}

#[test]
pub(super) fn test_delete_repo_form_deserialization() {
    let form: DeleteRepoForm = serde_urlencoded::from_str("namespace=ns&repo_name=repo").unwrap();
    assert_eq!(form.namespace, "ns");
    assert_eq!(form.repo_name, "repo");
}

#[test]
pub(super) fn test_move_repo_form_deserialization() {
    let form: MoveRepoForm = serde_urlencoded::from_str(
        "source_namespace=source&repo_name=repo&target_namespace=target",
    )
    .unwrap();
    assert_eq!(form.source_namespace, "source");
    assert_eq!(form.repo_name, "repo");
    assert_eq!(form.target_namespace, "target");
}

#[test]
pub(super) fn test_rename_repo_form_deserialization() {
    let form: RenameRepoForm =
        serde_urlencoded::from_str("namespace=ns&repo_name=old&new_name=new").unwrap();
    assert_eq!(form.namespace, "ns");
    assert_eq!(form.repo_name, "old");
    assert_eq!(form.new_name, "new");
}

#[test]
pub(super) fn test_rename_namespace_form_deserialization() {
    let form: RenameNamespaceForm =
        serde_urlencoded::from_str("namespace=oldns&new_name=newns").unwrap();
    assert_eq!(form.namespace, "oldns");
    assert_eq!(form.new_name, "newns");
}
