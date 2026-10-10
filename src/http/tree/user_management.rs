use actix_web::{HttpRequest, HttpResponse, get, post, web};
use maud::{Markup, html};
use serde::Deserialize;

use crate::{
    auth::{
        Namespace, NamespaceInvitation, NamespaceRole, NewNamespaceInvitation, TwigContext, User,
        UserManagementEntry,
    },
    config,
    email::{InvitationDelivery, invitation_url, send_invitation_if_configured},
    http::view::{render_error, render_layout},
};

#[derive(Debug, Deserialize)]
pub struct UserManagementQuery {
    tab: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvitationForm {
    email: String,
    namespace: String,
    #[serde(default = "default_invitation_role")]
    role: String,
    expires: String,
}

fn default_invitation_role() -> String {
    "contributor".to_string()
}

struct UserManagementScope {
    user: User,
    is_site_admin: bool,
    namespaces: Vec<Namespace>,
}

async fn user_management_scope(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
) -> Result<UserManagementScope, HttpResponse> {
    let Some(user_id) = auth_state.user_id_from_request(req).await else {
        return Err(HttpResponse::Found()
            .insert_header(("Location", "/auth/login"))
            .finish());
    };

    let db = auth_state.db();
    let user = match db.get_user_by_id(&user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return Err(HttpResponse::Unauthorized().finish()),
        Err(error) => {
            log::error!("Failed to load User Management actor: {error}");
            return Err(HttpResponse::InternalServerError().finish());
        }
    };

    let is_site_admin = server.is_configured_admin(&user.username);
    let namespaces: Vec<Namespace> = if is_site_admin {
        match db.get_all_namespaces_with_owners().await {
            Ok(namespaces) => namespaces
                .into_iter()
                .map(|(namespace, _)| namespace)
                .collect(),
            Err(error) => {
                log::error!("Failed to load namespaces for User Management: {error}");
                return Err(HttpResponse::InternalServerError().finish());
            }
        }
    } else {
        match db.get_owned_namespaces_for_user(&user_id).await {
            Ok(namespaces) => namespaces,
            Err(error) => {
                log::error!("Failed to load owned namespaces for User Management: {error}");
                return Err(HttpResponse::InternalServerError().finish());
            }
        }
    };

    if namespaces.is_empty() && !is_site_admin {
        return Err(HttpResponse::Forbidden().body("Namespace owner access required"));
    }

    Ok(UserManagementScope {
        user,
        is_site_admin,
        namespaces,
    })
}

fn render_user_management(
    scope: &UserManagementScope,
    active_tab: &str,
    invitations: &[NamespaceInvitation],
    users: &[UserManagementEntry],
) -> Markup {
    let is_invites = active_tab == "invites";

    html! {
        nav class="twig-tabs" aria-label="User management" {
            a
                class="twig-tab"
                aria-current=[(!is_invites).then_some("page")]
                href="/tree/users?tab=users"
                hx-get="/tree/users?tab=users"
                hx-target="#user-management-content"
                hx-push-url="true"
            { "Users" }
            a
                class="twig-tab"
                aria-current=[is_invites.then_some("page")]
                href="/tree/users?tab=invites"
                hx-get="/tree/users?tab=invites"
                hx-target="#user-management-content"
                hx-push-url="true"
            { "Invites" }
        }
        div id="user-management-content" {
            @if is_invites {
                section class="twig-panel" aria-labelledby="invite-heading" {
                    header class="twig-panel-head" {
                        h1 class="twig-eyebrow" id="invite-heading" { "Invite someone" }
                    }
                    div class="twig-panel-body twig-stack" {
                        p class="twig-body-sm twig-ink-secondary" {
                            "Create a namespace invitation. Copy and share the link; email delivery is not wired yet."
                        }
                        form
                            class="twig-form"
                            hx-post="/tree/invites"
                            hx-target="#invite-result"
                            hx-swap="innerHTML"
                            "hx-status:4xx"="swap:innerHTML target:#invite-result"
                            "hx-status:5xx"="swap:innerHTML target:#invite-result"
                        {
                            div class="twig-field" {
                                label class="twig-label" for="invite-email" { "Email address" }
                                input class="twig-input" id="invite-email" name="email" type="email" required placeholder="person@example.com";
                            }
                            div class="twig-field" {
                                label class="twig-label" for="invite-namespace" { "Namespace" }
                                select class="twig-input" id="invite-namespace" name="namespace" required {
                                    @for namespace in &scope.namespaces {
                                        option value=(namespace.name) { (namespace.name) }
                                    }
                                }
                            }
                            div class="twig-field" {
                                label class="twig-label" for="invite-role" { "Role in namespace" }
                                select class="twig-input" id="invite-role" name="role" {
                                    option value="contributor" selected { "Contributor" }
                                    option value="owner" { "Owner" }
                                }
                            }
                            div class="twig-field" {
                                label class="twig-label" for="invite-expires" { "Link expires" }
                                select class="twig-input" id="invite-expires" name="expires" {
                                    option value="1d" { "In 1 day" }
                                    option value="7d" selected { "In 7 days" }
                                    option value="30d" { "In 30 days" }
                                    option value="never" { "Never" }
                                }
                            }
                            div class="twig-form-actions" {
                                button class="twig-btn twig-btn--primary" type="submit" { "Create invitation" }
                            }
                        }
                        div id="invite-result" aria-live="polite" {}
                    }
                }
                (render_invitation_list(invitations, false))
            } @else {
                section class="twig-panel" aria-labelledby="users-heading" {
                    header class="twig-panel-head" {
                        h1 class="twig-eyebrow" id="users-heading" { "Users" }
                    }
                    div class="twig-panel-body" {
                        @if users.is_empty() {
                            p class="twig-empty-body" { "No users found." }
                        } @else {
                            div class="twig-list" {
                                @for user in users {
                                    div class="twig-row" {
                                        span class="twig-row-id" { (&user.username) }
                                        span class="twig-row-meta" {
                                            @if let Some(email) = &user.email {
                                                (email)
                                            } @else {
                                                "Email not set"
                                            }
                                        }
                                        @if user.memberships.is_empty() {
                                            span class="twig-row-meta" {
                                                "No namespace membership"
                                            }
                                        } @else {
                                            span class="twig-row-meta" {
                                                @for (index, (namespace, role)) in user.memberships.iter().enumerate() {
                                                    @if index > 0 { ", " }
                                                    (namespace) " · " (role.display_name())
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
        }
    }
}

fn render_invitation_list(invitations: &[NamespaceInvitation], oob: bool) -> Markup {
    let now = chrono::Utc::now();
    html! {
        section
            id="invitation-list"
            hx-swap-oob=[oob.then_some("outerHTML")]
            class="twig-panel"
            aria-labelledby="invitation-list-heading"
        {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="invitation-list-heading" { "Invitations" }
            }
            div class="twig-panel-body" {
                @if invitations.is_empty() {
                    p class="twig-empty-body" { "No invitations yet." }
                } @else {
                    div class="twig-list" {
                        @for invitation in invitations {
                            @let status = if invitation.accepted_at.is_some()
                                || invitation.accepted_user_id.is_some() {
                                "Accepted"
                            } else if is_expired(invitation, now) {
                                "Expired"
                            } else {
                                "Pending"
                            };
                            div class="twig-row" {
                                span class="twig-row-id" { (&invitation.email) }
                                span class="twig-row-meta" {
                                    (&invitation.namespace_name) " · " (invitation.role.display_name()) " · " (status)
                                    @if let Some(expires_at) = &invitation.expires_at {
                                        " · expires " (format_expiry(expires_at))
                                    } @else {
                                        " · never expires"
                                    }
                                }
                                @if status == "Pending" {
                                    form
                                        hx-post=(format!("/tree/invites/{}/resend", invitation.token))
                                        hx-target="#invite-result"
                                        hx-swap="innerHTML"
                                        "hx-status:4xx"="swap:innerHTML target:#invite-result"
                                        "hx-status:5xx"="swap:innerHTML target:#invite-result"
                                    {
                                        button
                                            class="twig-btn twig-btn--ghost"
                                            type="submit"
                                            aria-label=(format!("Resend invitation to {}", invitation.email))
                                        { "Resend email" }
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

fn is_expired(invitation: &NamespaceInvitation, now: chrono::DateTime<chrono::Utc>) -> bool {
    invitation.expires_at.as_deref().is_some_and(|expires_at| {
        chrono::DateTime::parse_from_rfc3339(expires_at).is_ok_and(|expires_at| expires_at <= now)
    })
}

fn format_expiry(expires_at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(expires_at)
        .map(|date| date.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn invitations_for_scope(scope: &UserManagementScope) -> Option<Vec<String>> {
    (!scope.is_site_admin).then(|| {
        scope
            .namespaces
            .iter()
            .map(|namespace| namespace.id.clone())
            .collect()
    })
}

fn parse_expiry(value: &str, now: chrono::DateTime<chrono::Utc>) -> Result<Option<String>, ()> {
    let days = match value {
        "1d" => 1,
        "7d" => 7,
        "30d" => 30,
        "never" => return Ok(None),
        _ => return Err(()),
    };
    Ok(Some((now + chrono::Duration::days(days)).to_rfc3339()))
}

fn render_invitation_created(
    invitation: &NamespaceInvitation,
    invite_url: &str,
    invitations: &[NamespaceInvitation],
    delivery: &InvitationDelivery,
) -> Markup {
    let (eyebrow, message, class, role) = match delivery {
        InvitationDelivery::Sent => (
            "EMAIL SENT",
            format!("Invitation email sent to {}.", invitation.email),
            "twig-notice--success",
            "status",
        ),
        InvitationDelivery::SkippedNoApiKey => (
            "INVITATION CREATED",
            format!(
                "No email was sent because RESEND_API_KEY is not set. Copy and share the link with {}.",
                invitation.email
            ),
            "twig-notice--warning",
            "status",
        ),
        InvitationDelivery::Failed(reason) => (
            "EMAIL NOT SENT",
            if reason.contains("PUBLIC_BASE_URL") {
                "Email was not sent: configure PUBLIC_BASE_URL before using Resend.".to_string()
            } else {
                "Email was not sent. Retry delivery or share this link directly.".to_string()
            },
            "twig-notice--warning",
            "alert",
        ),
    };
    html! {
        div class=(format!("twig-notice {class}")) role=(role) {
            p class="twig-eyebrow" { (eyebrow) }
            p class="twig-notice-body" { (message) }
            p class="twig-code" { (invite_url) }
            a class="twig-btn twig-btn--primary" href=(format!("/auth/accept-invite/{}", invitation.token)) {
                "Open account setup"
            }
        }
        (render_invitation_list(invitations, true))
    }
}

fn display_invite_url(req: &HttpRequest, server: &config::Server, token: &str) -> String {
    if let Some(public_base_url) = server.public_base_url()
        && let Ok(url) = invitation_url(public_base_url, token)
    {
        return url;
    }
    let connection_info = req.connection_info();
    format!(
        "{}://{}/auth/accept-invite/{token}",
        connection_info.scheme(),
        connection_info.host(),
    )
}

async fn deliver_invitation(
    server: &config::Server,
    invitation: &NamespaceInvitation,
) -> InvitationDelivery {
    send_invitation_if_configured(
        server.resend_api_key(),
        server.resend_from(),
        server.public_base_url(),
        &invitation.email,
        &invitation.namespace_name,
        invitation.role,
        &invitation.token,
    )
    .await
}

#[get("/tree/users")]
pub async fn user_management_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<UserManagementQuery>,
) -> HttpResponse {
    let scope = match user_management_scope(&req, &server, &auth_state).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let active_tab = query
        .tab
        .as_deref()
        .filter(|tab| matches!(*tab, "users" | "invites"))
        .unwrap_or("invites");
    let namespace_ids = invitations_for_scope(&scope);
    let invitations = match auth_state
        .db()
        .list_namespace_invitations(namespace_ids.as_deref())
        .await
    {
        Ok(invitations) => invitations,
        Err(error) => {
            log::error!("Failed to load namespace invitations: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let users = if active_tab == "users" {
        match auth_state
            .db()
            .list_users_for_management(namespace_ids.as_deref())
            .await
        {
            Ok(users) => users,
            Err(error) => {
                log::error!("Failed to load users for User Management: {error}");
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        Vec::new()
    };
    let content = render_user_management(&scope, active_tab, &invitations, &users);
    let content = if req.headers().contains_key("HX-Request") {
        content
    } else {
        let page = maud::html! {
            (crate::http::tree::pages::render_tree_hub(
                server.is_test_user_enabled(),
                scope.is_site_admin,
                true,
                Some("users"),
            ))
            (content)
        };
        render_layout(&page, Some(&scope.user.username), Some("User Management"))
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}

#[post("/tree/invites")]
pub async fn create_namespace_invitation(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<CreateInvitationForm>,
) -> HttpResponse {
    let scope = match user_management_scope(&req, &server, &auth_state).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let email = form.email.trim();
    if email.is_empty() || !email.contains('@') {
        return HttpResponse::BadRequest()
            .body(render_error("Enter a valid email address").into_string());
    }
    if !scope
        .namespaces
        .iter()
        .any(|namespace| namespace.name == form.namespace)
    {
        return HttpResponse::Forbidden()
            .body(render_error("You cannot invite people to that namespace").into_string());
    }
    let Some(role) = NamespaceRole::parse(&form.role) else {
        return HttpResponse::BadRequest()
            .body(render_error("Choose a supported role").into_string());
    };
    let expires_at = match parse_expiry(&form.expires, chrono::Utc::now()) {
        Ok(expires_at) => expires_at,
        Err(()) => {
            return HttpResponse::BadRequest()
                .body(render_error("Choose a supported invitation expiry").into_string());
        }
    };
    let namespace = scope
        .namespaces
        .iter()
        .find(|namespace| namespace.name == form.namespace)
        .expect("namespace was checked against the authorized scope");
    let invitation = match auth_state
        .db()
        .create_namespace_invitation(&NewNamespaceInvitation {
            email: email.to_string(),
            namespace_id: namespace.id.clone(),
            role,
            expires_at,
        })
        .await
    {
        Ok(invitation) => invitation,
        Err(error) => {
            log::error!("Failed to create namespace invitation: {error}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to create invitation").into_string());
        }
    };
    let invite_url = display_invite_url(&req, &server, &invitation.token);
    let delivery = deliver_invitation(&server, &invitation).await;
    if let InvitationDelivery::Failed(error) = &delivery {
        log::warn!(
            "Resend delivery failed for namespace {}: {error}",
            invitation.namespace_name
        );
    }
    let namespace_ids = invitations_for_scope(&scope);
    let invitations = match auth_state
        .db()
        .list_namespace_invitations(namespace_ids.as_deref())
        .await
    {
        Ok(invitations) => invitations,
        Err(error) => {
            log::error!("Failed to refresh namespace invitations: {error}");
            return HttpResponse::InternalServerError().body(
                render_error("Invitation created, but the list could not be refreshed")
                    .into_string(),
            );
        }
    };

    HttpResponse::Ok().content_type("text/html").body(
        render_invitation_created(&invitation, &invite_url, &invitations, &delivery).into_string(),
    )
}

#[post("/tree/invites/{id}/resend")]
pub async fn resend_namespace_invitation(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<String>,
) -> HttpResponse {
    let scope = match user_management_scope(&req, &server, &auth_state).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let invitation = match auth_state.db().get_namespace_invitation(&path).await {
        Ok(Some(invitation)) => invitation,
        Ok(None) => {
            return HttpResponse::NotFound()
                .body(render_error("Invitation not found").into_string());
        }
        Err(error) => {
            log::error!("Failed to load invitation for resend: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    if !scope
        .namespaces
        .iter()
        .any(|namespace| namespace.id == invitation.namespace_id)
    {
        return HttpResponse::Forbidden()
            .body(render_error("You cannot resend this invitation").into_string());
    }
    if invitation.accepted_at.is_some() || invitation.accepted_user_id.is_some() {
        return HttpResponse::Gone()
            .body(render_error("This invitation has already been accepted").into_string());
    }
    if is_expired(&invitation, chrono::Utc::now()) {
        return HttpResponse::Gone()
            .body(render_error("This invitation has expired").into_string());
    }

    let delivery = deliver_invitation(&server, &invitation).await;
    if let InvitationDelivery::Failed(error) = &delivery {
        log::warn!(
            "Resend delivery retry failed for namespace {}: {error}",
            invitation.namespace_name
        );
    }
    let invite_url = display_invite_url(&req, &server, &invitation.token);
    let namespace_ids = invitations_for_scope(&scope);
    let invitations = match auth_state
        .db()
        .list_namespace_invitations(namespace_ids.as_deref())
        .await
    {
        Ok(invitations) => invitations,
        Err(error) => {
            log::error!("Failed to refresh invitations after resend: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().content_type("text/html").body(
        render_invitation_created(&invitation, &invite_url, &invitations, &delivery).into_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> UserManagementScope {
        UserManagementScope {
            user: User {
                id: "user-1".to_string(),
                username: "admin".to_string(),
                email: Some("admin@example.com".to_string()),
                password_hash: "hash".to_string(),
                created_at: "2026-10-10T00:00:00Z".to_string(),
            },
            is_site_admin: true,
            namespaces: vec![Namespace {
                id: "namespace-1".to_string(),
                name: "acme".to_string(),
                owner_id: "user-1".to_string(),
                created_at: "2026-10-10T00:00:00Z".to_string(),
            }],
        }
    }

    fn invitation() -> NamespaceInvitation {
        NamespaceInvitation {
            token: "f".repeat(64),
            email: "invitee@example.com".to_string(),
            namespace_id: "namespace-1".to_string(),
            namespace_name: "acme".to_string(),
            role: NamespaceRole::Contributor,
            created_at: "2026-10-10T00:00:00Z".to_string(),
            expires_at: Some((chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339()),
            accepted_at: None,
            accepted_user_id: None,
        }
    }

    #[test]
    fn invite_tab_renders_htmx_form_with_contributor_default_and_expiry() {
        let markup = render_user_management(&scope(), "invites", &[], &[]).into_string();
        for expected in [
            "hx-post=\"/tree/invites\"",
            "name=\"email\"",
            "name=\"namespace\"",
            "value=\"contributor\" selected",
            "value=\"7d\" selected",
            "Never",
            "hx-target=\"#invite-result\"",
            "No invitations yet.",
            "Create invitation",
        ] {
            assert!(markup.contains(expected), "missing {expected}: {markup}");
        }
    }

    #[test]
    fn users_tab_is_reachable_with_htmx_and_renders_scoped_placeholder() {
        let markup = render_user_management(&scope(), "users", &[], &[]).into_string();
        assert!(markup.contains("hx-get=\"/tree/users?tab=users\""));
        assert!(markup.contains("No users found."));
    }

    #[test]
    fn user_memberships_render_in_a_single_table_cell() {
        let user = UserManagementEntry {
            username: "silen".to_string(),
            email: Some("silen@example.com".to_string()),
            memberships: vec![
                ("experiments".to_string(), NamespaceRole::Owner),
                ("hco".to_string(), NamespaceRole::Owner),
            ],
        };

        let markup = render_user_management(&scope(), "users", &[], &[user]).into_string();

        assert!(markup.contains("experiments · Owner, hco · Owner"));
        assert_eq!(markup.matches("class=\"twig-row-meta\"").count(), 2);
    }

    #[test]
    fn expiry_values_map_to_expected_expiration() {
        let now = chrono::Utc::now();
        assert!(parse_expiry("never", now).unwrap().is_none());
        for (value, days) in [("1d", 1), ("7d", 7), ("30d", 30)] {
            let expiry = parse_expiry(value, now).unwrap().unwrap();
            let expiry = chrono::DateTime::parse_from_rfc3339(&expiry).unwrap();
            assert_eq!(
                expiry.timestamp(),
                (now + chrono::Duration::days(days)).timestamp()
            );
        }
        assert!(parse_expiry("forever", now).is_err());
    }

    #[test]
    fn pending_invitation_lists_have_an_htmx_resend_action() {
        let invitation = invitation();
        let markup = render_invitation_list(&[invitation], false).into_string();
        assert!(markup.contains("Pending"));
        assert!(markup.contains("hx-post=\"/tree/invites/"));
        assert!(markup.contains("/resend\""));
        assert!(markup.contains("aria-label=\"Resend invitation to invitee@example.com\""));
    }

    #[test]
    fn delivery_notices_preserve_link_and_distinguish_sent_skipped_and_failed() {
        let invitation = invitation();
        let url = format!(
            "https://twig.example/auth/accept-invite/{}",
            invitation.token
        );
        for (delivery, expected) in [
            (InvitationDelivery::Sent, "EMAIL SENT"),
            (
                InvitationDelivery::SkippedNoApiKey,
                "RESEND_API_KEY is not set",
            ),
            (
                InvitationDelivery::Failed("network error".to_string()),
                "Retry delivery or share this link directly",
            ),
        ] {
            let markup =
                render_invitation_created(&invitation, &url, &[invitation.clone()], &delivery)
                    .into_string();
            assert!(markup.contains(expected), "missing {expected}: {markup}");
            assert!(markup.contains(&url));
            assert!(markup.contains("hx-swap-oob=\"outerHTML\""));
        }
    }
}
