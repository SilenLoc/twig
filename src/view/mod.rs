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
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Anton&family=Bricolage+Grotesque:opsz,wght@12..96,300..700&display=swap";
                link rel="stylesheet" href="/assets/t.css";
                link rel="stylesheet" href="/assets/fig.css";
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
                nav class="flex items-center bg-black bb b--white-20 pa3" style="position: sticky; top: 0; z-index: 10;" {
                    div class="tf-container flex items-center justify-between flex-wrap" {
                        div {
                        a
                            href="/"
                            class="display link white hover-white no-underline f3"
                            style="letter-spacing: 0.04em;"
                        {
                            "Fig"
                        }
                    }
                    div class="flex items-center tf-kicker" {
                        @match username {
                            Some(name) => {
                                span class="white-40 mr2 mr4-ns dn dib-ns" { (name) }
                                a href="/settings" class="link white-70 hover-white no-underline mr2 mr4-ns" {
                                    "Settings"
                                }
                                form method="POST" action="/auth/logout" class="dib ma0" {
                                    button
                                        type="submit"
                                        class="link white-70 hover-white no-underline bg-transparent bn pointer pa0"
                                    {
                                        "Logout"
                                    }
                                }
                            }
                            None => {
                                a href="/auth/login" class="link white-70 hover-white no-underline mr2 mr4-ns" {
                                    "Login"
                                }
                                a href="/auth/signup" class="link white no-underline" {
                                    "Signup"
                                }
                            }
                        }
                    }
                    }
                }
                main id="feature" class="pa3" {
                    div class="tf-container" {
                        (main_content)
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_layout_contains_doctype_and_title() {
        let content = maud::html! { p { "hello" } };
        let layout = render_layout(&content, Some("testuser"));
        let html = layout.into_string();
        assert!(
            html.contains("<!DOCTYPE html>"),
            "layout should contain DOCTYPE"
        );
        assert!(
            html.contains("<title>Fig</title>"),
            "layout should contain title"
        );
        assert!(
            html.contains("/assets/t.css"),
            "layout should reference t.css"
        );
        assert!(
            html.contains("/assets/h.js"),
            "layout should reference h.js"
        );
        assert!(html.contains("testuser"), "layout should show username");
    }

    #[test]
    fn test_render_layout_shows_login_when_no_user() {
        let content = maud::html! { p { "hello" } };
        let layout = render_layout(&content, None);
        let html = layout.into_string();
        assert!(
            html.contains("/auth/login"),
            "layout should show login link"
        );
        assert!(
            html.contains("/auth/signup"),
            "layout should show signup link"
        );
    }

    #[test]
    fn test_render_error_contains_message() {
        let markup = render_error("something went wrong");
        let html = markup.into_string();
        assert!(html.contains("something went wrong"));
        assert!(html.contains("bg-dark-red"));
    }

    #[test]
    fn test_render_success_contains_message() {
        let markup = render_success("operation completed");
        let html = markup.into_string();
        assert!(html.contains("operation completed"));
        assert!(html.contains("bg-dark-green"));
    }
}
