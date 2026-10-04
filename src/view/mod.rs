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

#[cfg(test)]
pub(crate) mod test_util;

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
                        (page_title) " · Twig"
                    } @else {
                        "Twig"
                    }
                }
                link rel="icon" type="image/svg+xml" href="/assets/twig.svg";
                script src="/assets/theme.js" {}
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@300..700&display=swap";
                link rel="stylesheet" href="/assets/t.css";
                link rel="stylesheet" href="/assets/twig.css";
                meta name="htmx-config" content=(r#"{"implicitInheritance":true,"noSwap":[204,304]}"#);
                script src="/assets/h.js" {}
                script src="/assets/hx-live.js" {}
            }
            body
                class="twig-shell"
                "hx-status:4xx"="swap:none"
                "hx-status:5xx"="swap:none"
            {
                a class="twig-skip" href="#main" { "Skip to content" }
                header class="twig-masthead" {
                    div class="twig-page" {
                        a class="twig-wordmark" href="/" aria-label="Twig" title="Twig" {
                            svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512"
                                width="28" height="28" fill="currentColor"
                                aria-hidden="true" focusable="false" {
                                path d="M277.854 21.719c-42.24 50.44-30.12 165.073-10.278 229.41-8.823-8.305-17.446-16.118-25.812-23.387-5.581-55.64-117.363-188.328-202.098-190.19 2.771 78.685 124.137 172.463 180.691 195.653 14.428 12.115 29.969 26.308 46.149 42.027-59.03-33.653-178.97-59.817-234.844-23.816 59.766 44.872 233.049 63.704 265.484 54.621a1693.737 1693.737 0 0 1 36.25 38.797c-55.853-23.885-157.472-36.098-202.011-1.172 54.464 40.555 188.82 44.708 229.54 32.21 3.934 4.535 7.853 9.094 11.753 13.675 2.996.302 6.094.98 9.295 2.123 12.626 4.507 20.422 15.172 22.92 26.547.833 3.796 1.186 7.685 1.152 11.605 14.986 18.605 29.373 37.188 42.752 55.297l14.476-10.697c-16.839-22.792-35.148-46.257-54.228-69.565 31.873-41.549 67.814-172.887 55.117-219.543-52.034 31.759-73.942 139.617-73.437 197.495a1941.033 1941.033 0 0 0-27.368-31.727c31.31-41.627 43.085-205.433 6.63-265.377-50.245 49.897-44.597 179.608-27.876 241.664-14.942-16.378-29.879-32.145-44.525-46.933 22.143-51.978 26.677-206.07-19.732-258.717zm61.59 376.996c-12.783 1.613-26.198 3.251-39.692 4.355-12.109 18.294-16.618 46.407-3.14 50.87 3.905 1.292 9.837.202 16.66-4.172 3.934-2.523 7.937-6.049 11.605-10.125 2.34-13.053 7.71-25.83 15.504-35.243-.079-2.09-.362-3.974-.938-5.685zm-77.397 5.52c-29.695 7.102-56.292 19.962-70.83 39.75 21.218 1.826 49.561 2.081 78.683.898-1.865-12.53 1.392-27.381 7.995-40.604-5.354.134-10.655.142-15.848-.045zm107.88 3.185c-28.637-1.056-37.024 69.093-14.624 71.232 4.095.391 9.635-1.993 15.312-7.777 5.678-5.784 10.93-14.503 14.076-23.566 3.146-9.063 4.114-18.425 2.62-25.233-1.495-6.807-4.378-10.953-11.39-13.455-2.085-.744-4.084-1.13-5.993-1.201z";
                            }
                        }
                        nav aria-label="Primary" class="twig-nav" {
                            @match username {
                                Some(name) => {
                                    span class="twig-nav-user" { (name) }
                                    a class="twig-btn twig-btn--quiet" href="/_info" { "Docs" }
                                    a class="twig-btn twig-btn--quiet" href="/tree" aria-label="Tree" title="Tree" {
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
                                        button class="twig-btn twig-btn--quiet" type="submit" { "Logout" }
                                    }
                                }
                                None => {
                                    a class="twig-btn twig-btn--quiet" href="/_info" { "Docs" }
                                    a class="twig-btn twig-btn--quiet" href="/auth/login" { "Login" }
                                    a class="twig-btn twig-btn--quiet" href="/auth/signup" { "Signup" }
                                }
                            }
                            (render_theme_toggle())
                        }
                    }
                }
                div class="twig-optic-rule" aria-hidden="true" {}
                main id="main" class="twig-main" {
                    div class="twig-page" {
                        (main_content)
                    }
                }
            }
        }
    }
}

