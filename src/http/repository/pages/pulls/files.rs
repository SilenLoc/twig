//! The Files changed view: every file the request changes, hunk by hunk with line
//! numbers, the inline review comments placed on their lines, and a form to add one.

use actix_web::{HttpRequest, HttpResponse, Result as AwResult, get, post, web};
use maud::{Markup, PreEscaped, html};
use serde::Deserialize;

use crate::{
    auth::{NamespacePermission, TwigContext},
    config,
    db::pull_requests::{PullRequest, comments::PullRequestComment, reviews::ReviewState},
    git::bare::{DiffFile, DiffLine, FileStatus, LineKind, RepoHandle},
    http::repository::pages::{
        context::{RepoContext, open_repo},
        tabs::render_tab_response,
    },
    md,
    pulls::{self, Actor, InlineInput},
};

use super::{request_for_page, request_nav, signed_in};

#[derive(Debug, Deserialize)]
pub(super) struct FilesPath {
    namespace: String,
    repo: String,
    number: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct InlineForm {
    path: String,
    side: String,
    line: i64,
    body: String,
}

/// `GET /{namespace}/{repo}/pulls/{number}/files`
#[get("/{namespace}/{repo}/pulls/{number:\\d+}/files")]
pub async fn files_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<FilesPath>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, &path.namespace, &path.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };
    let viewer = signed_in(&req, &server, &auth_state, &path.namespace).await;
    let pull = match request_for_page(&ctx, &auth_state, path.number).await {
        Ok(pull) => pull,
        Err(notice) => return Ok(notice),
    };
    let comments = match auth_state.db().list_review_comments(&pull.id).await {
        Ok(comments) => comments,
        Err(error) => {
            log::error!("Failed to load review comments: {error}");
            return Ok(crate::http::view::render_error(
                "Files could not be loaded.",
            ));
        }
    };
    let files = match changed_files(&ctx.handle, &pull) {
        Some(files) => files,
        None => {
            return Ok(crate::http::view::render_error(
                "The changes could not be compared.",
            ));
        }
    };
    let can_comment = viewer
        .as_ref()
        .is_some_and(|(_, perms)| perms.contains(NamespacePermission::PushChanges));
    let body = render(&ctx, &pull, &files, &comments, can_comment);
    Ok(render_tab_response(&req, &ctx.frame(), "pulls", &body))
}

/// `POST /{namespace}/{repo}/pulls/{number}/files/comments`: one inline comment as a review.
#[post("/{namespace}/{repo}/pulls/{number:\\d+}/files/comments")]
pub async fn inline_comment_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<FilesPath>,
    form: web::Form<InlineForm>,
) -> HttpResponse {
    let Some((user, permissions)) = signed_in(&req, &server, &auth_state, &path.namespace).await
    else {
        return HttpResponse::SeeOther()
            .insert_header((
                "Location",
                format!(
                    "/auth/login?return_to=/{}/{}/pulls/{}/files",
                    path.namespace, path.repo, path.number
                ),
            ))
            .finish();
    };
    let handle = match crate::git::bare::RepoHandle::open(
        server.project_root(),
        &path.namespace,
        &path.repo,
    ) {
        Ok(handle) => handle,
        Err(_) => return error_page(actix_web::http::StatusCode::NOT_FOUND, "Not found."),
    };
    let repo_id = match handle.repo_id() {
        Ok(id) => id,
        Err(_) => {
            return error_page(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "The comment could not be saved.",
            );
        }
    };
    let pull = match pulls::get(
        auth_state.db(),
        &crate::pulls::RepoRef { repo_id: &repo_id },
        path.number,
    )
    .await
    {
        Ok(pull) => pull,
        Err(_) => {
            return error_page(
                actix_web::http::StatusCode::NOT_FOUND,
                "Pull request was not found.",
            );
        }
    };
    let head = match handle.branch_tip(&pull.head_branch).ok().flatten() {
        Some(oid) => oid.to_string(),
        None => {
            return error_page(
                actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                "The head branch is missing.",
            );
        }
    };
    let line = form.line.max(0);
    let (old_position, new_position) = if form.side == "old" {
        (line, 0)
    } else {
        (0, line)
    };
    let inline = [InlineInput {
        path: &form.path,
        body: &form.body,
        old_position,
        new_position,
    }];
    let actor = Actor { user_id: &user.id };
    match pulls::submit_review(
        auth_state.db(),
        &pull,
        &actor,
        &permissions,
        ReviewState::Comment,
        "",
        &head,
        &inline,
    )
    .await
    {
        Ok(_) => HttpResponse::SeeOther()
            .insert_header((
                "Location",
                format!(
                    "/{}/{}/pulls/{}/files",
                    path.namespace, path.repo, path.number
                ),
            ))
            .finish(),
        Err(error) => {
            let message = match &error {
                crate::pulls::PullError::Invalid(message) => message.clone(),
                crate::pulls::PullError::Forbidden => {
                    "You do not have permission to comment here.".to_string()
                }
                _ => "The comment could not be saved.".to_string(),
            };
            error_page(actix_web::http::StatusCode::UNPROCESSABLE_ENTITY, &message)
        }
    }
}

