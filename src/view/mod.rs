use maud::DOCTYPE;

pub mod auth;
pub mod info;
pub mod namespace;
pub mod overview;
pub mod repo;
pub mod session_auth;
pub mod settings;
pub mod test_page;
pub mod tree;

pub fn render_layout(
    main_content: &maud::Markup,
    username: Option<&str>,
    page_title: Option<&str>,
) -> maud::Markup {
    maud::html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title {
                    @if let Some(page_title) = page_title {
                        (page_title) " · Fig"
                    } @else {
                        "Fig"
                    }
                }
                link rel="icon" type="image/svg+xml" href="/assets/fig.svg";
                script src="/assets/theme.js" {}
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@300..700&display=swap";
                link rel="stylesheet" href="/assets/t.css";
                link rel="stylesheet" href="/assets/fig.css";
                meta name="htmx-config" content=(r#"{"implicitInheritance":true,"noSwap":[204,304]}"#);
                script src="/assets/h.js" {}
                script src="/assets/hx-live.js" {}
            }
            body
                class="fig-shell"
                "hx-status:4xx"="swap:none"
                "hx-status:5xx"="swap:none"
            {
                a class="fig-skip" href="#main" { "Skip to content" }
                header class="fig-masthead" {
                    div class="fig-page" {
                        a class="fig-wordmark" href="/" { "Fig" }
                        nav aria-label="Primary" class="fig-nav" {
                            @match username {
                                Some(name) => {
                                    span class="fig-nav-user" { (name) }
                                    a class="fig-btn fig-btn--quiet" href="/_info" { "Information" }
                                    a class="fig-btn fig-btn--quiet" href="/tree" aria-label="Tree" title="Tree" {
                                        svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" {
                                            path d="M5 3v18";
                                            path d="M5 6h8a3 3 0 0 1 3 3v0";
                                            path d="M5 12h5a3 3 0 0 1 3 3v0";
                                            circle cx="5" cy="3" r="2";
                                            circle cx="16" cy="9" r="2";
                                            circle cx="13" cy="15" r="2";
                                            circle cx="5" cy="21" r="2";
                                        }
                                        "Tree"
                                    }
                                    form method="POST" action="/auth/logout" {
                                        button class="fig-btn fig-btn--quiet" type="submit" { "Logout" }
                                    }
                                }
                                None => {
                                    a class="fig-btn fig-btn--quiet" href="/_info" { "Information" }
                                    a class="fig-btn fig-btn--quiet" href="/auth/login" { "Login" }
                                    a class="fig-btn fig-btn--quiet" href="/auth/signup" { "Signup" }
                                }
                            }
                            (render_theme_toggle())
                        }
                    }
                }
                div class="fig-optic-rule" aria-hidden="true" {}
                main id="main" class="fig-main" {
                    div class="fig-page" {
                        (main_content)
                    }
                }
            }
        }
    }
}

pub fn render_theme_toggle() -> maud::Markup {
    maud::html! {
        button class="fig-btn fig-btn--quiet fig-theme-toggle" type="button"
            aria-label="Toggle light and dark mode" title="Toggle light and dark mode" {
            svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"
                stroke-width="1.5" stroke="currentColor" class="fig-theme-icon fig-theme-dark" aria-hidden="true" {
                path stroke-linecap="round" stroke-linejoin="round"
                    d="M21.752 15.002A9.72 9.72 0 0 1 18 15.75c-5.385 0-9.75-4.365-9.75-9.75 0-1.33.266-2.597.748-3.752A9.753 9.753 0 0 0 3 11.25C3 16.635 7.365 21 12.75 21a9.753 9.753 0 0 0 9.002-5.998Z";
            }
            svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"
                stroke-width="1.5" stroke="currentColor" class="fig-theme-icon fig-theme-light" aria-hidden="true" {
                path stroke-linecap="round" stroke-linejoin="round"
                    d="M12 3v2.25m6.364.386-1.591 1.591M21 12h-2.25m-.386 6.364-1.591-1.591M12 18.75V21m-4.773-4.227-1.591 1.591M5.25 12H3m4.227-4.773L5.636 5.636M15.75 12a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0Z";
            }
        }
    }
}

