use super::{
    AwResult, Deserialize, HttpRequest, HttpResponse, Responder, TwigContext, config, get, info,
    post, render_error, render_error_with_action, render_success, web,
};

#[derive(Deserialize)]
pub(super) struct UpdateEmailForm {
    pub(super) email: String,
}

/// Renders the settings page body shared by the full-page and HTMX responses.
pub(super) fn render_settings(user: &crate::auth::User) -> maud::Markup {
    maud::html! {
        nav class="twig-crumbs twig-crumbs--page" aria-label="Breadcrumb" {
            a href="/" { "Namespaces" }
            span class="twig-crumb-sep" aria-hidden="true" { "/" }
            span aria-current="page" { "Account" }
        }

        section class="twig-pagehead" {
        }

        div class="twig-bento" {
            (render_profile_panel(user))
        }
    }
}

/// Identity values plus the email update form.
pub(super) fn render_profile_panel(user: &crate::auth::User) -> maud::Markup {
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

    let user_management_enabled = if server.is_configured_admin(&user.username) {
        true
    } else {
        match db.get_owned_namespaces_for_user(&user_id).await {
            Ok(namespaces) => !namespaces.is_empty(),
            Err(error) => {
                log::error!("Failed to check User Management access: {error}");
                false
            }
        }
    };

    let content = render_settings(&user);

    let content = maud::html! {
        (crate::http::tree::pages::render_tree_hub(
            server.is_test_user(&user.username),
            server.is_configured_admin(&user.username),
            user_management_enabled,
            Some("account"),
        ))
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

pub(super) fn render_settings_auth_error(req: &HttpRequest, message: &str) -> maud::Markup {
    let content = render_error_with_action(message, "/auth/login", "Log in");
    if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::http::view::render_layout(&content, None, Some("Account"))
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
