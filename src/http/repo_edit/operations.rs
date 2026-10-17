use super::validation::{
    parent_path, read_text_file, validate_existing_text_path, validate_new_path,
};
use super::{
    PendingFileChange, PendingFileState, WorkspaceContext, WorkspaceDraft, WorkspaceOperationInput,
};
use crate::git::bare::is_safe_repo_path;

pub(super) fn apply_file_operation(
    context: &WorkspaceContext,
    draft: &mut WorkspaceDraft,
    input: &WorkspaceOperationInput,
) -> Result<String, String> {
    match input.kind.as_str() {
        "restore" => Ok("Workspace draft restored.".to_string()),
        "create" => {
            create_workspace_file(context, draft, input.path.as_deref().unwrap_or_default())
        }
        "rename" => rename_workspace_file(
            context,
            draft,
            input.from.as_deref().unwrap_or_default(),
            input.to.as_deref().unwrap_or_default(),
        ),
        "delete" => {
            delete_workspace_file(context, draft, input.path.as_deref().unwrap_or_default())
        }
        "discard" => {
            discard_workspace_file(context, draft, input.path.as_deref().unwrap_or_default())
        }
        _ => Err("Unknown workspace operation.".to_string()),
    }
}

fn create_workspace_file(
    context: &WorkspaceContext,
    draft: &mut WorkspaceDraft,
    path: &str,
) -> Result<String, String> {
    validate_new_path(context, path)?;
    if draft.changes.iter().any(|change| change.path == path) {
        return Err("A draft change already exists at that path.".to_string());
    }
    draft.changes.push(PendingFileChange {
        path: path.to_string(),
        state: PendingFileState::Content(String::new()),
    });
    draft.active_path = Some(path.to_string());
    draft.tree_path = parent_path(path).to_string();
    Ok(format!("Created {path} in the workspace draft."))
}

fn rename_workspace_file(
    context: &WorkspaceContext,
    draft: &mut WorkspaceDraft,
    from: &str,
    to: &str,
) -> Result<String, String> {
    if from == to {
        if from.is_empty() || !is_safe_repo_path(from) || context.config.should_ignore(from) {
            return Err("The source file path is not available for editing.".to_string());
        }
        draft.active_path = Some(from.to_string());
        return Ok("The file name is unchanged.".to_string());
    }
    if from.is_empty() || !is_safe_repo_path(from) || context.config.should_ignore(from) {
        return Err("The source file path is not available for editing.".to_string());
    }
    validate_new_path(context, to)?;
    if draft.changes.iter().any(|change| change.path == to) {
        return Err("A draft change already exists at the new path.".to_string());
    }
    let source_index = draft.changes.iter().position(|change| change.path == from);
    let source_exists = context
        .handle
        .file_mode(from)
        .map_err(|_| "The source file could not be checked.".to_string())?
        .is_some();
    if source_exists {
        validate_existing_text_path(context, from)?;
    } else if source_index.is_none() {
        return Err("The source file does not exist in the workspace.".to_string());
    }
    let file_text = match source_index.map(|index| &draft.changes[index].state) {
        Some(PendingFileState::Content(file_text)) => file_text.clone(),
        Some(PendingFileState::Deleted) => {
            return Err("A deleted file cannot be renamed.".to_string());
        }
        None => read_text_file(context, from)?,
    };
    if let Some(index) = source_index {
        draft.changes.remove(index);
    }
    if source_exists {
        draft.changes.push(PendingFileChange {
            path: from.to_string(),
            state: PendingFileState::Deleted,
        });
    }
    draft.changes.push(PendingFileChange {
        path: to.to_string(),
        state: PendingFileState::Content(file_text),
    });
    draft.active_path = Some(to.to_string());
    draft.tree_path = parent_path(to).to_string();
    Ok(format!("Renamed {from} to {to} in the workspace draft."))
}

fn delete_workspace_file(
    context: &WorkspaceContext,
    draft: &mut WorkspaceDraft,
    path: &str,
) -> Result<String, String> {
    let source_index = draft.changes.iter().position(|change| change.path == path);
    let source_exists = context
        .handle
        .file_mode(path)
        .map_err(|_| "The file could not be checked.".to_string())?
        .is_some();
    if !source_exists && source_index.is_none() {
        return Err("The file does not exist in this workspace.".to_string());
    }
    if source_exists {
        validate_existing_text_path(context, path)?;
        read_text_file(context, path)?;
        if let Some(index) = source_index {
            if matches!(&draft.changes[index].state, PendingFileState::Deleted) {
                return Err("The file is already marked for deletion.".to_string());
            }
            draft.changes.remove(index);
        }
        draft.changes.push(PendingFileChange {
            path: path.to_string(),
            state: PendingFileState::Deleted,
        });
    } else if let Some(index) = source_index {
        draft.changes.remove(index);
    }
    if draft.active_path.as_deref() == Some(path) {
        draft.active_path = None;
    }
    draft.tree_path = parent_path(path).to_string();
    Ok(format!(
        "Marked {path} for removal from the workspace draft."
    ))
}

fn discard_workspace_file(
    context: &WorkspaceContext,
    draft: &mut WorkspaceDraft,
    path: &str,
) -> Result<String, String> {
    let Some(index) = draft.changes.iter().position(|change| change.path == path) else {
        return Err("There is no pending change for that file.".to_string());
    };
    draft.changes.remove(index);
    let base_exists = context
        .handle
        .file_mode(path)
        .map_err(|_| "The file could not be checked.".to_string())?
        .is_some();
    draft.active_path = base_exists.then(|| path.to_string());
    draft.tree_path = parent_path(path).to_string();
    Ok(format!("Discarded the pending change to {path}."))
}
