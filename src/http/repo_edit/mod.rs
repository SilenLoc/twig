use actix_web::{HttpRequest, HttpResponse, get, web};
use maud::Markup;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{
        TwigContext, User,
        permissions::{self, NamespacePermission},
    },
    config,
    git::bare::{CommitChangesOutcome, RepoEditChange, RepoHandle, TwigConfig, is_safe_repo_path},
};

mod pages;

const MAX_EDIT_BYTES: usize = 1024 * 1024;
const ZERO_OID: &str = "0000000000000000000000000000000000000000";

#[derive(Deserialize)]
pub struct RepoParams {
    namespace: String,
    repo: String,
}

#[derive(Deserialize)]
pub struct TreeParams {
    namespace: String,
    repo: String,
    path: String,
}

#[derive(Deserialize)]
pub struct FileParams {
    namespace: String,
    repo: String,
    path: String,
}

#[derive(Deserialize, Default)]
pub struct WorkspaceQuery {
    path: Option<String>,
}

struct WorkspaceContext {
    handle: RepoHandle,
    user: User,
    config: TwigConfig,
    head: Option<git2::Oid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkspaceDraft {
    pub base_head: String,
    pub active_path: Option<String>,
    pub tree_path: String,
    pub changes: Vec<PendingFileChange>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PendingFileChange {
    pub path: String,
    pub state: PendingFileState,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", content = "text", rename_all = "snake_case")]
pub enum PendingFileState {
    Content(String),
    Deleted,
}

#[derive(Deserialize)]
pub struct WorkspaceOperationInput {
    draft: String,
    kind: String,
    path: Option<String>,
    from: Option<String>,
    to: Option<String>,
}

#[derive(Deserialize)]
pub struct CommitWorkspaceInput {
    draft: String,
    message: String,
}

async fn open_workspace_context(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
    repo: &str,
    permission: NamespacePermission,
) -> Result<WorkspaceContext, HttpResponse> {
    let Some(user_id) = auth_state.user_id_from_request(req).await else {
        return Err(HttpResponse::Found()
            .insert_header(("Location", "/auth/login"))
            .finish());
    };

    let user = match auth_state.db().get_user_by_id(&user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return Err(HttpResponse::Unauthorized().body("Session expired. Log in again.")),
        Err(error) => {
            log::error!("Failed to load repository editor user: {error}");
            return Err(HttpResponse::InternalServerError().finish());
        }
    };

    match permissions::has(
        auth_state.db(),
        &user_id,
        namespace,
        server.is_configured_admin(&user.username),
        permission,
    )
    .await
    {
        Ok(true) => {}
        Ok(false) => return Err(HttpResponse::Forbidden().body("No access to this namespace.")),
        Err(error) => {
            log::error!("Failed to verify repository editor access: {error}");
            return Err(HttpResponse::InternalServerError().finish());
        }
    }

    let handle = RepoHandle::open(server.project_root(), namespace, repo)
        .map_err(|_| HttpResponse::NotFound().finish())?;
    let config = handle.load_config_with_raw().config;
    if config.private && auth_state.user_id_from_request(req).await.is_none() {
        return Err(HttpResponse::Unauthorized().body("Log in to view this repository."));
    }
    let head = handle.head_oid().map_err(|error| {
        log::error!("Failed to resolve repository editor HEAD: {error}");
        HttpResponse::InternalServerError().finish()
    })?;

    Ok(WorkspaceContext {
        handle,
        user,
        config,
        head,
    })
}

fn html_response(markup: Markup) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(markup.into_string())
}

fn encode_file_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

pub(crate) fn workspace_file_url(namespace: &str, repo: &str, path: &str) -> String {
    format!("/{namespace}/{repo}/edit?path={}", encode_file_path(path))
}

fn editor_file_fragment(
    context: &WorkspaceContext,
    namespace: &str,
    repo: &str,
    path: &str,
) -> Markup {
    let unavailable = |message: &str| pages::file_unavailable(path, message);
    if path.is_empty() || !is_safe_repo_path(path) || context.config.should_ignore(path) {
        return unavailable("This file cannot be opened in the workspace.");
    }

    let mode = match context.handle.file_mode(path) {
        Ok(Some(mode)) => mode,
        Ok(None) => return unavailable("This file cannot be opened in the workspace."),
        Err(error) => {
            log::error!("Failed to inspect repository editor file '{path}': {error}");
            return unavailable("The file could not be read.");
        }
    };
    if !matches!(mode, 0o100_644 | 0o100_755) {
        return unavailable("Only regular text files can be opened in this workspace.");
    }

    let bytes = match context.handle.read_blob_bytes(path) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return unavailable("This file cannot be opened in the workspace."),
        Err(error) => {
            log::error!("Failed to read repository editor file '{path}': {error}");
            return unavailable("The file could not be read.");
        }
    };
    if bytes.len() > MAX_EDIT_BYTES {
        return unavailable("Files larger than 1 MiB cannot be opened in this workspace.");
    }
    if bytes.contains(&0) {
        return unavailable("Binary files cannot be opened in this workspace.");
    }
    let Ok(file_text) = String::from_utf8(bytes) else {
        return unavailable("Only UTF-8 text files can be opened in this workspace.");
    };

    pages::file_editor(
        namespace,
        repo,
        path,
        &file_text,
        crate::md::is_markdown(path),
        true,
    )
}

#[get("/{namespace}/{repo}/edit")]
pub async fn workspace_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<RepoParams>,
    query: web::Query<WorkspaceQuery>,
) -> HttpResponse {
    let params = params.into_inner();
    let context = match open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };
    let draft = WorkspaceDraft {
        base_head: context
            .head
            .map_or_else(|| ZERO_OID.to_string(), |head| head.to_string()),
        active_path: query.path.clone(),
        tree_path: String::new(),
        changes: Vec::new(),
    };
    let page = pages::workspace_shell(
        &params.namespace,
        &params.repo,
        query.path.as_deref(),
        &draft,
        context.user.email.is_some(),
    );
    if req.headers().contains_key("HX-Request") {
        return html_response(page);
    }

    html_response(crate::http::view::render_layout(
        &page,
        Some(&context.user.username),
        Some("Repository workspace"),
    ))
}

