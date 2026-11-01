# Continuous Integration

Twig runs a repository's checks itself. The definition lives in the repository,
in `.twig/mise.toml`, and it is a [mise](https://mise.jdx.dev) config: it names
the tools a check needs and the `ci` task to run. Twig does not use git hooks
for this. A push queues a run after git has accepted it, and the run executes in
the background, so the push never waits for it.

CI is off until the server operator enables it (see [Settings](#settings)). With
CI off, Twig accepts pushes as before and lists no runs.

## Defining CI

Commit a `.twig/mise.toml` at the repository root:

```toml
[tools]
rust = { version = "nightly-2026-10-31" }
"cargo-binstall" = "latest"
"cargo:cargo-nextest" = "latest"

[tasks.ci]
dir = ".."
run = ['cargo fmt --check', 'cargo clippy --all-targets', 'cargo nextest run']
```

- Only `.twig/mise.toml` is read. A `mise.toml` at the repository root is a
  developer's configuration, so Twig never merges it into a run.
- The `ci` task must exist for a commit to be runnable. A commit without it
  queues nothing, and a manual dispatch of a commit without it is refused.
- Tasks run from `.twig/` by default. Set `dir = ".."` to run from the repository
  root, as above.
- `mise install` installs the `[tools]` before the task runs.

## When a run starts

- **A branch push.** Each branch the push moves to a commit that defines `ci`
  queues one run of that commit. Deleting a branch queues nothing, and a push the
  server rejects queues nothing.
- **A manual dispatch.** `fj actions dispatch ci <branch>` queues a run of the
  branch's tip. Inputs pass as `-I NAME=value`.
- **Pull requests** show the statuses of their head commit. A push to the head
  branch queues a run as usual.

## How a run executes

Each run executes in isolation from the server's state, not from the Git
repository:

1. The commit is exported as plain files into a new empty directory under the
   server's temporary directory. Symbolic links and submodules are not exported.
2. `mise install` and then `mise run ci` run in that directory with a cleared
   environment. Only these variables reach the job:
   - `PATH`, `LANG`, and a private `HOME` (the job's own directory).
   - `CI=true`, `TWIG_CI=1`, `TWIG_CI_EVENT`, `TWIG_CI_REPOSITORY`,
     `TWIG_CI_REF`, and `TWIG_CI_SHA`, describing the run.
   - `TWIG_INPUT_<NAME>` for each dispatch input.
   - A Git author and committer identity (`Twig CI`), so tests that commit work
     without a global Git configuration.
   - Tool caches under `TWIG_CI_CACHE_DIR`, shared by every run: `MISE_*`,
     `CARGO_HOME`, and `RUSTUP_HOME`.

   Server secrets such as `API_KEY`, `SESSION_KEY`, and `RESEND_API_KEY` are not
   among them.
3. The run has one time limit covering both commands, `TWIG_CI_TIMEOUT_SECS`
   (30 minutes by default). When it expires, the job's whole process group is
   killed.
4. The job's output becomes the run's log. Each run keeps its newest 1 MiB of
   output and drops the oldest lines beyond that.
5. The export directory is removed.

A run is `waiting` until one of `TWIG_CI_CONCURRENCY` slots is free, then
`running`, then `success` or `failure`. A run that was waiting or running when
the server restarted is recorded as `failure`.

### What isolation does not give you

The job is not a sandbox. It runs as the user that runs Twig, with that user's
filesystem access and network access. Anyone who can push to a namespace can
therefore run code on the server host, and that code can read whatever the Twig
process can read, including the database file and other repositories. Enable CI
only on a host where that is acceptable, or run Twig inside a container or a VM
that confines it.

## Settings

| Variable | Meaning | Default |
|----------|---------|---------|
| `TWIG_CI` | Set to `1` or `true` to run CI. Off otherwise. | off |
| `TWIG_CI_PROGRAM` | The `mise` executable to run. Must be on the server's `PATH` or be a path. | `mise` |
| `TWIG_CI_TIMEOUT_SECS` | Wall-clock limit for one run, install included | `1800` |
| `TWIG_CI_CONCURRENCY` | Runs executing at once; the rest wait | `1` |
| `TWIG_CI_CACHE_DIR` | Shared tool, Cargo, and Rustup caches | `<temp dir>/twig-ci` |

The server needs `mise` installed (or `TWIG_CI_PROGRAM` pointing at it). The
release image does not include it, so a deployment that enables CI on that image
must add it.

## Forgejo-compatible API

These routes use Forgejo's shapes and the `fj` command. The repository is
`{owner}/{repo}`, where `owner` is the namespace. Authentication is the same as
the rest of the [Forgejo-compatible API](pull-requests.md#api).

| Method | Path | Required permission | Purpose |
|--------|------|---------------------|---------|
| `GET` | `/api/v1/repos/{o}/{r}/actions/tasks?page=&limit=` | `view_namespace` | Runs, newest first (`fj actions tasks`) |
| `POST` | `/api/v1/repos/{o}/{r}/actions/workflows/ci/dispatches` | `push_changes` | Queue a run of `ci` on a branch (`fj actions dispatch`) |
| `GET` | `/api/v1/repos/{o}/{r}/actions/tasks/{id}/log` | `view_namespace` | A run's log, as plain text (Twig's own route) |
| `GET` | `/api/v1/repos/{o}/{r}/commits/{sha}/status` | `view_namespace` | Combined status of a commit |
| `GET` | `/api/v1/repos/{o}/{r}/statuses/{sha}` | `view_namespace` | The statuses CI reported for a commit |

- A task lists `id`, `name` and `workflow_id` (both `ci`), `status`,
  `display_title`, `event` (`push` or `workflow_dispatch`), `head_branch`,
  `head_sha`, `run_number`, `created_at`, `updated_at`, `run_started_at`, and
  `url`, the log address. Lists default to 20 items and accept at most 50.
- `status` is `waiting`, `running`, `success`, or `failure`.
- Each run is one commit status with context `ci`. Its `state` is `pending` while
  the run waits or runs, otherwise `success` or `failure`. The combined `state` is
  `failure` if any run failed, `pending` while a run is unfinished or when there
  are none, and `success` otherwise.
- The dispatch body is `{"ref": "<branch>", "inputs": {"NAME": "value"}}`. It
  answers `204` and does not return run details. Refusals:
  - `404` when the workflow is not `ci` or CI is off.
  - `422` for an unknown branch, a commit without the `ci` task, or an input name
    that is not an identifier.
  - `403` without `push_changes`.

## Not supported

- Variables and secrets for runs.
- Cancelling or re-running a run.
- Workflows other than the one `ci` task.
- A web page for runs; read logs through the log route or `fj`.
