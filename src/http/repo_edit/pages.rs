use maud::Markup;

use crate::git::bare::TreeEntry;

use super::WorkspaceDraft;

pub(super) fn workspace_shell(
    namespace: &str,
    repo: &str,
    selected_path: Option<&str>,
    draft: &WorkspaceDraft,
    has_email: bool,
) -> Markup {
    let tree_url = format!("/{namespace}/{repo}/edit/tree/");
    let document_url = selected_path.map(|path| workspace_document_url(namespace, repo, path));
    let operation_url = format!("/{namespace}/{repo}/edit/operation");
    let commit_url = format!("/{namespace}/{repo}/edit/commit");
    let draft_json = serde_json::to_string(draft).unwrap_or_default();

    maud::html! {
        div
            id="repo-edit-workspace"
            class="twig-repo-edit twig-workspace"
            data-namespace=(namespace)
            data-repo=(repo)
            data-base-head=(draft.base_head)
            data-initial-path=[selected_path]
            data-has-email=(if has_email { "true" } else { "false" })
            data-draft-key=(format!("twig:repo-edit:{namespace}/{repo}"))
        {
            input type="hidden" id="repo-edit-draft-state" name="draft" value=(draft_json);
            div class="twig-workspace-layout" {
                aside class="twig-workspace-nav" aria-label="Repository navigation" {
                    (workspace_navigation_header(namespace, repo))
                    (workspace_files_panel(&tree_url, &operation_url))
                    (workspace_changes_panel(&commit_url, draft, has_email, &draft_json))
                    div id="repo-edit-operation-result" role="status" aria-live="polite" {}
                }
                div class="twig-workspace-canvas" {
                    (workspace_document_panel(document_url.as_deref()))
                }
            }
            (workspace_restore_form(&operation_url, &draft_json))
            (editor_assets())
        }
    }
}

fn workspace_document_url(namespace: &str, repo: &str, path: &str) -> String {
    format!(
        "/{namespace}/{repo}/edit/file/{}",
        super::encode_file_path(path)
    )
}

fn workspace_navigation_header(namespace: &str, repo: &str) -> Markup {
    maud::html! {
        header class="twig-workspace-nav-head" {
            a class="twig-workspace-back" href=(format!("/{namespace}/{repo}")) {
                "← Repository overview"
            }
            h1 class="twig-workspace-repo" { (namespace) " / " (repo) }
            p class="twig-workspace-caption" { "Repository workspace" }
        }
    }
}

fn workspace_files_panel(tree_url: &str, operation_url: &str) -> Markup {
    maud::html! {
        section class="twig-workspace-files twig-repo-edit-files" aria-label="Repository files" {
            div class="twig-panel-head" {
                h2 class="twig-section" { "Files" }
                details class="twig-repo-new-file" {
                    summary
                        class="twig-btn twig-btn--quiet"
                        aria-label="New file"
                    {
                        span class="twig-repo-new-file-label" { "New file" }
                        span class="twig-repo-new-file-symbol" aria-hidden="true" { "+" }
                    }
                    form
                        class="twig-stack twig-repo-new-file-form"
                        hx-post=(operation_url)
                        hx-action=(operation_url)
                        hx-method="POST"
                        hx-trigger="submit"
                        hx-target="#repo-edit-operation-result"
                        hx-swap="innerHTML"
                        data-repo-edit-operation
                    {
                        input type="hidden" name="kind" value="create";
                        label class="twig-label" for="repo-edit-new-path" { "Repository path" }
                        input class="twig-input" id="repo-edit-new-path" name="path" required placeholder="docs/notes.md";
                        input type="hidden" name="draft" value="";
                        button class="twig-btn twig-btn--primary" type="submit" { "Create file" }
                    }
                }
            }
            div
                id="repo-edit-tree"
                hx-get=(tree_url)
                hx-trigger="load"
                hx-target="#repo-edit-tree"
                hx-swap="innerHTML"
                aria-live="polite"
            {
                div data-repo-edit-tree-path="" {
                    p class="twig-hint" { "Loading repository files…" }
                }
            }
        }
    }
}