#[get("/{namespace}/{repo}/edit/tree/{path:.*}")]
pub async fn tree_fragment(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<TreeParams>,
) -> HttpResponse {
    let params = params.into_inner();
    if !is_safe_repo_path(&params.path) {
        return HttpResponse::NotFound().finish();
    }
    let context = match open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };
    let entries = match context.handle.list_dir(&params.path, Some(&context.config)) {
        Ok(entries) => entries,
        Err(error) => {
            log::error!(
                "Failed to list repository editor path '{}': {error}",
                params.path
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

    html_response(pages::file_tree(
        &params.namespace,
        &params.repo,
        &params.path,
        &entries,
    ))
}

pub async fn apply_operation(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<RepoParams>,
    input: web::Form<WorkspaceOperationInput>,
) -> HttpResponse {
    let params = params.into_inner();
    let context = match open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };
    let mut draft: WorkspaceDraft = match serde_json::from_str(&input.draft) {
        Ok(draft) => draft,
        Err(error) => {
            log::warn!("Invalid repository editor draft submitted: {error}");
            return html_response(pages::operation_result(
                None,
                "The workspace draft is invalid.",
                false,
            ));
        }
    };

    let current_head = context
        .head
        .map_or_else(|| ZERO_OID.to_string(), |head| head.to_string());
    if draft.base_head != current_head {
        let serialized = serde_json::to_string(&draft).unwrap_or_default();
        return html_response(pages::operation_result(
            Some(&serialized),
            "The repository changed since this workspace was opened. The draft remains in this tab.",
            false,
        ));
    }

    let message = match apply_file_operation(&context, &mut draft, &input) {
        Ok(message) => message,
        Err(message) => {
            let serialized = serde_json::to_string(&draft).unwrap_or_default();
            return html_response(pages::operation_result(Some(&serialized), &message, false));
        }
    };
    if let Err(message) = validate_draft(&context, &draft) {
        let serialized = serde_json::to_string(&draft).unwrap_or_default();
        return html_response(pages::operation_result(Some(&serialized), &message, false));
    }

    let serialized = serde_json::to_string(&draft).unwrap_or_default();
    html_response(pages::operation_result(Some(&serialized), &message, true))
}

pub async fn commit_workspace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<RepoParams>,
    input: web::Form<CommitWorkspaceInput>,
) -> HttpResponse {
    let params = params.into_inner();
    let context = match open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };
    let draft: WorkspaceDraft = match serde_json::from_str(&input.draft) {
        Ok(draft) => draft,
        Err(error) => {
            log::warn!("Invalid repository editor draft submitted for commit: {error}");
            return render_commit_result(None, "The workspace draft is invalid.", false, false);
        }
    };
    let prepared = match prepare_workspace_commit(&context, draft, &input.message) {
        Ok(prepared) => prepared,
        Err(error) => {
            return render_commit_result(
                error.draft.as_deref(),
                &error.message,
                false,
                error.reload,
            );
        }
    };
    let result = context.handle.commit_changes(
        &prepared.changes,
        prepared.expected_head,
        &context.user.username,
        &prepared.email,
        &prepared.message,
    );
    render_workspace_commit_result(&params, &context, prepared, result)
}

