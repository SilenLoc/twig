//! Continuous integration. A repository's `ci` task, defined in
//! `.twig/mise.toml`, runs for each branch push and each manual dispatch. No
//! git hook is involved: the push handler queues runs once it has accepted the
//! push, and each run executes in the background (see [`runner`]).

use std::collections::BTreeMap;

use actix_web::web;
use git2::Oid;

use crate::config::{self, CiSettings};
use crate::db::{
    Database,
    ci_runs::{CiRun, NewCiRun},
};
use crate::git::bare::RepoHandle;

pub mod runner;

/// The workflow name Forgejo clients use for CI, as in `fj actions dispatch ci`.
pub const WORKFLOW: &str = "ci";
/// Where a commit defines its CI: a mise config inside `.twig/`.
pub const DEFINITION_PATH: &str = ".twig/mise.toml";
const TASK: &str = "ci";
const MAX_INPUTS: usize = 32;
const MAX_INPUT_VALUE_BYTES: usize = 4096;

/// Whether `commit` defines the `ci` task in its `.twig/mise.toml`. A missing or
/// unparsable file defines nothing.
pub fn defines_ci_task(handle: &RepoHandle, commit: Oid) -> bool {
    let Ok(Some(bytes)) = handle.read_file_at_commit(commit, DEFINITION_PATH) else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return false;
    };
    let Ok(config) = text.parse::<toml::Table>() else {
        return false;
    };
    config
        .get("tasks")
        .and_then(toml::Value::as_table)
        .is_some_and(|tasks| tasks.contains_key(TASK))
}

/// Checks dispatch inputs and returns them as ordered pairs. Names become
/// `TWIG_INPUT_<NAME>` variables, so they must be plain identifiers.
pub fn validate_inputs(inputs: BTreeMap<String, String>) -> Result<Vec<(String, String)>, String> {
    if inputs.len() > MAX_INPUTS {
        return Err(format!("At most {MAX_INPUTS} inputs are allowed"));
    }
    for (name, value) in &inputs {
        let is_identifier = !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            && !name.as_bytes()[0].is_ascii_digit();
        if !is_identifier {
            return Err(format!(
                "Input name '{name}' must be letters, digits, or underscores"
            ));
        }
        if value.len() > MAX_INPUT_VALUE_BYTES {
            return Err(format!(
                "Input '{name}' is longer than {MAX_INPUT_VALUE_BYTES} bytes"
            ));
        }
    }
    Ok(inputs.into_iter().collect())
}

/// One run to queue.
pub struct NewRun<'a> {
    pub namespace: &'a str,
    pub repo: &'a str,
    pub repo_id: &'a str,
    pub event: &'a str,
    pub branch: &'a str,
    pub commit: &'a str,
    pub title: &'a str,
    pub user_id: Option<&'a str>,
    pub inputs: Vec<(String, String)>,
}

/// Records a run as waiting and starts it in the background.
pub async fn enqueue(
    settings: &CiSettings,
    db: &Database,
    project_root: &str,
    run: NewRun<'_>,
) -> Result<CiRun, String> {
    let stored = db
        .create_ci_run(NewCiRun {
            repo_id: run.repo_id,
            workflow: WORKFLOW,
            event: run.event,
            head_branch: run.branch,
            head_sha: run.commit,
            display_title: run.title,
            triggered_by_user_id: run.user_id,
        })
        .await?;
    runner::spawn(
        db.clone(),
        settings.clone(),
        runner::Job {
            project_root: project_root.to_string(),
            namespace: run.namespace.to_string(),
            repo: run.repo.to_string(),
            run_id: stored.id,
            commit: run.commit.to_string(),
            branch: run.branch.to_string(),
            event: run.event.to_string(),
            inputs: run.inputs,
        },
    );
    Ok(stored)
}

/// A branch a push updated, and the commit the push sent to it.
pub struct BranchUpdate {
    pub branch: String,
    pub commit: String,
}

/// Queues CI for the branch updates a push made. Runs in the background so a
/// push never waits on CI. A branch that no longer points at the pushed commit
/// (the push was rejected, or the branch moved on) and a commit that defines no
/// `ci` task are skipped.
pub fn queue_after_push(
    server: web::Data<config::Server>,
    db: Database,
    namespace: String,
    repo: String,
    user_id: String,
    updates: Vec<BranchUpdate>,
) {
    if !server.ci().is_enabled() || updates.is_empty() {
        return;
    }
    actix_web::rt::spawn(async move {
        let project_root = server.project_root().to_string();
        let settings = server.ci().clone();
        let prepared = {
            let (root, namespace, repo) = (project_root.clone(), namespace.clone(), repo.clone());
            tokio::task::spawn_blocking(move || prepare_push(&root, &namespace, &repo, &updates))
                .await
        };
        let prepared = match prepared {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(error)) => {
                log::warn!("Skipped CI for a push to {namespace}/{repo}: {error}");
                return;
            }
            Err(error) => {
                log::error!("CI push preparation failed: {error}");
                return;
            }
        };
        for push in prepared {
            let result = enqueue(
                &settings,
                &db,
                &project_root,
                NewRun {
                    namespace: &namespace,
                    repo: &repo,
                    repo_id: &push.repo_id,
                    event: "push",
                    branch: &push.branch,
                    commit: &push.commit,
                    title: &push.title,
                    user_id: Some(&user_id),
                    inputs: Vec::new(),
                },
            )
            .await;
            if let Err(error) = result {
                log::error!("Failed to queue CI run for {namespace}/{repo}: {error}");
            }
        }
    });
}

struct PreparedPush {
    repo_id: String,
    branch: String,
    commit: String,
    title: String,
}

fn prepare_push(
    project_root: &str,
    namespace: &str,
    repo: &str,
    updates: &[BranchUpdate],
) -> Result<Vec<PreparedPush>, String> {
    let handle = RepoHandle::open(project_root, namespace, repo)
        .map_err(|error| format!("failed to open repository: {error}"))?;
    let repo_id = handle
        .repo_id()
        .map_err(|error| format!("failed to read repository ID: {error}"))?;
    let mut prepared = Vec::new();
    for update in updates {
        let Ok(commit) = Oid::from_str(&update.commit) else {
            continue;
        };
        if !matches!(handle.branch_tip(&update.branch), Ok(Some(tip)) if tip == commit) {
            continue;
        }
        if !defines_ci_task(&handle, commit) {
            continue;
        }
        let title = handle
            .commit_summary(commit)
            .map_or_else(|_| update.branch.clone(), |summary| summary.summary);
        prepared.push(PreparedPush {
            repo_id: repo_id.clone(),
            branch: update.branch.clone(),
            commit: update.commit.clone(),
            title,
        });
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests;
