//! Small, dependency-free syntax highlighting for Twig's repository viewer.
//!
//! A grammar recognizes tokens and writes them through [`HtmlWriter`], which
//! escapes source text while adding semantic CSS classes. New grammars can be
//! implemented without changing the tokenizer infrastructure; add their
//! language name to [`highlight`] to make them available to the app.

mod rust;

use std::path::Path;

/// The semantic role assigned to a highlighted token.
///
/// The matching `twig-syn-*` CSS variables live in the app stylesheet so the
/// palette can change independently of language grammars.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Keyword,
    Type,
    Function,
    Macro,
    String,
    Lifetime,
    Number,
    Comment,
    Attribute,
}

impl TokenKind {
    const fn class(self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Type => "type",
            Self::Function => "function",
            Self::Macro => "macro",
            Self::String => "string",
            Self::Lifetime => "lifetime",
            Self::Number => "number",
            Self::Comment => "comment",
            Self::Attribute => "attribute",
        }
    }
}

/// Output target for a grammar. Both plain text and tokens are HTML-escaped.
pub struct HtmlWriter<'a> {
    output: &'a mut String,
}

impl HtmlWriter<'_> {
    /// Writes source text without adding markup.
    pub fn text(&mut self, text: &str) {
        escape_into(self.output, text);
    }

    /// Writes a highlighted token using its stable semantic CSS class.
    pub fn token(&mut self, kind: TokenKind, text: &str) {
        self.output.push_str("<span class=\"twig-syn-");
        self.output.push_str(kind.class());
        self.output.push_str("\">");
        escape_into(self.output, text);
        self.output.push_str("</span>");
    }
}

/// A language grammar that emits escaped source and semantic token spans.
pub trait Grammar {
    /// Writes highlighted `source` into `output`.
    fn highlight(&self, source: &str, output: &mut HtmlWriter<'_>);
}

/// Highlights source using a registered language name (`rust` or `rs`).
///
/// Returns `None` for an unknown language so callers can render source plainly.
pub fn highlight(language: &str, source: &str) -> Option<String> {
    let grammar: &dyn Grammar =
        if language.eq_ignore_ascii_case("rust") || language.eq_ignore_ascii_case("rs") {
            &rust::Rust
        } else {
            return None;
        };

    Some(highlight_with(grammar, source))
}

/// Highlights a file when its extension maps to a registered grammar.
pub fn highlight_path(path: &str, source: &str) -> Option<String> {
    let extension = Path::new(path).extension()?.to_str()?;
    highlight(extension, source)
}

/// Runs a grammar directly, useful for grammars that are not in the built-in
/// language registry.
pub fn highlight_with<G: Grammar + ?Sized>(grammar: &G, source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    grammar.highlight(
        source,
        &mut HtmlWriter {
            output: &mut output,
        },
    );
    output
}

fn escape_into(output: &mut String, text: &str) {
    let bytes = text.as_bytes();
    let mut plain_start = 0;

    for (index, byte) in bytes.iter().enumerate() {
        let escaped = match byte {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&#39;",
            _ => continue,
        };

        output.push_str(&text[plain_start..index]);
        output.push_str(escaped);
        plain_start = index + 1;
    }

    output.push_str(&text[plain_start..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_languages_are_not_highlighted() {
        assert_eq!(highlight("python", "print(1)"), None);
    }

    #[test]
    fn extension_lookup_is_case_insensitive() {
        assert!(highlight_path("src/main.RS", "fn main() {}").is_some());
        assert_eq!(highlight_path("README.md", "text"), None);
    }

    #[test]
    fn source_is_escaped_even_when_it_is_not_a_token() {
        assert_eq!(highlight("rust", "<&"), Some("&lt;&amp;".into()));
    }

    #[test]
    fn custom_grammars_use_the_shared_safe_writer() {
        struct TinyGrammar;

        impl Grammar for TinyGrammar {
            fn highlight(&self, source: &str, output: &mut HtmlWriter<'_>) {
                output.token(TokenKind::Keyword, source);
            }
        }

        assert_eq!(
            highlight_with(&TinyGrammar, "<hello>"),
            "<span class=\"twig-syn-keyword\">&lt;hello&gt;</span>"
        );
    }
}
