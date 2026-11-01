//! Forgejo Actions routes that `fj actions` uses: listing tasks, dispatching a
//! workflow, and reading back commit statuses. Twig has one workflow, `ci`,
//! which runs the `ci` task in `.twig/mise.toml` (see [`crate::ci`]). Runs are
//! off unless the server enables them; with CI off, the lists stay empty.

use std::collections::BTreeMap;

use actix_web::{HttpRequest, HttpResponse, get, http::StatusCode, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{NamespacePermission, TwigContext},
    ci, config,
    db::{
        Database,
        ci_runs::{CiRun, RunStatus},
    },
};

use super::{RepoPath, authorized_repo, compat_error, origin, query_number};

const DEFAULT_PAGE_SIZE: u32 = 20;
const MAX_PAGE_SIZE: u32 = 50;
/// The context a CI commit status is reported under.
const STATUS_CONTEXT: &str = "ci";

#[derive(Debug, Deserialize)]
pub struct TaskPath {
    pub owner: String,
    pub repo: String,
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct DispatchPath {
    pub owner: String,
    pub repo: String,
    pub workflow: String,
}

/// The body of `POST .../actions/workflows/{workflow}/dispatches`.
#[derive(Debug, Deserialize)]
pub struct DispatchBody {
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default)]
    pub inputs: BTreeMap<String, String>,
}

/// Forgejo's `ActionTask`. Twig's runs are the tasks; `url` is the run's log.
#[derive(Debug, Serialize)]
struct ActionTaskJson {
    id: i64,
    name: &'static str,
    display_title: String,
    status: &'static str,
    event: String,
    head_branch: String,
    head_sha: String,
    run_number: i64,
    workflow_id: &'static str,
    url: String,
    created_at: String,
    updated_at: String,
    run_started_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct ActionTaskListJson {
    total_count: i64,
    workflow_runs: Vec<ActionTaskJson>,
}

/// Forgejo's `CommitStatus`, as CI reports it.
#[derive(Debug, Serialize)]
pub struct CommitStatusJson {
    id: i64,
    context: &'static str,
    state: &'static str,
    description: String,
    target_url: String,
    created_at: String,
    updated_at: String,
}

fn task_status(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Waiting => "waiting",
        RunStatus::Running => "running",
        RunStatus::Success => "success",
        RunStatus::Failure => "failure",
    }
}

fn status_state(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Waiting | RunStatus::Running => "pending",
        RunStatus::Success => "success",
        RunStatus::Failure => "failure",
    }
}

fn log_url(origin: &str, owner: &str, repo: &str, id: i64) -> String {
    format!("{origin}/api/v1/repos/{owner}/{repo}/actions/tasks/{id}/log")
}

fn task_json(run: &CiRun, origin: &str, owner: &str, repo: &str) -> ActionTaskJson {
    ActionTaskJson {
        id: run.id,
        name: ci::WORKFLOW,
        display_title: run.display_title.clone(),
        status: task_status(run.status),
        event: run.event.clone(),
        head_branch: run.head_branch.clone(),
        head_sha: run.head_sha.clone(),
        run_number: run.run_number,
        workflow_id: ci::WORKFLOW,
        url: log_url(origin, owner, repo, run.id),
        created_at: run.created_at.clone(),
        updated_at: run
            .finished_at
            .clone()
            .or_else(|| run.started_at.clone())
            .unwrap_or_else(|| run.created_at.clone()),
        run_started_at: run.started_at.clone(),
    }
}

