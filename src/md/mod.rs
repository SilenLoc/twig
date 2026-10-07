use std::collections::HashMap;

use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd, html};

pub fn replace_mustache(input: &str, vars: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, value) in vars {
        let pattern = format!("{{{{{key}}}}}");
        result = result.replace(&pattern, value);
    }
    result
}

/// Whether `name` points at a markdown file, matching the extension
/// case-insensitively so `README.MD` counts just like `README.md`.
pub fn is_markdown(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

pub fn markdown_to_ast(markdown: &str) -> Vec<Event<'_>> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(markdown, options);
    parser.collect()
}

pub fn ast_to_html(events: Vec<Event<'_>>) -> String {
    let mut html_output = String::new();
    let events = render_mermaid_blocks(events);
    html::push_html(
        &mut html_output,
        highlight_rust_code_blocks(events.into_iter()),
    );
    html_output
}

/// Adds Rust token markup to fenced Rust blocks while leaving every other
/// Markdown event untouched. The highlighted source is generated and escaped
/// by the dependency-free `twig-highlight` crate before it enters raw HTML.
pub fn highlight_rust_code_blocks<'a>(
    mut events: impl Iterator<Item = Event<'a>> + 'a,
) -> impl Iterator<Item = Event<'a>> + 'a {
    let mut code_block: Option<String> = None;

    std::iter::from_fn(move || {
        loop {
            let Some(event) = events.next() else {
                return code_block
                    .take()
                    .map(|source| highlighted_rust_event(&source));
            };

            if code_block.is_none() && is_rust_block_start(&event) {
                code_block = Some(String::new());
                continue;
            }

            if let Some(source) = &mut code_block {
                let is_end = matches!(&event, Event::End(TagEnd::CodeBlock));
                if let Event::Text(text) = &event {
                    source.push_str(text);
                }

                if is_end {
                    let source = code_block.take().expect("active code block");
                    return Some(highlighted_rust_event(&source));
                }
                continue;
            }

            return Some(event);
        }
    })
}

fn highlighted_rust_event(source: &str) -> Event<'static> {
    let highlighted =
        twig_highlight::highlight("rust", source).expect("Rust grammar is registered");
    Event::Html(CowStr::Boxed(
        format!("<pre><code class=\"language-rust\">{highlighted}</code></pre>").into_boxed_str(),
    ))
}

fn is_rust_block_start(event: &Event<'_>) -> bool {
    matches!(event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info)))
        if info.split_whitespace().next()
            .and_then(|language| language.split(',').next())
            .is_some_and(|language| language.eq_ignore_ascii_case("rust") || language.eq_ignore_ascii_case("rs")))
}

/// Replace valid fenced Mermaid blocks with inline SVG. If parsing or rendering
/// fails, keep the original events so the source remains visible as code.
fn render_mermaid_blocks<'a>(events: Vec<Event<'a>>) -> Vec<Event<'a>> {
    let mut rendered = Vec::with_capacity(events.len());
    let mut code_block: Option<(Vec<Event<'a>>, String)> = None;

    for event in events {
        if code_block.is_none() && is_mermaid_block_start(&event) {
            code_block = Some((vec![event], String::new()));
            continue;
        }

        if code_block.is_some() {
            let is_end = matches!(&event, Event::End(TagEnd::CodeBlock));
            if let Some((block_events, source)) = &mut code_block {
                if let Event::Text(text) = &event {
                    source.push_str(text);
                }
                block_events.push(event);
            }

            if is_end {
                let (original_events, source) = code_block.take().expect("active code block");
                if let Ok(svg) = mermaid_rs_renderer::render(&source) {
                    rendered.push(Event::Html(CowStr::Boxed(
                        format!("<div class=\"twig-mermaid\">{svg}</div>").into_boxed_str(),
                    )));
                } else {
                    rendered.extend(original_events);
                }
            }
        } else {
            rendered.push(event);
        }
    }

    if let Some((original_events, _)) = code_block {
        rendered.extend(original_events);
    }

    rendered
}