pub fn render_error(message: &str) -> maud::Markup {
    maud::html! {
        div class="fig-notice fig-notice--danger" role="alert" {
            p class="fig-eyebrow" { "ERROR" }
            p class="fig-notice-body" { (message) }
        }
    }
}

pub fn render_error_with_action(message: &str, href: &str, label: &str) -> maud::Markup {
    maud::html! {
        div class="fig-notice fig-notice--danger" role="alert" {
            p class="fig-eyebrow" { "ERROR" }
            p class="fig-notice-body" { (message) }
            div class="fig-notice-actions" {
                a class="fig-btn fig-btn--ghost" href=(href) { (label) }
            }
        }
    }
}

pub fn render_success(message: &str) -> maud::Markup {
    maud::html! {
        div class="fig-notice fig-notice--success" role="status" {
            p class="fig-eyebrow" { "DONE" }
            p class="fig-notice-body" { (message) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_html(username: Option<&str>) -> String {
        let content = maud::html! { p { "hello" } };
        render_layout(&content, username, None).into_string()
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

    fn index_of(html: &str, needle: &str) -> usize {
        html.find(needle)
            .unwrap_or_else(|| panic!("expected markup to contain {needle}\n{html}"))
    }

    #[test]
    fn test_layout_declares_document_language_and_landmarks() {
        let html = layout_html(Some("testuser"));
        assert!(html.contains("<!DOCTYPE html>"), "{html}");
        assert!(html.contains("<title>Fig</title>"), "{html}");
        assert!(
            html.contains("<html lang=\"en\""),
            "document language must be declared: {html}"
        );
        for landmark in [
            "<header",
            "<nav aria-label=\"Primary\"",
            "<main id=\"main\"",
        ] {
            assert!(
                html.contains(landmark),
                "missing landmark {landmark}: {html}"
            );
        }
        assert!(html.contains("hello"), "page content must be rendered");
    }

    #[test]
    fn test_layout_prefixes_named_page_title() {
        let content = maud::html! { p { "hello" } };
        let html = render_layout(&content, None, Some("Settings")).into_string();

        assert!(html.contains("<title>Settings · Fig</title>"), "{html}");
    }

    #[test]
    fn test_layout_escapes_named_page_title() {
        let content = maud::html! { p { "hello" } };
        let html = render_layout(&content, None, Some("<script>boom()</script>")).into_string();

        assert!(!html.contains("<title><script>"), "{html}");
        assert!(
            html.contains("<title>&lt;script&gt;boom()&lt;/script&gt; · Fig</title>"),
            "{html}"
        );
    }

    #[test]
    fn test_layout_skip_link_is_the_first_focusable_element() {
        let html = layout_html(None);
        let skip = index_of(&html, "href=\"#main\"");
        assert!(
            skip < index_of(&html, "<header"),
            "skip link must precede the masthead: {html}"
        );
        assert!(
            skip < index_of(&html, "<a class=\"fig-wordmark\""),
            "skip link must precede the wordmark: {html}"
        );
    }

    #[test]
    fn test_layout_loads_space_grotesk_and_tachyons_before_fig_css() {
        let html = layout_html(None);
        assert!(
            html.contains("family=Space+Grotesk"),
            "Space Grotesk is the sole grotesque: {html}"
        );
        for outgoing in ["Anton", "Bricolage"] {
            assert!(
                !html.contains(outgoing),
                "outgoing face {outgoing} must be removed: {html}"
            );
        }
        assert!(
            index_of(&html, "/assets/t.css") < index_of(&html, "/assets/fig.css"),
            "fig.css must override t.css: {html}"
        );
    }

    #[test]
    fn test_layout_offers_theme_switch_before_rendering_styles() {
        for username in [Some("testuser"), None] {
            let html = layout_html(username);
            assert!(html.contains("Toggle light and dark mode"), "{html}");
            assert!(html.contains("fig-theme-toggle"), "{html}");
            assert!(
                index_of(&html, "/assets/theme.js") < index_of(&html, "/assets/fig.css"),
                "restore the theme before the stylesheet paints: {html}"
            );
        }
    }

    #[test]
    fn test_layout_preserves_htmx_wiring() {
        let html = layout_html(None);
        assert!(html.contains("implicitInheritance"), "{html}");
        assert!(html.contains("&quot;noSwap&quot;:[204,304]"), "{html}");
        assert!(html.contains("hx-status:4xx=\"swap:none\""), "{html}");
        assert!(html.contains("/assets/h.js"), "{html}");
        assert!(html.contains("/assets/hx-live.js"), "{html}");
        assert!(
            index_of(&html, "/assets/h.js") < index_of(&html, "/assets/hx-live.js"),
            "hx-live must load after htmx: {html}"
        );
    }

    #[test]
    fn test_layout_keeps_all_styling_in_stylesheets() {
        let html = layout_html(Some("testuser"));
        assert!(
            !html.contains("style=\""),
            "no inline style attributes: {html}"
        );
        assert!(
            !html.contains("<style"),
            "no inline style element; markdown table rules live in fig.css: {html}"
        );
    }

    #[test]
    fn test_layout_uses_only_fig_design_system_classes() {
        for username in [Some("testuser"), None] {
            let html = layout_html(username);
            let classes = classes_in(&html);
            assert!(!classes.is_empty(), "layout should carry classes: {html}");
            for class in classes {
                assert!(
                    class.starts_with("fig-"),
                    "non design-system class {class:?} in layout: {html}"
                );
            }
        }
    }

    #[test]
    fn test_layout_optic_rule_is_decorative() {
        let html = layout_html(None);
        assert!(
            html.contains("class=\"fig-optic-rule\" aria-hidden=\"true\""),
            "the masthead optic rule must be hidden from assistive tech: {html}"
        );
    }

    #[test]
    fn test_layout_authenticated_nav_exposes_user_actions() {
        let html = layout_html(Some("testuser"));
        assert!(html.contains("testuser"), "{html}");
        for target in ["/_info", "/tree"] {
            assert!(html.contains(target), "missing nav target {target}: {html}");
        }
        assert!(
            !html.contains("href=\"/settings\""),
            "settings should live under Tree: {html}"
        );
        assert!(
            html.contains("method=\"POST\" action=\"/auth/logout\""),
            "logout stays a real POST form: {html}"
        );
        assert!(
            !html.contains("/auth/login"),
            "authenticated nav has no login link: {html}"
        );
    }

    #[test]
    fn test_layout_anonymous_nav_offers_login_and_signup() {
        let html = layout_html(None);
        for target in ["/_info", "/auth/login", "/auth/signup"] {
            assert!(html.contains(target), "missing nav target {target}: {html}");
        }
        assert!(
            !html.contains("/auth/logout"),
            "anonymous nav has no logout: {html}"
        );
    }

    #[test]
    fn test_render_error_is_an_announced_danger_notice() {
        let html = render_error("something went wrong").into_string();
        assert!(html.contains("role=\"alert\""), "{html}");
        assert!(html.contains("fig-notice--danger"), "{html}");
        assert!(
            html.contains(">ERROR<"),
            "the signal word carries the meaning, not the colour: {html}"
        );
        assert!(html.contains("something went wrong"), "{html}");
    }

    #[test]
    fn test_render_error_with_action_uses_a_real_recovery_link() {
        let html =
            render_error_with_action("Sign in required.", "/auth/login", "Log in").into_string();

        assert!(html.contains("role=\"alert\""), "{html}");
        assert!(
            html.contains("<a class=\"fig-btn fig-btn--ghost\" href=\"/auth/login\">Log in</a>"),
            "{html}"
        );
    }

    #[test]
    fn test_render_success_is_an_announced_status_notice() {
        let html = render_success("operation completed").into_string();
        assert!(html.contains("role=\"status\""), "{html}");
        assert!(html.contains("fig-notice--success"), "{html}");
        assert!(
            html.contains(">DONE<"),
            "the signal word carries the meaning, not the colour: {html}"
        );
        assert!(html.contains("operation completed"), "{html}");
    }

    #[test]
    fn test_notices_escape_message_markup() {
        for html in [
            render_error("<script>boom()</script>").into_string(),
            render_error_with_action("<script>boom()</script>", "/auth/login", "Log in")
                .into_string(),
            render_success("<script>boom()</script>").into_string(),
        ] {
            assert!(!html.contains("<script>"), "{html}");
            assert!(html.contains("&lt;script&gt;"), "{html}");
        }
    }
}
