# Program Design: Easier-to-navigate Twig modules

## Files

- `src/main.rs` — retain process setup and route delegation.
- `src/http/mod.rs` — expose shared HTTP modules and feature modules.
- `src/http/routes.rs` — retain route order and Git user-agent guard; point routes at feature handlers.
- `src/http/assets.rs`, `src/http/health.rs` — keep shared transport endpoints at the HTTP boundary.
- `src/http/api.rs` — retain the general version endpoint, separate from feature APIs.
- `src/http/view.rs` — keep shared page layout and render helpers; retain test helpers under `src/http/view/test_util.rs`.
- `src/http/auth/` — add a module declaration; move existing auth request handlers, auth pages, and session request helper into `handlers.rs`, `pages.rs`, and `session.rs`.
- `src/http/tree/` — add a module declaration; move the tree API endpoint and tree UI page into `api.rs` and `pages.rs`.
- `src/http/repository/` — add a module declaration; move binary endpoints and repository pages into `binaries.rs` and `pages.rs`.
- `src/http/namespace/`, `overview/`, `settings/`, `info/`, and `test_page/` — add focused module declarations and move each existing page implementation to `pages.rs`.
- `src/http/mod.rs` — remove the generic `view` feature modules from its declarations, retaining `view` only for shared rendering primitives.
- `src/auth/mod.rs` — keep domain auth/session modules separate from HTTP auth feature modules.
- `src/git/backend.rs` and `src/git/mod.rs` — keep the Git backend grouped with Git and update its callers.
- `src/integration_tests.rs` — point tests at the feature-oriented route/page paths.

## Types & signatures

```rust
// src/http/routes.rs
pub(crate) fn configure_routes(cfg: &mut actix_web::web::ServiceConfig);
fn is_git() -> impl actix_web::guard::Guard;

// src/http/mod.rs
pub mod api; // general version endpoint
pub mod auth;
pub mod info;
pub mod namespace;
pub mod overview;
pub mod repository;
pub mod settings;
pub mod test_page;
pub mod tree;
pub mod view; // shared layout/render helpers only

// Feature module declarations
pub mod handlers; // auth
pub mod pages;    // auth, info, namespace, overview, repository, settings, test_page, tree
pub mod session;  // auth
pub mod api;      // tree
pub mod binaries; // repository

// src/git/mod.rs
pub mod backend;
```

No request/response types, domain signatures, serialized formats, endpoint behavior, or database behavior change. Database connection initialization remains lazy.

## Call stack

- Startup: `main` → `http::routes::configure_routes` → Actix route registration.
- Auth request: route → `http::auth::{handlers,pages,session}` → existing `auth`/`db` domain operations → response.
- Repository request: route → `http::repository::{pages,binaries}` → existing Git/auth/database domain operations → response.
- Tree request: route → `http::tree::{api,pages}` → existing auth/database/Git operations and shared `http::view` rendering → response.
- Other page requests: route → matching feature's `pages` module → shared `http::view` helpers as needed → response.
- Git smart HTTP: route → `git::git_handler` → auth checks → `git::backend` request classification and CGI execution → HTTP response.

## Test plan

- Existing Git guard tests — retain Git user-agent acceptance and browser/missing-header rejection.
- Existing auth handler/page/session tests — pass from `http::auth` paths with assertions unchanged.
- Existing tree endpoint/page tests — pass from `http::tree` paths; preserve MessagePack shape and page behavior.
- Existing repository, binary, namespace, settings, docs, overview, and optional test-page tests — pass from their feature paths.
- Existing integration tests — register and call the moved handlers through the same URLs and preserve route precedence.
- Full `cargo test`, `cargo fmt --check`, and `cargo clippy --all-targets` — verify behavior, formatting, and lint health.
- Source-path search — ensure no references remain to the old `http::view::{auth,namespace,overview,repo,settings,tree}` paths or flat `http::{api,auth,binaries}` paths.

## Least confident decisions

1. Keeping the `http` boundary while grouping within it by feature matches the requested two-level organization and avoids mixing transport concerns into domain modules.
2. Putting the cross-feature request identity helper at `http::auth::session` makes its ownership clear, though other features call it.
3. Keeping shared layout/render functions in `http::view` avoids duplicating common presentation code while ensuring feature pages are not hidden in a generic view layer.
4. Giving single-file page features (`namespace`, `overview`, `info`, and `test_page`) their own folders is slightly more structure, but makes the feature map consistent and easy to scan.