fn workspace_document_panel(document_url: Option<&str>) -> Markup {
    maud::html! {
        section class="twig-repo-edit-document" aria-label="Editor" {
            @if let Some(document_url) = document_url {
                div
                    id="repo-edit-document"
                    hx-get=(document_url)
                    hx-trigger="load"
                    hx-target="#repo-edit-document"
                    hx-swap="innerHTML"
                {
                    p class="twig-hint" { "Loading file…" }
                }
            } @else {
                div id="repo-edit-document" {
                    (file_placeholder())
                }
            }
        }
    }
}

fn workspace_changes_panel(
    commit_url: &str,
    draft: &WorkspaceDraft,
    has_email: bool,
    draft_json: &str,
) -> Markup {
    maud::html! {
        details class="twig-workspace-changes twig-panel twig-repo-edit-changes" id="repo-edit-changes-panel" {
            summary class="twig-repo-changes-summary" {
                h2 class="twig-section" { "Changes" }
                span class="twig-row-meta" id="repo-edit-change-count" { "0" }
            }
            div class="twig-panel-body" {
                p class="twig-hint" id="repo-edit-change-empty" { "No pending changes." }
                ul class="twig-repo-change-list" id="repo-edit-change-list" {}
                form
                    id="repo-edit-commit-form"
                    class="twig-stack twig-repo-commit-form"
                    hx-post=(commit_url)
                    hx-action=(commit_url)
                    hx-method="POST"
                    hx-trigger="submit"
                    hx-target="#repo-edit-commit-result"
                    hx-swap="innerHTML"
                    data-repo-edit-commit
                {
                    label class="twig-label" for="repo-edit-commit-message" { "Commit message" }
                    input
                        class="twig-input"
                        id="repo-edit-commit-message"
                        name="message"
                        maxlength="200"
                        required
                        value="Update repository";
                    @if !has_email {
                        p class="twig-editor-status" role="alert" {
                            "Set an email address in " a href="/settings" { "account settings" }
                            " before committing."
                        }
                    }
                    input type="hidden" name="draft" value=(draft_json);
                    button
                        class="twig-btn twig-btn--primary"
                        type="submit"
                        disabled[draft.changes.is_empty() || !has_email]
                    { "Commit changes" }
                    div id="repo-edit-commit-result" role="status" aria-live="polite" {}
                }
            }
        }
    }
}

fn workspace_restore_form(operation_url: &str, draft_json: &str) -> Markup {
    maud::html! {
        form
            id="repo-edit-restore-form"
            hidden
            hx-post=(operation_url)
            hx-action=(operation_url)
            hx-method="POST"
            hx-trigger="repo-edit-restore"
            hx-target="#repo-edit-operation-result"
            hx-swap="innerHTML"
            data-repo-edit-operation
        {
            input type="hidden" name="kind" value="restore";
            input type="hidden" name="draft" value=(draft_json);
        }
    }
}

