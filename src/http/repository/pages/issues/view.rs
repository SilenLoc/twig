//! One issue's page: the title and state, the description, the conversation, and the
//! controls the signed-in viewer may use. A sidebar holds the assignees and the close or
//! reopen action, as in the issue mockup. A pull request's number redirects to its page.

use actix_web::{HttpRequest, HttpResponse, Result as AwResult, get, http::StatusCode, web};
use maud::{Markup, PreEscaped, html};

use crate::{
    auth::{NamespacePermission, PermissionSet, TwigContext, User},
    config,
    db::issues::{Issue, IssueState, comments::IssueComment},
    http::repository::pages::{
        context::{RepoContext, open_repo},
        tabs::render_tab_response,
    },
    issues::{self, IssueError, RepoRef},
    md, pulls,
};

use super::{
    author_name,
    composer::{self, markdown_field},
    error_response, html_response, see_other, signed_in,
};

#[derive(serde::Deserialize)]
pub(super) struct IssuePath {
    namespace: String,
    repo: String,
    number: i64,
}

#[derive(serde::Deserialize, Default)]
pub(super) struct ViewQuery {
    /// The comment whose edit form is open, by its row ID.
    edit: Option<i64>,
}

/// `GET /{namespace}/{repo}/issues/{number}`
#[get("/{namespace}/{repo}/issues/{number:\\d+}")]
pub async fn issue_page_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    query: web::Query<ViewQuery>,
) -> AwResult<HttpResponse> {
    let ctx = match open_repo(&req, &server, &auth_state, &path.namespace, &path.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(html_response(StatusCode::NOT_FOUND, rendered)),
    };
    let repo_id = match ctx.handle.repo_id() {
        Ok(id) => id,
        Err(error) => {
            log::error!("Failed to read repository identity: {error}");
            return Ok(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Issue could not be loaded.",
            ));
        }
    };
    let scope = RepoRef {
        repo_id: &repo_id,
        namespace: &path.namespace,
    };
    match issues::get(auth_state.db(), &scope, path.number).await {
        Ok(issue) => {
            let viewer = signed_in(&req, &server, &auth_state, &path.namespace).await;
            let body = render(&ctx, &auth_state, &issue, viewer.as_ref(), query.edit).await;
            Ok(html_response(
                StatusCode::OK,
                render_tab_response(&req, &ctx.frame(), "issues", &body),
            ))
        }
        Err(IssueError::NotFound) => {
            // Issues and pull requests share numbers, so a pull request's number is a
            // link to the right page rather than a dead end.
            let pull_scope = pulls::RepoRef { repo_id: &repo_id };
            if pulls::get(auth_state.db(), &pull_scope, path.number)
                .await
                .is_ok()
            {
                return Ok(see_other(&format!(
                    "/{}/{}/pulls/{}",
                    path.namespace, path.repo, path.number
                )));
            }
            Ok(error_response(
                StatusCode::NOT_FOUND,
                "Issue was not found.",
            ))
        }
        Err(error) => {
            log::error!("Failed to load issue #{}: {error:?}", path.number);
            Ok(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Issue could not be loaded.",
            ))
        }
    }
}

