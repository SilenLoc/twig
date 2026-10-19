use actix_web::Result as AwResult;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use log::info;

use crate::auth::TwigContext;
use crate::config;
use crate::http::view::{render_error, render_success};

use super::UpdateEmailForm;
use super::shared::{render_account_tabs, render_settings_auth_error, render_settings_navigation};

/// Renders the settings page body shared by the full-page and HTMX responses.
pub(super) fn render_settings(user: &crate::auth::User) -> maud::Markup {
    maud::html! {
        (render_account_tabs("profile"))
        div class="twig-bento" {
            (render_profile_panel(user))
        }
    }
}

/// Identity values plus the email update form.
fn render_profile_panel(user: &crate::auth::User) -> maud::Markup {
    maud::html! {
        section class="twig-panel" aria-labelledby="settings-profile-heading" {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="settings-profile-heading" { "Account" }
            }
            div class="twig-panel-body twig-stack" {
                div class="twig-field" {
                    span class="twig-label" { "Username" }
                    p class="twig-body twig-break" { (user.username) }
                }

                div class="twig-field" {
                    span class="twig-label" { "Email" }
                    @match &user.email {
                        Some(email) => { p class="twig-body twig-break" { (email) } }
                        None => { p class="twig-body twig-ink-tertiary" { "Not set" } }
                    }
                }

                form
                    class="twig-form"
                    aria-labelledby="settings-email-heading"
                    hx-post="/settings/email"
                    hx-target="#settings-result"
                    hx-swap="innerHTML"
                {
                    h3 class="twig-subsection" id="settings-email-heading" { "Update Email" }

                    div class="twig-field" {
                        label class="twig-label" for="email" { "Email Address" }
                        input
                            class="twig-input"
                            type="email"
                            name="email"
                            id="email"
                            required
                            value=(user.email.as_deref().unwrap_or(""))
                            placeholder="Enter your email address";
                    }

                    div class="twig-form-actions" {
                        button class="twig-btn twig-btn--primary" type="submit" { "Save Email" }
                    }
                }

                div id="settings-result" aria-live="polite" {}
            }
        }
    }
}

/// Repositories the user may delete, one technical form row each.
#[get("/settings")]
pub async fn settings_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> AwResult<maud::Markup> {
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        let message = if req.cookie("session").is_some() {
            "Session expired. Please log in again."
        } else {
            "Not logged in. Please log in first."
        };
        return Ok(render_settings_auth_error(&req, message));
    };

    let db = auth_state.db();

    let Ok(Some(user)) = db.get_user_by_id(&user_id).await else {
        return Ok(render_error("Failed to load user."));
    };

    let navigation = render_settings_navigation(&server, db, &user).await;
    let content = render_settings(&user);

    let content = maud::html! {
        (navigation)
        (content)
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(crate::http::view::render_layout(
            &content,
            Some(&user.username),
            Some("Account"),
        ))
    }
}

#[post("/settings/email")]
pub async fn update_email(
    req: HttpRequest,
    auth_state: web::Data<TwigContext>,
    form: web::Form<UpdateEmailForm>,
) -> impl Responder {
    // Validate email format (basic validation) before requiring a session,
    // so malformed input is rejected as BAD_REQUEST regardless of auth state.
    if form.email.is_empty() || !form.email.contains('@') {
        return HttpResponse::BadRequest()
            .body(render_error("Please enter a valid email address").into_string());
    }

    // Get user from session
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };

    let db = auth_state.db();

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
