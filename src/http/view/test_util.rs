//! Test-only markup helpers shared by the view test modules.

/// All class names appearing in `class="..."` attributes, in document order.
pub(crate) fn classes_in(html: &str) -> Vec<String> {
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

/// Position of `needle` in `html`, panicking with the full markup when absent.
pub(crate) fn index_of(html: &str, needle: &str) -> usize {
    html.find(needle)
        .unwrap_or_else(|| panic!("expected markup to contain {needle}\n{html}"))
}