fn error_page(status: actix_web::http::StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status)
        .content_type("text/html; charset=utf-8")
        .body(crate::http::view::render_error(message).into_string())
}

/// The files a request changes, from its merge base to its head.
fn changed_files(handle: &RepoHandle, pull: &PullRequest) -> Option<Vec<DiffFile>> {
    let base = handle.branch_tip(&pull.base_branch).ok().flatten()?;
    let head = handle.branch_tip(&pull.head_branch).ok().flatten()?;
    let from = handle.merge_base(base, head).ok().flatten().unwrap_or(base);
    handle.diff_files(from, head).ok()
}

fn status_label(status: FileStatus) -> &'static str {
    match status {
        FileStatus::Added => "added",
        FileStatus::Deleted => "deleted",
        FileStatus::Renamed => "renamed",
        FileStatus::Modified => "modified",
    }
}

/// The inline comments that sit on one diff line: same path, same side, same number.
fn comments_on(line: &DiffLine, path: &str, comments: &[PullRequestComment]) -> Vec<String> {
    let number = match line.kind {
        LineKind::Removed => line.old_line,
        LineKind::Added | LineKind::Context => line.new_line,
    };
    let Some(number) = number.map(i64::from) else {
        return Vec::new();
    };
    comments
        .iter()
        .filter(|comment| comment.path.as_deref() == Some(path))
        .filter(|comment| match line.kind {
            LineKind::Removed => comment.old_position == Some(number),
            LineKind::Added | LineKind::Context => comment.new_position == Some(number),
        })
        .map(|comment| md::render_untrusted_markdown(&comment.body))
        .collect()
}

fn render(
    ctx: &RepoContext,
    pull: &PullRequest,
    files: &[DiffFile],
    comments: &[PullRequestComment],
    can_comment: bool,
) -> Markup {
    let namespace = &ctx.namespace;
    let repo = &ctx.repo;
    let number = pull.number;
    let additions: usize = files.iter().map(|file| file.additions).sum();
    let deletions: usize = files.iter().map(|file| file.deletions).sum();
    html! {
        div class="twig-stack twig-pr" {
            (request_nav(namespace, repo, number, "files"))
            p class="twig-hint" {
                (files.len()) " changed " (if files.len() == 1 { "file" } else { "files" })
                " with " (additions) " additions and " (deletions) " deletions"
            }
            @if can_comment && !files.is_empty() {
                form method="POST" action=(format!("/{namespace}/{repo}/pulls/{number}/files/comments")) class="twig-form twig-pr-inline-form" {
                    label class="twig-label" for="inline-path" { "File" }
                    select id="inline-path" name="path" class="twig-input twig-input--mono" {
                        @for file in files {
                            option value=(file.path()) { (file.path()) }
                        }
                    }
                    label class="twig-label" for="inline-side" { "Side" }
                    select id="inline-side" name="side" class="twig-input" {
                        option value="new" { "New" }
                        option value="old" { "Old" }
                    }
                    label class="twig-label" for="inline-line" { "Line" }
                    input id="inline-line" class="twig-input" type="number" min="1" name="line" required;
                    label class="twig-label" for="inline-body" { "Comment" }
                    textarea id="inline-body" class="twig-input twig-input--mono" name="body" rows="4" required {}
                    div class="twig-form-actions" {
                        button type="submit" class="twig-btn twig-btn--primary" { "Add review comment" }
                    }
                }
            }
            @for (index, file) in files.iter().enumerate() {
                section class="twig-pr-file" id=(format!("file-{index}")) {
                    header class="twig-pr-file-head" {
                        span class="twig-pr-file-status" { (status_label(file.status)) }
                        code class="twig-mono" { (file.path()) }
                        span class="twig-pr-file-counts" { "+" (file.additions) " −" (file.deletions) }
                    }
                    @if file.binary {
                        p class="twig-hint" { "Binary file not shown." }
                    }
                    @for hunk in &file.hunks {
                        div class="twig-pr-hunk" { code class="twig-mono" { (hunk.header) } }
                        table class="twig-pr-diff" {
                            tbody {
                                @for line in &hunk.lines {
                                    (line_row(line, file.path(), comments))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn line_row(line: &DiffLine, path: &str, comments: &[PullRequestComment]) -> Markup {
    let (class, sign) = match line.kind {
        LineKind::Added => ("twig-pr-line--added", "+"),
        LineKind::Removed => ("twig-pr-line--removed", "−"),
        LineKind::Context => ("twig-pr-line--context", " "),
    };
    let on_line = comments_on(line, path, comments);
    html! {
        tr class=(class) {
            td class="twig-pr-line-number" { (line.old_line.map_or(String::new(), |n| n.to_string())) }
            td class="twig-pr-line-number" { (line.new_line.map_or(String::new(), |n| n.to_string())) }
            td class="twig-pr-line-sign" { (sign) }
            td class="twig-pr-line-code" { pre { (line.text) } }
        }
        @for html_body in on_line {
            tr class="twig-pr-inline-row" {
                td colspan="4" { div class="twig-pr-inline" { (PreEscaped(html_body)) } }
            }
        }
    }
}
