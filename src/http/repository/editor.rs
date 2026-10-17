use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{TwigContext, User},
    config,
    http::auth::session::get_username_from_request,
};
use twig_git::bare::{CommitFileOutcome, RepoHandle, is_safe_repo_path};

mod render;
use render::render_editor_page;

const MAX_EDIT_BYTES: usize = 1024 * 1024;
const MAX_FILE_NAME_BYTES: usize = 255;

/// Percent-encode a repository path for a URL while retaining directory
/// separators. In particular, `?`, `#`, and `%` in a Git filename must not
/// change the query, fragment, or route when rendered as a link.
pub(crate) fn encode_file_path(path: &str) -> String {
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

fn sanitized_username(username: &str) -> String {
    let prefix: String = username
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .take(40)
        .collect();
    let prefix = prefix.trim_matches('-');
    if prefix.is_empty() {
        "user".to_string()
    } else {
        prefix.to_string()
    }
}

fn conflict_copy_candidate(
    source_path: &str,
    username: &str,
    timestamp: &str,
    collision: usize,
) -> Result<String, twig_git::GitError> {
    if !is_safe_repo_path(source_path) || source_path.is_empty() {
        return Err(twig_git::GitError::from_str("Invalid source file path"));
    }
    let (directory, original_name) = source_path
        .rsplit_once('/')
        .map_or(("", source_path), |(directory, name)| (directory, name));
    let disambiguator = if collision == 0 {
        String::new()
    } else {
        format!("{}-", collision + 1)
    };
    let prefix = format!(
        "{}-{timestamp}-{disambiguator}",
        sanitized_username(username)
    );
    let basename_budget = MAX_FILE_NAME_BYTES.saturating_sub(prefix.len());
    let mut truncate_at = basename_budget.min(original_name.len());
    while !original_name.is_char_boundary(truncate_at) {
        truncate_at -= 1;
    }
    if truncate_at == 0 {
        return Err(twig_git::GitError::from_str(
            "Could not make room for a conflict-copy filename prefix",
        ));
    }
    let name = format!("{prefix}{}", &original_name[..truncate_at]);
    Ok(if directory.is_empty() {
        name
    } else {
        format!("{directory}/{name}")
    })
}

fn conflict_copy_path(
    handle: &RepoHandle,
    source_path: &str,
    username: &str,
    timestamp: &str,
) -> Result<String, twig_git::GitError> {
    for collision in 0..1000 {
        let candidate = conflict_copy_candidate(source_path, username, timestamp, collision)?;
        if handle.file_mode(&candidate)?.is_none() {
            return Ok(candidate);
        }
    }
    Err(twig_git::GitError::from_str(
        "Could not find an unused conflict-copy filename",
    ))
}

#[derive(Deserialize)]
struct EditParams {
    namespace: String,
    repo: String,
    path: String,
}

#[derive(Deserialize)]
struct CommitFileInput {
    content: String,
    message: String,
    expected_head: String,
    #[serde(default)]
    save_conflict_copy: bool,
}

#[derive(Serialize)]
struct CommitFileResponse {
    location: String,
}

struct EditContext {
    handle: RepoHandle,
    user: User,
    content: String,
    head: twig_git::Oid,
    markdown: bool,
}

async fn open_editor_context(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    params: &EditParams,
    for_write: bool,
    allow_conflict_copy: bool,
) -> Result<EditContext, HttpResponse> {
    let Some(user_id) = auth_state.user_id_from_request(req).await else {
        return Err(if for_write {
            HttpResponse::Unauthorized().body("Please log in to edit repository files.")
        } else {
            HttpResponse::Found()
                .insert_header(("Location", "/auth/login"))
                .finish()
        });
    };
    let db = auth_state.db();
    let user = match db.get_user_by_id(&user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return Err(HttpResponse::Unauthorized().body("Session expired. Log in again.")),
        Err(error) => {
            log::error!("Failed to load editor user: {error}");
            return Err(HttpResponse::InternalServerError().finish());
        }
    };
    match db
        .user_has_namespace_access(&user_id, &params.namespace)
        .await
    {
        Ok(true) => {}
        Ok(false) => return Err(HttpResponse::Forbidden().body("No access to this namespace.")),
        Err(error) => {
            log::error!("Failed to verify editor namespace access: {error}");
            return Err(HttpResponse::InternalServerError().finish());
        }
    }

    if !is_safe_repo_path(&params.path) || params.path.is_empty() {
        return Err(HttpResponse::NotFound().finish());
    }
    let handle = RepoHandle::open(server.project_root(), &params.namespace, &params.repo)
        .map_err(|_| HttpResponse::NotFound().finish())?;
    let config = handle.load_config_with_raw().config;
    if !allow_conflict_copy && config.should_ignore(&params.path) {
        return Err(HttpResponse::NotFound().finish());
    }
    if config.private && get_username_from_request(req, auth_state).await.is_none() {
        return Err(HttpResponse::Unauthorized().body("Log in to view this repository."));
    }
    let head = handle
        .head_oid()
        .map_err(|_| HttpResponse::InternalServerError().finish())?
        .ok_or_else(|| HttpResponse::NotFound().body("Repository has no commits yet."))?;
    if allow_conflict_copy {
        return Ok(EditContext {
            handle,
            user,
            content: String::new(),
            head,
            markdown: crate::md::is_markdown(&params.path),
        });
    }
    let mode = handle
        .file_mode(&params.path)
        .map_err(|_| HttpResponse::InternalServerError().finish())?
        .ok_or_else(|| HttpResponse::NotFound().finish())?;
    if !matches!(mode, 0o100_644 | 0o100_755) {
        return Err(
            HttpResponse::UnsupportedMediaType().body("Only regular text files can be edited.")
        );
    }
    let bytes = handle
        .read_blob_bytes(&params.path)
        .map_err(|_| HttpResponse::InternalServerError().finish())?
        .ok_or_else(|| HttpResponse::NotFound().finish())?;
    if bytes.len() > MAX_EDIT_BYTES {
        return Err(
            HttpResponse::PayloadTooLarge().body("Files larger than 1 MiB cannot be edited here.")
        );
    }
    if bytes.contains(&0) {
        return Err(HttpResponse::UnsupportedMediaType().body("Binary files cannot be edited."));
    }
    let content = String::from_utf8(bytes).map_err(|_| {
        HttpResponse::UnsupportedMediaType().body("Only UTF-8 text files can be edited.")
    })?;

    Ok(EditContext {
        handle,
        user,
        content,
        head,
        markdown: crate::md::is_markdown(&params.path),
    })
}

#[get("/{namespace}/{repo}/edit/{path:.*}")]
pub async fn edit_file_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<EditParams>,
) -> HttpResponse {
    let ctx = match open_editor_context(&req, &server, &auth_state, &params, false, false).await {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    let page = render_editor_page(
        &params.namespace,
        &params.repo,
        &params.path,
        &ctx.content,
        ctx.head,
        ctx.markdown,
        ctx.user.email.is_some(),
    );
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(
            crate::http::view::render_layout(
                &page,
                Some(&ctx.user.username),
                Some(&format!("Edit {}", params.path)),
            )
            .into_string(),
        )
}

#[post("/{namespace}/{repo}/edit/{path:.*}")]
pub async fn commit_file_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<EditParams>,
    input: web::Json<CommitFileInput>,
) -> HttpResponse {
    let ctx = match open_editor_context(
        &req,
        &server,
        &auth_state,
        &params,
        true,
        input.save_conflict_copy,
    )
    .await
    {
        Ok(ctx) => ctx,
        Err(response) => return response,
    };

    if input.content.len() > MAX_EDIT_BYTES {
        return HttpResponse::PayloadTooLarge()
            .body("Files larger than 1 MiB cannot be committed here.");
    }
    let message = input.message.trim();
    if message.is_empty() {
        return HttpResponse::BadRequest().body("Enter a commit message.");
    }
    if message.chars().count() > 200 {
        return HttpResponse::BadRequest().body("Commit messages must be 200 characters or fewer.");
    }
    let Some(email) = ctx.user.email.as_deref() else {
        return HttpResponse::BadRequest()
            .insert_header(("Location", "/settings"))
            .body("Set an email address in account settings before committing.");
    };
    let Ok(expected_head) = twig_git::Oid::from_str(&input.expected_head) else {
        return HttpResponse::BadRequest().body("Invalid editor base revision.");
    };
    if input.save_conflict_copy {
        return save_conflict_copy(&ctx, &params, &input, expected_head, email, message);
    }
    if input.content == ctx.content {
        return HttpResponse::UnprocessableEntity().body("There are no changes to commit.");
    }

    match ctx.handle.commit_file(
        &params.path,
        input.content.as_bytes(),
        expected_head,
        &ctx.user.username,
        email,
        message,
    ) {
        Ok(CommitFileOutcome::Committed(oid)) => {
            log::info!(
                "Committed repository file '{}/{}:{}' as {} by {}",
                params.namespace,
                params.repo,
                params.path,
                oid,
                ctx.user.username
            );
            HttpResponse::Ok().json(CommitFileResponse {
                location: format!(
                    "/{}/{}/content/{}?committed={}",
                    params.namespace,
                    params.repo,
                    encode_file_path(&params.path),
                    oid
                ),
            })
        }
        Ok(CommitFileOutcome::Conflict) => HttpResponse::Conflict().body(
            "The repository changed since you opened this editor. Your draft is still here; save it as a new file or reload the latest revision.",
        ),
        Err(error) => {
            log::error!("Failed to commit repository editor change: {error}");
            HttpResponse::InternalServerError().body("The change could not be committed.")
        }
    }
}

