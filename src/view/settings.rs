use actix_web::Result as AwResult;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use log::info;
use serde::Deserialize;

use crate::auth::FigContext;

#[derive(Deserialize)]
struct UpdateEmailForm {
    email: String,
}

#[get("/settings")]
pub async fn settings_page(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
) -> AwResult<maud::Markup> {
    // Get user from session
    let token = match req.cookie("session") {
        Some(cookie) => cookie.value().to_string(),
        None => {
            return Ok(maud::html! {
                div class="ba b--red br2 pa3 bg-dark-red" {
                    p class="f6 white ma0" { "Not logged in. Please log in first." }
                }
            });
        }
    };

    let user_id = match auth_state.validate_token(&token).await {
        Some(user_id) => user_id,
        None => {
            return Ok(maud::html! {
                div class="ba b--red br2 pa3 bg-dark-red" {
                    p class="f6 white ma0" { "Session expired. Please log in again." }
                }
            });
        }
    };

    let user = match auth_state.db.get_user_by_id(&user_id).await {
        Ok(Some(user)) => user,
        _ => {
            return Ok(maud::html! {
                div class="ba b--red br2 pa3 bg-dark-red" {
                    p class="f6 white ma0" { "Failed to load user." }
                }
            });
        }
    };

    let content = maud::html! {
        // Breadcrumb navigation
        div class="mb4 f6 white-70" {
            a href="/" class="link white-70 hover-white no-underline" { "Home" }
            span class="mh2" { "/" }
            span class="white" { "Settings" }
        }

        h1 class="f3 fw6 white mb4" { "User Settings" }

        div class="ba b--white-20 br2 pa4 bg-black-20" {
            h2 class="f4 fw6 white mb3" { "Profile Information" }

            div class="mb4" {
                label class="db f6 white-70 mb2" { "Username" }
                p class="f5 white ma0" { (user.username) }
            }

            div class="mb4" {
                label class="db f6 white-70 mb2" { "Email" }
                @match &user.email {
                    Some(email) => {
                        p class="f5 white ma0" { (email) }
                    }
                    None => {
                        p class="f5 white-50 ma0" { "Not set" }
                    }
                }
            }

            hr class="bt b--white-20 mv4";

            h3 class="f5 fw6 white mb3" { "Update Email" }

            form
                hx-post="/settings/email"
                hx-target="#settings-result"
                hx-swap="innerHTML"
                class="mb3"
            {
                div class="mb3" {
                    label class="db f6 white-70 mb2" for="email" { "Email Address" }
                    input
                        type="email"
                        name="email"
                        id="email"
                        required
                        value=(user.email.as_deref().unwrap_or(""))
                        class="db w-100 pa2 bg-black white ba b--white-30 br1"
                        placeholder="Enter your email address";
                }
                button
                    type="submit"
                    class="pa2 bg-white black bn br1 pointer hover-bg-white-90"
                {
                    "Save Email"
                }
            }

            div id="settings-result" {}
        }
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(crate::view::render_layout(&content, Some(&user.username)))
    }
}

#[post("/settings/email")]
pub async fn update_email(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
    form: web::Form<UpdateEmailForm>,
) -> impl Responder {
    // Get user from session
    let token = match req.cookie("session") {
        Some(cookie) => cookie.value().to_string(),
        None => {
            return HttpResponse::Unauthorized()
                .body(render_error("Not logged in. Please log in first.").into_string());
        }
    };

    let user_id = match auth_state.validate_token(&token).await {
        Some(user_id) => user_id,
        None => {
            return HttpResponse::Unauthorized()
                .body(render_error("Session expired. Please log in again.").into_string());
        }
    };

    // Validate email format (basic validation)
    if form.email.is_empty() || !form.email.contains('@') {
        return HttpResponse::BadRequest()
            .body(render_error("Please enter a valid email address").into_string());
    }

    // Update email in database
    match auth_state.db.update_user_email(&user_id, &form.email).await {
        Ok(_) => {
            info!("Updated email for user: {}", user_id);
            HttpResponse::Ok()
                .content_type("text/html")
                .body(render_success("Email updated successfully!").into_string())
        }
        Err(e) => {
            log::error!("Failed to update email: {}", e);
            HttpResponse::InternalServerError()
                .body(render_error("Failed to update email").into_string())
        }
    }
}

fn render_error(message: &str) -> maud::Markup {
    maud::html! {
        div class="ba b--red br2 pa3 bg-dark-red mt3" {
            p class="f6 white ma0" { (message) }
        }
    }
}

fn render_success(message: &str) -> maud::Markup {
    maud::html! {
        div class="ba b--green br2 pa3 bg-dark-green mt3" {
            p class="f6 white ma0" { (message) }
        }
    }
}
