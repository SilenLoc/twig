use std::collections::HashMap;

use pulldown_cmark::{Event, Options, Parser, html};

pub fn replace_mustache(input: &str, vars: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, value) in vars {
        let pattern = format!("{{{{{}}}}}", key);
        result = result.replace(&pattern, value);
    }
    result
}

pub fn markdown_to_ast(markdown: &str) -> Vec<Event<'_>> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(markdown, options);
    parser.collect()
}

pub fn ast_to_html(events: Vec<Event<'_>>) -> String {
    let mut html_output = String::new();
    html::push_html(&mut html_output, events.into_iter());
    html_output
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
}
