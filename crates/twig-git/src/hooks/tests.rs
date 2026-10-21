use super::*;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("twig_hook_inventory_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn make_executable(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

fn init_bare_repo(path: &Path) {
    fs::create_dir_all(path).unwrap();
    let output = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .current_dir(path)
        .output()
        .expect("initialize bare repository");
    assert!(output.status.success(), "{output:?}");
    crate::repo::install_pre_receive_hook(path).expect("install managed hook");
}

fn run_hook(path: &Path, updates: &str, actor: &str, extra_env: &[(&str, &str)]) -> Output {
    let mut child = Command::new(path.join("hooks/pre-receive"))
        .current_dir(path)
        .env("GIT_DIR", path)
        .env("TWIG_NAMESPACE_ROLE", "owner")
        .env("REMOTE_USER", actor)
        .envs(extra_env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run managed pre-receive hook");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(updates.as_bytes())
        .unwrap();
    child.wait_with_output().expect("wait for managed hook")
}

#[test]
fn inspection_reports_missing_managed_hook_and_empty_chain() {
    let repository = TestDirectory::new();
    let inventory = inspect_repository_hooks(repository.path()).unwrap();

    assert_eq!(inventory.managed_state, ManagedHookState::Missing);
    assert!(inventory.custom_hooks.is_empty());
    assert!(inventory.recent_runs.is_empty());
}

#[cfg(unix)]
#[test]
fn inspection_reports_managed_health_and_custom_hook_source_and_state() {
    let repository = TestDirectory::new();
    let hooks = repository.path().join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    let managed = hooks.join("pre-receive");
    fs::write(
        &managed,
        "#!/bin/sh\n# TWIG_MANAGED_PRE_RECEIVE_V1\nexit 0\n",
    )
    .unwrap();
    make_executable(&managed, 0o755);

    let enabled = hooks.join("pre-receive.twig-user");
    fs::write(&enabled, "#!/bin/sh\necho custom\n").unwrap();
    make_executable(&enabled, 0o750);

    let disabled = hooks.join("pre-receive.twig-user.1");
    fs::write(&disabled, "#!/bin/sh\necho disabled\n").unwrap();
    make_executable(&disabled, 0o640);

    let inventory = inspect_repository_hooks(repository.path()).unwrap();
    assert_eq!(inventory.managed_state, ManagedHookState::Healthy);
    assert_eq!(inventory.custom_hooks.len(), 2);
    assert_eq!(inventory.custom_hooks[0].name, "pre-receive.twig-user");
    assert_eq!(inventory.custom_hooks[0].state, CustomHookState::Enabled);
    assert!(
        inventory.custom_hooks[0]
            .source
            .as_deref()
            .unwrap()
            .contains("echo custom")
    );
    assert_eq!(inventory.custom_hooks[1].name, "pre-receive.twig-user.1");
    assert_eq!(inventory.custom_hooks[1].state, CustomHookState::Disabled);
}

#[cfg(unix)]
#[test]
fn inspection_never_follows_a_custom_hook_symlink() {
    use std::os::unix::fs::symlink;

    let repository = TestDirectory::new();
    let hooks = repository.path().join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    let outside = repository.path().join("outside-hook");
    fs::write(&outside, "SECRET HOOK SOURCE\n").unwrap();
    symlink(&outside, hooks.join("pre-receive.twig-user")).unwrap();

    let inventory = inspect_repository_hooks(repository.path()).unwrap();
    assert_eq!(inventory.custom_hooks.len(), 1);
    assert_eq!(inventory.custom_hooks[0].state, CustomHookState::Unsafe);
    assert!(inventory.custom_hooks[0].source.is_none());
    assert!(
        !inventory
            .warnings
            .iter()
            .any(|warning| warning.contains("SECRET HOOK SOURCE"))
    );
}

#[cfg(unix)]
#[test]
fn inspection_rejects_a_symlinked_hooks_directory() {
    use std::os::unix::fs::symlink;

    let repository = TestDirectory::new();
    let outside = repository.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("pre-receive"), "TWIG_MANAGED_PRE_RECEIVE_V1").unwrap();
    symlink(&outside, repository.path().join("hooks")).unwrap();

    let inventory = inspect_repository_hooks(repository.path()).unwrap();
    assert_eq!(inventory.managed_state, ManagedHookState::Invalid);
    assert!(inventory.custom_hooks.is_empty());
    assert!(!inventory.warnings.is_empty());
}

#[cfg(unix)]
#[test]
fn disable_custom_hook_clears_execute_bits_without_changing_source() {
    use std::os::unix::fs::PermissionsExt;

    let repository = TestDirectory::new();
    let hooks = repository.path().join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    let custom_hook = hooks.join("pre-receive.twig-user");
    let source = "#!/bin/sh\necho preserve this source\n";
    fs::write(&custom_hook, source).unwrap();
    make_executable(&custom_hook, 0o750);

    disable_custom_hook(repository.path(), "pre-receive.twig-user").unwrap();

    assert_eq!(fs::read_to_string(&custom_hook).unwrap(), source);
    assert_eq!(
        fs::metadata(&custom_hook).unwrap().permissions().mode() & 0o111,
        0
    );
    let inventory = inspect_repository_hooks(repository.path()).unwrap();
    assert_eq!(inventory.custom_hooks[0].state, CustomHookState::Disabled);
}

#[test]
fn delete_custom_hook_only_removes_the_validated_sidecar() {
    let repository = TestDirectory::new();
    let hooks = repository.path().join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    let managed = hooks.join("pre-receive");
    let custom = hooks.join("pre-receive.twig-user.1");
    let unrelated = hooks.join("post-update");
    fs::write(&managed, "managed policy").unwrap();
    fs::write(&custom, "custom policy").unwrap();
    fs::write(&unrelated, "unrelated hook").unwrap();

    delete_custom_hook(repository.path(), "pre-receive.twig-user.1").unwrap();

    assert!(!custom.exists());
    assert_eq!(fs::read_to_string(managed).unwrap(), "managed policy");
    assert_eq!(fs::read_to_string(unrelated).unwrap(), "unrelated hook");
}

#[cfg(unix)]
#[test]
fn custom_hook_mutations_reject_symlinks_and_managed_hook_names() {
    use std::os::unix::fs::symlink;

    let repository = TestDirectory::new();
    let hooks = repository.path().join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    let outside = repository.path().join("outside");
    fs::write(&outside, "do not touch the target").unwrap();
    symlink(&outside, hooks.join("pre-receive.twig-user")).unwrap();

    assert!(matches!(
        disable_custom_hook(repository.path(), "pre-receive.twig-user"),
        Err(HookError::UnsafePath)
    ));
    assert!(matches!(
        delete_custom_hook(repository.path(), "pre-receive.twig-user"),
        Err(HookError::UnsafePath)
    ));
    assert!(matches!(
        delete_custom_hook(repository.path(), "pre-receive"),
        Err(HookError::InvalidName)
    ));
    assert_eq!(
        fs::read_to_string(outside).unwrap(),
        "do not touch the target"
    );
}

#[test]
fn hook_name_validation_matches_installer_sidecars() {
    for valid in [
        "pre-receive.twig-user",
        "pre-receive.twig-user.1",
        "pre-receive.twig-user.42",
    ] {
        assert!(validate_custom_hook_name(valid).is_ok(), "{valid}");
    }
    for invalid in [
        "pre-receive",
        "pre-receive.twig-user.0",
        "pre-receive.twig-user.backup",
        "pre-receive.twig-user/../outside",
        "../pre-receive.twig-user",
    ] {
        assert!(validate_custom_hook_name(invalid).is_err(), "{invalid}");
    }
}

#[cfg(unix)]
#[test]
fn custom_hook_run_records_metadata_and_replays_both_output_streams_and_exit_status() {
    let repository = TestDirectory::new();
    init_bare_repo(repository.path());
    let custom_hook = repository.path().join("hooks/pre-receive.twig-user");
    fs::write(
        &custom_hook,
        "#!/bin/sh\nprintf 'stdout diagnostic\\n'\nprintf 'stderr diagnostic\\n' >&2\nexit 23\n",
    )
    .unwrap();
    make_executable(&custom_hook, 0o750);
    let updates = format!("{} {} refs/heads/feature\n", "0".repeat(40), "1".repeat(40));

    let output = run_hook(repository.path(), &updates, "hook-user", &[]);

    assert_eq!(output.status.code(), Some(23));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "stdout diagnostic\n"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "stderr diagnostic\n"
    );
    let inventory = inspect_repository_hooks(repository.path()).unwrap();
    assert_eq!(inventory.recent_runs.len(), 1);
    let run = &inventory.recent_runs[0];
    assert_eq!(run.hook_name, "pre-receive.twig-user");
    assert_eq!(run.actor.as_deref(), Some("hook-user"));
    assert_eq!(run.ref_updates, vec![updates.trim().to_string()]);
    assert_eq!(run.status, HookRunStatus::Failed);
    assert_eq!(run.stdout, "stdout diagnostic\n");
    assert_eq!(run.stderr, "stderr diagnostic\n");
    assert!(!run.output_truncated);
}

