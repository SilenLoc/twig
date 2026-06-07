use maud::DOCTYPE;

pub mod auth;
pub mod namespace;
pub mod overview;
pub mod repo;
pub mod session_auth;
pub mod settings;

pub fn render_layout(main_content: &maud::Markup, username: Option<&str>) -> maud::Markup {
    maud::html! {
        (DOCTYPE)
        html class="h-100" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Fig" }
                link rel="icon" type="image/svg+xml" href="/assets/fig.svg";
                link rel="stylesheet" href="/assets/t.css";
                script src="/assets/h.js" {}
                script src="/assets/hx-response-targets.js" {}
                style {
                    ".markdown-body table { border-collapse: collapse; margin: 1rem 0; }"
                    ".markdown-body th, .markdown-body td { border: 1px solid rgba(255,255,255,0.3); padding: 0.5rem 1rem; }"
                    ".markdown-body th { background-color: rgba(255,255,255,0.1); font-weight: 600; }"
                    ".markdown-body tr:nth-child(even) { background-color: rgba(255,255,255,0.05); }"
                }
            }
            body hx-ext="response-targets" class="w-100 sans-serif ma0 bg-black white" style="min-height: 100vh;" {
                nav class="flex items-center justify-between flex-wrap bg-black bb b--white-20 pa3" style="position: sticky; top: 0; z-index: 10;" {
                    div {
                        a href="/" class="link white-90 hover-white no-underline fw6 f4 flex items-center" {
                            img src="/assets/fig.svg" alt="Fig logo" style="width: 24px; height: 24px; margin-right: 0.5rem;";
                            "Fig"
                        }
                    }
                    div class="flex items-center" {
                        @match username {
                            Some(name) => {
                                span class="white-70 f6 mr2 mr3-ns dn dib-ns" { (name) }
                                a href="/settings" class="link white-70 hover-white no-underline f6 mr2 mr3-ns" {
                                    "Settings"
                                }
                                form method="POST" action="/auth/logout" class="dib ma0" {
                                    button
                                        type="submit"
                                        class="link white-70 hover-white no-underline f6 bg-transparent bn pointer pa0"
                                    {
                                        "Logout"
                                    }
                                }
                            }
                            None => {
                                a href="/auth/login" class="link white-70 hover-white no-underline f6 mr2 mr3-ns" {
                                    "Login"
                                }
                                a href="/auth/signup" class="link white-70 hover-white no-underline f6" {
                                    "Signup"
                                }
                            }
                        }
                    }
                }
                main id="feature" class="pa3" {
                    (main_content)
                }
            }
        }
    }
}

pub fn render_error(message: &str) -> maud::Markup {
    maud::html! {
        div class="ba b--red br2 pa3 bg-dark-red mt3" {
            p class="f6 white ma0" { (message) }
        }
    }
}

pub fn render_success(message: &str) -> maud::Markup {
    maud::html! {
        div class="ba b--green br2 pa3 bg-dark-green mt3" {
            p class="f6 white ma0" { (message) }
        }
    }
}