#[cfg(test)]
#[path = "editor/tests.rs"]
mod tests;

fn save_conflict_copy(
    ctx: &EditContext,
    params: &EditParams,
    input: &CommitFileInput,
    stale_base: twig_git::Oid,
    email: &str,
    message: &str,
) -> HttpResponse {
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let copy_path =
        match conflict_copy_path(&ctx.handle, &params.path, &ctx.user.username, &timestamp) {
            Ok(path) => path,
            Err(error) => {
                log::error!("Failed to choose a conflict-copy filename: {error}");
                return HttpResponse::InternalServerError()
                    .body("Could not prepare a conflict-copy filename.");
            }
        };
    let commit_message = format!("Save conflict copy of {}: {message}", params.path);
    match ctx.handle.commit_new_file(
        &copy_path,
        input.content.as_bytes(),
        ctx.head,
        &ctx.user.username,
        email,
        &commit_message,
    ) {
        Ok(CommitFileOutcome::Committed(oid)) => {
            log::info!(
                "Saved conflicted draft '{}/{}' as '{}/{}' in commit {} by {} (stale base {})",
                params.namespace,
                params.repo,
                params.namespace,
                copy_path,
                oid,
                ctx.user.username,
                stale_base
            );
            HttpResponse::Ok().json(CommitFileResponse {
                location: format!(
                    "/{}/{}/content/{}?committed={}&conflict_copy=true",
                    params.namespace,
                    params.repo,
                    encode_file_path(&copy_path),
                    oid
                ),
            })
        }
        Ok(CommitFileOutcome::Conflict) => HttpResponse::Conflict().body(
            "The repository changed again while saving the copy. Your draft is still here; retry the copy or reload.",
        ),
        Err(error) => {
            log::error!("Failed to commit conflict copy '{copy_path}': {error}");
            HttpResponse::InternalServerError().body("The conflict copy could not be committed.")
        }
    }
}