#[cfg(unix)]
#[test]
fn recorded_output_is_capped_while_full_hook_output_is_replayed() {
    let repository = TestDirectory::new();
    init_bare_repo(repository.path());
    let custom_hook = repository.path().join("hooks/pre-receive.twig-user");
    fs::write(
        &custom_hook,
        "#!/bin/sh\ni=0\nwhile [ \"$i\" -lt 2000 ]; do printf '0123456789'; i=$((i + 1)); done\nexit 0\n",
    )
    .unwrap();
    make_executable(&custom_hook, 0o750);
    let updates = format!("{} {} refs/heads/feature\n", "0".repeat(40), "1".repeat(40));

    let output = run_hook(repository.path(), &updates, "hook-user", &[]);

    assert!(output.status.success());
    assert_eq!(
        output.stdout.len(),
        20_000,
        "the Git client gets full output"
    );
    let inventory = inspect_repository_hooks(repository.path()).unwrap();
    let run = inventory.recent_runs.first().unwrap();
    assert_eq!(run.stdout.len(), MAX_RECORDED_OUTPUT_BYTES);
    assert!(run.output_truncated);
}

#[cfg(unix)]
#[test]
fn ref_updates_are_capped_and_marked_in_the_run_record() {
    let repository = TestDirectory::new();
    init_bare_repo(repository.path());
    let custom_hook = repository.path().join("hooks/pre-receive.twig-user");
    fs::write(&custom_hook, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&custom_hook, 0o750);
    let one_update = format!("{} {} refs/heads/feature\n", "0".repeat(40), "1".repeat(40));
    let updates = one_update.repeat(500);

    let output = run_hook(repository.path(), &updates, "hook-user", &[]);

    assert!(output.status.success());
    let inventory = inspect_repository_hooks(repository.path()).unwrap();
    let run = inventory.recent_runs.first().unwrap();
    assert_eq!(run.ref_updates.last().unwrap(), "[Ref updates truncated.]");
    assert!(
        !run.output_truncated,
        "the output flag is separate from ref truncation"
    );
}