struct PreparedWorkspaceCommit {
    draft: WorkspaceDraft,
    changes: Vec<RepoEditChange>,
    expected_head: git2::Oid,
    email: String,
    message: String,
}

struct WorkspaceCommitPreparationError {
    draft: Option<Box<WorkspaceDraft>>,
    message: String,
    reload: bool,
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

fn prepare_workspace_commit(
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

fn render_commit_result(
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

fn render_workspace_commit_result(
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

fn apply_file_operation(
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

fn validate_new_path(context: &WorkspaceContext, path: &str) -> Result<(), String> {
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

fn validate_existing_text_path(context: &WorkspaceContext, path: &str) -> Result<(), String> {
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

fn read_text_file(context: &WorkspaceContext, path: &str) -> Result<String, String> {
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

fn validate_draft(context: &WorkspaceContext, draft: &WorkspaceDraft) -> Result<(), String> {
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

fn validate_draft_changes(
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

fn parent_path(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

#[get("/{namespace}/{repo}/edit/file/{path:.*}")]
pub async fn file_fragment(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<FileParams>,
) -> HttpResponse {
    let params = params.into_inner();
    let context = match open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };

    html_response(editor_file_fragment(
        &context,
        &params.namespace,
        &params.repo,
        &params.path,
    ))
}

#[get("/{namespace}/{repo}/edit/{path:.*}")]
pub async fn legacy_file_entry(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<FileParams>,
) -> HttpResponse {
    let params = params.into_inner();
    if !is_safe_repo_path(&params.path) || params.path.is_empty() {
        return HttpResponse::NotFound().finish();
    }
    if let Err(response) = open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        return response;
    }

    HttpResponse::Found()
        .insert_header((
            "Location",
            workspace_file_url(&params.namespace, &params.repo, &params.path),
        ))
        .finish()
}

#[cfg(test)]
mod tests {
    use super::{WorkspaceDraft, ZERO_OID, encode_file_path, pages};

    #[test]
    fn workspace_shell_loads_the_file_tree_through_htmx() {
        let draft = WorkspaceDraft {
            base_head: ZERO_OID.to_string(),
            active_path: None,
            tree_path: String::new(),
            changes: Vec::new(),
        };
        let html = pages::workspace_shell("acme", "notes", None, &draft, true).into_string();

        assert!(html.contains("Repository workspace"), "{html}");
        assert!(html.contains("hx-get=\"/acme/notes/edit/tree/\""), "{html}");
        assert!(html.contains("hx-trigger=\"load\""), "{html}");
        assert!(html.contains("hx-target=\"#repo-edit-tree\""), "{html}");
        assert!(
            html.contains("hx-action=\"/acme/notes/edit/commit\""),
            "{html}"
        );
        assert!(html.contains("hx-method=\"POST\""), "{html}");
        assert!(html.contains("hx-trigger=\"submit\""), "{html}");
        assert!(
            !html.contains("Run"),
            "repository code must not be run: {html}"
        );
    }

    #[test]
    fn workspace_paths_are_encoded_for_query_and_route_components() {
        assert_eq!(
            encode_file_path("docs/my file #1.md"),
            "docs/my%20file%20%231.md"
        );
        assert_eq!(encode_file_path("src/name?.rs"), "src/name%3F.rs");
    }

    #[test]
    fn editor_operation_forms_override_implicit_get_inheritance() {
        let html =
            pages::file_editor("acme", "notes", "README.md", "# Notes\n", true, true).into_string();

        assert!(
            html.contains("hx-action=\"/acme/notes/edit/operation\""),
            "{html}"
        );
        assert!(html.contains("hx-method=\"POST\""), "{html}");
        assert!(html.contains("hx-trigger=\"submit\""), "{html}");
    }
}
