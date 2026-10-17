use super::validation::validate_draft;
use super::{
    HttpResponse, PendingFileState, RepoParams, WorkspaceContext, WorkspaceDraft, ZERO_OID,
    html_response, pages,
};
use crate::git::bare::{CommitChangesOutcome, RepoEditChange};

pub(super) struct PreparedWorkspaceCommit {
    draft: WorkspaceDraft,
    pub(super) changes: Vec<RepoEditChange>,
    pub(super) expected_head: git2::Oid,
    pub(super) email: String,
    pub(super) message: String,
}

pub(super) struct WorkspaceCommitPreparationError {
    pub(super) draft: Option<Box<WorkspaceDraft>>,
    pub(super) message: String,
    pub(super) reload: bool,
}

fn workspace_commit_preparation_error(
    draft: WorkspaceDraft,
    message: impl Into<String>,
    reload: bool,
) -> WorkspaceCommitPreparationError {
    WorkspaceCommitPreparationError {
        draft: Some(Box::new(draft)),
        message: message.into(),
        reload,
    }
}

pub(super) fn prepare_workspace_commit(
    context: &WorkspaceContext,
    draft: WorkspaceDraft,
    message: &str,
) -> Result<PreparedWorkspaceCommit, WorkspaceCommitPreparationError> {
    let current_head = context
        .head
        .map_or_else(|| ZERO_OID.to_string(), |head| head.to_string());
    if draft.base_head != current_head {
        return Err(workspace_commit_preparation_error(
            draft,
            "The repository changed since this workspace was opened. Your draft is preserved; reload and reconcile before committing.",
            true,
        ));
    }
    if let Err(message) = validate_draft(context, &draft) {
        return Err(workspace_commit_preparation_error(draft, message, false));
    }
    if draft.changes.is_empty() {
        return Err(workspace_commit_preparation_error(
            draft,
            "There are no changes to commit.",
            false,
        ));
    }

    let message = message.trim();
    if message.is_empty() {
        return Err(workspace_commit_preparation_error(
            draft,
            "Enter a commit message.",
            false,
        ));
    }
    if message.chars().count() > 200 {
        return Err(workspace_commit_preparation_error(
            draft,
            "Commit messages must be 200 characters or fewer.",
            false,
        ));
    }
    let Some(email) = context.user.email.as_deref() else {
        return Err(workspace_commit_preparation_error(
            draft,
            "Set an email address in account settings before committing.",
            false,
        ));
    };
    let Ok(expected_head) = git2::Oid::from_str(&draft.base_head) else {
        return Err(workspace_commit_preparation_error(
            draft,
            "The workspace base revision is invalid. Your draft is preserved.",
            true,
        ));
    };
    let changes = draft
        .changes
        .iter()
        .map(|change| match &change.state {
            PendingFileState::Content(text) => RepoEditChange::Upsert {
                path: change.path.clone(),
                content: text.as_bytes().to_vec(),
            },
            PendingFileState::Deleted => RepoEditChange::Delete {
                path: change.path.clone(),
            },
        })
        .collect();

    Ok(PreparedWorkspaceCommit {
        draft,
        changes,
        expected_head,
        email: email.to_string(),
        message: message.to_string(),
    })
}

pub(super) fn render_commit_result(
    draft: Option<&WorkspaceDraft>,
    message: &str,
    succeeded: bool,
    reload: bool,
) -> HttpResponse {
    let serialized = draft.map(|draft| serde_json::to_string(draft).unwrap_or_default());
    html_response(pages::commit_result(
        serialized.as_deref(),
        message,
        succeeded,
        reload,
    ))
}

pub(super) fn render_workspace_commit_result(
    params: &RepoParams,
    context: &WorkspaceContext,
    mut prepared: PreparedWorkspaceCommit,
    result: Result<CommitChangesOutcome, git2::Error>,
) -> HttpResponse {
    match result {
        Ok(CommitChangesOutcome::Committed(oid)) => {
            log::info!(
                "Committed {} repository edit changes in '{}/{}' as {} by {}",
                prepared.changes.len(),
                params.namespace,
                params.repo,
                oid,
                context.user.username
            );
            prepared.draft.base_head = oid.to_string();
            prepared.draft.changes.clear();
            render_commit_result(
                Some(&prepared.draft),
                &format!("Changes committed successfully ({oid}). Continue editing here."),
                true,
                false,
            )
        }
        Ok(CommitChangesOutcome::Conflict) => render_commit_result(
            Some(&prepared.draft),
            "The repository changed while committing. Your draft is preserved; reload and reconcile before trying again.",
            false,
            true,
        ),
        Err(error) if error.message().contains("There are no changes to commit") => {
            render_commit_result(
                Some(&prepared.draft),
                "There are no changes to commit.",
                false,
                false,
            )
        }
        Err(error) => {
            log::error!(
                "Failed to commit repository workspace for {}/{}: {error}",
                params.namespace,
                params.repo
            );
            render_commit_result(
                Some(&prepared.draft),
                "The change set could not be committed. Your draft is preserved.",
                false,
                false,
            )
        }
    }
}
