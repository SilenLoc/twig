use actix_web::{HttpRequest, HttpResponse, web};
use maud::Markup;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{
        TwigContext, User,
        permissions::{self, NamespacePermission},
    },
    config,
    git::bare::{RepoHandle, TwigConfig, is_safe_repo_path},
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

mod commit;
mod handlers;
mod operations;
mod validation;

pub use handlers::{
    apply_operation, commit_workspace, file_fragment, legacy_file_entry, tree_fragment,
    workspace_page,
};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
