//! Executes one CI job in isolation. The commit is exported into a fresh
//! directory, and `mise install` and `mise run ci` run there with a cleared
//! environment, a private `HOME`, and one shared time limit. Only the tool
//! caches are shared between runs; the checkout and the home directory are not.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::config::CiSettings;
use crate::db::{Database, ci_runs::RunStatus};
use crate::git::bare::RepoHandle;

/// Longest log kept for one run. Longer output keeps its tail, where the
/// failure usually is.
pub const MAX_LOG_BYTES: usize = 1024 * 1024;

const FALLBACK_PATH: &str = "/usr/local/bin:/usr/bin:/bin";
const CI_GIT_IDENTITY_NAME: &str = "Twig CI";
const CI_GIT_IDENTITY_EMAIL: &str = "ci@twig.invalid";

/// Everything a run needs, captured when it is queued.
pub struct Job {
    pub project_root: String,
    pub namespace: String,
    pub repo: String,
    pub run_id: i64,
    pub commit: String,
    pub branch: String,
    pub event: String,
    pub inputs: Vec<(String, String)>,
}

static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();

/// Starts the job in the background. It waits in the `waiting` state until one
/// of the server's run slots is free.
pub fn spawn(db: Database, settings: CiSettings, job: Job) {
    let slots = Arc::clone(SLOTS.get_or_init(|| Arc::new(Semaphore::new(settings.concurrency()))));
    tokio::spawn(async move {
        let Ok(_slot) = slots.acquire().await else {
            return;
        };
        if let Err(error) = db.mark_ci_run_started(job.run_id).await {
            log::error!("Failed to start CI run {}: {error}", job.run_id);
        }
        let (status, log) = execute(&settings, &job).await;
        if let Err(error) = db.finish_ci_run(job.run_id, status, &log).await {
            log::error!("Failed to record CI run {}: {error}", job.run_id);
        }
    });
}

pub(super) async fn execute(settings: &CiSettings, job: &Job) -> (RunStatus, String) {
    let deadline = Instant::now() + settings.timeout();
    let workspace = std::env::temp_dir().join(format!("twig-ci-{}", Uuid::new_v4()));
    let mut log = format!("$ checkout {} {}\n", job.commit, job.branch);
    let outcome = run_in_workspace(settings, job, &workspace, deadline, &mut log).await;

    let cleanup_target = workspace.clone();
    match tokio::task::spawn_blocking(move || std::fs::remove_dir_all(cleanup_target)).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => log::warn!("Failed to remove CI workspace: {error}"),
        Err(error) => log::warn!("CI workspace cleanup task failed: {error}"),
    }

    let status = match outcome {
        Ok(()) => RunStatus::Success,
        Err(message) => {
            log.push_str(&message);
            log.push('\n');
            RunStatus::Failure
        }
    };
    (status, cap_log(log))
}

async fn run_in_workspace(
    settings: &CiSettings,
    job: &Job,
    workspace: &Path,
    deadline: Instant,
    log: &mut String,
) -> Result<(), String> {
    let source = workspace.join("source");
    let home = workspace.join("home");
    let cache = settings.cache_dir();
    for dir in [
        &source,
        &home,
        &cache.join("mise"),
        &cache.join("cargo"),
        &cache.join("rustup"),
    ] {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("Failed to create CI directory: {error}"))?;
    }

    export_commit(job, &source).await?;

    let env = environment(settings, job, &source, &home);
    let config_dir = source.join(".twig");
    let config_dir = config_dir.display().to_string();
    for args in [
        vec!["-C", config_dir.as_str(), "install"],
        vec!["-C", config_dir.as_str(), "run", "ci"],
    ] {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let _ = writeln!(log, "$ {} {}\n", settings.program(), args.join(" "));
        let args: Vec<String> = args.into_iter().map(str::to_string).collect();
        match run_step(settings.program(), &args, &source, &env, remaining, log).await {
            Step::Exited(Some(0)) => {}
            Step::Exited(Some(code)) => {
                return Err(format!("mise exited with status {code}"));
            }
            Step::Exited(None) => return Err("mise was terminated by a signal".to_string()),
            Step::TimedOut => {
                return Err(format!(
                    "The run exceeded its time limit of {} seconds",
                    settings.timeout().as_secs()
                ));
            }
            Step::Failed(message) => return Err(message),
        }
    }
    Ok(())
}

async fn export_commit(job: &Job, destination: &Path) -> Result<(), String> {
    let project_root = job.project_root.clone();
    let namespace = job.namespace.clone();
    let repo = job.repo.clone();
    let commit = job.commit.clone();
    let destination = destination.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let handle = RepoHandle::open(&project_root, &namespace, &repo)
            .map_err(|error| format!("Failed to open repository: {error}"))?;
        let oid =
            git2::Oid::from_str(&commit).map_err(|error| format!("Invalid commit ID: {error}"))?;
        handle.export_commit(oid, &destination)
    })
    .await
    .map_err(|error| format!("Checkout task failed: {error}"))?
}