async fn render(
    ctx: &RepoContext,
    auth_state: &web::Data<TwigContext>,
    issue: &Issue,
    viewer: Option<&(User, PermissionSet)>,
    editing: Option<i64>,
) -> Markup {
    let namespace = &ctx.namespace;
    let repo = &ctx.repo;
    let number = issue.number;
    let author = author_name(auth_state, &issue.author_user_id).await;
    let closed = issue.state == IssueState::Closed;
    let (label, key) = match issue.state {
        IssueState::Open => ("Open", "open"),
        IssueState::Closed => ("Closed", "closed"),
    };
    let opened = issue.created_at.split('T').next().unwrap_or_default();

    let comments = match auth_state.db().list_issue_comments(&issue.id).await {
        Ok(comments) => comments,
        Err(error) => {
            log::error!("Failed to load comments for issue #{number}: {error}");
            Vec::new()
        }
    };
    let mut timeline = Vec::with_capacity(comments.len());
    for comment in comments {
        let name = author_name(auth_state, &comment.author_user_id).await;
        timeline.push((comment, name));
    }

    let viewer_id = viewer.map(|(user, _)| user.id.as_str());
    let can_change_state = viewer.is_some_and(|(user, permissions)| {
        issue.author_user_id == user.id || permissions.contains(NamespacePermission::PushChanges)
    });
    let can_push = viewer
        .is_some_and(|(_, permissions)| permissions.contains(NamespacePermission::PushChanges));
    let assignees = super::actions::current_assignee_names(auth_state.db(), issue)
        .await
        .unwrap_or_default();
    let owner_choices = if can_push {
        unassigned_owners(auth_state, namespace, &assignees).await
    } else {
        Vec::new()
    };
    html! {
        div class="twig-stack" {
            header class="twig-issue-head" {
                h2 class="twig-section" {
                    (issue.title) " "
                    span class="twig-pr-number" { "#" (number) }
                }
                div class="twig-pr-byline" {
                    span class="twig-pr-state" data-state=(key) { (label) }
                    " " (author) " opened this issue on " (opened)
                }
            }
            div class="twig-issue-layout" {
                div class="twig-issue-main" {
                    div class="twig-pr-timeline" {
                        article class="twig-pr-entry" id="issue-description" {
                            div class="twig-pr-entry-body" {
                                @if issue.body.trim().is_empty() {
                                    p class="twig-hint" { "No description provided." }
                                } @else {
                                    (PreEscaped(md::render_untrusted_markdown(&issue.body)))
                                }
                            }
                            @if can_change_state {
                                footer class="twig-pr-entry-tools" {
                                    (edit_details(namespace, repo, issue))
                                }
                            }
                        }
                        @for (comment, name) in &timeline {
                            (comment_entry(namespace, repo, number, comment, name, viewer_id == Some(comment.author_user_id.as_str()), editing))
                        }
                    }
                    @if viewer.is_some() {
                        section class="twig-pr-composer" aria-label="Add a comment" {
                            form method="post" action=(format!("/{namespace}/{repo}/issues/{number}/comments")) class="twig-form" {
                                label class="twig-label" for="issue-comment" { "Comment" }
                                (markdown_field("issue-comment", "body", "Comment", "", 5, None))
                                div class="twig-form-actions" {
                                    button type="submit" class="twig-btn twig-btn--primary" { "Comment" }
                                }
                            }
                        }
                    } @else {
                        p class="twig-hint" { "Sign in to comment." }
                    }
                    @if viewer.is_some() {
                        (composer::scripts())
                    }
                }
                aside class="twig-issue-side" aria-label="Issue details" {
                    (assignees_panel(namespace, repo, number, &assignees, &owner_choices, can_push))
                    @if can_change_state {
                        section class="twig-issue-panel" aria-labelledby="issue-actions-heading" {
                            h3 class="twig-label" id="issue-actions-heading" { "Actions" }
                            form method="post" action=(format!("/{namespace}/{repo}/issues/{number}/state")) {
                                @if closed {
                                    button type="submit" name="action" value="reopen" class="twig-btn twig-btn--block" { "Reopen issue" }
                                } @else {
                                    button type="submit" name="action" value="close" class="twig-btn twig-btn--block" { "Close issue" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Namespace owners who are not assigned yet: the choices in the Assignees dropdown.
async fn unassigned_owners(
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
    assigned: &[String],
) -> Vec<String> {
    match auth_state.db().list_namespace_owners(namespace).await {
        Ok(owners) => owners
            .into_iter()
            .map(|owner| owner.username)
            .filter(|name| !assigned.contains(name))
            .collect(),
        Err(error) => {
            log::error!("Failed to load namespace owners for {namespace}: {error}");
            Vec::new()
        }
    }
}

/// One comment. Its author sees Edit and Delete; an open edit form replaces the text.
fn comment_entry(
    namespace: &str,
    repo: &str,
    number: i64,
    comment: &IssueComment,
    author: &str,
    own: bool,
    editing: Option<i64>,
) -> Markup {
    let row = comment.row_id;
    let posted = comment.created_at.split('T').next().unwrap_or_default();
    let is_editing = own && editing == Some(row);
    let page = format!("/{namespace}/{repo}/issues/{number}");
    html! {
        article class="twig-pr-entry" id=(format!("issuecomment-{row}")) {
            header class="twig-pr-entry-head twig-issue-comment-head" {
                div {
                    span class="twig-pr-entry-author" { (author) }
                    " commented on " (posted)
                    @if comment.updated_at.is_some() {
                        span class="twig-pr-entry-edited" { " · edited" }
                    }
                }
                @if own && !is_editing {
                    details class="twig-issue-menu" {
                        summary class="twig-issue-menu-toggle" aria-label="Comment actions" { "…" }
                        div class="twig-issue-menu-panel" {
                            a class="twig-issue-menu-item" href=(format!("{page}?edit={row}#issuecomment-{row}")) { "Edit" }
                            form method="post" action=(format!("{page}/comments/{row}/delete")) {
                                button type="submit" class="twig-issue-menu-item twig-issue-menu-item--danger" { "Delete" }
                            }
                        }
                    }
                }
            }
            @if is_editing {
                div class="twig-pr-entry-body" {
                    form method="post" action=(format!("{page}/comments/{row}/edit")) class="twig-form" {
                        (markdown_field(&format!("issue-comment-{row}"), "body", "Comment text", &comment.body, 5, None))
                        div class="twig-form-actions" {
                            button type="submit" class="twig-btn twig-btn--primary" { "Save" }
                            a class="twig-btn" href=(format!("{page}#issuecomment-{row}")) { "Cancel" }
                        }
                    }
                }
            } @else {
                div class="twig-pr-entry-body" {
                    (PreEscaped(md::render_untrusted_markdown(&comment.body)))
                }
            }
        }
    }
}

/// The title and description form, folded into a disclosure under the description.
/// It is shown only to an author or a pusher.
fn edit_details(namespace: &str, repo: &str, issue: &Issue) -> Markup {
    let number = issue.number;
    html! {
        details class="twig-pr-edit" {
            summary { "Edit title and description" }
            form method="post" action=(format!("/{namespace}/{repo}/issues/{number}/edit")) class="twig-form" {
                label class="twig-label" for="edit-title" { "Title" }
                input id="edit-title" class="twig-input" name="title" type="text" value=(issue.title) required;
                label class="twig-label" for="edit-body" { "Description" }
                (markdown_field("edit-body", "body", "Description", &issue.body, 8, None))
                div class="twig-form-actions" {
                    button type="submit" class="twig-btn twig-btn--primary" { "Save" }
                }
            }
        }
    }
}

/// The assignees, with Remove for people who can push. The Assign dropdown lists the
/// namespace's owners who are not assigned yet.
fn assignees_panel(
    namespace: &str,
    repo: &str,
    number: i64,
    assignees: &[String],
    owner_choices: &[String],
    can_push: bool,
) -> Markup {
    let action = format!("/{namespace}/{repo}/issues/{number}/assignees");
    html! {
        section class="twig-issue-panel" aria-labelledby="issue-assignees-heading" {
            h3 class="twig-label" id="issue-assignees-heading" { "Assignees" }
            @if assignees.is_empty() {
                p class="twig-hint" { "No one is assigned." }
            } @else {
                ul class="twig-issue-people" {
                    @for name in assignees {
                        li class="twig-issue-person" {
                            span class="twig-mono" { (name) }
                            @if can_push {
                                form method="post" action=(action.as_str()) {
                                    input type="hidden" name="action" value="remove";
                                    input type="hidden" name="username" value=(name);
                                    button type="submit" class="twig-btn twig-btn--quiet" { "Remove" }
                                }
                            }
                        }
                    }
                }
            }
            @if can_push && !owner_choices.is_empty() {
                form method="post" action=(action.as_str()) class="twig-form" {
                    input type="hidden" name="action" value="add";
                    label class="twig-sr" for="issue-assignee" { "Assign an owner" }
                    select id="issue-assignee" name="username" class="twig-input twig-input--mono" {
                        @for name in owner_choices {
                            option value=(name) { (name) }
                        }
                    }
                    button type="submit" class="twig-btn" { "Assign" }
                }
            }
        }
    }
}