pub fn render_theme_toggle() -> maud::Markup {
    maud::html! {
        button class="twig-btn twig-btn--quiet twig-theme-toggle" type="button"
            aria-label="Toggle light and dark mode" title="Toggle light and dark mode" {
            svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"
                stroke-width="1.5" stroke="currentColor" class="twig-theme-icon twig-theme-dark" aria-hidden="true" {
                path stroke-linecap="round" stroke-linejoin="round"
                    d="M21.752 15.002A9.72 9.72 0 0 1 18 15.75c-5.385 0-9.75-4.365-9.75-9.75 0-1.33.266-2.597.748-3.752A9.753 9.753 0 0 0 3 11.25C3 16.635 7.365 21 12.75 21a9.753 9.753 0 0 0 9.002-5.998Z";
            }
            svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"
                stroke-width="1.5" stroke="currentColor" class="twig-theme-icon twig-theme-light" aria-hidden="true" {
                path stroke-linecap="round" stroke-linejoin="round"
                    d="M12 3v2.25m6.364.386-1.591 1.591M21 12h-2.25m-.386 6.364-1.591-1.591M12 18.75V21m-4.773-4.227-1.591 1.591M5.25 12H3m4.227-4.773L5.636 5.636M15.75 12a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0Z";
            }
        }
    }
}

pub fn render_error(message: &str) -> maud::Markup {
    maud::html! {
        div class="twig-notice twig-notice--danger" role="alert" {
            p class="twig-eyebrow" { "ERROR" }
            p class="twig-notice-body" { (message) }
        }
    }
}

pub fn render_error_with_action(message: &str, href: &str, label: &str) -> maud::Markup {
    maud::html! {
        div class="twig-notice twig-notice--danger" role="alert" {
            p class="twig-eyebrow" { "ERROR" }
            p class="twig-notice-body" { (message) }
            div class="twig-notice-actions" {
                a class="twig-btn twig-btn--ghost" href=(href) { (label) }
            }
        }
    }
}

pub fn render_success(message: &str) -> maud::Markup {
    maud::html! {
        div class="twig-notice twig-notice--success" role="status" {
            p class="twig-eyebrow" { "DONE" }
            p class="twig-notice-body" { (message) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::test_util::{classes_in, index_of};

    fn layout_html(username: Option<&str>) -> String {
        let content = maud::html! { p { "hello" } };
        render_layout(&content, username, None).into_string()
    }

    #[test]
    fn test_layout_declares_document_language_and_landmarks() {
        let html = layout_html(Some("testuser"));
        assert!(html.contains("<!DOCTYPE html>"), "{html}");
        assert!(html.contains("<title>Twig</title>"), "{html}");
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

        assert!(html.contains("<title>Settings · Twig</title>"), "{html}");
    }

    #[test]
    fn test_layout_escapes_named_page_title() {
        let content = maud::html! { p { "hello" } };
        let html = render_layout(&content, None, Some("<script>boom()</script>")).into_string();

        assert!(!html.contains("<title><script>"), "{html}");
        assert!(
            html.contains("<title>&lt;script&gt;boom()&lt;/script&gt; · Twig</title>"),
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
            skip < index_of(&html, "<a class=\"twig-wordmark\""),
            "skip link must precede the wordmark: {html}"
        );
    }

    #[test]
    fn test_layout_loads_space_grotesk_and_tachyons_before_twig_css() {
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
            index_of(&html, "/assets/t.css") < index_of(&html, "/assets/twig.css"),
            "twig.css must override t.css: {html}"
        );
    }

    #[test]
    fn test_layout_offers_theme_switch_before_rendering_styles() {
        for username in [Some("testuser"), None] {
            let html = layout_html(username);
            assert!(html.contains("Toggle light and dark mode"), "{html}");
            assert!(html.contains("twig-theme-toggle"), "{html}");
            assert!(
                index_of(&html, "/assets/theme.js") < index_of(&html, "/assets/twig.css"),
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
            "no inline style element; markdown table rules live in twig.css: {html}"
        );
    }

    #[test]
    fn test_layout_uses_only_twig_design_system_classes() {
        for username in [Some("testuser"), None] {
            let html = layout_html(username);
            let classes = classes_in(&html);
            assert!(!classes.is_empty(), "layout should carry classes: {html}");
            for class in classes {
                assert!(
                    class.starts_with("twig-"),
                    "non design-system class {class:?} in layout: {html}"
                );
            }
        }
    }

    #[test]
    fn test_layout_optic_rule_is_decorative() {
        let html = layout_html(None);
        assert!(
            html.contains("class=\"twig-optic-rule\" aria-hidden=\"true\""),
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
        assert!(html.contains("twig-notice--danger"), "{html}");
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
            html.contains("<a class=\"twig-btn twig-btn--ghost\" href=\"/auth/login\">Log in</a>"),
            "{html}"
        );
    }

    #[test]
    fn test_render_success_is_an_announced_status_notice() {
        let html = render_success("operation completed").into_string();
        assert!(html.contains("role=\"status\""), "{html}");
        assert!(html.contains("twig-notice--success"), "{html}");
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