pub(super) fn file_tree(namespace: &str, repo: &str, path: &str, entries: &[TreeEntry]) -> Markup {
    let parent_path = path.rsplit_once('/').map_or("", |(parent, _)| parent);
    maud::html! {
        div data-repo-edit-tree-path=(path) {
            nav aria-label="Repository file tree" {
                ul class="twig-repo-tree" {
                    @if !path.is_empty() {
                        li {
                            a
                                class="twig-repo-tree-link twig-repo-tree-parent"
                                href=(format!("/{namespace}/{repo}/edit/tree/{}", super::encode_file_path(parent_path)))
                                hx-get=(format!("/{namespace}/{repo}/edit/tree/{}", super::encode_file_path(parent_path)))
                                hx-trigger="click"
                                hx-target="#repo-edit-tree"
                                hx-swap="innerHTML"
                                data-path=(parent_path)
                                data-directory="true"
                            { "← .." }
                        }
                    }
                    @for entry in entries {
                        @if entry.is_dir {
                            @let url = format!("/{namespace}/{repo}/edit/tree/{}", super::encode_file_path(&entry.path));
                            li data-repo-edit-tree-entry data-path=(entry.path) data-directory="true" {
                                a
                                    class="twig-repo-tree-link"
                                    href=(url)
                                    hx-get=(url)
                                    hx-trigger="click"
                                    hx-target="#repo-edit-tree"
                                    hx-swap="innerHTML"
                                    data-path=(entry.path)
                                    data-directory="true"
                                    data-base-exists="true"
                                {
                                    span class="twig-repo-tree-icon" aria-hidden="true" { "▸" }
                                    (entry.name)
                                }
                            }
                        } @else {
                            @let hx_url = format!("/{namespace}/{repo}/edit/file/{}", super::encode_file_path(&entry.path));
                            @let page_url = super::workspace_file_url(namespace, repo, &entry.path);
                            li data-repo-edit-tree-entry data-path=(entry.path) data-directory="false" {
                                a
                                    class="twig-repo-tree-link"
                                    href=(page_url)
                                    hx-get=(hx_url)
                                    hx-trigger="click"
                                    hx-target="#repo-edit-document"
                                    hx-swap="innerHTML"
                                    hx-push-url=(page_url)
                                    data-path=(entry.path)
                                    data-directory="false"
                                    data-base-exists="true"
                                {
                                    span class="twig-repo-tree-icon" aria-hidden="true" { "¶" }
                                    (entry.name)
                                }
                            }
                        }
                    }
                    @if entries.is_empty() {
                        li class="twig-hint twig-repo-tree-empty" { "This directory is empty." }
                    }
                }
            }
        }
    }
}

pub(super) fn file_editor(
    namespace: &str,
    repo: &str,
    path: &str,
    content: &str,
    markdown: bool,
    base_exists: bool,
) -> Markup {
    let operation_url = format!("/{namespace}/{repo}/edit/operation");
    maud::html! {
        div
            class="twig-repo-file-editor twig-editor"
            data-markdown=[markdown.then_some("true")]
            data-path=(path)
            data-base-exists=(if base_exists { "true" } else { "false" })
        {
            header class="twig-panel-head twig-repo-file-head" {
                div class="twig-stack twig-stack--tight" {
                    p class="twig-eyebrow" { "FILE" }
                    code class="twig-mono" { (path) }
                }
                div class="twig-repo-file-actions" {
                    details {
                        summary class="twig-btn twig-btn--quiet" { "Rename" }
                        form
                            class="twig-stack twig-repo-file-action-form"
                            hx-post=(operation_url)
                            hx-action=(operation_url)
                            hx-method="POST"
                            hx-trigger="submit"
                            hx-target="#repo-edit-operation-result"
                            hx-swap="innerHTML"
                            data-repo-edit-operation
                        {
                            input type="hidden" name="kind" value="rename";
                            input type="hidden" name="from" value=(path);
                            label class="twig-label" for="repo-edit-rename-path" { "New path" }
                            input class="twig-input" id="repo-edit-rename-path" name="to" required value=(path);
                            input type="hidden" name="draft" value="";
                            button class="twig-btn twig-btn--primary" type="submit" { "Rename file" }
                        }
                    }
                    form
                        hx-post=(operation_url)
                        hx-action=(operation_url)
                        hx-method="POST"
                        hx-trigger="submit"
                        hx-target="#repo-edit-operation-result"
                        hx-swap="innerHTML"
                        hx-confirm=(format!("Delete {path} in this pending workspace?"))
                        data-repo-edit-operation
                    {
                        input type="hidden" name="kind" value="delete";
                        input type="hidden" name="path" value=(path);
                        input type="hidden" name="draft" value="";
                        button class="twig-btn twig-btn--quiet" type="submit" { "Delete" }
                    }
                    @if markdown {
                        nav class="twig-editor-modes" aria-label="Editing mode" {
                            button type="button" class="twig-btn twig-btn--quiet" data-editor-mode="rich" aria-pressed="true" { "Rich text" }
                            button type="button" class="twig-btn twig-btn--quiet" data-editor-mode="source" aria-pressed="false" { "Markdown source" }
                        }
                    } @else {
                        span class="twig-row-meta" { "Source text" }
                    }
                }
            }
            @if markdown {
                div class="twig-editor-toolbar ql-toolbar ql-snow" data-editor-toolbar aria-label="Formatting" {
                    span class="ql-formats" {
                        button class="ql-bold" type="button" aria-label="Bold" {}
                        button class="ql-italic" type="button" aria-label="Italic" {}
                        button class="ql-strike" type="button" aria-label="Strikethrough" {}
                    }
                    span class="ql-formats" {
                        select class="ql-header" aria-label="Heading" {
                            option selected value="" { "Paragraph" }
                            option value="2" { "Heading 2" }
                            option value="3" { "Heading 3" }
                        }
                        button class="ql-blockquote" type="button" aria-label="Block quote" {}
                        button class="ql-code-block" type="button" aria-label="Code block" {}
                    }
                    span class="ql-formats" {
                        button class="ql-list" type="button" value="ordered" aria-label="Numbered list" {}
                        button class="ql-list" type="button" value="bullet" aria-label="Bulleted list" {}
                        button class="ql-link" type="button" aria-label="Link" {}
                        button class="ql-clean" type="button" aria-label="Clear formatting" {}
                    }
                }
                div class="twig-editor-rich" data-editor-rich aria-label=(format!("Rich text editor for {path}")) {}
                textarea class="twig-editor-source" data-editor-source aria-label=(format!("Markdown source for {path}")) hidden { (content) }
            } @else {
                textarea class="twig-editor-source" data-editor-source aria-label=(format!("Source text for {path}")) { (content) }
            }
            footer class="twig-repo-file-foot" {
                span class="twig-row-meta" { @if markdown { "Markdown" } @else { "UTF-8 text" } }
                span class="twig-row-meta" data-repo-edit-save-state { "Draft saved in this tab" }
            }
        }
    }
}

