use crate::{Grammar, HtmlWriter, TokenKind};

pub(super) struct Rust;

impl Grammar for Rust {
    fn highlight(&self, source: &str, output: &mut HtmlWriter<'_>) {
        let bytes = source.as_bytes();
        let mut index = 0;
        let mut plain_start = 0;

        while index < bytes.len() {
            let token = token_at(source, bytes, index);
            if let Some((end, kind)) = token {
                output.text(&source[plain_start..index]);
                if let Some(kind) = kind {
                    output.token(kind, &source[index..end]);
                } else {
                    output.text(&source[index..end]);
                }
                index = end;
                plain_start = end;
            } else if bytes[index].is_ascii() {
                index += 1;
            } else {
                // Non-ASCII identifier characters are left unclassified, but
                // advance by a whole scalar so every source slice stays valid.
                index += source[index..]
                    .chars()
                    .next()
                    .expect("index is inside a valid UTF-8 string")
                    .len_utf8();
            }
        }

        output.text(&source[plain_start..]);
    }
}

fn token_at(source: &str, bytes: &[u8], index: usize) -> Option<(usize, Option<TokenKind>)> {
    if starts_with(bytes, index, b"//") {
        let end = bytes[index..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |offset| index + offset);
        return Some((end, Some(TokenKind::Comment)));
    }

    if starts_with(bytes, index, b"/*") {
        return Some((block_comment_end(bytes, index), Some(TokenKind::Comment)));
    }

    if let Some(end) = attribute_end(source, bytes, index) {
        return Some((end, Some(TokenKind::Attribute)));
    }

    if let Some(end) = raw_string_end(bytes, index) {
        return Some((end, Some(TokenKind::String)));
    }

    if let Some(quote_index) = string_quote_index(bytes, index) {
        return Some((
            quoted_end(bytes, quote_index, b'"'),
            Some(TokenKind::String),
        ));
    }

    if bytes[index] == b'\'' {
        if let Some(end) = character_end(source, index) {
            return Some((end, Some(TokenKind::String)));
        }
        if bytes
            .get(index + 1)
            .is_some_and(|byte| is_identifier_start(*byte))
        {
            return Some((identifier_end(bytes, index + 1), Some(TokenKind::Lifetime)));
        }
    }

    if bytes[index].is_ascii_digit() {
        return Some((number_end(bytes, index), Some(TokenKind::Number)));
    }

    if is_identifier_start(bytes[index]) {
        let (identifier_start, end, is_raw) = if starts_with(bytes, index, b"r#")
            && bytes
                .get(index + 2)
                .is_some_and(|byte| is_identifier_start(*byte))
        {
            let end = identifier_end(bytes, index + 2);
            (index + 2, end, true)
        } else {
            (index, identifier_end(bytes, index), false)
        };

        let identifier = &source[identifier_start..end];
        let kind = if !is_raw && is_keyword(identifier) {
            Some(TokenKind::Keyword)
        } else if is_type(identifier) {
            Some(TokenKind::Type)
        } else if follows_macro_bang(source, end) {
            Some(TokenKind::Macro)
        } else if follows_call_paren(source, end) {
            Some(TokenKind::Function)
        } else {
            None
        };
        return if is_raw {
            Some((end, kind))
        } else {
            kind.map(|kind| (end, Some(kind)))
        };
    }

    None
}

fn starts_with(bytes: &[u8], index: usize, prefix: &[u8]) -> bool {
    bytes.get(index..index + prefix.len()) == Some(prefix)
}

fn block_comment_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 2;
    let mut depth = 1usize;

    while index + 1 < bytes.len() {
        match (bytes[index], bytes[index + 1]) {
            (b'/', b'*') => {
                depth += 1;
                index += 2;
            }
            (b'*', b'/') => {
                depth -= 1;
                index += 2;
                if depth == 0 {
                    return index;
                }
            }
            _ => index += 1,
        }
    }

    bytes.len()
}

fn attribute_end(source: &str, bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'#') {
        return None;
    }

    let mut bracket = start + 1;
    if bytes.get(bracket) == Some(&b'!') {
        bracket += 1;
    }
    if bytes.get(bracket) != Some(&b'[') {
        return None;
    }

    let mut depth = 0usize;
    let mut index = bracket;
    while index < bytes.len() {
        if starts_with(bytes, index, b"//") {
            index = bytes[index..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| index + offset);
        } else if starts_with(bytes, index, b"/*") {
            index = block_comment_end(bytes, index);
        } else if let Some(end) = raw_string_end(bytes, index) {
            index = end;
        } else if let Some(quote) = string_quote_index(bytes, index) {
            index = quoted_end(bytes, quote, b'"');
        } else if bytes[index] == b'\'' {
            if let Some(end) = character_end(source, index) {
                index = end;
            } else {
                index += 1;
            }
        } else {
            match bytes[index] {
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index + 1);
                    }
                }
                _ => {}
            }
            index += 1;
        }
    }

    Some(bytes.len())
}

fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let raw_start = if bytes.get(start) == Some(&b'r') {
        start
    } else if starts_with(bytes, start, b"br") {
        start + 1
    } else {
        return None;
    };

    let mut quote = raw_start + 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }

    let hashes = quote - raw_start - 1;
    let mut index = quote + 1;
    while index < bytes.len() {
        if bytes[index] == b'"'
            && bytes.get(index + 1..index + 1 + hashes) == Some(&bytes[raw_start + 1..quote])
        {
            return Some(index + 1 + hashes);
        }
        index += 1;
    }

    Some(bytes.len())
}

fn string_quote_index(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) == Some(&b'"') {
        Some(start)
    } else if matches!(bytes.get(start..start + 2), Some(b"b\"") | Some(b"c\"")) {
        Some(start + 1)
    } else {
        None
    }
}

fn quoted_end(bytes: &[u8], quote: usize, delimiter: u8) -> usize {
    let mut index = quote + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index = (index + 2).min(bytes.len()),
            byte if byte == delimiter => return index + 1,
            b'\n' if delimiter == b'\'' => return index,
            _ => index += 1,
        }
    }
    bytes.len()
}

fn character_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut index = start + 1;
    match bytes.get(index).copied()? {
        b'\\' => {
            index += 1;
            match bytes.get(index).copied()? {
                b'u' if bytes.get(index + 1) == Some(&b'{') => {
                    index += 2;
                    while bytes
                        .get(index)
                        .is_some_and(|byte| *byte != b'}' && *byte != b'\n')
                    {
                        index += 1;
                    }
                    if bytes.get(index) != Some(&b'}') {
                        return None;
                    }
                    index += 1;
                }
                _ => {
                    let character = source.get(index..)?.chars().next()?;
                    index += character.len_utf8();
                }
            }
        }
        b'\n' | b'\r' => return None,
        _ => {
            let character = source.get(index..)?.chars().next()?;
            index += character.len_utf8();
        }
    }

    (bytes.get(index) == Some(&b'\'')).then_some(index + 1)
}

fn number_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start;
    while index < bytes.len() {
        let byte = bytes[index];
        let decimal_point = byte == b'.'
            && bytes.get(index + 1).is_some_and(u8::is_ascii_digit)
            && bytes.get(index + 1) != Some(&b'.');
        let exponent_sign = matches!(byte, b'+' | b'-')
            && index > start
            && matches!(bytes[index - 1], b'e' | b'E' | b'p' | b'P');
        if byte.is_ascii_alphanumeric() || byte == b'_' || decimal_point || exponent_sign {
            index += 1;
        } else {
            break;
        }
    }
    index
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn identifier_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        index += 1;
    }
    index
}

fn follows_macro_bang(source: &str, end: usize) -> bool {
    source[end..].trim_start().starts_with('!')
}

fn follows_call_paren(source: &str, end: usize) -> bool {
    source[end..].trim_start().starts_with('(')
}

fn is_type(identifier: &str) -> bool {
    identifier == "Self"
        || matches!(
            identifier,
            "bool"
                | "char"
                | "str"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "f32"
                | "f64"
        )
        || identifier
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_uppercase)
}

fn is_keyword(identifier: &str) -> bool {
    matches!(
        identifier,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "union"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
    )
}

#[cfg(test)]
mod tests {
    use crate::highlight;

    fn rust(source: &str) -> String {
        highlight("rust", source).expect("Rust is registered")
    }

    #[test]
    fn highlights_rust_tokens_and_escapes_source() {
        let html =
            rust("#[derive(Debug)]\npub fn greet(name: &str) { println!(\"hi <{}>\", name); }");
        assert!(html.contains("<span class=\"twig-syn-attribute\">#[derive(Debug)]</span>"));
        assert!(html.contains("<span class=\"twig-syn-keyword\">pub</span>"));
        assert!(html.contains("<span class=\"twig-syn-function\">greet</span>"));
        assert!(html.contains("<span class=\"twig-syn-type\">str</span>"));
        assert!(html.contains("<span class=\"twig-syn-macro\">println</span>!"));
        assert!(html.contains("&lt;{}&gt;"));
        assert!(!html.contains("<{}>"));
    }

    #[test]
    fn handles_comments_strings_raw_identifiers_and_lifetimes() {
        let source = "/* outer /* inner */ done */ let r#type = r###\"<tag>\"###; // 'a\nlet x = 'static; let c = '\\'';";
        let html = rust(source);
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
        let html = rust("/* unfinished <tag>\n\"unfinished");
        assert!(html.contains("&lt;tag&gt;"));
        assert!(!html.contains("<tag>"));
    }
}
