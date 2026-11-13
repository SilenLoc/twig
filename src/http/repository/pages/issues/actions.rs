//! POST handlers for one issue: comments, their edit and delete, and close or reopen.
//! Each one checks the session and the rules in `crate::issues`, then redirects to the
//! issue page, so a refresh never repeats the change.

use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::Deserialize;

use crate::{
    auth::TwigContext,
    config,
    db::issues::{Issue, IssueState},
    issues::{self, Actor, IssueChange, IssueError, RepoRef},
};

use super::{Writer, issue_error_response, see_other, writer_for};

#[derive(Debug, Deserialize)]
pub(super) struct IssuePath {
    namespace: String,
    repo: String,
    number: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct CommentPath {
    namespace: String,
    repo: String,
    number: i64,
    id: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct BodyForm {
    #[serde(default)]
    body: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct StateForm {
    action: String,
}

fn location(namespace: &str, repo: &str, number: i64) -> String {
    format!("/{namespace}/{repo}/issues/{number}")
}

/// The signed-in viewer and the issue a POST addresses.
async fn target(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
    repo: &str,
    number: i64,
) -> Result<(Writer, Issue), HttpResponse> {
    let writer = writer_for(req, server, auth_state, namespace, repo).await?;
    let scope = RepoRef {
        repo_id: &writer.repo_id,
        namespace,
    };
    let issue = issues::get(auth_state.db(), &scope, number)
        .await
        .map_err(issue_error_response)?;
    Ok((writer, issue))
}

/// `POST /{namespace}/{repo}/issues/{number}/comments`
#[post("/{namespace}/{repo}/issues/{number:\\d+}/comments")]
pub async fn comment_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    form: web::Form<BodyForm>,
) -> HttpResponse {
    let (writer, issue) = match target(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(target) => target,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &writer.user.id,
    };
    match issues::add_comment(
        auth_state.db(),
        &issue,
        &actor,
        &writer.permissions,
        &form.body,
    )
    .await
    {
        Ok(_) => see_other(&location(&path.namespace, &path.repo, path.number)),
        Err(error) => issue_error_response(error),
    }
}

/// `POST /{namespace}/{repo}/issues/{number}/comments/{id}/edit`
#[post("/{namespace}/{repo}/issues/{number:\\d+}/comments/{id:\\d+}/edit")]
pub async fn edit_comment_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CommentPath>,
    form: web::Form<BodyForm>,
) -> HttpResponse {
    let (writer, issue) = match target(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(target) => target,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &writer.user.id,
    };
    match issues::edit_comment(
        auth_state.db(),
        &issue,
        path.id,
        &actor,
        &writer.permissions,
        &form.body,
    )
    .await
    {
        Ok(_) => see_other(&location(&path.namespace, &path.repo, path.number)),
        Err(error) => issue_error_response(error),
    }
}

/// `POST /{namespace}/{repo}/issues/{number}/comments/{id}/delete`
#[post("/{namespace}/{repo}/issues/{number:\\d+}/comments/{id:\\d+}/delete")]
pub async fn delete_comment_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CommentPath>,
) -> HttpResponse {
    let (writer, issue) = match target(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(target) => target,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &writer.user.id,
    };
    match issues::delete_comment(
        auth_state.db(),
        &issue,
        path.id,
        &actor,
        &writer.permissions,
    )
    .await
    {
        Ok(()) => see_other(&location(&path.namespace, &path.repo, path.number)),
        Err(error) => issue_error_response(error),
    }
}

/// `POST /{namespace}/{repo}/issues/{number}/state` with `action=close` or `action=reopen`
#[post("/{namespace}/{repo}/issues/{number:\\d+}/state")]
pub async fn state_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    form: web::Form<StateForm>,
) -> HttpResponse {
    let state = match form.action.as_str() {
        "close" => IssueState::Closed,
        "reopen" => IssueState::Open,
        _ => {
            return issue_error_response(IssueError::Invalid(
                "Unknown action for an issue".to_string(),
            ));
        }
    };
    let (writer, issue) = match target(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(target) => target,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &writer.user.id,
    };
    let scope = RepoRef {
        repo_id: &writer.repo_id,
        namespace: &path.namespace,
    };
    match issues::change(
        auth_state.db(),
        &scope,
        &issue,
        &actor,
        &writer.permissions,
        IssueChange {
            state: Some(state),
            ..Default::default()
        },
    )
    .await
    {
        Ok(_) => see_other(&location(&path.namespace, &path.repo, path.number)),
        Err(error) => issue_error_response(error),
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct IssueEditForm {
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct AssigneeForm {
    action: String,
    #[serde(default)]
    username: String,
}

/// `POST /{namespace}/{repo}/issues/{number}/edit` with `title` and `body`
#[post("/{namespace}/{repo}/issues/{number:\\d+}/edit")]
pub async fn edit_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    form: web::Form<IssueEditForm>,
) -> HttpResponse {
    let (writer, issue) = match target(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(target) => target,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &writer.user.id,
    };
    let scope = RepoRef {
        repo_id: &writer.repo_id,
        namespace: &path.namespace,
    };
    match issues::change(
        auth_state.db(),
        &scope,
        &issue,
        &actor,
        &writer.permissions,
        IssueChange {
            title: Some(form.title.clone()),
            body: Some(form.body.clone()),
            ..Default::default()
        },
    )
    .await
    {
        Ok(_) => see_other(&location(&path.namespace, &path.repo, path.number)),
        Err(error) => issue_error_response(error),
    }
}

/// `POST /{namespace}/{repo}/issues/{number}/assignees` with `action=add|remove` and `username`
#[post("/{namespace}/{repo}/issues/{number:\\d+}/assignees")]
pub async fn assignees_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    form: web::Form<AssigneeForm>,
) -> HttpResponse {
    let (writer, issue) = match target(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        &path.repo,
        path.number,
    )
    .await
    {
        Ok(target) => target,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &writer.user.id,
    };
    let scope = RepoRef {
        repo_id: &writer.repo_id,
        namespace: &path.namespace,
    };
    let current = match current_assignee_names(auth_state.db(), &issue).await {
        Ok(names) => names,
        Err(error) => return issue_error_response(IssueError::Internal(error)),
    };
    let username = form.username.trim().to_string();
    let next: Vec<String> = match form.action.as_str() {
        "add" => {
            let mut names = current;
            if !names.contains(&username) {
                names.push(username);
            }
            names
        }
        "remove" => current
            .into_iter()
            .filter(|name| *name != username)
            .collect(),
        _ => {
            return issue_error_response(IssueError::Invalid(
                "Unknown assignee action".to_string(),
            ));
        }
    };
    match issues::change(
        auth_state.db(),
        &scope,
        &issue,
        &actor,
        &writer.permissions,
        IssueChange {
            assignees: Some(next),
            ..Default::default()
        },
    )
    .await
    {
        Ok(_) => see_other(&location(&path.namespace, &path.repo, path.number)),
        Err(error) => issue_error_response(error),
    }
}

/// The usernames of an issue's assignees, in the order they were assigned. An account
/// that no longer exists is left out.
pub(super) async fn current_assignee_names(
    db: &crate::db::Database,
    issue: &Issue,
) -> Result<Vec<String>, String> {
    let mut names = Vec::with_capacity(issue.assignee_user_ids.len());
    for user_id in &issue.assignee_user_ids {
        if let Some(user) = db.get_user_by_id(user_id).await? {
            names.push(user.username);
        }
    }
    Ok(names)
}
