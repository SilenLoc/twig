use actix_web::{HttpRequest, HttpResponse, get, post, web};
use maud::Markup;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{TwigContext, User},
    config,
    git::bare::{CommitFileOutcome, RepoHandle, is_safe_repo_path},
    http::auth::session::get_username_from_request,
};

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
) -> Result<String, git2::Error> {
    if !is_safe_repo_path(source_path) || source_path.is_empty() {
        return Err(git2::Error::from_str("Invalid source file path"));
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
        return Err(git2::Error::from_str(
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
) -> Result<String, git2::Error> {
    for collision in 0..1000 {
        let candidate = conflict_copy_candidate(source_path, username, timestamp, collision)?;
        if handle.file_mode(&candidate)?.is_none() {
            return Ok(candidate);
        }
    }
    Err(git2::Error::from_str(
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
    head: git2::Oid,
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
    let Ok(expected_head) = git2::Oid::from_str(&input.expected_head) else {
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

fn save_conflict_copy(
    ctx: &EditContext,
    params: &EditParams,
    input: &CommitFileInput,
    stale_base: git2::Oid,
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

fn render_editor_page(
    namespace: &str,
    repo: &str,
    path: &str,
    content: &str,
    head: git2::Oid,
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

fn render_conflict_actions(save_url: &str) -> Markup {
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

fn render_editor_scripts(markdown: bool) -> Markup {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_page_includes_source_and_commit_precondition() {
        let html = render_editor_page(
            "acme",
            "twig",
            "docs/guide.md",
            "# Guide\n",
            git2::Oid::ZERO_SHA1,
            true,
            true,
        )
        .into_string();

        assert!(html.contains("id=\"repository-editor-form\""));
        assert!(
            html.contains(
                "name=\"expected_head\" value=\"0000000000000000000000000000000000000000\""
            )
        );
        assert!(html.contains("# Guide\n"));
        assert!(html.contains("Markdown source"));
        assert!(html.contains("data-editor-reload"));
        assert!(html.contains("data-editor-conflict-copy"));
        assert!(html.contains(&crate::http::assets::url("quill.js")));
        assert!(html.contains(&crate::http::assets::url("marked.js")));
        assert!(html.contains(&crate::http::assets::url("purify.js")));
        assert!(html.contains(&crate::http::assets::url("turndown.js")));
        assert!(html.contains(&crate::http::assets::url("quill.snow.css")));
        assert!(
            !html.contains("https://"),
            "editor dependencies must be local"
        );
        assert!(html.contains("Commit changes"));
    }

    #[test]
    fn source_page_does_not_offer_rich_controls() {
        let html = render_editor_page(
            "acme",
            "twig",
            "src/main.rs",
            "fn main() {}\n",
            git2::Oid::ZERO_SHA1,
            false,
            true,
        )
        .into_string();

        assert!(html.contains("Source editor"));
        assert!(!html.contains("Markdown source"));
        assert!(html.contains("fn main() {}"));
    }

    #[test]
    fn file_paths_are_encoded_without_losing_directory_separators() {
        assert_eq!(
            encode_file_path("docs/my file #1.md"),
            "docs/my%20file%20%231.md"
        );
        assert_eq!(encode_file_path("src/name?.rs"), "src/name%3F.rs");
    }

    #[test]
    fn conflict_copy_names_include_a_safe_user_timestamp_and_collision_suffix() {
        assert_eq!(
            conflict_copy_candidate("docs/README.md", "alice@example.com", "20261010-123456", 0,)
                .unwrap(),
            "docs/alice-example-com-20261010-123456-README.md"
        );
        assert_eq!(
            conflict_copy_candidate("docs/README.md", "alice@example.com", "20261010-123456", 1,)
                .unwrap(),
            "docs/alice-example-com-20261010-123456-2-README.md"
        );
        assert!(conflict_copy_candidate("../README.md", "alice", "now", 0).is_err());
    }

    #[test]
    fn conflict_copy_filename_truncation_preserves_utf8_and_component_limit() {
        let source = format!("docs/{}.md", "語".repeat(200));
        let candidate = conflict_copy_candidate(&source, "alice", "20261010-123456", 0).unwrap();
        let name = candidate.rsplit('/').next().unwrap();
        assert!(name.len() <= 255);
        assert!(std::str::from_utf8(name.as_bytes()).is_ok());
    }
}
