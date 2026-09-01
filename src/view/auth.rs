use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};

use super::render_layout;
use super::session_auth::get_username_from_request;
use crate::auth::FigContext;

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
        "fig-input fig-input--mono"
    } else {
        "fig-input"
    };

    maud::html! {
        div class="fig-field" {
            label class="fig-label" for=(field.id) { (field.label) }
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
                p class="fig-hint" id=(hint_id) { (hint) }
            }
        }
    }
}

fn render_submit(label: &str) -> maud::Markup {
    maud::html! {
        div class="fig-form-actions" {
            button type="submit" class="fig-btn fig-btn--primary fig-btn--block" { (label) }
        }
    }
}

/// Frames an auth form in the quiet page head, its optic baseline, and the content stack.
fn wrap_auth_content(_title: &str, content: &maud::Markup) -> maud::Markup {
    maud::html! {
        div id="auth-content" {

            div class="fig-stack fig-form--narrow" {
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
        section class="fig-panel" aria-labelledby="invite-panel-title" {
            header class="fig-panel-head" {
                h2 class="fig-eyebrow" id="invite-panel-title" { "INVITE" }
            }
            div class="fig-panel-body" {
                div class="fig-stack" {
                    div class="fig-stack fig-stack--tight" {
                        p class="fig-body-sm fig-ink-secondary" {
                            "To create an account, you first need a signup invite."
                        }
                        p class="fig-body-sm fig-ink-secondary" {
                            "Enter your API key below to generate a one-time invite."
                        }
                    }
                    form
                        hx-post="/auth/invite"
                        hx-target="#invite-result"
                        "hx-status:4xx"="swap:innerHTML target:#invite-result"
                        "hx-status:5xx"="swap:innerHTML target:#invite-result"
                        hx-swap="innerHTML"
                        class="fig-form"
                    {
                        (api_key)
                        (render_submit("Generate Invite"))
                    }
                }
                div id="invite-result" aria-live="polite" {}
            }
        }

        div class="fig-cluster" {
            p class="fig-body-sm fig-ink-secondary" { "Already have an invite?" }
            a class="fig-btn fig-btn--ghost" href="/auth/signup" { "Sign up now" }
        }
    };

    wrap_auth_content("Get Signup Invite", &content)
}

#[get("/auth/invite")]
pub async fn invite_page(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
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
        section class="fig-panel" aria-labelledby="signup-panel-title" {
            header class="fig-panel-head" {
                h2 class="fig-eyebrow" id="signup-panel-title" { "ACCOUNT" }
            }
            div class="fig-panel-body" {
                div class="fig-stack" {
                    p class="fig-body-sm fig-ink-secondary" {
                        "Enter your signup invite along with your desired username and password."
                    }
                    form
                        hx-post="/auth/signup"
                        hx-target="#signup-result"
                        "hx-status:4xx"="swap:innerHTML target:#signup-result"
                        "hx-status:5xx"="swap:innerHTML target:#signup-result"
                        hx-swap="innerHTML"
                        class="fig-form"
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

        div class="fig-stack fig-stack--tight" {
            div class="fig-cluster" {
                p class="fig-body-sm fig-ink-secondary" { "Need an invite?" }
                a class="fig-btn fig-btn--ghost" href="/auth/invite" { "Get one here" }
            }
            div class="fig-cluster" {
                p class="fig-body-sm fig-ink-secondary" { "Already have an account?" }
                a class="fig-btn fig-btn--ghost" href="/auth/login" { "Log in" }
            }
        }
    };

    wrap_auth_content("Create Account", &content)
}

#[get("/auth/signup")]
pub async fn signup_page(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
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

fn render_login_page() -> maud::Markup {
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
        section class="fig-panel" aria-labelledby="login-panel-title" {
            header class="fig-panel-head" {
                h2 class="fig-eyebrow" id="login-panel-title" { "CREDENTIALS" }
            }
            div class="fig-panel-body" {
                form
                    hx-post="/auth/login"
                    hx-target="#login-result"
                    "hx-status:4xx"="swap:innerHTML target:#login-result"
                    "hx-status:5xx"="swap:innerHTML target:#login-result"
                    hx-swap="innerHTML"
                    class="fig-form"
                {
                    (username)
                    (password)
                    (render_submit("Log In"))
                }
                div id="login-result" aria-live="polite" {}
            }
        }

        div class="fig-cluster" {
            p class="fig-body-sm fig-ink-secondary" { "Don't have an account?" }
            a class="fig-btn fig-btn--ghost" href="/auth/invite" { "Get an invite" }
        }
    };

    maud::html! {
        div class="fig-auth-login" {
            (wrap_auth_content("Log In", &content))
        }
    }
}

#[get("/auth/login")]
pub async fn login_page(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
) -> AwResult<maud::Markup> {
    let username = get_username_from_request(&req, &auth_state).await;
    let content = render_login_page();

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
        section class="fig-panel" aria-labelledby="namespace-panel-title" {
            header class="fig-panel-head" {
                h2 class="fig-eyebrow" id="namespace-panel-title" { "NAMESPACE" }
            }
            div class="fig-panel-body" {
                div class="fig-stack" {
                    div class="fig-stack fig-stack--tight" {
                        p class="fig-body-sm fig-ink-secondary" {
                            "Create a new namespace for your repositories."
                        }
                        p class="fig-body-sm fig-ink-secondary" {
                            "You can create as many namespaces as you want. Just enter a unique name below."
                        }
                    }
                    form
                        hx-post="/auth/namespace"
                        hx-target="#namespace-result"
                        "hx-status:4xx"="swap:innerHTML target:#namespace-result"
                        "hx-status:5xx"="swap:innerHTML target:#namespace-result"
                        hx-swap="innerHTML"
                        class="fig-form"
                    {
                        (name)
                        (render_submit("Create Namespace"))
                    }
                }
                div id="namespace-result" aria-live="polite" {}
            }
        }

        div class="fig-cluster" {
            a class="fig-btn fig-btn--quiet" href="/" { "Back to home" }
        }
    };

    wrap_auth_content("Create Namespace", &content)
}

#[get("/auth/namespace")]
pub async fn namespace_page(
    req: HttpRequest,
    auth_state: web::Data<FigContext>,
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
        div class="fig-notice fig-notice--success" role="status" {
            p class="fig-eyebrow" { "DONE" }
            p class="fig-notice-body" {
                "Invite Generated! Use this one-time signup invite to create your account. It can only be used once."
            }
            pre class="fig-code" { (invite) }
            div class="fig-notice-actions" {
                a class="fig-btn fig-btn--primary" href="/auth/signup" { "Create your account" }
            }
        }
    }
}

// Handler for successful signup
pub fn render_signup_success(username: &str) -> maud::Markup {
    maud::html! {
        div class="fig-notice fig-notice--success" role="status" {
            p class="fig-eyebrow" { "DONE" }
            p class="fig-notice-body" {
                "Welcome, " (username) "! Your account has been created successfully."
            }
            div class="fig-notice-actions" {
                a class="fig-btn fig-btn--primary" href="/auth/login" { "Log in" }
                a class="fig-btn fig-btn--ghost" href="/auth/namespace" { "Create a namespace" }
            }
        }
    }
}

// Handler for login success
pub fn render_login_success(username: &str) -> maud::Markup {
    maud::html! {
        div class="fig-notice fig-notice--success" role="status" {
            p class="fig-eyebrow" { "DONE" }
            p class="fig-notice-body" {
                "Welcome back, " (username) "! You are now logged in. You can create as many namespaces as you want."
            }
            div class="fig-notice-actions" {
                a class="fig-btn fig-btn--primary" href="/auth/namespace" { "Create namespace" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth_pages() -> [(&'static str, String); 4] {
        [
            ("invite", render_invite_page().into_string()),
            ("signup", render_signup_page().into_string()),
            ("login", render_login_page().into_string()),
            ("namespace", render_namespace_page().into_string()),
        ]
    }

    fn success_notices() -> [(&'static str, String); 3] {
        [
            (
                "invite",
                render_invite_success("abc-123-test").into_string(),
            ),
            ("signup", render_signup_success("testuser").into_string()),
            ("login", render_login_success("testuser").into_string()),
        ]
    }

    fn classes_in(html: &str) -> Vec<String> {
        let marker = "class=\"";
        html.match_indices(marker)
            .flat_map(|(start, _)| {
                let rest = &html[start + marker.len()..];
                let end = rest.find('"').expect("class attribute must be closed");
                rest[..end]
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn input_tags(html: &str) -> Vec<&str> {
        html.match_indices("<input")
            .map(|(start, _)| {
                let end = start + html[start..].find('>').expect("input tag must close");
                &html[start..=end]
            })
            .collect()
    }

    fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
        let marker = format!(" {name}=\"");
        let start = tag.find(&marker)? + marker.len();
        let end = start + tag[start..].find('"')?;
        Some(&tag[start..end])
    }

    fn field_tag<'a>(html: &'a str, id: &str) -> &'a str {
        input_tags(html)
            .into_iter()
            .find(|tag| attr_value(tag, "id") == Some(id))
            .unwrap_or_else(|| panic!("no input carries id {id}:\n{html}"))
    }

    #[test]
    fn test_auth_markup_uses_only_fig_design_system_classes() {
        for (page, html) in auth_pages().iter().chain(success_notices().iter()) {
            let classes = classes_in(html);
            assert!(!classes.is_empty(), "{page} should carry classes:\n{html}");
            for class in classes {
                assert!(
                    class.starts_with("fig-"),
                    "non design-system class {class:?} on {page}:\n{html}"
                );
            }
        }
    }

    #[test]
    fn test_auth_markup_carries_no_inline_styles_or_glyph_decoration() {
        for (page, html) in auth_pages().iter().chain(success_notices().iter()) {
            assert!(
                !html.contains("style=\""),
                "no inline style attributes on {page}:\n{html}"
            );
            for glyph in ['\u{2192}', '\u{2190}'] {
                assert!(
                    !html.contains(glyph),
                    "arrow decoration {glyph:?} remains on {page}:\n{html}"
                );
            }
        }
    }

    #[test]
    fn test_auth_pages_label_and_describe_every_input() {
        for (page, html) in auth_pages() {
            let tags = input_tags(&html);
            assert!(!tags.is_empty(), "{page} should render inputs:\n{html}");
            for tag in tags {
                let id = attr_value(tag, "id")
                    .unwrap_or_else(|| panic!("input without id on {page}: {tag}"));
                assert_eq!(
                    attr_value(tag, "name"),
                    Some(id),
                    "input name and id must agree on {page}: {tag}"
                );
                assert!(
                    html.contains(&format!("<label class=\"fig-label\" for=\"{id}\">")),
                    "input {id} needs a real label on {page}:\n{html}"
                );
                assert!(
                    tag.contains(" class=\"fig-input"),
                    "input {id} must use the design-system field on {page}: {tag}"
                );
                if let Some(described_by) = attr_value(tag, "aria-describedby") {
                    assert!(
                        html.contains(&format!("<p class=\"fig-hint\" id=\"{described_by}\">")),
                        "input {id} points at a missing hint on {page}:\n{html}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_auth_pages_announce_their_result_targets() {
        for (page, html) in auth_pages() {
            assert!(
                html.contains(&format!("<div id=\"{page}-result\" aria-live=\"polite\">")),
                "the {page} result target must announce swaps:\n{html}"
            );
        }
    }

    #[test]
    fn test_invite_page_preserves_its_form_contract() {
        let html = render_invite_page().into_string();
        for attribute in [
            "hx-post=\"/auth/invite\"",
            "hx-target=\"#invite-result\"",
            "hx-status:4xx=\"swap:innerHTML target:#invite-result\"",
            "hx-status:5xx=\"swap:innerHTML target:#invite-result\"",
            "hx-swap=\"innerHTML\"",
        ] {
            assert!(html.contains(attribute), "missing {attribute}:\n{html}");
        }

        let api_key = field_tag(&html, "api_key");
        assert_eq!(attr_value(api_key, "type"), Some("password"));
        assert!(api_key.contains(" required"), "{api_key}");
        assert!(!api_key.contains("minlength"), "{api_key}");
        assert!(
            api_key.contains("fig-input--mono"),
            "an API key is a machine value: {api_key}"
        );
    }

    #[test]
    fn test_signup_page_preserves_its_form_contract() {
        let html = render_signup_page().into_string();
        for attribute in [
            "hx-post=\"/auth/signup\"",
            "hx-target=\"#signup-result\"",
            "hx-status:4xx=\"swap:innerHTML target:#signup-result\"",
            "hx-status:5xx=\"swap:innerHTML target:#signup-result\"",
            "hx-swap=\"innerHTML\"",
        ] {
            assert!(html.contains(attribute), "missing {attribute}:\n{html}");
        }

        let invite = field_tag(&html, "invite");
        assert_eq!(attr_value(invite, "type"), Some("text"));
        assert!(invite.contains(" required"), "{invite}");
        assert!(
            invite.contains("fig-input--mono"),
            "an invite is a machine value: {invite}"
        );

        let username = field_tag(&html, "username");
        assert_eq!(attr_value(username, "type"), Some("text"));
        assert_eq!(attr_value(username, "minlength"), Some("3"));
        assert!(
            !username.contains("fig-input--mono"),
            "a username is prose-typed: {username}"
        );

        let email = field_tag(&html, "email");
        assert_eq!(attr_value(email, "type"), Some("email"));
        assert!(email.contains(" required"), "{email}");

        let password = field_tag(&html, "password");
        assert_eq!(attr_value(password, "type"), Some("password"));
        assert_eq!(attr_value(password, "minlength"), Some("8"));
    }

    #[test]
    fn test_login_page_preserves_its_form_contract() {
        let html = render_login_page().into_string();
        for attribute in [
            "hx-post=\"/auth/login\"",
            "hx-target=\"#login-result\"",
            "hx-status:4xx=\"swap:innerHTML target:#login-result\"",
            "hx-status:5xx=\"swap:innerHTML target:#login-result\"",
            "hx-swap=\"innerHTML\"",
        ] {
            assert!(html.contains(attribute), "missing {attribute}:\n{html}");
        }

        let username = field_tag(&html, "username");
        assert_eq!(attr_value(username, "type"), Some("text"));
        assert!(username.contains(" required"), "{username}");
        assert!(
            !username.contains("minlength"),
            "login must not tighten signup's constraints: {username}"
        );

        let password = field_tag(&html, "password");
        assert_eq!(attr_value(password, "type"), Some("password"));
        assert!(password.contains(" required"), "{password}");
        assert!(
            !password.contains("minlength"),
            "login must not tighten signup's constraints: {password}"
        );
    }

    #[test]
    fn test_namespace_page_preserves_its_form_contract() {
        let html = render_namespace_page().into_string();
        for attribute in [
            "hx-post=\"/auth/namespace\"",
            "hx-target=\"#namespace-result\"",
            "hx-status:4xx=\"swap:innerHTML target:#namespace-result\"",
            "hx-status:5xx=\"swap:innerHTML target:#namespace-result\"",
            "hx-swap=\"innerHTML\"",
        ] {
            assert!(html.contains(attribute), "missing {attribute}:\n{html}");
        }

        let name = field_tag(&html, "name");
        assert_eq!(attr_value(name, "type"), Some("text"));
        assert_eq!(attr_value(name, "minlength"), Some("2"));
        assert!(name.contains(" required"), "{name}");
        assert!(
            name.contains("fig-input--mono"),
            "a namespace is a machine value: {name}"
        );
    }

    #[test]
    fn test_auth_submits_are_block_primary_buttons() {
        for (page, html) in auth_pages() {
            assert!(
                html.contains(
                    "<button type=\"submit\" class=\"fig-btn fig-btn--primary fig-btn--block\">"
                ),
                "{page} needs the primary block submit:\n{html}"
            );
        }
    }

    #[test]
    fn test_auth_copy_says_an_invite() {
        for (page, html) in auth_pages() {
            assert!(
                !html.contains("a invite"),
                "grammar slip on {page}:\n{html}"
            );
        }
    }

    #[test]
    fn test_success_notices_are_announced_status_notices() {
        for (flow, html) in success_notices() {
            assert!(
                html.contains("class=\"fig-notice fig-notice--success\" role=\"status\""),
                "{flow} success must be a status notice:\n{html}"
            );
            assert!(
                html.contains("<p class=\"fig-eyebrow\">DONE</p>"),
                "the signal word carries the meaning, not the colour, on {flow}:\n{html}"
            );
            assert!(
                html.contains("<p class=\"fig-notice-body\">"),
                "{flow} success needs body text:\n{html}"
            );
            assert!(
                html.contains("<div class=\"fig-notice-actions\">"),
                "{flow} success needs its next step:\n{html}"
            );
        }
    }

    #[test]
    fn test_render_invite_success_shows_the_invite_as_code() {
        let invite = "abc-123-test";
        let html = render_invite_success(invite).into_string();
        assert!(
            html.contains(&format!("<pre class=\"fig-code\">{invite}</pre>")),
            "the invite is a machine value:\n{html}"
        );
        assert!(
            html.contains("href=\"/auth/signup\""),
            "link to signup should be present:\n{html}"
        );
    }

    #[test]
    fn test_render_signup_success_offers_login_and_namespace() {
        let username = "testuser";
        let html = render_signup_success(username).into_string();
        assert!(
            html.contains(username),
            "username should be displayed:\n{html}"
        );
        for target in ["/auth/login", "/auth/namespace"] {
            assert!(html.contains(target), "missing next step {target}:\n{html}");
        }
    }

    #[test]
    fn test_render_login_success_offers_namespace_creation() {
        let username = "testuser";
        let html = render_login_success(username).into_string();
        assert!(
            html.contains(username),
            "username should be displayed:\n{html}"
        );
        assert!(
            html.contains("/auth/namespace"),
            "link to namespace should be present:\n{html}"
        );
    }

    #[test]
    fn test_success_notices_escape_untrusted_values() {
        for html in [
            render_invite_success("<script>boom()</script>").into_string(),
            render_signup_success("<script>boom()</script>").into_string(),
            render_login_success("<script>boom()</script>").into_string(),
        ] {
            assert!(!html.contains("<script>"), "{html}");
            assert!(html.contains("&lt;script&gt;"), "{html}");
        }
    }
}
