use actix_web::{HttpRequest, HttpResponse, get, post, web};
use maud::{Markup, html};
use serde::Deserialize;

use crate::{
    auth::{NamespaceInvitation, TwigContext, create_user},
    http::view::{render_error, render_layout},
};

#[derive(Debug, Deserialize)]
pub struct AcceptInvitationForm {
    username: String,
    password: String,
}

fn render_invitation_setup(invitation: &NamespaceInvitation) -> Markup {
    let action = format!("/auth/accept-invite/{}", invitation.token);
    html! {
        div id="auth-content" {
            div class="twig-stack twig-form--narrow" {
                section class="twig-panel" aria-labelledby="accept-invite-title" {
                    header class="twig-panel-head" {
                        h1 class="twig-eyebrow" id="accept-invite-title" {
                            "Join the " (&invitation.namespace_name) " namespace"
                        }
                    }
                    div class="twig-panel-body twig-stack" {
                        p class="twig-body-sm twig-ink-secondary" {
                            "You were invited as a " (invitation.role.display_name()) ". Set up your account to continue."
                        }
                        form
                            class="twig-form"
                            hx-post=(action)
                            hx-target="#accept-invite-result"
                            hx-swap="innerHTML"
                            "hx-status:4xx"="swap:innerHTML target:#accept-invite-result"
                            "hx-status:5xx"="swap:innerHTML target:#accept-invite-result"
                        {
                            div class="twig-field" {
                                label class="twig-label" for="invite-email" { "Email address" }
                                input class="twig-input" id="invite-email" type="email" value=(invitation.email) readonly;
                            }
                            div class="twig-field" {
                                label class="twig-label" for="invite-username" { "Username" }
                                input class="twig-input" id="invite-username" name="username" type="text" required minlength="3" autocomplete="username";
                            }
                            div class="twig-field" {
                                label class="twig-label" for="invite-password" { "Password" }
                                input class="twig-input" id="invite-password" name="password" type="password" required minlength="8" autocomplete="new-password";
                            }
                            div class="twig-form-actions" {
                                button class="twig-btn twig-btn--primary" type="submit" { "Create account and join namespace" }
                            }
                        }
                        div id="accept-invite-result" aria-live="polite" {}
                    }
                }
            }
        }
    }
}

fn render_account_created(username: &str, namespace: &str) -> Markup {
    html! {
        div class="twig-notice twig-notice--success" role="status" {
            p class="twig-eyebrow" { "ACCOUNT CREATED" }
            p class="twig-notice-body" {
                "Welcome, " (username) ". Your account joined the " (namespace) " namespace. Log in to continue."
            }
            div class="twig-notice-actions" {
                a class="twig-btn twig-btn--primary" href="/auth/login" { "Log in" }
            }
        }
    }
}

async fn load_active_invitation(
    token: &str,
    auth_state: &web::Data<TwigContext>,
) -> Result<NamespaceInvitation, HttpResponse> {
    let invitation = match auth_state.db().get_namespace_invitation(token).await {
        Ok(Some(invitation)) => invitation,
        Ok(None) => {
            return Err(
                HttpResponse::NotFound().body(render_error("Invitation not found").into_string())
            );
        }
        Err(error) => {
            log::error!("Failed to load namespace invitation: {error}");
            return Err(HttpResponse::InternalServerError()
                .body(render_error("Failed to load invitation").into_string()));
        }
    };

    if invitation.accepted_at.is_some() || invitation.accepted_user_id.is_some() {
        return Err(HttpResponse::Gone()
            .body(render_error("This invitation has already been accepted").into_string()));
    }
    if let Some(expires_at) = invitation.expires_at.as_deref() {
        let expires_at = match chrono::DateTime::parse_from_rfc3339(expires_at) {
            Ok(expires_at) => expires_at,
            Err(error) => {
                log::error!("Invitation expiry could not be parsed: {error}");
                return Err(HttpResponse::Gone()
                    .body(render_error("This invitation has an invalid expiry").into_string()));
            }
        };
        if expires_at <= chrono::Utc::now() {
            return Err(HttpResponse::Gone()
                .body(render_error("This invitation has expired").into_string()));
        }
    }

    Ok(invitation)
}

