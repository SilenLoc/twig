use super::{
    MAX_EDIT_BYTES, PendingFileChange, PendingFileState, WorkspaceContext, WorkspaceDraft,
};
use crate::git::bare::is_safe_repo_path;

pub(super) fn validate_new_path(context: &WorkspaceContext, path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.len() > 4096
        || !is_safe_repo_path(path)
        || path.split('/').any(|component| component.len() > 255)
        || context.config.should_ignore(path)
    {
        return Err("Choose a safe path that is not ignored by this repository.".to_string());
    }
    if context
        .handle
        .file_mode(path)
        .map_err(|_| "The destination path could not be checked.".to_string())?
        .is_some()
    {
        return Err("A file or directory already exists at that path.".to_string());
    }
    let mut parent = path;
    while let Some((ancestor, _)) = parent.rsplit_once('/') {
        if let Some(mode) = context
            .handle
            .file_mode(ancestor)
            .map_err(|_| "The destination path could not be checked.".to_string())?
            && mode != 0o040_000
        {
            return Err("A file already blocks part of the destination path.".to_string());
        }
        parent = ancestor;
    }
    Ok(())
}

pub(super) fn validate_existing_text_path(
    context: &WorkspaceContext,
    path: &str,
) -> Result<(), String> {
    if path.is_empty() || !is_safe_repo_path(path) || context.config.should_ignore(path) {
        return Err("This file cannot be changed in the workspace.".to_string());
    }
    let mode = context
        .handle
        .file_mode(path)
        .map_err(|_| "The file could not be checked.".to_string())?
        .ok_or_else(|| "The file does not exist in the repository.".to_string())?;
    if !matches!(mode, 0o100_644 | 0o100_755) {
        return Err("Only regular text files can be changed in this workspace.".to_string());
    }
    Ok(())
}

pub(super) fn read_text_file(context: &WorkspaceContext, path: &str) -> Result<String, String> {
    validate_existing_text_path(context, path)?;
    let bytes = context
        .handle
        .read_blob_bytes(path)
        .map_err(|_| "The file could not be read.".to_string())?
        .ok_or_else(|| "The file does not exist in the repository.".to_string())?;
    if bytes.len() > MAX_EDIT_BYTES {
        return Err("Files larger than 1 MiB cannot be changed in this workspace.".to_string());
    }
    if bytes.contains(&0) {
        return Err("Binary files cannot be changed in this workspace.".to_string());
    }
    String::from_utf8(bytes).map_err(|_| "Only UTF-8 text files can be changed here.".to_string())
}

pub(super) fn validate_draft(
    context: &WorkspaceContext,
    draft: &WorkspaceDraft,
) -> Result<(), String> {
    if !is_safe_repo_path(&draft.tree_path)
        || draft.active_path.as_deref().is_some_and(|path| {
            path.is_empty() || !is_safe_repo_path(path) || context.config.should_ignore(path)
        })
    {
        return Err("The draft contains an unsafe workspace path.".to_string());
    }

    validate_draft_changes(context, draft)?;
    validate_file_parent_conflicts(draft)
}

pub(super) fn validate_draft_changes(
    context: &WorkspaceContext,
    draft: &WorkspaceDraft,
) -> Result<(), String> {
    let mut paths = std::collections::HashSet::new();
    for change in &draft.changes {
        if change.path.is_empty()
            || !is_safe_repo_path(&change.path)
            || context.config.should_ignore(&change.path)
            || !paths.insert(change.path.as_str())
        {
            return Err("The draft contains an unsafe, ignored, or duplicate path.".to_string());
        }
        validate_draft_change(context, change)?;
    }
    Ok(())
}

fn validate_draft_change(
    context: &WorkspaceContext,
    change: &PendingFileChange,
) -> Result<(), String> {
    match &change.state {
        PendingFileState::Content(draft_text) => {
            if draft_text.len() > MAX_EDIT_BYTES {
                return Err("Files larger than 1 MiB cannot be changed here.".to_string());
            }
            if draft_text.as_bytes().contains(&0) {
                return Err("Binary content cannot be stored in this workspace.".to_string());
            }
            let mode = context
                .handle
                .file_mode(&change.path)
                .map_err(|_| "A draft path could not be checked.".to_string())?;
            match mode {
                Some(0o100_644 | 0o100_755) => {
                    read_text_file(context, &change.path)?;
                }
                Some(_) => {
                    return Err("Only regular text files can be changed here.".to_string());
                }
                None => validate_new_path(context, &change.path)?,
            }
        }
        PendingFileState::Deleted => {
            read_text_file(context, &change.path)?;
        }
    }
    Ok(())
}

fn validate_file_parent_conflicts(draft: &WorkspaceDraft) -> Result<(), String> {
    for left in &draft.changes {
        if matches!(&left.state, PendingFileState::Content(_)) {
            let prefix = format!("{}/", left.path);
            if draft
                .changes
                .iter()
                .any(|right| right.path.starts_with(&prefix))
            {
                return Err("A draft file path cannot also be a parent directory.".to_string());
            }
        }
    }
    Ok(())
}

pub(super) fn parent_path(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}
