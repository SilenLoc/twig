use super::*;
use crate::http::view::test_util::classes_in;

fn auth_pages() -> [(&'static str, String); 4] {
    [
        ("invite", render_invite_page().into_string()),
        ("signup", render_signup_page().into_string()),
        ("login", render_login_page(None).into_string()),
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
fn test_auth_markup_uses_only_twig_design_system_classes() {
    for (page, html) in auth_pages().iter().chain(success_notices().iter()) {
        let classes = classes_in(html);
        assert!(!classes.is_empty(), "{page} should carry classes:\n{html}");
        for class in classes {
            assert!(
                class.starts_with("twig-"),
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
                html.contains(&format!("<label class=\"twig-label\" for=\"{id}\">")),
                "input {id} needs a real label on {page}:\n{html}"
            );
            assert!(
                tag.contains(" class=\"twig-input"),
                "input {id} must use the design-system field on {page}: {tag}"
            );
            if let Some(described_by) = attr_value(tag, "aria-describedby") {
                assert!(
                    html.contains(&format!("<p class=\"twig-hint\" id=\"{described_by}\">")),
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
        api_key.contains("twig-input--mono"),
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
        invite.contains("twig-input--mono"),
        "an invite is a machine value: {invite}"
    );

    let username = field_tag(&html, "username");
    assert_eq!(attr_value(username, "type"), Some("text"));
    assert_eq!(attr_value(username, "minlength"), Some("3"));
    assert!(
        !username.contains("twig-input--mono"),
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
    let html = render_login_page(None).into_string();
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
fn login_page_preserves_only_a_safe_device_return_target() {
    let html = render_login_page(Some("/auth/device?user_code=WQJH-8K2M")).into_string();
    assert!(
        html.contains("name=\"return_to\" value=\"/auth/device?user_code=WQJH-8K2M\""),
        "the safe device return target must survive browser login: {html}"
    );

    let unsafe_html = render_login_page(Some("https://attacker.example/")).into_string();
    assert!(
        !unsafe_html.contains("name=\"return_to\""),
        "an external redirect must not be added to the login form: {unsafe_html}"
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
        name.contains("twig-input--mono"),
        "a namespace is a machine value: {name}"
    );
}

#[test]
fn test_auth_submits_are_block_primary_buttons() {
    for (page, html) in auth_pages() {
        assert!(
            html.contains(
                "<button type=\"submit\" class=\"twig-btn twig-btn--primary twig-btn--block\">"
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
            html.contains("class=\"twig-notice twig-notice--success\" role=\"status\""),
            "{flow} success must be a status notice:\n{html}"
        );
        assert!(
            html.contains("<p class=\"twig-eyebrow\">DONE</p>"),
            "the signal word carries the meaning, not the colour, on {flow}:\n{html}"
        );
        assert!(
            html.contains("<p class=\"twig-notice-body\">"),
            "{flow} success needs body text:\n{html}"
        );
        assert!(
            html.contains("<div class=\"twig-notice-actions\">"),
            "{flow} success needs its next step:\n{html}"
        );
    }
}

#[test]
fn test_render_invite_success_shows_the_invite_as_code() {
    let invite = "abc-123-test";
    let html = render_invite_success(invite).into_string();
    assert!(
        html.contains(&format!("<pre class=\"twig-code\">{invite}</pre>")),
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
