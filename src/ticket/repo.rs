//! Creation of the per-namespace ticket repository.
//!
//! The ticket repository is server-managed. `refs/heads/main` is only ever
//! advanced by the ingest pipeline, never directly by a client push, which is
//! what allows concurrent writers to be merged without the user ever seeing a
//! rejected push or a conflict marker.
//!
//! Two hooks enforce that:
//!
//! * `pre-receive` refuses any client update to `refs/heads/*` and tells the
//!   user the one config line that makes ordinary `git push` work;
//! * `proc-receive` handles `refs/for/main`, which is always a *ref creation*
//!   from the client's point of view and therefore never trips git's
//!   client-side fast-forward check.

use std::path::{Path, PathBuf};

use xshell::cmd;

use crate::git::reserved::TICKET_REPO;

/// Schema version written into `.fig-tickets.toml` at creation.
pub const SCHEMA_VERSION: u32 = 1;

/// The canonical, server-owned branch.
pub const CANONICAL_REF: &str = "refs/heads/main";

/// The one-time client configuration that makes `git push` work transparently.
pub const PUSH_CONFIG_LINE: &str = "git config remote.origin.push HEAD:refs/for/main";

/// Path of the ticket repository for `namespace`.
pub fn ticket_repo_path(project_root: &str, namespace: &str) -> PathBuf {
    Path::new(project_root).join(namespace).join(TICKET_REPO)
}

/// Creates the ticket repository for `namespace` if it does not already exist.
///
/// Idempotent: an existing repository is left untouched apart from having its
/// hooks refreshed, so an upgraded server repairs older namespaces in place.
pub fn ensure_ticket_repo(
    project_root: &str,
    namespace: &str,
    author_name: &str,
    author_email: &str,
) -> Result<(), String> {
    let repo_path = ticket_repo_path(project_root, namespace);

    if repo_path.join("HEAD").exists() {
        // Already initialised. Refresh the hooks so a server upgrade repairs
        // namespaces created by an older version.
        return install_hooks(&repo_path);
    }

    let ns_path = Path::new(project_root).join(namespace);
    if !ns_path.exists() {
        std::fs::create_dir_all(&ns_path)
            .map_err(|e| format!("Failed to create namespace directory: {e}"))?;
    }
    std::fs::create_dir_all(&repo_path)
        .map_err(|e| format!("Failed to create ticket repo directory: {e}"))?;

    log::info!("Creating ticket repository for namespace '{namespace}'");

    init_bare(&repo_path, namespace, author_name, author_email)?;
    install_hooks(&repo_path)?;

    Ok(())
}

fn init_bare(
    repo_path: &Path,
    namespace: &str,
    author_name: &str,
    author_email: &str,
) -> Result<(), String> {
    let sh = xshell::Shell::new().map_err(|e| format!("Failed to create shell: {e}"))?;
    sh.change_dir(repo_path);

    cmd!(sh, "git init --bare --initial-branch=main")
        .ignore_stdout()
        .run()
        .map_err(|e| format!("Failed to init ticket repo: {e}"))?;

    cmd!(sh, "git config http.receivepack true")
        .run()
        .map_err(|e| format!("Failed to enable http.receivepack: {e}"))?;

    // Route pushes to the submit pseudo-ref through our ingest hook.
    cmd!(sh, "git config receive.procReceiveRefs refs/for")
        .run()
        .map_err(|e| format!("Failed to set receive.procReceiveRefs: {e}"))?;

    // Deletions are never a legitimate client operation here.
    cmd!(sh, "git config receive.denyDeletes true")
        .run()
        .map_err(|e| format!("Failed to set receive.denyDeletes: {e}"))?;

    let config = crate::ticket::model::render_repo_config(namespace, SCHEMA_VERSION, 1);
    let readme = render_readme(namespace);

    let config_blob = write_blob(&sh, &config)?;
    let readme_blob = write_blob(&sh, &readme)?;

    // Entries must be listed in git's sort order: '.' (0x2E) sorts before 'R'.
    let tree_entries = format!(
        "100644 blob {config_blob}\t.fig-tickets.toml\n100644 blob {readme_blob}\tREADME.md\n"
    );
    let tree = cmd!(sh, "git mktree")
        .stdin(tree_entries)
        .read()
        .map_err(|e| format!("Failed to create tree: {e}"))?;

    let commit = cmd!(
        sh,
        "git commit-tree {tree} -m 'Initialise ticket repository'"
    )
    .env("GIT_AUTHOR_NAME", author_name)
    .env("GIT_AUTHOR_EMAIL", author_email)
    .env("GIT_COMMITTER_NAME", author_name)
    .env("GIT_COMMITTER_EMAIL", author_email)
    .read()
    .map_err(|e| format!("Failed to create commit: {e}"))?;

    cmd!(sh, "git update-ref {CANONICAL_REF} {commit}")
        .run()
        .map_err(|e| format!("Failed to update ref: {e}"))?;

    Ok(())
}

fn write_blob(sh: &xshell::Shell, content: &str) -> Result<String, String> {
    cmd!(sh, "git hash-object -w --stdin")
        .stdin(content)
        .read()
        .map_err(|e| format!("Failed to write blob: {e}"))
}

/// Path of the `fig` executable, used to invoke the ingest subcommand from the
/// hook. Falls back to a bare `fig` resolved through `PATH`.
fn fig_binary() -> String {
    std::env::current_exe().map_or_else(|_| "fig".to_string(), |p| p.to_string_lossy().into_owned())
}

