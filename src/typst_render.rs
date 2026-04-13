//! Typst rendering module
//!
//! For now, this module provides basic typst content display.
//! Full typst-to-HTML rendering requires additional font and layout setup.

/// Renders typst content to HTML
/// Currently shows the source with basic formatting
pub fn render_typst_to_html(content: &str) -> Result<String, Vec<String>> {
    // For now, render as preformatted text with syntax highlighting
    // Full typst compilation requires font setup which is complex
    let escaped = html_escape(content);
    let html = format!(
        r#"<div class="typst-output">
            <div class="typst-source pa3 bg-black-20 br2 overflow-x-auto">
                <pre class="f6 white lh-copy ma0"><code>{}</code></pre>
            </div>
        </div>"#,
        escaped
    );
    Ok(html)
}

/// Simple HTML escaping
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typst_render() {
        let content = r#"# Hello World

This is a test."#;

        let result = render_typst_to_html(content);
        assert!(result.is_ok());
        let html = result.unwrap();
        assert!(html.contains("Hello World"));
    }

    #[test]
    fn test_html_escape() {
        let text = "<script>alert('xss')</script>";
        let escaped = html_escape(text);
        assert!(!escaped.contains("<script>"));
        assert!(escaped.contains("&lt;script&gt;"));
    }
}
