//! One issue's page: the title and state, the description, the conversation, and the
//! controls the signed-in viewer may use. A pull request's number redirects to its page.

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

use super::{author_name, error_response, html_response, see_other, signed_in};

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
    html! {
        div class="twig-stack" {
            header {
                h1 class="twig-pr-title" {
                    (issue.title) " "
                    span class="twig-mono" { "#" (number) }
                }
                span class="twig-pr-state" data-state=(key) { (label) }
                div class="twig-row-meta" {
                    "opened " (opened) " by " (author)
                }
            }
            section class="twig-panel" {
                div class="twig-panel-body" {
                    @if issue.body.trim().is_empty() {
                        p class="twig-hint" { "No description provided." }
                    } @else {
                        div class="twig-pr-entry-body" {
                            (PreEscaped(md::render_untrusted_markdown(&issue.body)))
                        }
                    }
                }
            }
            @if can_change_state {
                (edit_panel(namespace, repo, issue))
            }
            (assignees_panel(namespace, repo, number, &assignees, can_push))
            @for (comment, name) in &timeline {
                (comment_block(namespace, repo, number, comment, name, viewer_id == Some(comment.author_user_id.as_str()), editing))
            }
            @if viewer.is_some() {
                form class="twig-stack" method="post" action=(format!("/{namespace}/{repo}/issues/{number}/comments")) {
                    textarea name="body" rows="5" placeholder="Leave a comment" required {}
                    div class="twig-row-actions" {
                        button type="submit" class="twig-btn twig-btn--primary" { "Comment" }
                    }
                }
            } @else {
                p class="twig-hint" { "Sign in to comment." }
            }
            @if can_change_state {
                form method="post" action=(format!("/{namespace}/{repo}/issues/{number}/state")) {
                    @if closed {
                        button type="submit" name="action" value="reopen" class="twig-btn" { "Reopen issue" }
                    } @else {
                        button type="submit" name="action" value="close" class="twig-btn" { "Close issue" }
                    }
                }
            }
            a href=(format!("/{namespace}/{repo}/issues")) { "Back to issues" }
        }
    }
}

/// One comment. Its author sees Edit and Delete; an open edit form replaces the text.
fn comment_block(
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
    html! {
        section class="twig-panel" id=(format!("issuecomment-{row}")) {
            div class="twig-panel-body" {
                div class="twig-row-meta" {
                    strong { (author) } " commented " (posted)
                    @if comment.updated_at.is_some() { " · edited" }
                }
                @if is_editing {
                    form method="post" action=(format!("/{namespace}/{repo}/issues/{number}/comments/{row}/edit")) {
                        textarea name="body" rows="5" required { (comment.body) }
                        div class="twig-row-actions" {
                            button type="submit" class="twig-btn twig-btn--primary" { "Save" }
                        }
                    }
                } @else {
                    div class="twig-pr-entry-body" {
                        (PreEscaped(md::render_untrusted_markdown(&comment.body)))
                    }
                }
                @if own && !is_editing {
                    div class="twig-row-actions" {
                        a href=(format!("/{namespace}/{repo}/issues/{number}?edit={row}#issuecomment-{row}")) { "Edit" }
                        form method="post" action=(format!("/{namespace}/{repo}/issues/{number}/comments/{row}/delete")) {
                            button type="submit" class="twig-btn" { "Delete" }
                        }
                    }
                }
            }
        }
    }
}

/// The title and description form, for an author or a pusher.
fn edit_panel(namespace: &str, repo: &str, issue: &Issue) -> Markup {
    let number = issue.number;
    html! {
        details class="twig-panel" {
            summary { "Edit title and description" }
            form class="twig-stack" method="post" action=(format!("/{namespace}/{repo}/issues/{number}/edit")) {
                label for="edit-title" { "Title" }
                input id="edit-title" name="title" type="text" value=(issue.title) required;
                label for="edit-body" { "Description" }
                textarea id="edit-body" name="body" rows="8" { (issue.body) }
                div class="twig-row-actions" {
                    button type="submit" class="twig-btn twig-btn--primary" { "Save" }
                }
            }
        }
    }
}

/// The assignees, with Remove and Assign controls for people who can push.
fn assignees_panel(
    namespace: &str,
    repo: &str,
    number: i64,
    assignees: &[String],
    can_push: bool,
) -> Markup {
    let action = format!("/{namespace}/{repo}/issues/{number}/assignees");
    html! {
        section class="twig-panel" {
            div class="twig-panel-body" {
                div class="twig-row-meta" { "Assignees" }
                @if assignees.is_empty() {
                    p class="twig-hint" { "No one is assigned." }
                } @else {
                    ul {
                        @for name in assignees {
                            li {
                                span class="twig-mono" { (name) }
                                @if can_push {
                                    form method="post" action=(action.as_str()) {
                                        input type="hidden" name="action" value="remove";
                                        input type="hidden" name="username" value=(name);
                                        button type="submit" class="twig-btn" { "Remove" }
                                    }
                                }
                            }
                        }
                    }
                }
                @if can_push {
                    form method="post" action=(action.as_str()) {
                        input type="hidden" name="action" value="add";
                        label for="assign-user" { "Assign a member" }
                        input id="assign-user" name="username" type="text" required;
                        button type="submit" class="twig-btn" { "Assign" }
                    }
                }
            }
        }
    }
}
