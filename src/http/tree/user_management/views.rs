use maud::{Markup, html};

use crate::auth::{NamespaceInvitation, UserManagementEntry, permissions::PermissionSet};
use crate::email::InvitationDelivery;

use super::access::UserManagementScope;
use super::invitations::{format_expiry, is_expired};

pub(super) fn render_user_management_tabs(active_tab: &str) -> Markup {
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
    }
}

pub(super) fn render_user_management_content(
    scope: &UserManagementScope,
    active_tab: &str,
    invitations: &[NamespaceInvitation],
    users: &[UserManagementEntry],
) -> Markup {
    let is_invites = active_tab == "invites";

    html! {
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
                        div class="twig-access-list" {
                            @for user in users {
                                (render_user_access_tree(user))
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_permission_list(permissions: &PermissionSet) -> Markup {
    html! {
        ul class="twig-access-permissions" aria-label="Permissions" {
            @for permission in permissions.iter() {
                li class="twig-access-permission" { (permission.display_name()) }
            }
        }
    }
}

fn render_user_access_tree(user: &UserManagementEntry) -> Markup {
    html! {
        details class="twig-access-user" open {
            summary class="twig-access-user-summary" {
                span class="twig-row-id" { (&user.username) }
                span class="twig-row-meta" {
                    @if let Some(email) = &user.email {
                        (email)
                    } @else {
                        "Email not set"
                    }
                }
                @if user.is_site_admin {
                    span class="twig-access-role twig-access-role--admin" { "Admin" }
                } @else {
                    span class="twig-access-role" {
                        (user.memberships.len()) " namespace" @if user.memberships.len() != 1 { "s" }
                    }
                }
            }
            @if user.is_site_admin {
                ul class="twig-access-namespaces" {
                    li class="twig-access-namespace" {
                        details open {
                            summary { "All namespaces" }
                            div class="twig-access-role-group" {
                                p class="twig-access-role" { "Admin" }
                                (render_permission_list(&PermissionSet::all()))
                            }
                        }
                    }
                }
            } @else if user.memberships.is_empty() {
                p class="twig-access-empty" { "No namespace membership" }
            } @else {
                ul class="twig-access-namespaces" {
                    @for (namespace, role) in &user.memberships {
                        li class="twig-access-namespace" {
                            details open {
                                summary { (namespace) }
                                div class="twig-access-role-group" {
                                    p class="twig-access-role" { (role.display_name()) }
                                    (render_permission_list(&PermissionSet::from_role(*role)))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn render_user_management(
    scope: &UserManagementScope,
    active_tab: &str,
    invitations: &[NamespaceInvitation],
    users: &[UserManagementEntry],
) -> Markup {
    html! {
        (render_user_management_tabs(active_tab))
        div id="user-management-content" {
            (render_user_management_content(scope, active_tab, invitations, users))
        }
    }
}

pub(super) fn render_invitation_list(invitations: &[NamespaceInvitation], oob: bool) -> Markup {
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

pub(super) fn render_invitation_created(
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
