use super::{CallerRole, generate_qr_svg, list_test_endpoints};

fn render_pagehead(role: &CallerRole) -> maud::Markup {
    maud::html! {
        section class="twig-pagehead" {
            div class="twig-cluster" {
                div {
                    p class="twig-eyebrow" { (role.display_name()) }
                    h1 class="twig-title" { "Test Suite & Rapid Endpoint Runner" }
                }
                div class="twig-cluster" {
                    button id="btn-start"
                        class="twig-btn twig-btn--primary"
                        hx-get="/_test/runner"
                        hx-target="#test-runner-container"
                        hx-swap="innerHTML" {
                        "Start"
                    }
                    button id="btn-stop"
                        class="twig-btn twig-btn--ghost"
                        hx-get="/_test/stopped"
                        hx-target="#test-runner-container"
                        hx-swap="innerHTML" {
                        "Stop"
                    }
                }
            }
        }
    }
}

pub fn render_pin_panel(is_admin: bool, active_pin: Option<&str>, base_url: &str) -> maud::Markup {
    maud::html! {
        section id="pin-management-panel" class="twig-panel" aria-label="Multi-user PIN and QR code access" {
            div class="twig-panel-head" {
                span class="twig-eyebrow" { "MULTI-USER ACCESS" }
                span class="twig-title" { "Session PIN & QR Code" }
            }
            div class="twig-panel-body" {
                @if let Some(pin) = active_pin {
                    div class="twig-stack" {
                        div class="twig-cluster" {
                            div {
                                p class="twig-label" { "Active Session PIN" }
                                p id="active-pin-display" class="twig-data-lg" { (pin) }
                            }
                            @if is_admin {
                                button id="btn-remove-pin"
                                    class="twig-btn twig-btn--danger"
                                    hx-post="/_test/pin/remove"
                                    hx-target="#pin-management-panel"
                                    hx-swap="outerHTML" {
                                    "Remove PIN"
                                }
                            }
                        }

                        @let redirect_url = format!("{base_url}/_test?pin={pin}");
                        div class="twig-panel" {
                            div class="twig-panel-body" {
                                div class="twig-cluster" {
                                    @if let Ok(qr_svg) = generate_qr_svg(&redirect_url) {
                                        div id="qr-code-container" class="twig-panel" {
                                            (maud::PreEscaped(qr_svg))
                                        }
                                    }
                                    div class="twig-stack twig-stack--tight" {
                                        p class="twig-eyebrow" { "SCAN TO JOIN" }
                                        p class="twig-body-sm" {
                                            "Scan this QR code or use the link below to allow multiple users to run tests simultaneously. Access is revoked when the admin removes the PIN."
                                        }
                                        p class="twig-label" { "Redirect Link:" }
                                        a id="pin-direct-link" class="twig-data" href=(redirect_url) { (redirect_url) }
                                    }
                                }
                            }
                        }
                    }
                } @else if is_admin {
                    div class="twig-cluster" {
                        div class="twig-stack twig-stack--tight" {
                            p class="twig-body-sm" {
                                "No PIN active. Create a session PIN to let multiple users access and run the test suite concurrently."
                            }
                        }
                        button id="btn-create-pin"
                            class="twig-btn twig-btn--primary"
                            hx-post="/_test/pin/create"
                            hx-target="#pin-management-panel"
                            hx-swap="outerHTML" {
                            "Create Session PIN & QR Code"
                        }
                    }
                } @else {
                    p class="twig-body-sm" {
                        "Connected via Session PIN. You can run all test endpoints concurrently until the admin ends the session."
                    }
                }
            }
        }
    }
}

