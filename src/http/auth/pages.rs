use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use serde::Deserialize;

use crate::{
    auth::TwigContext,
    http::{
        auth::session::{get_username_from_request, safe_device_return_to},
        view::render_layout,
    },
};

#[derive(Deserialize)]
struct LoginQuery {
    return_to: Option<String>,
}

#[derive(Default)]
struct AuthField<'a> {
    id: &'a str,
    label: &'a str,
    input_type: &'a str,
    placeholder: Option<&'a str>,
    hint: Option<&'a str>,
    minlength: Option<&'a str>,
    mono: bool,
}

fn render_field(field: &AuthField<'_>) -> maud::Markup {
    let hint_id = field.hint.map(|_| format!("{}-hint", field.id));
    let input_class = if field.mono {
        "twig-input twig-input--mono"
    } else {
        "twig-input"
    };

    maud::html! {
        div class="twig-field" {
            label class="twig-label" for=(field.id) { (field.label) }
            input
                type=(field.input_type)
                name=(field.id)
                id=(field.id)
                required
                minlength=[field.minlength]
                placeholder=[field.placeholder]
                aria-describedby=[hint_id.as_deref()]
                class=(input_class);
            @if let Some((hint, hint_id)) = field.hint.zip(hint_id.as_deref()) {
                p class="twig-hint" id=(hint_id) { (hint) }
            }
        }
    }
}

fn render_submit(label: &str) -> maud::Markup {
    maud::html! {
        div class="twig-form-actions" {
            button type="submit" class="twig-btn twig-btn--primary twig-btn--block" { (label) }
        }
    }
}

/// Frames an auth form in the quiet page head, its optic baseline, and the content stack.
fn wrap_auth_content(content: &maud::Markup) -> maud::Markup {
    maud::html! {
        div id="auth-content" {

            div class="twig-stack twig-form--narrow" {
                (content)
            }
        }
    }
}

fn render_invite_page() -> maud::Markup {
    let api_key = render_field(&AuthField {
        id: "api_key",
        label: "API Key",
        input_type: "password",
        mono: true,
        ..AuthField::default()
    });

    let content = maud::html! {
        section class="twig-panel" aria-labelledby="invite-panel-title" {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="invite-panel-title" { "INVITE" }
            }
            div class="twig-panel-body" {
                div class="twig-stack" {
                    div class="twig-stack twig-stack--tight" {
                        p class="twig-body-sm twig-ink-secondary" {
                            "To create an account, you first need a signup invite."
                        }
                        p class="twig-body-sm twig-ink-secondary" {
                            "Enter your API key below to generate a one-time invite."
                        }
                    }
                    form
                        hx-post="/auth/invite"
                        hx-target="#invite-result"
                        "hx-status:4xx"="swap:innerHTML target:#invite-result"
                        "hx-status:5xx"="swap:innerHTML target:#invite-result"
                        hx-swap="innerHTML"
                        class="twig-form"
                    {
                        (api_key)
                        (render_submit("Generate Invite"))
                    }
                }
                div id="invite-result" aria-live="polite" {}
            }
        }

        div class="twig-cluster" {
            p class="twig-body-sm twig-ink-secondary" { "Already have an invite?" }
            a class="twig-btn twig-btn--ghost" href="/auth/signup" { "Sign up now" }
        }
    };

    wrap_auth_content(&content)
}

#[get("/auth/invite")]
pub async fn invite_page(
    req: HttpRequest,
    auth_state: web::Data<TwigContext>,
) -> AwResult<maud::Markup> {
    let username = get_username_from_request(&req, &auth_state).await;
    let content = render_invite_page();

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(render_layout(
            &content,
            username.as_deref(),
            Some("Get Signup Invite"),
        ))
    }
}

fn render_signup_page() -> maud::Markup {
    let invite = render_field(&AuthField {
        id: "invite",
        label: "Signup Invite",
        input_type: "text",
        mono: true,
        ..AuthField::default()
    });
    let username = render_field(&AuthField {
        id: "username",
        label: "Username",
        input_type: "text",
        minlength: Some("3"),
        hint: Some("At least 3 characters."),
        ..AuthField::default()
    });
    let email = render_field(&AuthField {
        id: "email",
        label: "Email",
        input_type: "email",
        ..AuthField::default()
    });
    let password = render_field(&AuthField {
        id: "password",
        label: "Password",
        input_type: "password",
        minlength: Some("8"),
        hint: Some("At least 8 characters."),
        ..AuthField::default()
    });

    let content = maud::html! {
        section class="twig-panel" aria-labelledby="signup-panel-title" {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="signup-panel-title" { "ACCOUNT" }
            }
            div class="twig-panel-body" {
                div class="twig-stack" {
                    p class="twig-body-sm twig-ink-secondary" {
                        "Enter your signup invite along with your desired username and password."
                    }
                    form
                        hx-post="/auth/signup"
                        hx-target="#signup-result"
                        "hx-status:4xx"="swap:innerHTML target:#signup-result"
                        "hx-status:5xx"="swap:innerHTML target:#signup-result"
                        hx-swap="innerHTML"
                        class="twig-form"
                    {
                        (invite)
                        (username)
                        (email)
                        (password)
                        (render_submit("Create Account"))
                    }
                }
                div id="signup-result" aria-live="polite" {}
            }
        }

        div class="twig-stack twig-stack--tight" {
            p class="twig-body-sm twig-ink-secondary" {
                "Need an invite? Ask the person running this Twig server."
            }
            p class="twig-body-sm twig-ink-secondary" {
                "Already have an account? "
                a href="/auth/login" { "Log in" }
                "."
            }
        }
    };

    wrap_auth_content(&content)
}

