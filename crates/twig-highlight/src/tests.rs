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
fn css_grammar_highlights_styles_and_maps_css_files() {
    let source = "@media screen {\n/* card */\n.card:hover { color: rgb(12, 34, 56); margin: 1.5rem !IMPORTANT; content: \"<ready>\"; }\n}";
    let html = highlight("CSS", source).expect("CSS TOML grammar is registered");
    assert!(html.contains("<span class=\"twig-syn-keyword\">@media</span>"));
    assert!(html.contains("<span class=\"twig-syn-comment\">/* card */</span>"));
    assert!(html.contains("<span class=\"twig-syn-attribute\">.card</span>"));
    assert!(html.contains("<span class=\"twig-syn-attribute\">:hover</span>"));
    assert!(html.contains("<span class=\"twig-syn-keyword\">color</span>"));
    assert!(html.contains("<span class=\"twig-syn-function\">rgb</span>("));
    assert!(html.contains("<span class=\"twig-syn-number\">1.5rem</span>"));
    assert!(html.contains("<span class=\"twig-syn-keyword\">!IMPORTANT</span>"));
    assert!(html.contains("&lt;ready&gt;"));
    assert!(!html.contains("<ready>"));
    assert!(highlight_path("styles/site.CSS", "body { color: red; }").is_some());
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
    assert!(html.contains("<span class=\"twig-syn-comment\">/* outer /* inner */ done */</span>"));
    assert!(html.contains("<span class=\"twig-syn-string\">r###&quot;&lt;tag&gt;&quot;###</span>"));
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
