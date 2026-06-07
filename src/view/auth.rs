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

#[get("/auth/ticket")]
pub async fn ticket_page(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
) -> AwResult<maud::Markup> {
    let username = get_username_from_request(&req, &auth_state).await;
    let content = maud::html! {
        h1 class="f3 fw6 mb4 white tc" { "Get Signup Ticket" }

        div class="ba b--white-20 br2 pa4 bg-black-20 mb4" {
            p class="f6 white-70 mb2" {
                "To create an account, you first need a signup ticket."
            }
            p class="f6 white-70" {
                "Enter your API key below to generate a one-time ticket."
            }
        }

        form
            hx-post="/auth/ticket"
            hx-target="#ticket-result"
            hx-target-error="#ticket-result"
            hx-swap="innerHTML"
            class="ba b--white-20 br2 pa4 bg-black-20"
        {
            div class="mb4" {
                label class="db f6 white-70 mb2" for="api_key" { "API Key" }
                input
                    type="password"
                    name="api_key"
                    id="api_key"
                    required
                    class="db w-100 pa2 bg-black white ba b--white-30 br1"
                    placeholder="Enter your API key";
            }

            button
                type="submit"
                class="w-100 pa2 bg-white black bn br1 pointer hover-bg-white-90"
            {
                "Generate Ticket"
            }
        }

        div id="ticket-result" class="mt3" {}

        div class="mt3 tc" {
            span class="white-60 f6" { "Already have a ticket? " }
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
        h1 class="f3 fw6 mb4 white tc" { "Create Account" }

        div class="ba b--white-20 br2 pa4 bg-black-20 mb4" {
            p class="f6 white-70" {
                "Enter your signup ticket along with your desired username and password."
            }
        }

        form
            hx-post="/auth/signup"
            hx-target="#signup-result"
            hx-target-error="#signup-result"
            hx-swap="innerHTML"
            class="ba b--white-20 br2 pa4 bg-black-20"
        {
            div class="mb3" {
                label class="db f6 white-70 mb2" for="ticket" { "Signup Ticket" }
                input
                    type="text"
                    name="ticket"
                    id="ticket"
                    required
                    class="db w-100 pa2 bg-black white ba b--white-30 br1"
                    placeholder="Enter your signup ticket";
            }

            div class="mb3" {
                label class="db f6 white-70 mb2" for="username" { "Username" }
                input
                    type="text"
                    name="username"
                    id="username"
                    required
                    minlength="3"
                    class="db w-100 pa2 bg-black white ba b--white-30 br1"
                    placeholder="Choose a username (min 3 characters)";
            }

            div class="mb3" {
                label class="db f6 white-70 mb2" for="email" { "Email" }
                input
                    type="email"
                    name="email"
                    id="email"
                    required
                    class="db w-100 pa2 bg-black white ba b--white-30 br1"
                    placeholder="Enter your email address";
            }

            div class="mb4" {
                label class="db f6 white-70 mb2" for="password" { "Password" }
                input
                    type="password"
                    name="password"
                    id="password"
                    required
                    minlength="8"
                    class="db w-100 pa2 bg-black white ba b--white-30 br1"
                    placeholder="Choose a password (min 8 characters)";
            }

            button
                type="submit"
                class="w-100 pa2 bg-white black bn br1 pointer hover-bg-white-90"
            {
                "Create Account"
            }
        }

        div id="signup-result" class="mt3" {}

        div class="mt3 tc" {
            span class="white-60 f6" { "Need a ticket? " }
            a href="/auth/ticket" class="link white hover-white-90 underline f6" {
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
        h1 class="f3 fw6 mb4 white tc" { "Log In" }

        form
            hx-post="/auth/login"
            hx-target="#login-result"
            hx-target-error="#login-result"
            hx-swap="innerHTML"
            class="ba b--white-20 br2 pa4 bg-black-20"
        {
            div class="mb3" {
                label class="db f6 white-70 mb2" for="username" { "Username" }
                input
                    type="text"
                    name="username"
                    id="username"
                    required
                    class="db w-100 pa2 bg-black white ba b--white-30 br1"
                    placeholder="Enter your username";
            }

            div class="mb4" {
                label class="db f6 white-70 mb2" for="password" { "Password" }
                input
                    type="password"
                    name="password"
                    id="password"
                    required
                    class="db w-100 pa2 bg-black white ba b--white-30 br1"
                    placeholder="Enter your password";
            }

            button
                type="submit"
                class="w-100 pa2 bg-white black bn br1 pointer hover-bg-white-90"
            {
                "Log In"
            }
        }

        div id="login-result" class="mt3" {}

        div class="mt3 tc" {
            span class="white-60 f6" { "Don't have an account? " }
            a href="/auth/ticket" class="link white hover-white-90 underline f6" {
                "Get a ticket"
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
        h1 class="f3 fw6 mb4 white tc" { "Create Namespace" }

        div class="ba b--white-20 br2 pa4 bg-black-20 mb4" {
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
            class="ba b--white-20 br2 pa4 bg-black-20"
        {
            div class="mb3" {
                label class="db f6 white-70 mb2" for="name" { "Namespace Name" }
                input
                    type="text"
                    name="name"
                    id="name"
                    required
                    minlength="2"
                    class="db w-100 pa2 bg-black white ba b--white-30 br1"
                    placeholder="Enter namespace name (e.g., my-org)";
            }

            button
                type="submit"
                class="w-100 pa2 bg-white black bn br1 pointer hover-bg-white-90"
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

// Handler for successful ticket generation
pub fn render_ticket_success(ticket: &str) -> maud::Markup {
    maud::html! {
        div class="ba b--green br2 pa3 bg-dark-green" {
            h2 class="f4 fw6 mb3 white" { "Ticket Generated!" }
            p class="f6 white mb2" {
                "Your one-time signup ticket has been generated."
            }
            p class="f6 white-90 mb3" {
                "Use this ticket to create your account (it can only be used once):"
            }
            code class="db pa2 bg-black-50 white br1 mb3 f6" style="word-break: break-all;" {
                (ticket)
            }
            a
                href="/auth/signup"
                class="db tc pa2 bg-white black bn br1 pointer hover-bg-white-90 no-underline"
            {
                "Create Your Account →"
            }
        }
    }
}

// Handler for successful signup
pub fn render_signup_success(username: &str) -> maud::Markup {
    maud::html! {
        div class="ba b--green br2 pa3 bg-dark-green" {
            h2 class="f4 fw6 mb3 white" { "Account Created!" }
            p class="f6 white mb2" {
                "Welcome, " (username) "! Your account has been created successfully."
            }
            div class="flex flex-column" {
                a
                    href="/auth/login"
                    class="db tc pa2 bg-white black bn br1 pointer hover-bg-white-90 no-underline mb2"
                {
                    "Log In →"
                }
                a
                    href="/auth/namespace"
                    class="db tc pa2 bg-white-20 white bn br1 pointer hover-bg-white-30 no-underline"
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
        div class="ba b--green br2 pa3 bg-dark-green" {
            h2 class="f4 fw6 mb3 white" { "Login Successful!" }
            p class="f6 white mb2" {
                "Welcome back, " (username) "!"
            }
            p class="f6 white-90 mb3" {
                "You are now logged in. You can create as many namespaces as you want."
            }
            a
                href="/auth/namespace"
                class="db tc pa2 bg-white black bn br1 pointer hover-bg-white-90 no-underline"
            {
                "Create Namespace →"
            }
        }
    }
}
