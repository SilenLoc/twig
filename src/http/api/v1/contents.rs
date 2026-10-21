use actix_web::{HttpRequest, HttpResponse, delete, get, post, put, web};
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthenticatedPrincipal, NamespacePermission, TwigContext},
    config,
    git::bare::{CommitChangesOutcome, RepoEditChange, RepoHandle, is_safe_repo_path},
};

use super::{ApiError, RepositoryPath, authenticate, json_error, require_namespace_permission};

const MAX_FILE_BYTES: usize = 1 << 20;

#[derive(Debug, Deserialize)]
struct ContentQuery {
    #[serde(rename = "ref")]
    reference: String,
    path: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ContentEncoding {
    Utf8,
    Base64,
}

#[derive(Debug, Deserialize)]
struct WriteContentRequest {
    content: String,
    encoding: ContentEncoding,
    message: String,
    expected_head: String,
}

#[derive(Debug, Deserialize)]
struct DeleteContentRequest {
    message: String,
    expected_head: String,
}

#[derive(Debug, Deserialize)]
struct RenameContentRequest {
    #[serde(rename = "ref")]
    reference: String,
    from: String,
    to: String,
    message: String,
    expected_head: String,
}

#[derive(Debug, Serialize)]
struct TreeEntryResponse {
    name: String,
    path: String,
    is_directory: bool,
}

#[derive(Debug, Serialize)]
struct TreeResponse {
    reference: String,
    path: String,
    entries: Vec<TreeEntryResponse>,
}

#[derive(Debug, Serialize)]
struct ContentResponse {
    path: String,
    reference: String,
    encoding: &'static str,
    content: String,
    object_id: String,
    size_bytes: usize,
}

#[derive(Debug, Serialize)]
struct MutationResponse {
    reference: String,
    path: String,
    commit: String,
    message: String,
}

async fn open_authorized_repo(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    path: &RepositoryPath,
    permission: NamespacePermission,
) -> Result<(AuthenticatedPrincipal, RepoHandle), HttpResponse> {
    let principal = authenticate(req, auth_state).await.map_err(json_error)?;
    super::validate_namespace_name(&path.namespace).map_err(json_error)?;
    super::validate_repository_name(&path.repo).map_err(json_error)?;
    require_namespace_permission(&principal, server, auth_state, &path.namespace, permission)
        .await
        .map_err(json_error)?;
    let handle = RepoHandle::open(server.project_root(), &path.namespace, &path.repo)
        .map_err(|_| json_error(ApiError::NotFound))?;
    Ok((principal, handle))
}

fn validate_reference(handle: &RepoHandle, reference: &str) -> Result<git2::Oid, ApiError> {
    match handle.branch_tip(reference) {
        Ok(Some(oid)) => Ok(oid),
        Ok(None) => Err(ApiError::NotFound),
        Err(_) => Err(ApiError::BadRequest("Invalid branch name".to_string())),
    }
}

fn validate_path(path: Option<&str>, allow_root: bool) -> Result<String, ApiError> {
    let path = path.unwrap_or_default();
    if (!allow_root && path.is_empty()) || !is_safe_repo_path(path) {
        return Err(ApiError::BadRequest("Invalid repository path".to_string()));
    }
    Ok(path.to_string())
}

#[get("/api/v1/namespaces/{namespace}/repositories/{repo}/tree")]
pub async fn get_tree(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepositoryPath>,
    query: web::Query<ContentQuery>,
) -> HttpResponse {
    let (_, handle) = match open_authorized_repo(
        &req,
        &server,
        &auth_state,
        &path,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    if let Err(error) = validate_reference(&handle, &query.reference) {
        return json_error(error);
    }
    let dir = match validate_path(query.path.as_deref(), true) {
        Ok(path) => path,
        Err(error) => return json_error(error),
    };
    let twig_config = match handle.load_config_at(&query.reference) {
        Ok(config) => config.config,
        Err(error) => {
            log::error!("Failed to load branch-specific Twig config: {error}");
            return json_error(ApiError::Internal);
        }
    };
    if !dir.is_empty() && twig_config.should_ignore(&dir) {
        return json_error(ApiError::NotFound);
    }
    let entries = match handle.list_dir_at(&query.reference, &dir, Some(&twig_config)) {
        Ok(entries) => entries,
        Err(error) => {
            log::error!("Failed to list API branch tree: {error}");
            return json_error(ApiError::Internal);
        }
    };
    if !dir.is_empty() {
        match handle.path_exists_at(&query.reference, &dir) {
            Ok(true) => {}
            Ok(false) => return json_error(ApiError::NotFound),
            Err(error) => {
                log::error!("Failed to verify API tree path: {error}");
                return json_error(ApiError::Internal);
            }
        }
        match handle.file_mode_at(&query.reference, &dir) {
            Ok(Some(_)) => return json_error(ApiError::NotFound),
            Ok(None) => {}
            Err(error) => {
                log::error!("Failed to inspect API tree object: {error}");
                return json_error(ApiError::Internal);
            }
        }
        if !handle
            .path_exists_at(&query.reference, &dir)
            .unwrap_or(false)
        {
            return json_error(ApiError::NotFound);
        }
    }
    HttpResponse::Ok().json(TreeResponse {
        reference: query.reference.clone(),
        path: dir,
        entries: entries
            .into_iter()
            .map(|entry| TreeEntryResponse {
                name: entry.name,
                path: entry.path,
                is_directory: entry.is_dir,
            })
            .collect(),
    })
}

#[get("/api/v1/namespaces/{namespace}/repositories/{repo}/contents")]
pub async fn get_content(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepositoryPath>,
    query: web::Query<ContentQuery>,
) -> HttpResponse {
    let (_, handle) = match open_authorized_repo(
        &req,
        &server,
        &auth_state,
        &path,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    if let Err(error) = validate_reference(&handle, &query.reference) {
        return json_error(error);
    }
    let file_path = match validate_path(query.path.as_deref(), false) {
        Ok(path) => path,
        Err(error) => return json_error(error),
    };
    let twig_config = match handle.load_config_at(&query.reference) {
        Ok(config) => config.config,
        Err(error) => {
            log::error!("Failed to load branch-specific Twig config: {error}");
            return json_error(ApiError::Internal);
        }
    };
    if twig_config.should_ignore(&file_path) {
        return json_error(ApiError::NotFound);
    }
    match handle.file_mode_at(&query.reference, &file_path) {
        Ok(Some(0o100_644 | 0o100_755)) => {}
        Ok(Some(_) | None) => return json_error(ApiError::NotFound),
        Err(error) => {
            log::error!("Failed to inspect API content object: {error}");
            return json_error(ApiError::Internal);
        }
    }
    match handle.read_blob_bytes_at(&query.reference, &file_path) {
        Ok(Some((object_id, bytes))) => {
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            HttpResponse::Ok().json(ContentResponse {
                path: file_path,
                reference: query.reference.clone(),
                encoding: "base64",
                content: encoded,
                object_id: object_id.to_string(),
                size_bytes: bytes.len(),
            })
        }
        Ok(None) => json_error(ApiError::NotFound),
        Err(error) => {
            log::error!("Failed to read API branch content: {error}");
            json_error(ApiError::Internal)
        }
    }
}

fn validate_message(message: &str) -> Result<(), ApiError> {
    let message = message.trim();
    if message.is_empty() || message.chars().count() > 200 {
        return Err(ApiError::BadRequest(
            "Commit message must contain 1 to 200 characters".to_string(),
        ));
    }
    Ok(())
}

fn expected_oid(value: &str) -> Result<git2::Oid, ApiError> {
    git2::Oid::from_str(value)
        .map_err(|_| ApiError::BadRequest("Invalid expected_head object ID".to_string()))
}

fn decode_content(body: &WriteContentRequest) -> Result<Vec<u8>, ApiError> {
    let content = match body.encoding {
        ContentEncoding::Utf8 => body.content.as_bytes().to_vec(),
        ContentEncoding::Base64 => base64::engine::general_purpose::STANDARD
            .decode(&body.content)
            .map_err(|_| ApiError::BadRequest("content is not valid Base64".to_string()))?,
    };
    if content.len() > MAX_FILE_BYTES {
        return Err(ApiError::PayloadTooLarge);
    }
    Ok(content)
}

fn require_regular_file_mode(
    handle: &RepoHandle,
    reference: &str,
    path: &str,
) -> Result<(), ApiError> {
    let mode = handle.file_mode_at(reference, path).map_err(|error| {
        log::error!("Failed to inspect API file mode: {error}");
        ApiError::Internal
    })?;
    match mode {
        Some(0o100_644 | 0o100_755) | None => Ok(()),
        Some(_) => Err(ApiError::BadRequest(
            "Only regular files can be changed through the API".to_string(),
        )),
    }
}

fn commit_changes(
    handle: &RepoHandle,
    principal: &AuthenticatedPrincipal,
    reference: &str,
    changes: &[RepoEditChange],
    expected_head: git2::Oid,
    message: &str,
) -> Result<git2::Oid, ApiError> {
    let email = principal.user.email.as_deref().ok_or_else(|| {
        ApiError::BadRequest("Set an email address before committing changes".to_string())
    })?;
    match handle
        .commit_changes_at(
            reference,
            changes,
            expected_head,
            &principal.user.username,
            email,
            message.trim(),
        )
        .map_err(|error| {
            log::error!("Failed to commit API repository change: {error}");
            ApiError::BadRequest("The requested file changes are invalid".to_string())
        })? {
        CommitChangesOutcome::Committed(commit) => Ok(commit),
        CommitChangesOutcome::Conflict => Err(ApiError::Conflict(
            "Branch tip changed; reload and retry with the current expected_head".to_string(),
        )),
    }
}

fn rename_changes(
    handle: &RepoHandle,
    reference: &str,
    from: &str,
    to: &str,
) -> Result<[RepoEditChange; 2], ApiError> {
    if from == to {
        return Err(ApiError::BadRequest(
            "Choose a different destination path".to_string(),
        ));
    }
    for file_path in [from, to] {
        if file_path.is_empty() || !is_safe_repo_path(file_path) {
            return Err(ApiError::BadRequest("Invalid repository path".to_string()));
        }
    }
    let twig_config = handle
        .load_config_at(reference)
        .map_err(|error| {
            log::error!("Failed to load branch-specific Twig config: {error}");
            ApiError::Internal
        })?
        .config;
    if twig_config.should_ignore(from) || twig_config.should_ignore(to) {
        return Err(ApiError::NotFound);
    }
    require_regular_file_mode(handle, reference, from)?;
    require_regular_file_mode(handle, reference, to)?;
    match handle.path_exists_at(reference, from) {
        Ok(true) => {}
        Ok(false) => return Err(ApiError::NotFound),
        Err(error) => {
            log::error!("Failed to check API rename source: {error}");
            return Err(ApiError::Internal);
        }
    }
    match handle.path_exists_at(reference, to) {
        Ok(true) => {
            return Err(ApiError::Conflict(
                "A file already exists at the destination path".to_string(),
            ));
        }
        Ok(false) => {}
        Err(error) => {
            log::error!("Failed to check API rename target: {error}");
            return Err(ApiError::Internal);
        }
    }
    let (_, bytes) = handle
        .read_blob_bytes_at(reference, from)
        .map_err(|error| {
            log::error!("Failed to read API rename source: {error}");
            ApiError::Internal
        })?
        .ok_or(ApiError::NotFound)?;
    Ok([
        RepoEditChange::Delete {
            path: from.to_string(),
        },
        RepoEditChange::Upsert {
            path: to.to_string(),
            content: bytes,
        },
    ])
}

#[put("/api/v1/namespaces/{namespace}/repositories/{repo}/contents")]
pub async fn put_content(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepositoryPath>,
    query: web::Query<ContentQuery>,
    body: web::Json<WriteContentRequest>,
) -> HttpResponse {
    let (principal, handle) = match open_authorized_repo(
        &req,
        &server,
        &auth_state,
        &path,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    let current_head = match validate_reference(&handle, &query.reference) {
        Ok(head) => head,
        Err(error) => return json_error(error),
    };
    let file_path = match validate_path(query.path.as_deref(), false) {
        Ok(path) => path,
        Err(error) => return json_error(error),
    };
    let twig_config = match handle.load_config_at(&query.reference) {
        Ok(config) => config.config,
        Err(error) => {
            log::error!("Failed to load branch-specific Twig config: {error}");
            return json_error(ApiError::Internal);
        }
    };
    if twig_config.should_ignore(&file_path) {
        return json_error(ApiError::NotFound);
    }
    if let Err(error) = require_regular_file_mode(&handle, &query.reference, &file_path) {
        return json_error(error);
    }
    if let Err(error) = validate_message(&body.message) {
        return json_error(error);
    }
    let expected = match expected_oid(&body.expected_head) {
        Ok(oid) => oid,
        Err(error) => return json_error(error),
    };
    if expected != current_head {
        return json_error(ApiError::Conflict(
            "Branch tip changed; reload and retry with the current expected_head".to_string(),
        ));
    }
    let bytes = match decode_content(&body) {
        Ok(bytes) => bytes,
        Err(error) => return json_error(error),
    };
    let existed = match handle.path_exists_at(&query.reference, &file_path) {
        Ok(existed) => existed,
        Err(error) => {
            log::error!("Failed to inspect API file path: {error}");
            return json_error(ApiError::Internal);
        }
    };
    let change = RepoEditChange::Upsert {
        path: file_path.clone(),
        content: bytes,
    };
    let commit = match commit_changes(
        &handle,
        &principal,
        &query.reference,
        &[change],
        expected,
        &body.message,
    ) {
        Ok(commit) => commit,
        Err(error) => return json_error(error),
    };
    let response = MutationResponse {
        reference: query.reference.clone(),
        path: file_path,
        commit: commit.to_string(),
        message: "File committed".to_string(),
    };
    if existed {
        HttpResponse::Ok().json(response)
    } else {
        HttpResponse::Created().json(response)
    }
}

#[delete("/api/v1/namespaces/{namespace}/repositories/{repo}/contents")]
pub async fn delete_content(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepositoryPath>,
    query: web::Query<ContentQuery>,
    body: web::Json<DeleteContentRequest>,
) -> HttpResponse {
    let (principal, handle) = match open_authorized_repo(
        &req,
        &server,
        &auth_state,
        &path,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    let current_head = match validate_reference(&handle, &query.reference) {
        Ok(head) => head,
        Err(error) => return json_error(error),
    };
    let file_path = match validate_path(query.path.as_deref(), false) {
        Ok(path) => path,
        Err(error) => return json_error(error),
    };
    let twig_config = match handle.load_config_at(&query.reference) {
        Ok(config) => config.config,
        Err(error) => {
            log::error!("Failed to load branch-specific Twig config: {error}");
            return json_error(ApiError::Internal);
        }
    };
    if twig_config.should_ignore(&file_path) {
        return json_error(ApiError::NotFound);
    }
    if let Err(error) = require_regular_file_mode(&handle, &query.reference, &file_path) {
        return json_error(error);
    }
    match handle.path_exists_at(&query.reference, &file_path) {
        Ok(true) => {}
        Ok(false) => return json_error(ApiError::NotFound),
        Err(error) => {
            log::error!("Failed to check API delete path: {error}");
            return json_error(ApiError::Internal);
        }
    }
    if let Err(error) = validate_message(&body.message) {
        return json_error(error);
    }
    let expected = match expected_oid(&body.expected_head) {
        Ok(oid) => oid,
        Err(error) => return json_error(error),
    };
    if expected != current_head {
        return json_error(ApiError::Conflict(
            "Branch tip changed; reload and retry with the current expected_head".to_string(),
        ));
    }
    let change = RepoEditChange::Delete {
        path: file_path.clone(),
    };
    let commit = match commit_changes(
        &handle,
        &principal,
        &query.reference,
        &[change],
        expected,
        &body.message,
    ) {
        Ok(commit) => commit,
        Err(error) => return json_error(error),
    };
    HttpResponse::Ok().json(MutationResponse {
        reference: query.reference.clone(),
        path: file_path,
        commit: commit.to_string(),
        message: "File deleted".to_string(),
    })
}

#[post("/api/v1/namespaces/{namespace}/repositories/{repo}/contents/rename")]
pub async fn rename_content(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepositoryPath>,
    body: web::Json<RenameContentRequest>,
) -> HttpResponse {
    let (principal, handle) = match open_authorized_repo(
        &req,
        &server,
        &auth_state,
        &path,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    let current_head = match validate_reference(&handle, &body.reference) {
        Ok(head) => head,
        Err(error) => return json_error(error),
    };
    if let Err(error) = validate_message(&body.message) {
        return json_error(error);
    }
    let expected = match expected_oid(&body.expected_head) {
        Ok(oid) => oid,
        Err(error) => return json_error(error),
    };
    if expected != current_head {
        return json_error(ApiError::Conflict(
            "Branch tip changed; reload and retry with the current expected_head".to_string(),
        ));
    }
    let changes = match rename_changes(&handle, &body.reference, &body.from, &body.to) {
        Ok(changes) => changes,
        Err(error) => return json_error(error),
    };
    let commit = match commit_changes(
        &handle,
        &principal,
        &body.reference,
        &changes,
        expected,
        &body.message,
    ) {
        Ok(commit) => commit,
        Err(error) => return json_error(error),
    };
    HttpResponse::Ok().json(MutationResponse {
        reference: body.reference.clone(),
        path: body.to.clone(),
        commit: commit.to_string(),
        message: "File renamed".to_string(),
    })
}
