use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};

use super::render_layout;
use super::session_auth::get_username_from_request;
use crate::auth::FigContext;

fn wrap_auth_content(content: &maud::Markup) -> maud::Markup {
    maud::html! {
        div id="auth-content" class="flex flex-column items-center justify-center" style="min-height: 60vh; padding: 1rem 0;" {
            div class="w-100 mw6" {
                (content)
            }
        }
    }
}

#[get("/auth/invite")]
pub async fn invite_page(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
) -> AwResult<maud::Markup> {
    let username = get_username_from_request(&req, &auth_state).await;
    let content = maud::html! {
        h1 class="tf-title mb4 white tc" { "Get Signup Invite" }

        div class="ba b--white-20 pa4 bg-black-20 mb4" {
            p class="f6 white-70 mb2" {
                "To create an account, you first need a signup invite."
            }
            p class="f6 white-70" {
                "Enter your API key below to generate a one-time invite."
            }
        }

        form
            hx-post="/auth/invite"
            hx-target="#invite-result"
            hx-target-error="#invite-result"
            hx-swap="innerHTML"
            class="ba b--white-20 pa4 bg-black-20"
        {
            div class="mb4" {
                label class="db tf-kicker white-50 mb2" for="api_key" { "API Key" }
                input
                    type="password"
                    name="api_key"
                    id="api_key"
                    required
                    class="tf-input db w-100"
                    placeholder="Enter your API key";
            }

            button
                type="submit"
                class="tf-btn tf-btn-block"
            {
                "Generate Invite"
            }
        }

        div id="invite-result" class="mt3" {}

        div class="mt3 tc" {
            span class="white-60 f6" { "Already have a invite? " }
            a href="/auth/signup" class="link white hover-white-90 underline f6" {
                "Sign up now"
            }
        }
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(render_layout(
            &wrap_auth_content(&content),
            username.as_deref(),
        ))
    }
}

#[get("/auth/signup")]
pub async fn signup_page(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
) -> AwResult<maud::Markup> {
    let username = get_username_from_request(&req, &auth_state).await;
    let content = maud::html! {
        h1 class="tf-title mb4 white tc" { "Create Account" }

        div class="ba b--white-20 pa4 bg-black-20 mb4" {
            p class="f6 white-70" {
                "Enter your signup invite along with your desired username and password."
            }
        }

        form
            hx-post="/auth/signup"
            hx-target="#signup-result"
            hx-target-error="#signup-result"
            hx-swap="innerHTML"
            class="ba b--white-20 pa4 bg-black-20"
        {
            div class="mb3" {
                label class="db tf-kicker white-50 mb2" for="invite" { "Signup Invite" }
                input
                    type="text"
                    name="invite"
                    id="invite"
                    required
                    class="tf-input db w-100"
                    placeholder="Enter your signup invite";
            }

            div class="mb3" {
                label class="db tf-kicker white-50 mb2" for="username" { "Username" }
                input
                    type="text"
                    name="username"
                    id="username"
                    required
                    minlength="3"
                    class="tf-input db w-100"
                    placeholder="Choose a username (min 3 characters)";
            }

            div class="mb3" {
                label class="db tf-kicker white-50 mb2" for="email" { "Email" }
                input
                    type="email"
                    name="email"
                    id="email"
                    required
                    class="tf-input db w-100"
                    placeholder="Enter your email address";
            }

            div class="mb4" {
                label class="db tf-kicker white-50 mb2" for="password" { "Password" }
                input
                    type="password"
                    name="password"
                    id="password"
                    required
                    minlength="8"
                    class="tf-input db w-100"
                    placeholder="Choose a password (min 8 characters)";
            }

            button
                type="submit"
                class="tf-btn tf-btn-block"
            {
                "Create Account"
            }
        }

        div id="signup-result" class="mt3" {}

        div class="mt3 tc" {
            span class="white-60 f6" { "Need a invite? " }
            a href="/auth/invite" class="link white hover-white-90 underline f6" {
                "Get one here"
            }
            br;
            span class="white-60 f6" { "Already have an account? " }
            a href="/auth/login" class="link white hover-white-90 underline f6" {
                "Log in"
            }
        }
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(render_layout(
            &wrap_auth_content(&content),
            username.as_deref(),
        ))
    }
}

#[get("/auth/login")]
pub async fn login_page(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
) -> AwResult<maud::Markup> {
    let username = get_username_from_request(&req, &auth_state).await;
    let content = maud::html! {
        h1 class="tf-title mb4 white tc" { "Log In" }

        form
            hx-post="/auth/login"
            hx-target="#login-result"
            hx-target-error="#login-result"
            hx-swap="innerHTML"
            class="ba b--white-20 pa4 bg-black-20"
        {
            div class="mb3" {
                label class="db tf-kicker white-50 mb2" for="username" { "Username" }
                input
                    type="text"
                    name="username"
                    id="username"
                    required
                    class="tf-input db w-100"
                    placeholder="Enter your username";
            }

            div class="mb4" {
                label class="db tf-kicker white-50 mb2" for="password" { "Password" }
                input
                    type="password"
                    name="password"
                    id="password"
                    required
                    class="tf-input db w-100"
                    placeholder="Enter your password";
            }

            button
                type="submit"
                class="tf-btn tf-btn-block"
            {
                "Log In"
            }
        }

        div id="login-result" class="mt3" {}

        div class="mt3 tc" {
            span class="white-60 f6" { "Don't have an account? " }
            a href="/auth/invite" class="link white hover-white-90 underline f6" {
                "Get a invite"
            }
        }
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(render_layout(
            &wrap_auth_content(&content),
            username.as_deref(),
        ))
    }
}