#[must_use]
pub fn render_pin_entry_page(admin_user: &str, has_active_pin: bool) -> maud::Markup {
    maud::html! {
        div class="twig-stack" {
            div class="twig-notice twig-notice--warning" role="alert" {
                p class="twig-eyebrow" { "ACCESS RESTRICTED" }
                p class="twig-notice-body" {
                    (format!("The test suite is restricted to admin '{admin_user}' or users with an active session PIN."))
                }
            }

            section class="twig-panel" {
                div class="twig-panel-head" {
                    span class="twig-eyebrow" { "SESSION PIN" }
                    span class="twig-title" { "Enter PIN to Join" }
                }
                div class="twig-panel-body" {
                    @if has_active_pin {
                        form method="GET" action="/_test" class="twig-form twig-form--narrow" {
                            div class="twig-field" {
                                label class="twig-label" for="pin-input" { "Enter 6-digit Session PIN" }
                                input id="pin-input" class="twig-input twig-input--mono" type="text" name="pin" placeholder="e.g. 123456" required;
                            }
                            div class="twig-form-actions" {
                                button class="twig-btn twig-btn--primary" type="submit" { "Join Session" }
                                a class="twig-btn twig-btn--ghost" href="/auth/login" { "Log in as Admin" }
                            }
                        }
                    } @else {
                        div class="twig-empty" {
                            p class="twig-eyebrow" { "NO ACTIVE SESSION" }
                            p class="twig-empty-body" {
                                "There is currently no active session PIN. Please ask the admin to generate a PIN on the test page, or log in as admin."
                            }
                            div class="twig-cluster" {
                                a class="twig-btn twig-btn--primary" href="/auth/login" { "Log in as Admin" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[must_use]
pub fn render_test_page(
    role: &CallerRole,
    active_pin: Option<&str>,
    base_url: &str,
    data_enabled: bool,
) -> maud::Markup {
    maud::html! {
        (crate::http::tree::pages::render_tree_hub(
            true,
            data_enabled,
            role.is_admin(),
            Some("test"),
        ))
        (render_pagehead(role))
        (render_pin_panel(role.is_admin(), active_pin, base_url))
        div id="test-runner-container" {
            (super::render_idle_content())
        }
    }
}

#[must_use]
pub fn render_idle_content() -> maud::Markup {
    maud::html! {
        div id="test-runner-idle" class="twig-panel" {
            div class="twig-panel-head" {
                span class="twig-eyebrow" { "IDLE" }
                span class="twig-title" { "Test run has not started" }
            }
            div class="twig-panel-body" {
                p class="twig-body-sm" {
                    "Press Start to run every read-only endpoint and feature check. Press Stop at any time to interrupt the run."
                }
            }
        }
    }
}

#[must_use]
pub fn render_stopped_content() -> maud::Markup {
    maud::html! {
        div id="test-runner-stopped" class="twig-panel" {
            div class="twig-panel-head" {
                span class="twig-eyebrow" { "STOPPED" }
                span class="twig-title" { "Test run interrupted" }
            }
            div class="twig-panel-body" {
                p class="twig-body-sm" {
                    "The run has been stopped. Press Start to run the endpoints again."
                }
            }
        }
    }
}

pub fn render_runner_content() -> maud::Markup {
    let endpoints = list_test_endpoints();
    maud::html! {
        div id="test-runner"
            class="twig-stack"
            hx-get="/_test/runner"
            hx-trigger="every 25ms"
            hx-target="#test-runner"
            hx-swap="outerHTML" {
            div class="twig-cluster" {
                span class="twig-label" { "Runner active: triggering all read-only endpoints every 5ms (container polling every 25ms)" }
            }
            div class="twig-bento" {
                @for ep in endpoints {
                    div class="twig-panel" {
                        div class="twig-panel-head" {
                            span class="twig-eyebrow" { (ep.category) " · " (ep.method) }
                            span class="twig-title" { (ep.url) }
                        }
                        div class="twig-panel-body"
                            hx-get=(ep.url)
                            hx-trigger="load, every 5ms"
                            hx-swap="none"
                            data-ep-id=(ep.id) {
                            span id=(format!("res-{}", ep.id)) class="twig-data" {
                                "Active"
                            }
                        }
                    }
                }
            }
        }
    }
}