/// The statuses CI reports for one commit, newest first. Each run is one status.
pub async fn commit_statuses(
    db: &Database,
    repo_id: &str,
    sha: &str,
    origin: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<CommitStatusJson>, String> {
    let runs = db.list_ci_runs_for_sha(repo_id, sha).await?;
    Ok(runs
        .iter()
        .map(|run| CommitStatusJson {
            id: run.id,
            context: STATUS_CONTEXT,
            state: status_state(run.status),
            description: format!("Run #{} is {}", run.run_number, task_status(run.status)),
            target_url: log_url(origin, owner, repo, run.id),
            created_at: run.created_at.clone(),
            updated_at: run
                .finished_at
                .clone()
                .unwrap_or_else(|| run.created_at.clone()),
        })
        .collect())
}

/// The combined state of a commit's statuses: `failure` if any run failed,
/// `pending` while one is queued or running or when there are none, and
/// `success` when every run succeeded.
pub fn combined_state(statuses: &[CommitStatusJson]) -> &'static str {
    if statuses.iter().any(|status| status.state == "failure") {
        "failure"
    } else if statuses.is_empty() || statuses.iter().any(|status| status.state == "pending") {
        "pending"
    } else {
        "success"
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/actions/tasks?page=&limit=`
#[get("/api/v1/repos/{owner}/{repo}/actions/tasks")]
pub async fn list_tasks(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
) -> HttpResponse {
    let repo = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    let repo_id = match repo.handle.repo_id() {
        Ok(id) => id,
        Err(error) => return internal(&error.to_string()),
    };
    let limit = query_number(&req, "limit")
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let page = query_number(&req, "page").unwrap_or(1).max(1);
    let offset = page.saturating_sub(1).saturating_mul(limit);
    match auth_state.db().list_ci_runs(&repo_id, limit, offset).await {
        Ok((runs, total)) => {
            let origin = origin(&req);
            HttpResponse::Ok().json(ActionTaskListJson {
                total_count: total,
                workflow_runs: runs
                    .iter()
                    .map(|run| task_json(run, &origin, &path.owner, &path.repo))
                    .collect(),
            })
        }
        Err(error) => internal(&error),
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/actions/tasks/{id}/log`: the run's log as
/// plain text. Twig's extension; Forgejo's API has no log route that clients share.
#[get("/api/v1/repos/{owner}/{repo}/actions/tasks/{id}/log")]
pub async fn task_log(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<TaskPath>,
) -> HttpResponse {
    let repo = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    let repo_id = match repo.handle.repo_id() {
        Ok(id) => id,
        Err(error) => return internal(&error.to_string()),
    };
    match auth_state.db().find_ci_run_log(&repo_id, path.id).await {
        Ok(Some(log)) => HttpResponse::Ok()
            .content_type("text/plain; charset=utf-8")
            .body(log),
        Ok(None) => not_found(),
        Err(error) => internal(&error),
    }
}

/// `POST /api/v1/repos/{owner}/{repo}/actions/workflows/{workflow}/dispatches`.
/// Queues a run of `ci` on a branch and answers `204`, as Forgejo does when the
/// caller does not ask for run details.
#[post("/api/v1/repos/{owner}/{repo}/actions/workflows/{workflow}/dispatches")]
pub async fn dispatch_workflow(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<DispatchPath>,
    body: web::Json<DispatchBody>,
) -> HttpResponse {
    let repo = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    if path.workflow != ci::WORKFLOW {
        return not_found();
    }
    if !server.ci().is_enabled() {
        return compat_error(StatusCode::NOT_FOUND, "CI is not enabled on this server");
    }
    let body = body.into_inner();
    let branch = body
        .reference
        .strip_prefix("refs/heads/")
        .unwrap_or(&body.reference);
    let Ok(Some(tip)) = repo.handle.branch_tip(branch) else {
        return compat_error(StatusCode::UNPROCESSABLE_ENTITY, "The ref does not exist");
    };
    if !ci::defines_ci_task(&repo.handle, tip) {
        return compat_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!(
                "No ci task is defined in {} at this ref",
                ci::DEFINITION_PATH
            ),
        );
    }
    let inputs = match ci::validate_inputs(body.inputs) {
        Ok(inputs) => inputs,
        Err(message) => return compat_error(StatusCode::UNPROCESSABLE_ENTITY, &message),
    };
    let repo_id = match repo.handle.repo_id() {
        Ok(id) => id,
        Err(error) => return internal(&error.to_string()),
    };
    let title = repo
        .handle
        .commit_summary(tip)
        .map_or_else(|_| branch.to_string(), |summary| summary.summary);
    let commit = tip.to_string();
    let result = ci::enqueue(
        server.ci(),
        auth_state.db(),
        server.project_root(),
        ci::NewRun {
            namespace: &path.owner,
            repo: &path.repo,
            repo_id: &repo_id,
            event: "workflow_dispatch",
            branch,
            commit: &commit,
            title: &title,
            user_id: Some(&repo.principal.user.id),
            inputs,
        },
    )
    .await;
    match result {
        Ok(_) => HttpResponse::NoContent().finish(),
        Err(error) => internal(&error),
    }
}

fn not_found() -> HttpResponse {
    compat_error(StatusCode::NOT_FOUND, "The target couldn't be found")
}

fn internal(detail: &str) -> HttpResponse {
    log::error!("Compatibility Actions request failed: {detail}");
    compat_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "The request could not be completed",
    )
}
