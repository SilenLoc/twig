use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use git2::Oid;
use uuid::Uuid;

use super::*;
use crate::config::CiSettings;
use crate::db::ci_runs::RunStatus;
use crate::git::bare::RepoHandle;

const NAMESPACE: &str = "ci-team";
const REPO: &str = "tooling";

/// A file in a test commit: repository path, contents, and Git mode.
type Entry<'a> = (&'a str, &'a str, i32);

fn build_tree(repo: &git2::Repository, entries: &[Entry<'_>]) -> Oid {
    let mut builder = repo.treebuilder(None).expect("tree builder");
    let mut directories: BTreeMap<&str, Vec<Entry<'_>>> = BTreeMap::new();
    for &(path, content, mode) in entries {
        if let Some((directory, rest)) = path.split_once('/') {
            directories
                .entry(directory)
                .or_default()
                .push((rest, content, mode));
        } else {
            let blob = repo.blob(content.as_bytes()).expect("blob");
            builder.insert(path, blob, mode).expect("insert file");
        }
    }
    for (directory, children) in directories {
        let tree = build_tree(repo, &children);
        builder
            .insert(directory, tree, 0o040_000)
            .expect("insert directory");
    }
    builder.write().expect("write tree")
}

/// A bare repository at `root/NAMESPACE/REPO` whose `main` has one commit with `entries`.
fn repository(entries: &[Entry<'_>]) -> (PathBuf, RepoHandle, Oid) {
    let root = std::env::temp_dir().join(format!("twig-ci-test-{}", Uuid::new_v4()));
    let path = root.join(NAMESPACE).join(REPO);
    std::fs::create_dir_all(&path).expect("repository directory");
    let repo = git2::Repository::init_bare(&path).expect("init bare repository");
    let tree = repo
        .find_tree(build_tree(&repo, entries))
        .expect("find tree");
    let signature = git2::Signature::now("Twig Test", "twig@example.com").expect("signature");
    let commit = repo
        .commit(
            Some("refs/heads/main"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )
        .expect("commit");
    drop(tree);
    drop(repo);
    let handle = RepoHandle::open(root.to_str().expect("UTF-8 path"), NAMESPACE, REPO)
        .expect("open repository");
    (root, handle, commit)
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn only_a_ci_task_in_twig_mise_toml_defines_ci() {
    let cases = [
        ("[tasks.ci]\nrun = \"true\"\n", true),
        ("[tasks.test]\nrun = \"true\"\n", false),
        ("[tools]\nrust = \"stable\"\n", false),
        ("not = = toml", false),
    ];
    for (content, expected) in cases {
        let (root, handle, commit) = repository(&[(DEFINITION_PATH, content, 0o100_644)]);
        assert_eq!(defines_ci_task(&handle, commit), expected, "{content}");
        cleanup(&root);
    }

    let (root, handle, commit) =
        repository(&[("mise.toml", "[tasks.ci]\nrun = \"true\"\n", 0o100_644)]);
    assert!(
        !defines_ci_task(&handle, commit),
        "the repository-root mise.toml is not the CI definition"
    );
    cleanup(&root);

    let (root, handle, commit) = repository(&[("README.md", "hello", 0o100_644)]);
    assert!(!defines_ci_task(&handle, commit), "no definition at all");
    cleanup(&root);
}

#[test]
fn export_writes_files_and_skips_links() {
    let (root, handle, commit) = repository(&[
        (DEFINITION_PATH, "[tasks.ci]\n", 0o100_644),
        ("scripts/run.sh", "#!/bin/sh\necho hi\n", 0o100_755),
        ("link", "scripts/run.sh", 0o120_000),
    ]);
    let destination = root.join("export");
    std::fs::create_dir_all(&destination).expect("destination");

    handle
        .export_commit(commit, &destination)
        .expect("export commit");

    assert_eq!(
        std::fs::read_to_string(destination.join(DEFINITION_PATH)).expect("definition"),
        "[tasks.ci]\n"
    );
    assert!(destination.join("scripts/run.sh").is_file());
    assert!(
        std::fs::symlink_metadata(destination.join("link")).is_err(),
        "symbolic links are not exported"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(destination.join("scripts/run.sh"))
            .expect("script metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "executable files stay executable");
    }
    cleanup(&root);
}

#[test]
fn inputs_must_be_identifiers() {
    let accepted = BTreeMap::from([
        ("TARGET".to_string(), "linux".to_string()),
        ("_retries".to_string(), "3".to_string()),
    ]);
    assert_eq!(
        validate_inputs(accepted).expect("valid inputs"),
        vec![
            ("TARGET".to_string(), "linux".to_string()),
            ("_retries".to_string(), "3".to_string()),
        ]
    );
    for name in ["", "1ST", "has-dash", "has space"] {
        let inputs = BTreeMap::from([(name.to_string(), "x".to_string())]);
        assert!(
            validate_inputs(inputs).is_err(),
            "{name:?} is not a valid name"
        );
    }
    let oversized = BTreeMap::from([("BIG".to_string(), "x".repeat(4097))]);
    assert!(validate_inputs(oversized).is_err());
}

#[test]
fn logs_keep_their_tail_within_the_limit() {
    let short = "short log\n".to_string();
    assert_eq!(runner::cap_log(short.clone()), short);

    let long = format!("{}END", "a".repeat(runner::MAX_LOG_BYTES + 100));
    let capped = runner::cap_log(long);
    assert!(capped.len() <= runner::MAX_LOG_BYTES + 64);
    assert!(capped.starts_with("[earlier output truncated]"));
    assert!(capped.ends_with("END"));
}

/// Writes an executable stand-in for `mise` that runs `body`.
fn fake_mise(directory: &Path, body: &str) -> String {
    let path = directory.join("fake-mise");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write fake mise");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake mise");
    }
    path.display().to_string()
}

fn settings(program: String, timeout: Duration, cache: &Path) -> CiSettings {
    CiSettings::enabled(program, timeout, 1, cache.join("cache"))
}

fn job(root: &Path, commit: Oid) -> runner::Job {
    runner::Job {
        project_root: root.display().to_string(),
        namespace: NAMESPACE.to_string(),
        repo: REPO.to_string(),
        run_id: 1,
        commit: commit.to_string(),
        branch: "main".to_string(),
        event: "push".to_string(),
        inputs: vec![("TARGET".to_string(), "linux".to_string())],
    }
}

#[tokio::test]
async fn a_run_executes_in_a_clean_checkout_with_only_its_own_environment() {
    let (root, _, commit) = repository(&[(DEFINITION_PATH, "[tasks.ci]\n", 0o100_644)]);
    let tools = root.join("tools");
    std::fs::create_dir_all(&tools).expect("tools dir");
    let program = fake_mise(
        &tools,
        "echo \"args: $*\"\necho \"cwd: $(pwd)\"\necho \"sha: $TWIG_CI_SHA input: $TWIG_INPUT_TARGET\"\n\
         env | sed 's/^/ENV:/'",
    );

    let (status, log) = runner::execute(
        &settings(program, Duration::from_secs(30), &tools),
        &job(&root, commit),
    )
    .await;

    assert_eq!(status, RunStatus::Success, "log:\n{log}");
    assert!(
        log.contains(&format!("sha: {commit} input: linux")),
        "log:\n{log}"
    );
    assert!(
        log.contains("args: -C ") && log.contains(" install"),
        "log:\n{log}"
    );
    assert!(log.contains(" run ci"), "log:\n{log}");
    assert!(
        log.contains("/source\n"),
        "commands run from the checkout, not the server: {log}"
    );

    let allowed = [
        "PATH",
        "HOME",
        "LANG",
        "CI",
        "GIT_AUTHOR_NAME",
        "GIT_AUTHOR_EMAIL",
        "GIT_COMMITTER_NAME",
        "GIT_COMMITTER_EMAIL",
        "TWIG_CI",
        "TWIG_CI_EVENT",
        "TWIG_CI_REPOSITORY",
        "TWIG_CI_REF",
        "TWIG_CI_SHA",
        "TWIG_INPUT_TARGET",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "PWD",
        "SHLVL",
        "_",
    ];
    let names: Vec<&str> = log
        .lines()
        .filter_map(|line| line.strip_prefix("ENV:"))
        .filter_map(|line| line.split('=').next())
        .collect();
    assert!(!names.is_empty(), "the environment was recorded: {log}");
    for name in names {
        assert!(
            allowed.contains(&name) || name.starts_with("MISE_"),
            "{name} leaked from the server environment"
        );
    }
    cleanup(&root);
}

#[tokio::test]
async fn a_failing_mise_step_fails_the_run_with_its_status() {
    let (root, _, commit) = repository(&[(DEFINITION_PATH, "[tasks.ci]\n", 0o100_644)]);
    let tools = root.join("tools");
    std::fs::create_dir_all(&tools).expect("tools dir");
    let program = fake_mise(&tools, "echo boom\nexit 3");

    let (status, log) = runner::execute(
        &settings(program, Duration::from_secs(30), &tools),
        &job(&root, commit),
    )
    .await;

    assert_eq!(status, RunStatus::Failure);
    assert!(log.contains("boom"), "output is kept: {log}");
    assert!(log.contains("mise exited with status 3"), "log:\n{log}");
    cleanup(&root);
}

#[tokio::test]
async fn a_run_over_its_time_limit_is_stopped_with_its_children() {
    let (root, _, commit) = repository(&[(DEFINITION_PATH, "[tasks.ci]\n", 0o100_644)]);
    let tools = root.join("tools");
    std::fs::create_dir_all(&tools).expect("tools dir");
    let program = fake_mise(&tools, "sleep 30");

    let started = Instant::now();
    let (status, log) = runner::execute(
        &settings(program, Duration::from_secs(1), &tools),
        &job(&root, commit),
    )
    .await;

    assert_eq!(status, RunStatus::Failure);
    assert!(log.contains("time limit"), "log:\n{log}");
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "the child process was killed, not awaited"
    );
    cleanup(&root);
}

#[tokio::test]
async fn a_missing_mise_fails_with_an_actionable_message() {
    let (root, _, commit) = repository(&[(DEFINITION_PATH, "[tasks.ci]\n", 0o100_644)]);
    let tools = root.join("tools");
    std::fs::create_dir_all(&tools).expect("tools dir");
    let program = tools.join("no-such-mise").display().to_string();

    let (status, log) = runner::execute(
        &settings(program, Duration::from_secs(30), &tools),
        &job(&root, commit),
    )
    .await;

    assert_eq!(status, RunStatus::Failure);
    assert!(log.contains("was not found"), "log:\n{log}");
    cleanup(&root);
}

#[tokio::test]
async fn a_chatty_run_keeps_only_the_newest_output() {
    let (root, _, commit) = repository(&[(DEFINITION_PATH, "[tasks.ci]\n", 0o100_644)]);
    let tools = root.join("tools");
    std::fs::create_dir_all(&tools).expect("tools dir");
    let program = fake_mise(
        &tools,
        "yes 'chatter chatter chatter' | head -c 4000000\necho FINAL",
    );

    let (status, log) = runner::execute(
        &settings(program, Duration::from_secs(60), &tools),
        &job(&root, commit),
    )
    .await;

    assert_eq!(status, RunStatus::Success);
    assert!(
        log.len() <= runner::MAX_LOG_BYTES + 64,
        "log is {} bytes",
        log.len()
    );
    assert!(
        log.starts_with("[earlier output truncated]"),
        "the oldest lines are the ones dropped"
    );
    assert!(log.ends_with("FINAL\n"), "the newest output is kept");
    cleanup(&root);
}