/// The environment of every CI step. Nothing from the server's environment
/// comes through except `PATH`, so server secrets never reach repository code.
/// Tool caches are shared; `HOME` and the checkout are not.
fn environment(
    settings: &CiSettings,
    job: &Job,
    source: &Path,
    home: &Path,
) -> Vec<(String, String)> {
    let cache = settings.cache_dir();
    let mut env = vec![
        (
            "PATH".to_string(),
            std::env::var("PATH").unwrap_or_else(|_| FALLBACK_PATH.to_string()),
        ),
        ("HOME".to_string(), home.display().to_string()),
        ("LANG".to_string(), "C.UTF-8".to_string()),
        ("CI".to_string(), "true".to_string()),
        // A clean HOME has no global Git identity, and commits made by the
        // checkout's own tools would otherwise fail to name an author.
        (
            "GIT_AUTHOR_NAME".to_string(),
            CI_GIT_IDENTITY_NAME.to_string(),
        ),
        (
            "GIT_AUTHOR_EMAIL".to_string(),
            CI_GIT_IDENTITY_EMAIL.to_string(),
        ),
        (
            "GIT_COMMITTER_NAME".to_string(),
            CI_GIT_IDENTITY_NAME.to_string(),
        ),
        (
            "GIT_COMMITTER_EMAIL".to_string(),
            CI_GIT_IDENTITY_EMAIL.to_string(),
        ),
        ("TWIG_CI".to_string(), "1".to_string()),
        ("TWIG_CI_EVENT".to_string(), job.event.clone()),
        (
            "TWIG_CI_REPOSITORY".to_string(),
            format!("{}/{}", job.namespace, job.repo),
        ),
        (
            "TWIG_CI_REF".to_string(),
            format!("refs/heads/{}", job.branch),
        ),
        ("TWIG_CI_SHA".to_string(), job.commit.clone()),
        ("MISE_YES".to_string(), "1".to_string()),
        // Stops mise before the repository root, so the root `mise.toml` (a
        // developer's config, not the CI definition) is never merged in.
        (
            "MISE_CEILING_PATHS".to_string(),
            source.display().to_string(),
        ),
        (
            "MISE_TRUSTED_CONFIG_PATHS".to_string(),
            source.display().to_string(),
        ),
        (
            "MISE_DATA_DIR".to_string(),
            cache.join("mise").join("data").display().to_string(),
        ),
        (
            "MISE_CACHE_DIR".to_string(),
            cache.join("mise").join("cache").display().to_string(),
        ),
        (
            "MISE_STATE_DIR".to_string(),
            cache.join("mise").join("state").display().to_string(),
        ),
        (
            "CARGO_HOME".to_string(),
            cache.join("cargo").display().to_string(),
        ),
        (
            "RUSTUP_HOME".to_string(),
            cache.join("rustup").display().to_string(),
        ),
    ];
    for (name, value) in &job.inputs {
        env.push((format!("TWIG_INPUT_{name}"), value.clone()));
    }
    env
}

enum Step {
    Exited(Option<i32>),
    TimedOut,
    Failed(String),
}

/// Runs one mise command, appending its output to `log`. The child leads its own
/// process group, so a timeout stops everything it started, not just mise.
async fn run_step(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &[(String, String)],
    limit: Duration,
    log: &mut String,
) -> Step {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(env.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Step::Failed(format!(
                "`{program}` was not found on the server's PATH. Install mise, or set TWIG_CI_PROGRAM."
            ));
        }
        Err(error) => return Step::Failed(format!("Failed to start `{program}`: {error}")),
    };
    let group = child.id();

    // Output is read while the process runs, keeping only each stream's newest bytes,
    // so a chatty task cannot grow the server's memory without bound.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let finished = async {
        tokio::join!(
            read_tail(stdout, MAX_LOG_BYTES),
            read_tail(stderr, MAX_LOG_BYTES),
            child.wait()
        )
    };
    let Ok((stdout, stderr, status)) = tokio::time::timeout(limit, finished).await else {
        if let Some(pid) = group {
            kill_process_group(pid).await;
        }
        return Step::TimedOut;
    };
    log.push_str(&String::from_utf8_lossy(&stdout));
    log.push_str(&String::from_utf8_lossy(&stderr));
    match status {
        Ok(status) => Step::Exited(status.code()),
        Err(error) => Step::Failed(format!("Failed to wait for `{program}`: {error}")),
    }
}

/// Reads a stream to its end, keeping only the newest `limit` bytes. The rest is
/// read and dropped, so the process never blocks on a full pipe.
async fn read_tail<R>(reader: Option<R>, limit: usize) -> Vec<u8>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut reader) = reader else {
        return Vec::new();
    };
    let mut kept = VecDeque::new();
    let mut chunk = vec![0u8; 8192];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                kept.extend(&chunk[..read]);
                while kept.len() > limit {
                    kept.pop_front();
                }
            }
        }
    }
    kept.into()
}

#[cfg(unix)]
async fn kill_process_group(pid: u32) {
    let _ = tokio::process::Command::new("kill")
        .arg("-KILL")
        .arg("--")
        .arg(format!("-{pid}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

#[cfg(not(unix))]
async fn kill_process_group(_pid: u32) {}

/// Keeps the newest `MAX_LOG_BYTES` of a log, cut on a character boundary.
pub(super) fn cap_log(log: String) -> String {
    if log.len() <= MAX_LOG_BYTES {
        return log;
    }
    let mut start = log.len() - MAX_LOG_BYTES;
    while !log.is_char_boundary(start) {
        start += 1;
    }
    format!("[earlier output truncated]\n{}", &log[start..])
}