#[get("/auth/namespace")]
pub async fn namespace_page(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
) -> AwResult<maud::Markup> {
    let username = get_username_from_request(&req, &auth_state).await;
    let content = maud::html! {
        h1 class="tf-title mb4 white tc" { "Create Namespace" }

        div class="ba b--white-20 pa4 bg-black-20 mb4" {
            p class="f6 white-70 mb2" {
                "Create a new namespace for your repositories."
            }
            p class="f6 white-70" {
                "You can create as many namespaces as you want - just enter a unique name below."
            }
        }

        form
            hx-post="/auth/namespace"
            hx-target="#namespace-result"
            hx-target-error="#namespace-result"
            hx-swap="innerHTML"
            class="ba b--white-20 pa4 bg-black-20"
        {
            div class="mb3" {
                label class="db tf-kicker white-50 mb2" for="name" { "Namespace Name" }
                input
                    type="text"
                    name="name"
                    id="name"
                    required
                    minlength="2"
                    class="tf-input db w-100"
                    placeholder="Enter namespace name (e.g., my-org)";
            }

            button
                type="submit"
                class="tf-btn tf-btn-block"
            {
                "Create Namespace"
            }
        }

        div id="namespace-result" class="mt3" {}

        div class="mt3 tc" {
            a href="/" class="link white-70 hover-white underline f6" {
                "← Back to home"
            }
        }
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(render_layout(
            &wrap_auth_content(&content),
            username.as_deref(),
        ))
    }
}

// Handler for successful invite generation
pub fn render_invite_success(invite: &str) -> maud::Markup {
    maud::html! {
        div class="bt b--white pa3 bg-black-20" style="border-top-width: 3px;" {
            h2 class="tf-section mb3 white" { "Invite Generated!" }
            p class="f6 white mb2" {
                "Your one-time signup invite has been generated."
            }
            p class="f6 white-70 mb3" {
                "Use this invite to create your account (it can only be used once):"
            }
            code class="db pa2 bg-black-50 white mb3 f6" style="word-break: break-all;" {
                (invite)
            }
            a
                href="/auth/signup"
                class="tf-btn tf-btn-block"
            {
                "Create Your Account →"
            }
        }
    }
}

// Handler for successful signup
pub fn render_signup_success(username: &str) -> maud::Markup {
    maud::html! {
        div class="bt b--white pa3 bg-black-20" style="border-top-width: 3px;" {
            h2 class="tf-section mb3 white" { "Account Created!" }
            p class="f6 white mb2" {
                "Welcome, " (username) "! Your account has been created successfully."
            }
            div class="flex flex-column" {
                a
                    href="/auth/login"
                    class="tf-btn tf-btn-block mb2"
                {
                    "Log In →"
                }
                a
                    href="/auth/namespace"
                    class="tf-btn tf-btn-ghost tf-btn-block"
                {
                    "Create a Namespace →"
                }
            }
        }
    }
}

// Handler for login success
pub fn render_login_success(username: &str) -> maud::Markup {
    maud::html! {
        div class="bt b--white pa3 bg-black-20" style="border-top-width: 3px;" {
            h2 class="tf-section mb3 white" { "Login Successful!" }
            p class="f6 white mb2" {
                "Welcome back, " (username) "!"
            }
            p class="f6 white-70 mb3" {
                "You are now logged in. You can create as many namespaces as you want."
            }
            a
                href="/auth/namespace"
                class="tf-btn tf-btn-block"
            {
                "Create Namespace →"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_invite_success_contains_invite() {
        let invite = "abc-123-test";
        let markup = render_invite_success(invite);
        let html = markup.into_string();
        assert!(html.contains(invite), "invite should be displayed");
        assert!(
            html.contains("Invite Generated"),
            "success heading should be present"
        );
        assert!(
            html.contains("/auth/signup"),
            "link to signup should be present"
        );
    }

    #[test]
    fn test_render_signup_success_contains_username() {
        let username = "testuser";
        let markup = render_signup_success(username);
        let html = markup.into_string();
        assert!(html.contains(username), "username should be displayed");
        assert!(
            html.contains("Account Created"),
            "success heading should be present"
        );
        assert!(
            html.contains("/auth/login"),
            "link to login should be present"
        );
        assert!(
            html.contains("/auth/namespace"),
            "link to namespace should be present"
        );
    }

    #[test]
    fn test_render_login_success_contains_username() {
        let username = "testuser";
        let markup = render_login_success(username);
        let html = markup.into_string();
        assert!(html.contains(username), "username should be displayed");
        assert!(
            html.contains("Login Successful"),
            "success heading should be present"
        );
        assert!(
            html.contains("/auth/namespace"),
            "link to namespace should be present"
        );
    }
}
