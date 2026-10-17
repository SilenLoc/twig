# Program Design: Git crate extraction

## Files

- `Cargo.toml` — add `crates/twig-git` to the workspace, depend on it from `twig`, and remove Git runtime dependencies from the app manifest.
- `Cargo.lock` — update the workspace dependency graph.
- `crates/twig-git/Cargo.toml` — define the new `twig-git` library and its Git/repository dependencies.
- `crates/twig-git/src/lib.rs` — export the crate API and the `GitError`/`Oid` types used by the app.
- `crates/twig-git/src/backend.rs` owns Git request classification, CGI environment preparation, and the `git http-backend` invocation used by the app.
- `src/git/bare/mod.rs` → `crates/twig-git/src/bare/mod.rs` — move the repository-reading module and its namespace helper.
- `src/git/bare/config.rs` and `src/git/bare/config/tests.rs` → `crates/twig-git/src/bare/config.rs` and `crates/twig-git/src/bare/config/tests.rs` — move repository config parsing and tests.
- `src/git/bare/handle.rs` and `src/git/bare/handle/edit_tests.rs` → `crates/twig-git/src/bare/handle.rs` and `crates/twig-git/src/bare/handle/edit_tests.rs` — move repository reads, file commits, and tests.
- `src/git/bare/history.rs` → `crates/twig-git/src/bare/history.rs` — move commit-history types and date conversion.
- `src/git/bare/listing.rs` → `crates/twig-git/src/bare/listing.rs` — move repository listing/search and privacy metadata.
- `src/git/bare/path.rs` → `crates/twig-git/src/bare/path.rs` — move path-safety checks.
- `src/git/reserved.rs` → `crates/twig-git/src/reserved.rs` — move repository-name policy.
- `src/git/repo.rs` → `crates/twig-git/src/repo.rs` — move repository initialization and managed receive-hook operations; leave the authenticated Actix init route in the app adapter.
- `src/git/repo/tests.rs` → `crates/twig-git/src/repo/tests.rs` — move repository creation and hook tests with their implementation.
- `src/git/hooks/pre-receive` → `crates/twig-git/src/hooks/pre-receive` — keep the managed hook beside the crate code that installs it.
- `src/git/mod.rs` — retains the app-owned Actix/auth/database adapter and CGI response tests; it delegates smart-HTTP handling and repository work to `twig-git`.
- `src/main.rs` — registers the app-owned `git` module.
- `src/http/routes.rs` — route the existing init and smart-HTTP endpoints through the app adapter.
- `src/http/namespace/pages.rs` — use the crate for repo validation/creation and reference `twig-git` for repo reads.
- `src/http/settings/pages.rs`, `src/http/settings/pages/namespaces.rs`, and `src/http/settings/pages/repositories.rs` — switch to crate types/APIs and move repository validation/open/delete/move/rename filesystem operations behind the crate boundary.
- `src/http/settings/pages/tests.rs` — update repository fixtures to use the crate's public metadata types.
- `src/http/overview/pages.rs` — use the crate's repository search/privacy APIs.
- `src/http/tree/pages.rs` and `src/http/tree/pages/tests.rs` — use the crate's repository and namespace APIs/types.
- `src/http/tree/api.rs` — use crate APIs for repository discovery and replace direct Git library calls in endpoint fixtures.
- `src/http/repository/pages.rs`, `src/http/repository/pages/tabs.rs`, `src/http/repository/pages/markdown.rs`, and `src/http/repository/pages/content.rs` — consume crate repository APIs, Git error/OID types, and value types while retaining rendering in the app.
- `src/http/repository/editor.rs`, `src/http/repository/editor/render.rs`, and `src/http/repository/editor/tests.rs` — consume crate repository APIs and public Git types rather than depending on `git2` directly.
- `src/http/repository/binaries.rs` — use the crate's repository handle.
- `src/http/repository/pages/tests/mod.rs` — construct the crate's public Git error type in rendering tests.
- `src/integration_tests/tests/support/repository.rs` — seed repositories through Git CLI/plumbing and the new crate API, not through the app's removed Git module.
- `src/integration_tests/tests/repository_auth.rs`, `src/integration_tests/tests/repository_editor.rs`, `src/integration_tests/tests/repository_content.rs`, and `src/integration_tests/tests/namespace.rs` — update crate paths and replace direct `git2` use in application-level integration fixtures/assertions.