fn is_mermaid_block_start(event: &Event<'_>) -> bool {
    matches!(event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info)))
        if info.split_whitespace().next().is_some_and(|language| language.eq_ignore_ascii_case("mermaid")))
}

pub fn process_markdown(markdown: &str, vars: &HashMap<String, String>) -> String {
    let replaced = replace_mustache(markdown, vars);
    let events = markdown_to_ast(&replaced);
    ast_to_html(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_mustache_simple() {
        let input = "Hello, {{author}}!";
        let mut vars = HashMap::new();
        vars.insert("author".to_string(), "Jane".to_string());
        assert_eq!(replace_mustache(input, &vars), "Hello, Jane!");
    }

    #[test]
    fn test_replace_mustache_multiple() {
        let input = "{{author}} - {{title}}";
        let mut vars = HashMap::new();
        vars.insert("author".to_string(), "Jane".to_string());
        vars.insert("title".to_string(), "My Talk".to_string());
        assert_eq!(replace_mustache(input, &vars), "Jane - My Talk");
    }

    #[test]
    fn test_replace_mustache_no_match() {
        let input = "Hello, {{author}}!";
        let vars = HashMap::new();
        assert_eq!(replace_mustache(input, &vars), "Hello, {{author}}!");
    }

    #[test]
    fn test_process_markdown() {
        let md = "# Hello {{name}}\n\nThis is **bold**.";
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "World".to_string());
        let html = process_markdown(md, &vars);
        assert!(html.contains("<h1>Hello World</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_markdown_to_ast() {
        let md = "# Hello\n\nParagraph";
        let events = markdown_to_ast(md);
        assert!(!events.is_empty());
    }

    #[test]
    fn test_ast_to_html() {
        let md = "# Hello\n\nParagraph";
        let events = markdown_to_ast(md);
        let html = ast_to_html(events);
        assert!(html.contains("<h1>Hello</h1>"));
    }

    #[test]
    fn test_process_markdown_renders_mermaid_fences_as_svg() {
        let md = "Before\n\n```mermaid\nflowchart LR\n    A[Start] --> B[End]\n```\n\nAfter";
        let html = process_markdown(md, &HashMap::new());

        assert!(html.contains("<div class=\"twig-mermaid\"><svg"), "{html}");
        assert!(
            html.contains("Start"),
            "the rendered diagram includes its labels: {html}"
        );
        assert!(
            !html.contains("<pre><code class=\"language-mermaid\">"),
            "{html}"
        );
        assert!(html.find("Before").unwrap() < html.find("twig-mermaid").unwrap());
        assert!(html.find("twig-mermaid").unwrap() < html.find("After").unwrap());
    }

    #[test]
    fn test_invalid_mermaid_fence_falls_back_to_code() {
        let html = process_markdown("```mermaid\nnot a diagram\n```", &HashMap::new());

        assert!(
            html.contains("<pre><code class=\"language-mermaid\">"),
            "{html}"
        );
        assert!(html.contains("not a diagram"), "{html}");
        assert!(!html.contains("twig-mermaid"), "{html}");
    }

    #[test]
    fn test_non_mermaid_fences_remain_code() {
        let html = process_markdown("```rust\nfn main() {}\n```", &HashMap::new());

        assert!(
            html.contains("<pre><code class=\"language-rust\">"),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"twig-syn-keyword\">fn</span>"),
            "{html}"
        );
        assert!(!html.contains("twig-mermaid"), "{html}");
    }

    #[test]
    fn test_rust_fences_are_highlighted_and_other_fences_are_unchanged() {
        let html = process_markdown(
            "```rust\nfn main() { let message = \"<hi>\"; }\n```\n\n```text\nfn main() {}\n```",
            &HashMap::new(),
        );

        assert!(
            html.contains("<pre><code class=\"language-rust\">"),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"twig-syn-keyword\">fn</span>"),
            "{html}"
        );
        assert!(html.contains("&lt;hi&gt;"), "{html}");
        assert!(
            html.contains("<pre><code class=\"language-text\">fn main() {}\n</code></pre>"),
            "{html}"
        );
    }
}
