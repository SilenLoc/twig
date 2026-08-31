//! Markdown rendering for ticket bodies and comments.
//!
//! Ticket text comes from authenticated but not necessarily trusted users, so
//! raw HTML in the source is escaped rather than passed through — the same
//! discipline `view::repo` applies to repository markdown. `@mentions` are
//! linkified only when they resolve to a real account, so a typo renders as
//! plain text instead of a dead link.

use std::collections::BTreeSet;

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};

/// Characters allowed in a mention. Keeping this narrow means the name can be
/// placed in an href and in element text without further escaping.
fn is_mention_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// Usernames referenced with `@` in `text`, whether or not they exist.
pub fn extract_mentions(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let bytes: Vec<char> = text.chars().collect();

    for (index, ch) in bytes.iter().enumerate() {
        if *ch != '@' {
            continue;
        }
        // Must start a word, so `email@example.com` is not a mention.
        if index > 0 && (is_mention_char(bytes[index - 1]) || bytes[index - 1] == '@') {
            continue;
        }
        let name: String = bytes[index + 1..]
            .iter()
            .take_while(|c| is_mention_char(**c))
            .collect();
        if !name.is_empty() {
            found.insert(name);
        }
    }

    found
}

/// Renders markdown to HTML, escaping any embedded HTML and highlighting
/// mentions that appear in `known_users`.
pub fn render(markdown: &str, known_users: &BTreeSet<String>) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let mut events: Vec<Event> = Vec::new();
    let mut in_code_block = false;

    for event in Parser::new_ext(markdown, options) {
        match event {
            // Raw HTML from the author is rendered as visible text, never as
            // markup.
            Event::Html(text) | Event::InlineHtml(text) => events.push(Event::Text(text)),

            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                events.push(Event::Start(Tag::CodeBlock(kind)));
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                events.push(Event::End(TagEnd::CodeBlock));
            }

            Event::Text(text) if !in_code_block => {
                push_with_mentions(&mut events, &text, known_users);
            }

            other => events.push(other),
        }
    }

    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
}

/// Splits `text` into plain runs and mention links.
fn push_with_mentions(events: &mut Vec<Event<'_>>, text: &str, known_users: &BTreeSet<String>) {
    let chars: Vec<char> = text.chars().collect();
    let mut plain = String::new();
    let mut index = 0;

    while index < chars.len() {
        let starts_word =
            index == 0 || !(is_mention_char(chars[index - 1]) || chars[index - 1] == '@');

        if chars[index] == '@' && starts_word {
            let name: String = chars[index + 1..]
                .iter()
                .take_while(|c| is_mention_char(**c))
                .collect();

            if !name.is_empty() && known_users.contains(&name) {
                if !plain.is_empty() {
                    events.push(Event::Text(CowStr::from(std::mem::take(&mut plain))));
                }
                // Rendered as a styled span rather than a link: Fig has no
                // per-user page, and a link to a 404 is worse than none.
                // `name` is restricted to [A-Za-z0-9_-], so it is safe as
                // element text without further escaping.
                events.push(Event::InlineHtml(CowStr::from(format!(
                    r#"<span class="tf-mention">@{name}</span>"#
                ))));
                index += 1 + name.chars().count();
                continue;
            }
        }

        plain.push(chars[index]);
        index += 1;
    }

    if !plain.is_empty() {
        events.push(Event::Text(CowStr::from(plain)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn users(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| (*n).to_string()).collect()
    }

    #[test]
    fn test_basic_markdown_renders() {
        let html = render("**bold** and *italic*", &users(&[]));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
    }

    #[test]
    fn test_tables_are_enabled() {
        let html = render("| a | b |\n|---|---|\n| 1 | 2 |", &users(&[]));
        assert!(html.contains("<table>"), "{html}");
    }

    #[test]
    fn test_raw_html_is_escaped_not_executed() {
        let html = render("<script>alert(1)</script>", &users(&[]));
        assert!(
            !html.contains("<script>"),
            "script tag must not survive: {html}"
        );
        assert!(html.contains("&lt;script&gt;"), "{html}");
    }

    #[test]
    fn test_inline_html_is_escaped() {
        let html = render("hello <img src=x onerror=alert(1)> world", &users(&[]));
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains("&lt;img"), "{html}");
    }

    #[test]
    fn test_known_mention_is_highlighted() {
        let html = render("cc @alice please", &users(&["alice"]));
        assert!(
            html.contains(r#"<span class="tf-mention">@alice</span>"#),
            "{html}"
        );
    }

    #[test]
    fn test_unknown_mention_stays_plain_text() {
        let html = render("cc @nobody please", &users(&["alice"]));
        assert!(!html.contains("tf-mention"), "{html}");
        assert!(html.contains("@nobody"), "{html}");
    }

    #[test]
    fn test_email_is_not_a_mention() {
        let html = render("write to bob@alice.com", &users(&["alice"]));
        assert!(!html.contains("tf-mention"), "{html}");
    }

    #[test]
    fn test_mention_inside_code_block_is_not_highlighted() {
        let html = render("```\ncc @alice\n```", &users(&["alice"]));
        assert!(
            !html.contains("tf-mention"),
            "mentions in code blocks stay literal: {html}"
        );
    }

    #[test]
    fn test_mention_inside_code_span_is_not_highlighted() {
        let html = render("use `@alice` here", &users(&["alice"]));
        assert!(!html.contains("tf-mention"), "{html}");
        assert!(html.contains("<code>@alice</code>"), "{html}");
    }

    #[test]
    fn test_extract_mentions_finds_all_names() {
        let found = extract_mentions("@alice and @bob-2, not bob@example.com, and @alice again");
        assert!(found.contains("alice"));
        assert!(found.contains("bob-2"));
        assert!(!found.contains("example"));
        assert_eq!(found.len(), 2, "duplicates collapse: {found:?}");
    }

    #[test]
    fn test_extract_mentions_ignores_bare_at() {
        assert!(extract_mentions("just an @ sign").is_empty());
    }

    #[test]
    fn test_mention_at_start_of_text_is_highlighted() {
        let html = render("@alice look", &users(&["alice"]));
        assert!(
            html.contains(r#"<span class="tf-mention">@alice</span>"#),
            "{html}"
        );
    }

    #[test]
    fn test_text_around_mention_is_preserved() {
        let html = render("before @alice after", &users(&["alice"]));
        assert!(html.contains("before "), "{html}");
        assert!(html.contains(" after"), "{html}");
    }
}
