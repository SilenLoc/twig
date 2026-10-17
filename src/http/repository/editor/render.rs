use super::encode_file_path;
use maud::Markup;

pub(super) fn render_editor_page(
    namespace: &str,
    repo: &str,
    path: &str,
    content: &str,
    head: twig_git::Oid,
    markdown: bool,
    has_email: bool,
) -> Markup {
    let encoded_path = encode_file_path(path);
    let save_url = format!("/{namespace}/{repo}/edit/{encoded_path}");
    let content_url = format!("/{namespace}/{repo}/content/{encoded_path}");
    maud::html! {
        div class="twig-stack twig-editor" {
            nav class="twig-crumbs twig-crumbs--page" aria-label="Breadcrumb" {
                a href="/" { "Namespaces" }
                span class="twig-crumb-sep" aria-hidden="true" { "/" }
                a href=(format!("/{namespace}")) { (namespace) }
                span class="twig-crumb-sep" aria-hidden="true" { "/" }
                a href=(format!("/{namespace}/{repo}")) { (repo) }
                span class="twig-crumb-sep" aria-hidden="true" { "/" }
                span aria-current="page" { (path) }
            }
            header class="twig-stack twig-stack--tight" {
                h1 class="twig-section" { "Edit file" }
                p class="twig-hint" { "Changes are saved as a new commit in this repository." }
            }
            @if !has_email {
                div class="twig-notice twig-notice--warning" role="status" {
                    p class="twig-notice-body" {
                        "Set an email address in " a href="/settings" { "account settings" }
                        " before committing."
                    }
                }
            }
            @if markdown {
                link rel="stylesheet" href=(crate::http::assets::url("quill.snow.css"));
            }
            form
                id="repository-editor-form"
                class="twig-panel twig-editor-panel"
                data-save-url=(save_url)
                data-markdown=[markdown.then_some("true")]
            {
                div class="twig-panel-head twig-editor-head" {
                    code class="twig-mono" { (path) }
                    @if markdown {
                        nav class="twig-editor-modes" aria-label="Editing mode" {
                            button type="button" class="twig-btn twig-btn--quiet" data-editor-mode="rich" aria-pressed="false" { "Rich text" }
                            button type="button" class="twig-btn twig-btn--quiet" data-editor-mode="source" aria-pressed="true" { "Markdown source" }
                        }
                    } @else {
                        span class="twig-row-meta" { "Source editor" }
                    }
                }
                @if markdown {
                    div class="twig-editor-toolbar" data-editor-toolbar hidden aria-label="Formatting" {
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
                    div class="twig-editor-rich" data-editor-rich hidden aria-label=(format!("Rich text editor for {path}")) {}
                }
                textarea
                    id="repository-editor-content"
                    class="twig-editor-source"
                    aria-label=(format!("{} source", path))
                    spellcheck=[(!markdown).then_some("false")]
                    rows="20"
                { (content) }
                input type="hidden" name="expected_head" value=(head);
                div class="twig-editor-commit" {
                    label class="twig-label" for="repository-editor-message" { "Commit message" }
                    input
                        id="repository-editor-message"
                        class="twig-input"
                        type="text"
                        maxlength="200"
                        required
                        value=(format!("Update {path}"));
                    p class="twig-editor-status" role="status" aria-live="polite" data-editor-status {}
                    (render_conflict_actions(&save_url))
                    div class="twig-form-actions" {
                        a class="twig-btn twig-btn--quiet" href=(content_url) { "Cancel" }
                        button class="twig-btn twig-btn--primary" type="submit" disabled[!has_email] { "Commit changes" }
                    }
                }
            }
            (render_editor_scripts(markdown))
        }
    }
}

pub(super) fn render_conflict_actions(save_url: &str) -> Markup {
    maud::html! {
        a class="twig-btn twig-btn--quiet" href=(save_url) data-editor-reload hidden {
            "Reload latest revision (discard this draft)"
        }
        button
            class="twig-btn twig-btn--primary"
            type="button"
            data-editor-conflict-copy
            hidden
        { "Save draft as a conflict copy" }
    }
}

pub(super) fn render_editor_scripts(markdown: bool) -> Markup {
    maud::html! {
        @if markdown {
            script src=(crate::http::assets::url("marked.js")) {}
            script src=(crate::http::assets::url("purify.js")) {}
            script src=(crate::http::assets::url("turndown.js")) {}
            script src=(crate::http::assets::url("quill.js")) {}
        }
        script src=(crate::http::assets::url("repository-editor.js")) {}
    }
}
