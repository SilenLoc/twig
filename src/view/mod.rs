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
                link rel="stylesheet" href="/assets/t.css";
                script src="/assets/h.js" {}
                script src="/assets/hx-response-targets.js" {}
            }
            body hx-ext="response-targets" class="w-100 sans-serif ma0 bg-black white" style="height: 100vh; overflow: hidden;" {
                nav class="dt w-100 bg-black bb b--white-20 fixed top-0 left-0 right-0 z-1" {
                    div class="dtc v-mid pa3" {
                        a href="/" class="link white-90 hover-white no-underline fw6 f4" {
                            "Fig"
                        }
                    }
                    div class="dtc v-mid tr pa3" {
                        @match username {
                            Some(name) => {
                                span class="white-70 f6 mr3" { (name) }
                                a href="/settings" class="link white-70 hover-white no-underline f6 mr3" {
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
                                a href="/auth/login" class="link white-70 hover-white no-underline f6 mr3" {
                                    "Login"
                                }
                                a href="/auth/signup" class="link white-70 hover-white no-underline f6" {
                                    "Signup"
                                }
                            }
                        }
                    }
                }
                main id="feature" class="flex flex-column" style="padding-top: 5rem; padding-left: 10px; padding-right: 10px; padding-bottom: 10px; height: 100vh; overflow: hidden;" {
                    div class="w-100 flex-auto" style="overflow: hidden; display: flex; flex-direction: column;" {
                        (main_content)
                    }
                }
            }
        }
    }
}