#[cfg(unix)]
#[test]
fn unsafe_run_history_symlink_does_not_change_hook_behavior_or_follow_target() {
    use std::os::unix::fs::symlink;

    let repository = TestDirectory::new();
    init_bare_repo(repository.path());
    let outside = repository.path().join("outside-run-history");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, repository.path().join("hooks/twig-runs")).unwrap();
    let custom_hook = repository.path().join("hooks/pre-receive.twig-user");
    fs::write(&custom_hook, "#!/bin/sh\necho fallback output\nexit 17\n").unwrap();
    make_executable(&custom_hook, 0o750);
    let updates = format!("{} {} refs/heads/feature\n", "0".repeat(40), "1".repeat(40));

    let output = run_hook(repository.path(), &updates, "hook-user", &[]);

    assert_eq!(output.status.code(), Some(17));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "fallback output\n");
    assert!(fs::read_dir(&outside).unwrap().next().is_none());
    let inventory = inspect_repository_hooks(repository.path()).unwrap();
    assert!(inventory.recent_runs.is_empty());
    assert!(
        inventory
            .warnings
            .iter()
            .any(|warning| warning.contains("history path"))
    );
}

#[cfg(unix)]
#[test]
fn run_history_retention_is_limited_to_the_recent_window() {
    let repository = TestDirectory::new();
    init_bare_repo(repository.path());
    let run_root = repository.path().join("hooks/twig-runs");
    fs::create_dir_all(&run_root).unwrap();
    for index in 0..MAX_RECENT_RUNS + 5 {
        let run = run_root.join(format!("run-0000000000-1-{index:03}-fixture"));
        fs::create_dir(&run).unwrap();
        fs::write(run.join("hook"), "pre-receive.twig-user\n").unwrap();
        fs::write(run.join("started_at"), "2026-10-21T00:00:00Z\n").unwrap();
        fs::write(run.join("actor"), "old-user\n").unwrap();
        fs::write(run.join("exit_status"), "0\n").unwrap();
        fs::write(run.join("refs"), "old refs\n").unwrap();
        fs::write(run.join("stdout"), "\n").unwrap();
        fs::write(run.join("stderr"), "\n").unwrap();
        fs::write(run.join("truncated"), "0\n").unwrap();
    }
    let custom_hook = repository.path().join("hooks/pre-receive.twig-user");
    fs::write(&custom_hook, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&custom_hook, 0o750);
    let updates = format!("{} {} refs/heads/feature\n", "0".repeat(40), "1".repeat(40));

    let output = run_hook(repository.path(), &updates, "hook-user", &[]);
    assert!(output.status.success());

    let retained = fs::read_dir(&run_root)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("run-"))
        .count();
    assert_eq!(retained, MAX_RECENT_RUNS);
    assert_eq!(
        inspect_repository_hooks(repository.path())
            .unwrap()
            .recent_runs
            .len(),
        MAX_RECENT_RUNS
    );
}

#[cfg(unix)]
#[test]
fn concurrent_custom_hook_runs_publish_distinct_records() {
    let repository = TestDirectory::new();
    init_bare_repo(repository.path());
    let custom_hook = repository.path().join("hooks/pre-receive.twig-user");
    fs::write(&custom_hook, "#!/bin/sh\nsleep 0.02\nexit 0\n").unwrap();
    make_executable(&custom_hook, 0o750);
    let updates = format!("{} {} refs/heads/feature\n", "0".repeat(40), "1".repeat(40));
    let repo_path = repository.path().to_path_buf();

    std::thread::scope(|scope| {
        let handles = (0..4)
            .map(|index| {
                let repo_path = repo_path.clone();
                let updates = updates.clone();
                scope.spawn(move || {
                    let actor = format!("hook-user-{index}");
                    run_hook(&repo_path, &updates, &actor, &[])
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            assert!(handle.join().unwrap().status.success());
        }
    });

    let inventory = inspect_repository_hooks(repository.path()).unwrap();
    assert_eq!(inventory.recent_runs.len(), 4);
    let ids = inventory
        .recent_runs
        .iter()
        .map(|run| run.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), 4);
}