#[get("/auth/accept-invite/{token}")]
pub async fn accept_invitation_page(
    req: HttpRequest,
    path: web::Path<String>,
    auth_state: web::Data<TwigContext>,
) -> HttpResponse {
    let invitation = match load_active_invitation(&path, &auth_state).await {
        Ok(invitation) => invitation,
        Err(response) => return response,
    };
    let content = render_invitation_setup(&invitation);
    let content = if req.headers().contains_key("HX-Request") {
        content
    } else {
        render_layout(&content, None, Some("Accept Invitation"))
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}

#[post("/auth/accept-invite/{token}")]
pub async fn accept_invitation_handler(
    path: web::Path<String>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<AcceptInvitationForm>,
) -> HttpResponse {
    let invitation = match load_active_invitation(&path, &auth_state).await {
        Ok(invitation) => invitation,
        Err(response) => return response,
    };

    let username = form.username.trim();
    if username.chars().count() < 3 {
        return HttpResponse::BadRequest()
            .body(render_error("Username must be at least 3 characters").into_string());
    }
    if form.password.len() < 8 {
        return HttpResponse::BadRequest()
            .body(render_error("Password must be at least 8 characters").into_string());
    }

    match auth_state.db().get_user_by_username(username).await {
        Ok(Some(_)) => {
            return HttpResponse::Conflict()
                .body(render_error("Username already exists").into_string());
        }
        Ok(None) => {}
        Err(error) => {
            log::error!("Failed to check invitation username: {error}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to check username").into_string());
        }
    }

    let user = match create_user(
        username.to_string(),
        invitation.email.clone(),
        &form.password,
    ) {
        Ok(user) => user,
        Err(error) => {
            log::error!("Failed to prepare invited account: {error}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to prepare account").into_string());
        }
    };

    match auth_state
        .db()
        .accept_namespace_invitation(&invitation.token, &user)
        .await
    {
        Ok(()) => HttpResponse::Ok()
            .content_type("text/html")
            .body(render_account_created(&user.username, &invitation.namespace_name).into_string()),
        Err(error) if error == "Username already exists" => {
            HttpResponse::Conflict().body(render_error("Username already exists").into_string())
        }
        Err(error) if error == "Invitation not found" => {
            HttpResponse::NotFound().body(render_error("Invitation not found").into_string())
        }
        Err(error)
            if error == "Invitation has already been accepted"
                || error == "Invitation has expired"
                || error == "Invitation expiry is invalid" =>
        {
            HttpResponse::Gone().body(render_error(&error).into_string())
        }
        Err(error) if error == "Account email does not match invitation" => {
            HttpResponse::BadRequest().body(render_error(&error).into_string())
        }
        Err(error) => {
            log::error!("Failed to accept namespace invitation: {error}");
            HttpResponse::InternalServerError()
                .body(render_error("Failed to create account").into_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::NamespaceRole;

    fn invitation() -> NamespaceInvitation {
        NamespaceInvitation {
            token: "token-123".to_string(),
            email: "sam@example.com".to_string(),
            namespace_id: "namespace-1".to_string(),
            namespace_name: "acme".to_string(),
            role: NamespaceRole::Contributor,
            created_at: "2026-10-10T00:00:00Z".to_string(),
            expires_at: Some("2026-10-17T00:00:00Z".to_string()),
            accepted_at: None,
            accepted_user_id: None,
        }
    }

    #[test]
    fn invitation_setup_prefills_email_and_shows_namespace_and_role() {
        let markup = render_invitation_setup(&invitation()).into_string();
        for expected in [
            "hx-post=\"/auth/accept-invite/token-123\"",
            "type=\"email\" value=\"sam@example.com\" readonly",
            "Join the acme namespace",
            "invited as a Contributor",
            "name=\"username\"",
            "name=\"password\"",
        ] {
            assert!(markup.contains(expected), "missing {expected}: {markup}");
        }
        assert!(!markup.contains("name=\"email\""));
    }

    #[test]
    fn account_created_notice_offers_login_and_names_the_namespace() {
        let markup = render_account_created("sam", "acme").into_string();
        assert!(markup.contains("ACCOUNT CREATED"));
        assert!(markup.contains("Welcome, sam"));
        assert!(markup.contains("acme namespace"));
        assert!(markup.contains("href=\"/auth/login\""));
    }
}