fn install_hooks(repo_path: &Path) -> Result<(), String> {
    let hooks_dir = repo_path.join("hooks");
    std::fs::create_dir_all(&hooks_dir)
        .map_err(|e| format!("Failed to create hooks directory: {e}"))?;

    let fig = fig_binary();

    write_hook(
        &hooks_dir.join("proc-receive"),
        &format!("#!/bin/sh\nexec {fig} ticket-ingest\n"),
    )?;
    write_hook(&hooks_dir.join("pre-receive"), PRE_RECEIVE_HOOK)?;

    Ok(())
}

fn write_hook(path: &Path, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents)
        .map_err(|e| format!("Failed to write hook {}: {e}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to make hook executable: {e}"))?;
    }

    Ok(())
}

/// Refuses direct client updates to server-owned refs and points the user at
/// the one-time configuration. Without this a fast-forward push straight to
/// `main` would bypass validation, identity normalisation and the merge.
const PRE_RECEIVE_HOOK: &str = r#"#!/bin/sh
# Fig ticket repository: refs/heads/* is server-owned.
status=0
while read -r _old _new ref; do
    case "$ref" in
        refs/for/*) ;;
        *)
            echo "" >&2
            echo "fig: '$ref' is managed by the server and cannot be pushed to directly." >&2
            echo "fig: run this once in your clone, then use 'git push' as normal:" >&2
            echo "" >&2
            echo "    git config remote.origin.push HEAD:refs/for/main" >&2
            echo "" >&2
            status=1
            ;;
    esac
done
exit $status
"#;

fn render_readme(namespace: &str) -> String {
    format!(
        r#"# Tickets for `{namespace}`

This repository holds every ticket in the `{namespace}` namespace, one readable
TOML file per ticket under `tickets/`.

## Working from the command line

Clone it and configure the push ref once:

```sh
git clone <this-url>
cd ticket
{PUSH_CONFIG_LINE}
```

After that, `git push` and `git pull` behave normally. You will never be asked to
resolve a merge conflict: the server merges concurrent edits field by field, and
your submitted commit is always a parent of the result, so pulls fast-forward.

If you skip the config line, pushes are refused with a reminder — nothing breaks.

## Editing a ticket

Edit the file and push:

```sh
$EDITOR tickets/42.toml
git commit -am "close #42"
git push
```

## Creating a ticket

Add a file named `tickets/new-<anything>.toml` with at least a title:

```toml
title = "Login fails on Safari"
status = "open"

[body]
markdown = "Clicking login does nothing."
```

The server assigns the number, author and timestamps, then renames the file to
`tickets/<number>.toml`. Pull afterwards to see it.

## What the server owns

`number`, `uuid`, `namespace`, `author` and `created_at` are assigned by the
server and cannot be changed by editing the file. The author of a ticket or a
comment is always the authenticated user who pushed it.
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> String {
        let root = format!("/tmp/test_fig_ticket_repo_{}", uuid::Uuid::new_v4());
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn test_ensure_ticket_repo_creates_bare_repo() {
        let root = temp_root();
        ensure_ticket_repo(&root, "acme", "Fig", "fig@localhost").expect("create ticket repo");

        let path = ticket_repo_path(&root, "acme");
        let repo = git2::Repository::open(&path).expect("repo should open");
        assert!(repo.is_bare(), "ticket repo must be bare");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_ensure_ticket_repo_seeds_config_and_readme() {
        let root = temp_root();
        ensure_ticket_repo(&root, "acme", "Fig", "fig@localhost").expect("create ticket repo");

        let handle = crate::git::bare::RepoHandle::open(&root, "acme", TICKET_REPO).unwrap();
        let config = handle
            .read_file(".fig-tickets.toml")
            .expect("read config")
            .expect("config blob exists");
        assert!(config.contains("schema_version = 1"));
        assert!(config.contains(r#"namespace = "acme""#));

        let readme = handle
            .read_file("README.md")
            .expect("read readme")
            .expect("readme blob exists");
        assert!(
            readme.contains(PUSH_CONFIG_LINE),
            "README must document the one-time push config"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_ensure_ticket_repo_installs_executable_hooks() {
        let root = temp_root();
        ensure_ticket_repo(&root, "acme", "Fig", "fig@localhost").expect("create ticket repo");

        let hooks = ticket_repo_path(&root, "acme").join("hooks");
        for hook in ["proc-receive", "pre-receive"] {
            let path = hooks.join(hook);
            assert!(path.exists(), "{hook} hook should exist");

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&path).unwrap().permissions().mode();
                assert_eq!(mode & 0o111, 0o111, "{hook} hook must be executable");
            }
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_ensure_ticket_repo_sets_proc_receive_config() {
        let root = temp_root();
        ensure_ticket_repo(&root, "acme", "Fig", "fig@localhost").expect("create ticket repo");

        let repo = git2::Repository::open(ticket_repo_path(&root, "acme")).unwrap();
        let config = repo.config().unwrap();
        assert_eq!(
            config.get_string("receive.procReceiveRefs").unwrap(),
            "refs/for"
        );
        assert!(config.get_bool("http.receivepack").unwrap());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_ensure_ticket_repo_is_idempotent() {
        let root = temp_root();
        ensure_ticket_repo(&root, "acme", "Fig", "fig@localhost").expect("first");

        let repo = git2::Repository::open(ticket_repo_path(&root, "acme")).unwrap();
        let first = repo.refname_to_id(CANONICAL_REF).unwrap();

        ensure_ticket_repo(&root, "acme", "Fig", "fig@localhost").expect("second");

        let repo = git2::Repository::open(ticket_repo_path(&root, "acme")).unwrap();
        let second = repo.refname_to_id(CANONICAL_REF).unwrap();
        assert_eq!(first, second, "re-running must not rewrite history");

        let _ = std::fs::remove_dir_all(&root);
    }
}