The implementation will first confirm this list against compiler/search results; any additional direct Git consumer must be added here before implementation starts.

## Types & signatures

The crate root exports `backend`, `bare`, `repo`, and `reserved` modules, plus these boundary types and operations:

```rust
pub type GitError = git2::Error;
pub use git2::Oid;

pub fn is_repository(path: &std::path::Path) -> bool;
pub fn is_bare_repository(path: &std::path::Path) -> bool;
pub fn delete_repository(path: &std::path::Path) -> Result<(), RepositoryError>;
pub fn move_repository(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), RepositoryError>;

pub enum RepositoryError {
    NotFound,
    NotRepository,
    DestinationExists,
    Io(std::io::Error),
}
```

Repository initialization and Git backend signatures remain crate-owned and usable without app/auth types:

```rust
pub fn default_branch() -> String;
pub fn create_repo(
    root: impl AsRef<std::path::Path>,
    namespace: impl Into<String>,
    repo: impl Into<String>,
    branch: impl Into<String>,
) -> Result<(), String>;
pub fn create_repo_with_author(
    root: impl AsRef<std::path::Path>,
    namespace: impl Into<String>,
    repo: impl Into<String>,
    branch: impl Into<String>,
    author_name: &str,
    author_email: &str,
) -> Result<(), String>;
pub fn bare_init(
    repo_path: &std::path::Path,
    branch: &str,
    author_name: &str,
    author_email: &str,
) -> Result<String, String>;

pub fn run_with_config(
    config: &backend::Config,
    namespace: &str,
    request: &backend::GitRequest,
    body: Vec<u8>,
    authenticated_user: Option<&str>,
    namespace_role: Option<&str>,
) -> Result<(String, Vec<u8>), String>;
```

The existing `RepoHandle`, `CommitFileOutcome`, `RepoInfo`, configuration, history, and validation APIs remain available under `twig_git::bare` / `twig_git::reserved`, with all libgit2-facing signatures using `GitError` and `Oid`. The crate has no dependency on Twig auth, database, Actix state, or markdown rendering modules.

## Call stack

- Smart HTTP: `routes::configure_routes` → `git::git_handler` → app authentication/authorization → `twig_git::backend::run_with_config` in `web::block` → CGI response conversion in `src/git/mod.rs`.
- UI repository creation: namespace or init Actix handler → existing app auth/database checks → `twig_git::reserved::validate_repo_name` → `twig_git::repo::bare_init` / `create_repo`.
- UI repository management: settings Actix handler → existing app auth/database checks → `twig_git` repository validation and delete/move operations → app response rendering.
- Repository browsing/editing: UI handler → `twig_git::bare::RepoHandle` / listing API → app rendering and response.

## Test plan

- Existing crate-unit tests for request classification, CGI header parsing/environment, repository config, history, listings, path safety, file editing, repo initialization, and receive-hook preservation continue to run under `cargo test -p twig-git`.
- `repository_operations_reject_non_repositories_and_preserve_destinations` asserts invalid sources are refused and a destination conflict leaves both repositories unchanged.
- `repository_operations_move_rename_and_delete_bare_repositories` asserts each operation changes only the expected on-disk path and preserves repository contents on move/rename.
- Existing app route tests assert smart HTTP routing, Git authentication, namespace permissions, and responses remain unchanged under `cargo test -p twig`.
- Existing repository browsing, editing, namespace move/rename/delete, and private-repository integration tests pass unchanged in behavior under `cargo test -p twig`.
- `cargo test --workspace` verifies both workspace members together.
- `cargo tree -p twig --edges normal` confirms the application no longer has a production dependency on `git2` or `xshell`; `twig-git` owns those Git implementation dependencies.

## Least confident decisions

1. Keep Actix/auth/database glue in Twig rather than making the new crate own the full Git HTTP handler; this avoids an application dependency cycle while still putting Git protocol/repository behavior in the crate.
2. Expose `git2::Error` and `Oid` through stable crate aliases for existing editor/view call sites instead of replacing them with custom Git-neutral types in this extraction.
3. Put safe repository move/delete operations in `twig-git` while leaving authorization, HTTP status selection, and UI messages in the app.
4. Keep the existing route-reservation list in `twig-git` even though its rationale refers to Twig's namespace UI routes; moving it avoids splitting repository-name policy during the extraction.