pub(super) fn file_unavailable(path: &str, message: &str) -> Markup {
    maud::html! {
        div class="twig-repo-file-unavailable" data-file-unavailable data-file-path=(path) role="status" {
            p class="twig-eyebrow" { "FILE UNAVAILABLE" }
            code class="twig-mono" { (path) }
            p class="twig-hint" { (message) }
        }
    }
}

pub(super) fn operation_result(draft: Option<&str>, message: &str, success: bool) -> Markup {
    maud::html! {
        div
            class="twig-repo-operation-result"
            data-operation-success=(if success { "true" } else { "false" })
            data-draft=[draft]
        {
            p class=(if success { "twig-hint" } else { "twig-editor-status" }) { (message) }
        }
    }
}

pub(super) fn commit_result(
    draft: Option<&str>,
    message: &str,
    success: bool,
    conflict: bool,
) -> Markup {
    maud::html! {
        div
            class="twig-repo-commit-result"
            data-commit-success=(if success { "true" } else { "false" })
            data-commit-conflict=(if conflict { "true" } else { "false" })
            data-draft=[draft]
        {
            p class=(if success { "twig-hint" } else { "twig-editor-status" }) { (message) }
        }
    }
}

fn file_placeholder() -> Markup {
    maud::html! {
        div class="twig-repo-edit-placeholder" {
            p class="twig-eyebrow" { "EDITOR" }
            p class="twig-hint" { "Select a file to edit." }
        }
    }
}

fn editor_assets() -> Markup {
    maud::html! {
        link rel="stylesheet" href=(crate::http::assets::url("quill.snow.css"));
        script src=(crate::http::assets::url("marked.js")) {}
        script src=(crate::http::assets::url("purify.js")) {}
        script src=(crate::http::assets::url("turndown.js")) {}
        script src=(crate::http::assets::url("quill.js")) {}
        script src=(crate::http::assets::url("repo-edit.js")) {}
    }
}
