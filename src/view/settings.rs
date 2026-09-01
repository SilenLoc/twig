use std::path::Path;

use actix_web::Result as AwResult;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use log::info;
use serde::Deserialize;

use super::{render_error, render_error_with_action, render_success};
use crate::auth::FigContext;
use crate::config;
use crate::git;

#[derive(Deserialize)]
struct UpdateEmailForm {
    email: String,
}

#[derive(Deserialize)]
struct DeleteRepoForm {
    namespace: String,
    repo_name: String,
}

/// Renders the settings page body shared by the full-page and HTMX responses.
fn render_settings(
    user: &crate::auth::User,
    repos_by_namespace: &[(String, Vec<git::bare::RepoInfo>)],
    deletable_namespaces: &[String],
) -> maud::Markup {
    maud::html! {
        nav class="fig-crumbs fig-crumbs--page" aria-label="Breadcrumb" {
            a href="/" { "Namespaces" }
            span class="fig-crumb-sep" aria-hidden="true" { "/" }
            span aria-current="page" { "Settings" }
        }

        section class="fig-pagehead" {
        }

        div class="fig-bento" {
            (render_profile_panel(user))
            (render_repo_deletion_panel(repos_by_namespace))
            (render_namespace_deletion_panel(deletable_namespaces))
        }
    }
}

/// Identity values plus the email update form.
fn render_profile_panel(user: &crate::auth::User) -> maud::Markup {
    maud::html! {
        section class="fig-panel" aria-labelledby="settings-profile-heading" {
            header class="fig-panel-head" {
                h2 class="fig-eyebrow" id="settings-profile-heading" { "Profile Information" }
            }
            div class="fig-panel-body fig-stack" {
                div class="fig-field" {
                    span class="fig-label" { "Username" }
                    p class="fig-body fig-break" { (user.username) }
                }

                div class="fig-field" {
                    span class="fig-label" { "Email" }
                    @match &user.email {
                        Some(email) => { p class="fig-body fig-break" { (email) } }
                        None => { p class="fig-body fig-ink-tertiary" { "Not set" } }
                    }
                }

                form
                    class="fig-form"
                    aria-labelledby="settings-email-heading"
                    hx-post="/settings/email"
                    hx-target="#settings-result"
                    hx-swap="innerHTML"
                {
                    h3 class="fig-subsection" id="settings-email-heading" { "Update Email" }

                    div class="fig-field" {
                        label class="fig-label" for="email" { "Email Address" }
                        input
                            class="fig-input"
                            type="email"
                            name="email"
                            id="email"
                            required
                            value=(user.email.as_deref().unwrap_or(""))
                            placeholder="Enter your email address";
                    }

                    div class="fig-form-actions" {
                        button class="fig-btn fig-btn--primary" type="submit" { "Save Email" }
                    }
                }

                div id="settings-result" aria-live="polite" {}
            }
        }
    }
}

