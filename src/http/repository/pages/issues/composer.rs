//! The Markdown field for issue text. The textarea is Markdown source and is what the form
//! posts. With the composer script, a Rich text mode edits the same Markdown through Quill,
//! as the repository file editor does. Without it, the field is a plain Markdown box.

use maud::{Markup, html};

/// One Markdown field: the source box, and the hidden Rich text mode and its toolbar.
/// The label is shown by the caller; `label` names the field for assistive technology.
pub(super) fn markdown_field(
    id: &str,
    name: &str,
    label: &str,
    value: &str,
    rows: u32,
    placeholder: Option<&str>,
) -> Markup {
    html! {
        div class="twig-md-composer" data-md-composer {
            nav class="twig-editor-modes" aria-label=(format!("Editing mode for {label}")) hidden data-editor-modes {
                button type="button" class="twig-btn twig-btn--quiet" data-editor-mode="rich" aria-pressed="false" { "Rich text" }
                button type="button" class="twig-btn twig-btn--quiet" data-editor-mode="source" aria-pressed="true" { "Markdown source" }
            }
            div class="twig-editor-toolbar ql-toolbar ql-snow" data-editor-toolbar hidden aria-label="Formatting" {
                select class="ql-header" aria-label="Heading" {
                    option selected value="" { "Paragraph" }
                    option value="2" { "Heading 2" }
                    option value="3" { "Heading 3" }
                }
                button class="ql-bold" type="button" aria-label="Bold" {}
                button class="ql-italic" type="button" aria-label="Italic" {}
                button class="ql-strike" type="button" aria-label="Strikethrough" {}
                button class="ql-blockquote" type="button" aria-label="Block quote" {}
                button class="ql-code-block" type="button" aria-label="Code block" {}
                button class="ql-list" type="button" value="ordered" aria-label="Numbered list" {}
                button class="ql-list" type="button" value="bullet" aria-label="Bulleted list" {}
                button class="ql-link" type="button" aria-label="Link" {}
                button class="ql-clean" type="button" aria-label="Clear formatting" {}
            }
            div class="twig-editor-rich" data-editor-rich hidden aria-label=(format!("Rich text editor for {label}")) {}
            textarea
                id=(id)
                class="twig-input twig-input--mono twig-md-source"
                name=(name)
                rows=(rows)
                aria-label=(label)
                placeholder=[placeholder]
                data-editor-source
            { (value) }
        }
    }
}

/// The stylesheet and scripts the composers need. Put them on a page that has composers.
pub(super) fn scripts() -> Markup {
    html! {
        link rel="stylesheet" href=(crate::http::assets::url("quill.snow.css"));
        script src=(crate::http::assets::url("marked.js")) {}
        script src=(crate::http::assets::url("purify.js")) {}
        script src=(crate::http::assets::url("turndown.js")) {}
        script src=(crate::http::assets::url("quill.js")) {}
        script src=(crate::http::assets::url("issue-composer.js")) {}
    }
}
