use actix_web::{HttpRequest, HttpResponse, get, post, web};

mod invitations;
pub use invitations::{create_namespace_invitation, resend_namespace_invitation};
use invitations::{invitations_for_scope, render_invitation_list};
#[cfg(test)]
use invitations::{parse_expiry, render_invitation_created};
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

#[cfg(test)]
#[path = "user_management/tests.rs"]
mod tests;
