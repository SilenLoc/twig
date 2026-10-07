//! Small TOML-driven syntax highlighting for Twig's repository viewer.
//!
//! Built-in grammars live in the crate's `grammars/` directory. They are
//! included at compile time, so adding a grammar file does not require
//! changing the tokenizer or a language registry in Rust code.

use regex::Regex;
use serde::Deserialize;
use std::{path::Path, sync::OnceLock};

mod included {
    include!(concat!(env!("OUT_DIR"), "/grammars.rs"));
}

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

/// Highlights source using a registered language name, alias, or extension.
///
/// Returns `None` for an unknown language so callers can render source plainly.
pub fn highlight(language: &str, source: &str) -> Option<String> {
    let grammar = grammars().iter().find(|grammar| {
        grammar.name.eq_ignore_ascii_case(language)
            || grammar
                .aliases
                .iter()
                .chain(&grammar.extensions)
                .any(|value| value.eq_ignore_ascii_case(language))
    })?;

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

static GRAMMARS: OnceLock<Vec<CompiledGrammar>> = OnceLock::new();

fn grammars() -> &'static [CompiledGrammar] {
    GRAMMARS.get_or_init(|| {
        included::BUILTIN_GRAMMARS
            .iter()
            .map(|source| {
                let definition: GrammarDefinition =
                    toml::from_str(source).expect("built-in grammar TOML must be valid");
                CompiledGrammar::try_from(definition).expect("built-in grammar rules must be valid")
            })
            .collect()
    })
}

impl Grammar for CompiledGrammar {
    fn highlight(&self, source: &str, output: &mut HtmlWriter<'_>) {
        let mut index = 0;
        let mut plain_start = 0;

        while index < source.len() {
            let matched = self
                .rules
                .iter()
                .find_map(|rule| rule.match_at(&source[index..]));
            if let Some(matched) = matched {
                debug_assert!(matched.end > 0, "grammar rules must consume source");
                output.text(&source[plain_start..index]);

                if let Some((token_start, token_end)) = matched.token {
                    output.text(&source[index..index + token_start]);
                    output.token(
                        matched.kind.expect("token matches have a token kind"),
                        &source[index + token_start..index + token_end],
                    );
                    output.text(&source[index + token_end..index + matched.end]);
                } else {
                    output.text(&source[index..index + matched.end]);
                }

                index += matched.end;
                plain_start = index;
            } else {
                let character = source[index..]
                    .chars()
                    .next()
                    .expect("index is inside a valid UTF-8 string");
                index += character.len_utf8();
            }
        }

        output.text(&source[plain_start..]);
    }
}

#[derive(Deserialize)]
struct GrammarDefinition {
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    extensions: Vec<String>,
    rules: Vec<RuleDefinition>,
}

struct CompiledGrammar {
    name: String,
    aliases: Vec<String>,
    extensions: Vec<String>,
    rules: Vec<CompiledRule>,
}

impl TryFrom<GrammarDefinition> for CompiledGrammar {
    type Error = String;