#[get("/auth/signup")]
pub async fn signup_page(
    req: HttpRequest,
    auth_state: web::Data<TwigContext>,
) -> AwResult<maud::Markup> {
    let username = get_username_from_request(&req, &auth_state).await;
    let content = render_signup_page();

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(render_layout(
            &content,
            username.as_deref(),
            Some("Create Account"),
        ))
    }
}

fn render_login_page(return_to: Option<&str>) -> maud::Markup {
    let username = render_field(&AuthField {
        id: "username",
        label: "Username",
        input_type: "text",
        ..AuthField::default()
    });
    let password = render_field(&AuthField {
        id: "password",
        label: "Password",
        input_type: "password",
        ..AuthField::default()
    });

    let content = maud::html! {
        section class="twig-panel" aria-labelledby="login-panel-title" {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="login-panel-title" { "CREDENTIALS" }
            }
            div class="twig-panel-body" {
                form
                    hx-post="/auth/login"
                    hx-target="#login-result"
                    "hx-status:4xx"="swap:innerHTML target:#login-result"
                    "hx-status:5xx"="swap:innerHTML target:#login-result"
                    hx-swap="innerHTML"
                    class="twig-form"
                {
                    (username)
                    (password)
                    @if let Some(return_to) = return_to.and_then(safe_device_return_to) {
                        input type="hidden" name="return_to" value=(return_to);
                    }
                    (render_submit("Log In"))
                }
                div id="login-result" aria-live="polite" {}
            }
        }

        div class="twig-stack twig-stack--tight" {
            p class="twig-body-sm twig-ink-secondary" {
                "No account? Ask the person running this Twig server for an invite."
            }
            p class="twig-body-sm twig-ink-secondary" {
                "Have an invite? "
                a href="/auth/signup" { "Create your account" }
                "."
            }
        }
    };

    maud::html! {
        div class="twig-auth-login" {
            (wrap_auth_content(&content))
        }
    }
}

#[get("/auth/login")]
pub async fn login_page(
    req: HttpRequest,
    auth_state: web::Data<TwigContext>,
    query: web::Query<LoginQuery>,
) -> AwResult<maud::Markup> {
    let username = get_username_from_request(&req, &auth_state).await;
    let content = render_login_page(query.return_to.as_deref());

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(render_layout(&content, username.as_deref(), Some("Log In")))
    }
}

fn render_namespace_page() -> maud::Markup {
    let name = render_field(&AuthField {
        id: "name",
        label: "Namespace Name",
        input_type: "text",
        placeholder: Some("my-org"),
        hint: Some("At least 2 characters."),
        minlength: Some("2"),
        mono: true,
    });

    let content = maud::html! {
        section class="twig-panel" aria-labelledby="namespace-panel-title" {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="namespace-panel-title" { "NAMESPACE" }
            }
            div class="twig-panel-body" {
                div class="twig-stack" {
                    div class="twig-stack twig-stack--tight" {
                        p class="twig-body-sm twig-ink-secondary" {
                            "Create a new namespace for your repositories."
                        }
                        p class="twig-body-sm twig-ink-secondary" {
                            "You can create as many namespaces as you want. Just enter a unique name below."
                        }
                    }
                    form
                        hx-post="/auth/namespace"
                        hx-target="#namespace-result"
                        "hx-status:4xx"="swap:innerHTML target:#namespace-result"
                        "hx-status:5xx"="swap:innerHTML target:#namespace-result"
                        hx-swap="innerHTML"
                        class="twig-form"
                    {
                        (name)
                        (render_submit("Create Namespace"))
                    }
                }
                div id="namespace-result" aria-live="polite" {}
            }
        }

        div class="twig-cluster" {
            a class="twig-btn twig-btn--quiet" href="/" { "Back to home" }
        }
    };

    wrap_auth_content(&content)
}

#[get("/auth/namespace")]
pub async fn namespace_page(
    req: HttpRequest,
    auth_state: web::Data<TwigContext>,
) -> AwResult<maud::Markup> {
    let username = get_username_from_request(&req, &auth_state).await;
    let content = render_namespace_page();

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(render_layout(
            &content,
            username.as_deref(),
            Some("Create Namespace"),
        ))
    }
}

// Handler for successful invite generation
pub fn render_invite_success(invite: &str) -> maud::Markup {
    maud::html! {
        div class="twig-notice twig-notice--success" role="status" {
            p class="twig-eyebrow" { "DONE" }
            p class="twig-notice-body" {
                "Invite Generated! Use this one-time signup invite to create your account. It can only be used once."
            }
            pre class="twig-code" { (invite) }
            div class="twig-notice-actions" {
                a class="twig-btn twig-btn--primary" href="/auth/signup" { "Create your account" }
            }
        }
    }
}

// Handler for successful signup
pub fn render_signup_success(username: &str) -> maud::Markup {
    maud::html! {
        div class="twig-notice twig-notice--success" role="status" {
            p class="twig-eyebrow" { "DONE" }
            p class="twig-notice-body" {
                "Welcome, " (username) "! Your account has been created successfully."
            }
            div class="twig-notice-actions" {
                a class="twig-btn twig-btn--primary" href="/auth/login" { "Log in" }
                a class="twig-btn twig-btn--ghost" href="/auth/namespace" { "Create a namespace" }
            }
        }
    }
}

// Handler for login success
pub fn render_login_success(username: &str) -> maud::Markup {
    maud::html! {
        div class="twig-notice twig-notice--success" role="status" {
            p class="twig-eyebrow" { "DONE" }
            p class="twig-notice-body" {
                "Welcome back, " (username) "! You are now logged in."
            }
            div class="twig-notice-actions" {
                a class="twig-btn twig-btn--primary" href="/auth/namespace" { "Create namespace" }
            }
        }
    }
}

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
