use super::*;
use std::io::Write;
use std::process::{Command, Output, Stdio};

use super::hooks::{PRE_RECEIVE_HOOK_MARKER, PREVIOUS_PRE_RECEIVE_HOOK_MARKER};

fn run_pre_receive(
    repo_path: &Path,
    may_rewrite_protected_branches: Option<bool>,
    updates: &str,
    extra_env: &[(&str, &str)],
) -> Output {
    let mut command = Command::new(repo_path.join("hooks/pre-receive"));
    command
        .current_dir(repo_path)
        .env("GIT_DIR", repo_path)
        .env_remove("TWIG_CAN_REWRITE_PROTECTED_BRANCHES")
        .envs(extra_env.iter().copied());
    if let Some(may_rewrite) = may_rewrite_protected_branches {
        command.env(
            "TWIG_CAN_REWRITE_PROTECTED_BRANCHES",
            if may_rewrite { "1" } else { "0" },
        );
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run managed pre-receive hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(updates.as_bytes())
        .unwrap();
    child.wait_with_output().expect("wait for receive hook")
}

#[test]
fn test_default_branch_is_main() {
    assert_eq!(default_branch(), "main");
}

#[test]
fn test_create_repo_creates_directory_structure() {
    let temp_root = format!("/tmp/test_twig_repo_{}", uuid::Uuid::new_v4());
    let result = create_repo(&temp_root, "myns", "myrepo", "main");
    assert!(result.is_ok(), "create_repo failed: {result:?}");

    let repo_path = Path::new(&temp_root).join("myns").join("myrepo");
    assert!(repo_path.exists(), "repo directory should exist");
    assert!(
        repo_path.join("HEAD").exists(),
        "bare repo HEAD should exist"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_root);
}

#[test]
fn test_bare_init_creates_valid_repository() {
    let temp_dir = format!("/tmp/test_twig_bare_init_{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&temp_dir).unwrap();

    let repo_path = Path::new(&temp_dir);
    let result = bare_init(repo_path, "main", "Test Author", "test@example.com");
    assert!(result.is_ok(), "bare_init failed: {result:?}");

    let git_dir = repo_path.join("HEAD");
    assert!(git_dir.exists(), "HEAD should exist after bare_init");
    assert!(repo_path.join("hooks/pre-receive").exists());

    // Verify git2 can open it
    let repo = git2::Repository::open(repo_path);
    assert!(repo.is_ok(), "repo should be openable by git2");

    // The initial commit's .twig.toml points editors at the TOML schema.
    let repo = repo.unwrap();
    let commit = repo.head().unwrap().peel_to_commit().unwrap();
    let entry = commit
        .tree()
        .unwrap()
        .get_path(Path::new(".twig.toml"))
        .unwrap();
    let blob = entry.to_object(&repo).unwrap().into_blob().unwrap();
    let content = std::str::from_utf8(blob.content()).unwrap();
    assert!(
        content.starts_with("#:schema https://twig.silenlocatelli.ch/assets/twig.schema.json"),
        "default .twig.toml should reference the schema, got: {content}"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn contributor_hook_allows_fast_forwards_and_rejects_main_rewrites_and_deletions() {
    let temp_dir = format!("/tmp/test_twig_receive_policy_{}", uuid::Uuid::new_v4());
    std::fs::create_dir_all(&temp_dir).unwrap();
    let repo_path = Path::new(&temp_dir);
    bare_init(repo_path, "main", "Test Author", "test@example.com").unwrap();
    let repo = git2::Repository::open(repo_path).unwrap();
    let base = repo.head().unwrap().peel_to_commit().unwrap();
    let tree = base.tree().unwrap();
    let signature = git2::Signature::now("Test Author", "test@example.com").unwrap();
    let fast_forward = repo
        .commit(
            None,
            &signature,
            &signature,
            "fast forward",
            &tree,
            &[&base],
        )
        .unwrap();
    let rewrite = repo
        .commit(None, &signature, &signature, "rewrite", &tree, &[])
        .unwrap();
    let zero_oid = "0".repeat(base.id().to_string().len());

    let allowed = run_pre_receive(
        repo_path,
        Some(false),
        &format!("{} {} refs/heads/main\n", base.id(), fast_forward),
        &[],
    );
    assert!(allowed.status.success());

    let force_push = run_pre_receive(
        repo_path,
        Some(false),
        &format!("{} {} refs/heads/main\n", base.id(), rewrite),
        &[],
    );
    assert!(!force_push.status.success());
    assert!(String::from_utf8_lossy(&force_push.stderr).contains("cannot rewrite history on main"));

    let deletion = run_pre_receive(
        repo_path,
        Some(false),
        &format!("{} {} refs/heads/main\n", base.id(), zero_oid),
        &[],
    );
    assert!(!deletion.status.success());
    assert!(String::from_utf8_lossy(&deletion.stderr).contains("cannot delete main"));

    let missing_permission = run_pre_receive(
        repo_path,
        None,
        &format!("{} {} refs/heads/main\n", base.id(), zero_oid),
        &[],
    );
    assert!(!missing_permission.status.success());
    assert!(String::from_utf8_lossy(&missing_permission.stderr).contains("cannot delete main"));

    let new_main = run_pre_receive(
        repo_path,
        Some(false),
        &format!("{zero_oid} {} refs/heads/main\n", base.id()),
        &[],
    );
    assert!(new_main.status.success());

    let owner_rewrite = run_pre_receive(
        repo_path,
        Some(true),
        &format!("{} {} refs/heads/main\n", base.id(), rewrite),
        &[],
    );
    assert!(owner_rewrite.status.success());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn receive_hook_installation_preserves_chains_and_is_idempotent() {
    let temp_dir = format!("/tmp/test_twig_hook_chain_{}", uuid::Uuid::new_v4());
    let repo_path = Path::new(&temp_dir);
    std::fs::create_dir_all(repo_path.join("hooks")).unwrap();
    let init_output = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .current_dir(repo_path)
        .output()
        .expect("initialize bare repository");
    assert!(init_output.status.success());

    let custom_hook = "#!/bin/sh\nprintf '%s\\n' \"${TWIG_CAN_REWRITE_PROTECTED_BRANCHES:-}\" > \"$TWIG_HOOK_PERMISSION_CAPTURE\"\nprintf '%s\\n' \"${REMOTE_USER:-}\" > \"$TWIG_HOOK_REMOTE_USER_CAPTURE\"\ncat > \"$TWIG_HOOK_INPUT_CAPTURE\"\nexit \"${TWIG_CUSTOM_HOOK_EXIT:-0}\"\n";
    let original_hook = repo_path.join("hooks/pre-receive");
    std::fs::write(&original_hook, custom_hook).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&original_hook, std::fs::Permissions::from_mode(0o750)).unwrap();
    }

    install_pre_receive_hook(repo_path).expect("install managed receive hook");
    let preserved = repo_path.join("hooks/pre-receive.twig-user");
    assert_eq!(std::fs::read_to_string(&preserved).unwrap(), custom_hook);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&preserved).unwrap().permissions().mode() & 0o777,
            0o750
        );
        assert_eq!(
            std::fs::metadata(&original_hook)
                .unwrap()
                .permissions()
                .mode()
                & 0o111,
            0o111
        );
    }

    install_pre_receive_hook(repo_path).expect("reinstall managed receive hook");
    let sidecars: Vec<_> = std::fs::read_dir(repo_path.join("hooks"))
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("pre-receive.twig-user")
        })
        .collect();
    assert_eq!(
        sidecars.len(),
        1,
        "repeated install does not duplicate hooks"
    );

    let input = format!("{} {} refs/heads/feature\n", "0".repeat(40), "1".repeat(40));
    let permission_capture = repo_path.join("permission.capture");
    let remote_user_capture = repo_path.join("remote-user.capture");
    let input_capture = repo_path.join("input.capture");
    let output = run_pre_receive(
        repo_path,
        Some(false),
        &input,
        &[
            ("REMOTE_USER", "hook-user"),
            (
                "TWIG_HOOK_PERMISSION_CAPTURE",
                permission_capture.to_str().unwrap(),
            ),
            (
                "TWIG_HOOK_REMOTE_USER_CAPTURE",
                remote_user_capture.to_str().unwrap(),
            ),
            ("TWIG_HOOK_INPUT_CAPTURE", input_capture.to_str().unwrap()),
        ],
    );
    assert!(output.status.success());
    assert_eq!(std::fs::read_to_string(permission_capture).unwrap(), "0\n");
    assert_eq!(
        std::fs::read_to_string(remote_user_capture).unwrap(),
        "hook-user\n"
    );
    assert_eq!(std::fs::read_to_string(input_capture).unwrap(), input);

    let rejection = run_pre_receive(
        repo_path,
        Some(true),
        &input,
        &[("TWIG_CUSTOM_HOOK_EXIT", "23")],
    );
    assert_eq!(rejection.status.code(), Some(23));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn receive_hook_installation_upgrades_previous_managed_hook_in_place() {
    let temp_dir = format!("/tmp/test_twig_hook_upgrade_{}", uuid::Uuid::new_v4());
    let repo_path = Path::new(&temp_dir);
    let hooks_dir = repo_path.join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    let hook_path = hooks_dir.join("pre-receive");
    let previous_hook =
        "#!/bin/sh\n# TWIG_MANAGED_PRE_RECEIVE_V1\nrole=${TWIG_NAMESPACE_ROLE:-contributor}\n";
    std::fs::write(&hook_path, previous_hook).unwrap();

    install_pre_receive_hook(repo_path).expect("upgrade previous managed receive hook");

    let installed_hook = std::fs::read_to_string(&hook_path).unwrap();
    assert!(installed_hook.contains(PRE_RECEIVE_HOOK_MARKER));
    assert!(!installed_hook.contains(PREVIOUS_PRE_RECEIVE_HOOK_MARKER));
    assert!(!hooks_dir.join("pre-receive.twig-user").exists());

    // Reinstallation recognizes V2 and does not create a sidecar or replace
    // the current managed hook again.
    install_pre_receive_hook(repo_path).expect("reinstall current managed receive hook");
    assert_eq!(std::fs::read_to_string(&hook_path).unwrap(), installed_hook);

    let _ = std::fs::remove_dir_all(&temp_dir);
}