    fn try_from(definition: GrammarDefinition) -> Result<Self, Self::Error> {
        let rules = definition
            .rules
            .into_iter()
            .map(CompiledRule::try_from)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            name: definition.name,
            aliases: definition.aliases,
            extensions: definition.extensions,
            rules,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RuleKind {
    Keyword,
    Type,
    Function,
    Macro,
    String,
    Lifetime,
    Number,
    Comment,
    Attribute,
    Plain,
}

impl RuleKind {
    fn token_kind(&self) -> Option<TokenKind> {
        match self {
            Self::Keyword => Some(TokenKind::Keyword),
            Self::Type => Some(TokenKind::Type),
            Self::Function => Some(TokenKind::Function),
            Self::Macro => Some(TokenKind::Macro),
            Self::String => Some(TokenKind::String),
            Self::Lifetime => Some(TokenKind::Lifetime),
            Self::Number => Some(TokenKind::Number),
            Self::Comment => Some(TokenKind::Comment),
            Self::Attribute => Some(TokenKind::Attribute),
            Self::Plain => None,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum MatcherKind {
    #[default]
    Regex,
    Delimited,
    HashRawString,
}

#[derive(Deserialize)]
struct RuleDefinition {
    kind: RuleKind,
    #[serde(default)]
    matcher: MatcherKind,
    pattern: Option<String>,
    capture: Option<usize>,
    start: Option<String>,
    end: Option<String>,
    #[serde(default)]
    nested: bool,
    #[serde(default)]
    prefixes: Vec<String>,
}

struct CompiledRule {
    kind: Option<TokenKind>,
    matcher: CompiledMatcher,
}

enum CompiledMatcher {
    Regex {
        regex: Regex,
        capture: Option<usize>,
    },
    Delimited {
        start: String,
        end: String,
        nested: bool,
    },
    HashRawString {
        prefixes: Vec<String>,
    },
}

struct MatchedRule {
    end: usize,
    token: Option<(usize, usize)>,
    kind: Option<TokenKind>,
}

impl TryFrom<RuleDefinition> for CompiledRule {
    type Error = String;

    fn try_from(definition: RuleDefinition) -> Result<Self, Self::Error> {
        let kind = definition.kind.token_kind();
        let matcher = match definition.matcher {
            MatcherKind::Regex => {
                let pattern = definition
                    .pattern
                    .ok_or_else(|| "regex rules must have a pattern".to_owned())?;
                let regex = Regex::new(&format!(r"\A(?:{pattern})"))
                    .map_err(|error| format!("invalid grammar regex {pattern:?}: {error}"))?;
                if regex.is_match("") {
                    return Err(format!(
                        "grammar regex {pattern:?} can match empty input and would not advance"
                    ));
                }
                if let Some(capture) = definition.capture
                    && (capture == 0 || capture >= regex.captures_len())
                {
                    return Err(format!("regex capture {capture} does not exist"));
                }
                CompiledMatcher::Regex {
                    regex,
                    capture: definition.capture,
                }
            }
            MatcherKind::Delimited => {
                let start = definition
                    .start
                    .filter(|start| !start.is_empty())
                    .ok_or_else(|| "delimited rules must have a non-empty start".to_owned())?;
                let end = definition
                    .end
                    .filter(|end| !end.is_empty())
                    .ok_or_else(|| "delimited rules must have a non-empty end".to_owned())?;
                CompiledMatcher::Delimited {
                    start,
                    end,
                    nested: definition.nested,
                }
            }
            MatcherKind::HashRawString => {
                if definition.prefixes.is_empty()
                    || definition.prefixes.iter().any(String::is_empty)
                {
                    return Err("hash raw string rules need non-empty prefixes".to_owned());
                }
                let mut prefixes = definition.prefixes;
                prefixes.sort_by_key(|prefix| std::cmp::Reverse(prefix.len()));
                CompiledMatcher::HashRawString { prefixes }
            }
        };

        Ok(Self { kind, matcher })
    }
}

impl CompiledRule {
    fn match_at(&self, source: &str) -> Option<MatchedRule> {
        match &self.matcher {
            CompiledMatcher::Regex { regex, capture } => {
                let captures = regex.captures(source)?;
                let full = captures.get(0)?;
                let token = if let Some(capture) = capture {
                    let token = captures.get(*capture)?;
                    Some((token.start(), token.end()))
                } else {
                    self.kind.map(|_| (0, full.end()))
                };
                Some(MatchedRule {
                    end: full.end(),
                    token,
                    kind: self.kind,
                })
            }
            CompiledMatcher::Delimited { start, end, nested } => {
                let matched_end = delimited_end(source, start, end, *nested)?;
                Some(MatchedRule {
                    end: matched_end,
                    token: self.kind.map(|_| (0, matched_end)),
                    kind: self.kind,
                })
            }
            CompiledMatcher::HashRawString { prefixes } => {
                let matched_end = hash_raw_string_end(source, prefixes)?;
                Some(MatchedRule {
                    end: matched_end,
                    token: self.kind.map(|_| (0, matched_end)),
                    kind: self.kind,
                })
            }
        }
    }
}

fn delimited_end(source: &str, start: &str, end: &str, nested: bool) -> Option<usize> {
    if !source.starts_with(start) {
        return None;
    }

    let bytes = source.as_bytes();
    let start = start.as_bytes();
    let end = end.as_bytes();
    let mut index = start.len();
    let mut depth = 1usize;

    while index < bytes.len() {
        if nested && bytes.get(index..index + start.len()) == Some(start) {
            depth += 1;
            index += start.len();
        } else if bytes.get(index..index + end.len()) == Some(end) {
            depth -= 1;
            index += end.len();
            if depth == 0 {
                return Some(index);
            }
        } else {
            index += 1;
        }
    }

    Some(bytes.len())
}

fn hash_raw_string_end(source: &str, prefixes: &[String]) -> Option<usize> {
    let bytes = source.as_bytes();
    let prefix_len = prefixes
        .iter()
        .find(|prefix| source.starts_with(prefix.as_str()))?
        .len();
    let mut quote = prefix_len;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }

    let hashes = quote - prefix_len;
    let mut index = quote + 1;
    while index < bytes.len() {
        if bytes[index] == b'"'
            && bytes
                .get(index + 1..index + 1 + hashes)
                .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
        {
            return Some(index + 1 + hashes);
        }
        index += 1;
    }

    Some(bytes.len())
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
    fn rust_grammar_is_loaded_from_the_toml_registry() {
        let html = highlight("RS", "pub fn greet() { println!(\"hi\"); }")
            .expect("Rust TOML grammar is registered");
        assert!(html.contains("<span class=\"twig-syn-keyword\">pub</span>"));
        assert!(html.contains("<span class=\"twig-syn-function\">greet</span>"));
        assert!(html.contains("<span class=\"twig-syn-macro\">println</span>!"));
    }

    #[test]
    fn javascript_grammar_is_loaded_from_toml_and_maps_its_extensions() {
        let source = "const greet = (name) => console.log(`hi, ${name}`);";
        let html = highlight("javascript", source).expect("JavaScript TOML grammar is registered");
        assert!(html.contains("<span class=\"twig-syn-keyword\">const</span>"));
        assert!(html.contains("<span class=\"twig-syn-function\">log</span>("));
        assert!(html.contains("<span class=\"twig-syn-string\">`hi, ${name}`</span>"));
        assert!(highlight_path("src/app.MJS", "export default 42;").is_some());
        assert!(highlight("node", "console.log(1)").is_some());
    }

    #[test]
    fn shell_grammar_highlights_script_tokens_and_maps_shell_extensions() {
        let html = highlight(
            "sh",
            "function greet() { echo 'hello'; }\nif [ -n \"$name\" ]; then echo 'hello'; fi # done",
        )
        .expect("shell TOML grammar is registered");
        assert!(html.contains("<span class=\"twig-syn-keyword\">if</span>"));
        assert!(html.contains("<span class=\"twig-syn-function\">greet</span>"));
        assert!(html.contains("<span class=\"twig-syn-string\">&quot;$name&quot;</span>"));
        assert!(html.contains("<span class=\"twig-syn-comment\"># done</span>"));
        assert!(highlight_path("scripts/deploy.BASH", "echo ready").is_some());
        assert!(highlight("zsh", "echo ready").is_some());
    }

    #[test]
    fn grammar_rules_highlight_tokens_and_escape_source() {
        let html = highlight(
            "rust",
            "#[derive(Debug)]\npub fn greet(name: &str) { println!(\"hi <{}>\", name); }",
        )
        .unwrap();
        assert!(html.contains("<span class=\"twig-syn-attribute\">#[derive(Debug)]</span>"));
        assert!(html.contains("<span class=\"twig-syn-keyword\">pub</span>"));
        assert!(html.contains("<span class=\"twig-syn-function\">greet</span>"));
        assert!(html.contains("<span class=\"twig-syn-type\">str</span>"));
        assert!(html.contains("<span class=\"twig-syn-macro\">println</span>!"));
        assert!(html.contains("&lt;{}&gt;"));
        assert!(!html.contains("<{}>"));
    }

    #[test]
    fn handles_nested_comments_raw_strings_raw_identifiers_and_lifetimes() {
        let source = "/* outer /* inner */ done */ let r#type = r###\"<tag>\"###; // 'a\nlet x = 'static; let c = '\\'';";
        let html = highlight("rust", source).unwrap();
        assert!(
            html.contains("<span class=\"twig-syn-comment\">/* outer /* inner */ done */</span>")
        );
        assert!(
            html.contains("<span class=\"twig-syn-string\">r###&quot;&lt;tag&gt;&quot;###</span>")
        );
        assert!(html.contains("<span class=\"twig-syn-keyword\">let</span> r#type ="));
        assert!(html.contains(
            "<span class=\"twig-syn-keyword\">let</span> x = <span class=\"twig-syn-lifetime\">&#39;static</span>;"
        ));
        assert!(html.contains("<span class=\"twig-syn-string\">&#39;\\&#39;&#39;</span>"));
    }

    #[test]
    fn unclosed_constructs_are_still_safely_rendered() {
        let html = highlight("rust", "/* unfinished <tag>\n\"unfinished").unwrap();
        assert!(html.contains("&lt;tag&gt;"));
        assert!(!html.contains("<tag>"));
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