/// Repositories the user may delete, one technical form row each.
fn render_repo_deletion_panel(
    repos_by_namespace: &[(String, Vec<git::bare::RepoInfo>)],
) -> maud::Markup {
    maud::html! {
        section
            class="fig-panel fig-panel--danger"
            aria-labelledby="settings-delete-repo-heading"
        {
            header class="fig-panel-head" {
                h2 class="fig-eyebrow" id="settings-delete-repo-heading" { "Delete Repository" }
            }

            @if repos_by_namespace.is_empty() {
                div class="fig-empty" {
                    p class="fig-eyebrow" { "NO REPOSITORIES" }
                    p class="fig-empty-body" { "You don't have any repositories to delete." }
                }
            } @else {
                div class="fig-panel-body" {
                    div class="fig-notice fig-notice--warning" role="alert" {
                        p class="fig-eyebrow" { "CAUTION" }
                        p class="fig-notice-body" { "Select a repository to permanently delete it. This action cannot be undone." }
                    }
                    div id="delete-repo-result" aria-live="polite" {}
                }

                div class="fig-list" {
                    @for (ns, repos) in repos_by_namespace {
                        @for repo in repos {
                            form
                                class="fig-row fig-row--form"
                                hx-post="/settings/delete-repo"
                                hx-target="#delete-repo-result"
                                hx-swap="innerHTML"
                                hx-confirm=(format!("Are you sure you want to permanently delete '{}/{}'?", ns, repo.name))
                            {
                                input type="hidden" name="namespace" value=(ns);
                                input type="hidden" name="repo_name" value=(repo.name);
                                span class="fig-row-id" { (ns) "/" (repo.name) }
                                button class="fig-btn fig-btn--danger" type="submit" { "Delete" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Namespaces the user owns that hold no repositories, and so can be deleted.
fn render_namespace_deletion_panel(deletable_namespaces: &[String]) -> maud::Markup {
    maud::html! {
        section
            class="fig-panel fig-panel--danger"
            aria-labelledby="settings-delete-namespace-heading"
        {
            header class="fig-panel-head" {
                h2 class="fig-eyebrow" id="settings-delete-namespace-heading" { "Delete Namespace" }
            }

            @if deletable_namespaces.is_empty() {
                div class="fig-empty" {
                    p class="fig-eyebrow" { "NO NAMESPACES" }
                    p class="fig-empty-body" { "No namespaces available for deletion. You can only delete namespaces you own that have no repositories." }
                }
            } @else {
                div class="fig-panel-body" {
                    div class="fig-notice fig-notice--warning" role="alert" {
                        p class="fig-eyebrow" { "CAUTION" }
                        p class="fig-notice-body" { "Select a namespace to permanently delete it. This action cannot be undone." }
                    }
                    div id="delete-namespace-result" aria-live="polite" {}
                }

                div class="fig-list" {
                    @for ns_name in deletable_namespaces {
                        form
                            class="fig-row fig-row--form"
                            hx-post="/settings/delete-namespace/"
                            hx-target="#delete-namespace-result"
                            hx-swap="innerHTML"
                            hx-confirm=(format!("Are you sure you want to permanently delete the namespace '{}'? This cannot be undone.", ns_name))
                        {
                            input type="hidden" name="namespace" value=(ns_name);
                            span class="fig-row-id" { (ns_name) }
                            button class="fig-btn fig-btn--danger" type="submit" { "Delete" }
                        }
                    }
                }
            }
        }
    }
}

#[get("/settings")]
pub async fn settings_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
) -> AwResult<maud::Markup> {
    // Get user from session
    let Some(cookie) = req.cookie("session") else {
        return Ok(render_settings_auth_error(
            &req,
            "Not logged in. Please log in first.",
        ));
    };
    let token = cookie.value().to_string();

    let Some(user_id) = auth_state.validate_token(&token).await else {
        return Ok(render_settings_auth_error(
            &req,
            "Session expired. Please log in again.",
        ));
    };

    let db = auth_state.db();

    let Ok(Some(user)) = db.get_user_by_id(&user_id).await else {
        return Ok(render_error("Failed to load user."));
    };

    // Load namespaces and repos the user has access to
    let namespaces = match db.get_namespaces_for_user(&user_id).await {
        Ok(n) => n,
        Err(e) => {
            log::error!("Failed to get namespaces: {e}");
            Vec::new()
        }
    };

    let mut repos_by_namespace: Vec<(String, Vec<git::bare::RepoInfo>)> = Vec::new();
    let mut deletable_namespaces: Vec<String> = Vec::new();
    for ns in &namespaces {
        let has_repos = git::bare::namespace::has_any_repository(server.project_root(), &ns.name);
        if ns.owner_id == user_id && !has_repos {
            deletable_namespaces.push(ns.name.clone());
        }
        let repos = git::bare::get_repos_with_info(server.project_root(), &ns.name);
        let deletable_repos: Vec<_> = repos
            .into_iter()
            .filter(|repo| {
                let config =
                    git::bare::FigConfig::load(server.project_root(), &ns.name, &repo.name);
                config.deleteable
            })
            .collect();
        if !deletable_repos.is_empty() {
            repos_by_namespace.push((ns.name.clone(), deletable_repos));
        }
    }

    let content = render_settings(&user, &repos_by_namespace, &deletable_namespaces);

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(crate::view::render_layout(
            &content,
            Some(&user.username),
            Some("Settings"),
        ))
    }
}

fn render_settings_auth_error(req: &HttpRequest, message: &str) -> maud::Markup {
    let content = render_error_with_action(message, "/auth/login", "Log in");
    if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::view::render_layout(&content, None, Some("Settings"))
    }
}

#[post("/settings/email")]
pub async fn update_email(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
    form: web::Form<UpdateEmailForm>,
) -> impl Responder {
    // Validate email format (basic validation) before requiring a session,
    // so malformed input is rejected as BAD_REQUEST regardless of auth state.
    if form.email.is_empty() || !form.email.contains('@') {
        return HttpResponse::BadRequest()
            .body(render_error("Please enter a valid email address").into_string());
    }

    // Get user from session
    let Some(cookie) = req.cookie("session") else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };
    let token = cookie.value().to_string();

    let Some(user_id) = auth_state.validate_token(&token).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Session expired. Please log in again.").into_string());
    };

    let db = auth_state.db();

    // Update email in database
    match db.update_user_email(&user_id, &form.email).await {
        Ok(()) => {
            info!("Updated email for user: {user_id}");
            HttpResponse::Ok()
                .content_type("text/html")
                .body(render_success("Email updated successfully!").into_string())
        }
        Err(e) => {
            log::error!("Failed to update email: {e}");
            HttpResponse::InternalServerError()
                .body(render_error("Failed to update email").into_string())
        }
    }
}

#[post("/settings/delete-repo")]
pub async fn delete_repo(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    form: web::Form<DeleteRepoForm>,
) -> impl Responder {
    // Get user from session
    let Some(cookie) = req.cookie("session") else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };
    let token = cookie.value().to_string();

    let Some(user_id) = auth_state.validate_token(&token).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Session expired. Please log in again.").into_string());
    };

    let db = auth_state.db();

    // Verify user has access to the namespace
    match db
        .user_has_namespace_access(&user_id, &form.namespace)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Forbidden()
                .body(render_error("Access denied to namespace").into_string());
        }
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    }

    // Build the repository path
    let repo_path = Path::new(server.project_root())
        .join(&form.namespace)
        .join(&form.repo_name);

    // Check that the path exists and is a git repository
    if !repo_path.exists() {
        return HttpResponse::NotFound().body(render_error("Repository not found").into_string());
    }

    if git2::Repository::open(&repo_path).is_err() {
        return HttpResponse::BadRequest()
            .body(render_error("Path is not a valid git repository").into_string());
    }

    // Check if repository is marked as deletable in fig.toml
    let fig_config =
        git::bare::FigConfig::load(server.project_root(), &form.namespace, &form.repo_name);
    if !fig_config.deleteable {
        return HttpResponse::Forbidden()
            .body(render_error("Repository is not marked as deletable. Set deleteable=true in .fig.toml to enable deletion.").into_string());
    }

    // Delete the repository
    match std::fs::remove_dir_all(&repo_path) {
        Ok(()) => {
            info!(
                "Deleted repository '{}/{}' by user: {}",
                form.namespace, form.repo_name, user_id
            );
            HttpResponse::Ok()
                .content_type("text/html")
                .body(render_success("Repository deleted successfully.").into_string())
        }
        Err(e) => {
            log::error!("Failed to delete repository: {e}");
            HttpResponse::InternalServerError()
                .body(render_error("Failed to delete repository").into_string())
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamespaceForm {
    pub namespace: String,
}

#[post("/settings/delete-namespace/")]
pub async fn delete_namespace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
    form: web::Form<NamespaceForm>,
) -> impl Responder {
    let Some(cookie) = req.cookie("session") else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };
    let token = cookie.value().to_string();

    let Some(user_id) = auth_state.validate_token(&token).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Session expired. Please log in again.").into_string());
    };

    let db = auth_state.db();

    let namespace = match db.get_namespace_by_name(&form.namespace).await {
        Ok(Some(ns)) => ns,
        Ok(None) => {
            return HttpResponse::NotFound()
                .body(render_error("Namespace not found").into_string());
        }
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    };

    if namespace.owner_id != user_id {
        return HttpResponse::Forbidden()
            .body(render_error("Only the namespace owner can delete it").into_string());
    }

    if git::bare::namespace::has_any_repository(server.project_root(), &form.namespace) {
        return HttpResponse::BadRequest().body(
            render_error(
                "Cannot delete namespace: it still contains repositories. Delete all repositories first.",
            )
            .into_string(),
        );
    }

    if let Err(e) = db.delete_namespace(&namespace.id).await {
        log::error!("Failed to delete namespace: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to delete namespace").into_string());
    }

    let namespace_dir = Path::new(server.project_root()).join(&form.namespace);
    if namespace_dir.is_dir() {
        let _ = std::fs::remove_dir(&namespace_dir);
    }

    info!(
        "Deleted namespace '{}' by user: {}",
        form.namespace, user_id
    );

    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_success("Namespace deleted successfully.").into_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }

    fn populated_html() -> String {
        render_settings(
            &user_fixture(Some("silen@example.com")),
            &[("acme".to_string(), vec![repo_fixture("fig")])],
            &["solo".to_string()],
        )
        .into_string()
    }

    fn empty_html() -> String {
        render_settings(&user_fixture(None), &[], &[]).into_string()
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
    fn test_settings_leads_with_crumbs_and_page_head() {
        let html = populated_html();
        assert!(
            html.contains("<nav class=\"fig-crumbs fig-crumbs--page\" aria-label=\"Breadcrumb\">"),
            "breadcrumb must be a labelled nav landmark: {html}"
        );
        assert!(
            html.contains("<span class=\"fig-crumb-sep\" aria-hidden=\"true\">/</span>"),
            "separators are decorative: {html}"
        );
        assert!(
            html.contains("<span aria-current=\"page\">Settings</span>"),
            "final crumb is a non-link current segment: {html}"
        );
        let crumbs = index_of(&html, "fig-crumbs");
        let head = index_of(&html, "<section class=\"fig-pagehead\">");
        assert!(
            crumbs < head && head < index_of(&html, "fig-bento"),
            "crumbs sit above the page head and bento: {html}"
        );
    }

    #[test]
    fn test_settings_panels_are_a_three_cell_bento_with_head_and_body() {
        let html = populated_html();
        assert_eq!(
            html.matches("class=\"fig-bento\"").count(),
            1,
            "one bento container: {html}"
        );
        assert_eq!(
            html.matches("<section class=\"fig-panel").count(),
            3,
            "Profile, Delete Repository and Delete Namespace panels: {html}"
        );
        assert_eq!(
            html.matches("class=\"fig-panel-head\"").count(),
            3,
            "every panel declares a head: {html}"
        );
        assert_eq!(
            html.matches("<h2 class=\"fig-eyebrow\"").count(),
            3,
            "panel headings are h2 eyebrows under the page h1: {html}"
        );
        for (heading_id, heading_text) in [
            ("settings-profile-heading", "Profile Information"),
            ("settings-delete-repo-heading", "Delete Repository"),
            ("settings-delete-namespace-heading", "Delete Namespace"),
        ] {
            assert!(
                html.contains(&format!("aria-labelledby=\"{heading_id}\"")),
                "panel {heading_id} must be named by its heading: {html}"
            );
            assert!(
                html.contains(&format!(
                    "<h2 class=\"fig-eyebrow\" id=\"{heading_id}\">{heading_text}</h2>"
                )),
                "missing heading {heading_id}: {html}"
            );
        }
        assert!(
            !html.contains("<h4"),
            "panels never nest a fourth heading level: {html}"
        );
    }

    #[test]
    fn test_settings_deletion_panels_carry_danger_and_caution_affordances() {
        let html = populated_html();
        assert_eq!(
            html.matches("fig-panel fig-panel--danger").count(),
            2,
            "both deletion panels take the danger variant: {html}"
        );
        assert_eq!(
            html.matches("<div class=\"fig-notice fig-notice--warning\" role=\"alert\">")
                .count(),
            2,
            "each deletion panel warns before it lists targets: {html}"
        );
        assert_eq!(
            html.matches(">CAUTION<").count(),
            2,
            "the signal word carries the meaning, not the colour: {html}"
        );
        assert!(
            html.contains(
                "Select a repository to permanently delete it. This action cannot be undone."
            ),
            "{html}"
        );
        assert!(
            html.contains(
                "Select a namespace to permanently delete it. This action cannot be undone."
            ),
            "{html}"
        );
        assert_eq!(
            html.matches(
                "<button class=\"fig-btn fig-btn--danger\" type=\"submit\">Delete</button>"
            )
            .count(),
            2,
            "destructive controls are full-height danger buttons: {html}"
        );
    }

    #[test]
    fn test_settings_delete_repo_rows_preserve_the_htmx_contract() {
        let html = populated_html();
        for attribute in [
            "hx-post=\"/settings/delete-repo\"",
            "hx-target=\"#delete-repo-result\"",
            "hx-swap=\"innerHTML\"",
            "hx-confirm=\"Are you sure you want to permanently delete 'acme/fig'?\"",
        ] {
            assert!(
                html.contains(attribute),
                "missing exact attribute {attribute}: {html}"
            );
        }
        assert!(
            html.contains("<form class=\"fig-row fig-row--form\""),
            "deletion targets are technical form rows: {html}"
        );
        assert!(
            html.contains("<span class=\"fig-row-id\">acme/fig</span>"),
            "the row identifier names the same object as the confirm text: {html}"
        );
        for hidden in [
            "<input type=\"hidden\" name=\"namespace\" value=\"acme\">",
            "<input type=\"hidden\" name=\"repo_name\" value=\"fig\">",
        ] {
            assert!(html.contains(hidden), "missing payload {hidden}: {html}");
        }
    }

    #[test]
    fn test_settings_delete_namespace_rows_preserve_the_htmx_contract() {
        let html = populated_html();
        for attribute in [
            "hx-post=\"/settings/delete-namespace/\"",
            "hx-target=\"#delete-namespace-result\"",
            "hx-confirm=\"Are you sure you want to permanently delete the namespace 'solo'? This cannot be undone.\"",
        ] {
            assert!(
                html.contains(attribute),
                "missing exact attribute {attribute}: {html}"
            );
        }
        assert!(
            html.contains("<span class=\"fig-row-id\">solo</span>"),
            "{html}"
        );
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
            html.contains("<label class=\"fig-label\" for=\"email\">Email Address</label>"),
            "every input keeps a real label: {html}"
        );
        assert!(
            html.contains(
                "<input class=\"fig-input\" type=\"email\" name=\"email\" id=\"email\" required value=\"silen@example.com\""
            ),
            "the email input keeps its type, requirement and current value: {html}"
        );
        assert!(
            html.contains(
                "<button class=\"fig-btn fig-btn--primary\" type=\"submit\">Save Email</button>"
            ),
            "{html}"
        );
    }

    #[test]
    fn test_settings_result_targets_announce_swaps() {
        let html = populated_html();
        for target in [
            "settings-result",
            "delete-repo-result",
            "delete-namespace-result",
        ] {
            assert!(
                html.contains(&format!("<div id=\"{target}\" aria-live=\"polite\">")),
                "swap target {target} must be a polite live region: {html}"
            );
        }
    }

    #[test]
    fn test_settings_keeps_identity_visible_and_email_break_safe() {
        let html = populated_html();
        assert!(
            html.contains("<span class=\"fig-label\">Username</span>"),
            "{html}"
        );
        assert!(
            html.contains("<p class=\"fig-body fig-break\">silen</p>"),
            "{html}"
        );
        assert!(
            html.contains("<p class=\"fig-body fig-break\">silen@example.com</p>"),
            "long addresses must wrap rather than overflow: {html}"
        );

        let unset = empty_html();
        assert!(
            unset.contains("<p class=\"fig-body fig-ink-tertiary\">Not set</p>"),
            "a missing email reads as tertiary ink, not as an error: {unset}"
        );
        assert!(unset.contains("silen"), "username stays visible: {unset}");
    }

    #[test]
    fn test_settings_empty_states_name_their_condition_and_offer_no_targets() {
        let html = empty_html();
        assert_eq!(
            html.matches("<div class=\"fig-empty\">").count(),
            2,
            "both deletion panels degrade to empty states: {html}"
        );
        for (eyebrow, body) in [
            (
                ">NO REPOSITORIES<",
                "You don't have any repositories to delete.",
            ),
            (
                ">NO NAMESPACES<",
                "No namespaces available for deletion. You can only delete namespaces you own that have no repositories.",
            ),
        ] {
            assert!(html.contains(eyebrow), "missing {eyebrow}: {html}");
            assert!(html.contains(body), "missing {body}: {html}");
        }
        assert!(
            !html.contains("hx-confirm"),
            "no deletion affordance without a deletable object: {html}"
        );
        assert!(
            !html.contains("fig-btn--danger"),
            "no danger control without a deletable object: {html}"
        );
    }

    #[test]
    fn test_settings_uses_only_fig_design_system_classes() {
        for html in [populated_html(), empty_html()] {
            let classes = classes_in(&html);
            assert!(!classes.is_empty(), "settings should carry classes: {html}");
            for class in classes {
                assert!(
                    class.starts_with("fig-"),
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
        let form: DeleteRepoForm =
            serde_urlencoded::from_str("namespace=ns&repo_name=repo").unwrap();
        assert_eq!(form.namespace, "ns");
        assert_eq!(form.repo_name, "repo");
    }
}
