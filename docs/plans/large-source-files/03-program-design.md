# Program Design: Smaller, clearer source modules

## Files

### Existing files changed

- `src/http/repository/pages.rs` — shared context, public route re-exports, and tab coordination after feature renderers/handlers move out.
- `src/integration_tests.rs` — test-only entry point that declares the feature-grouped suite.
- `src/http/settings/pages.rs` — stable route re-exports for split account/repository/namespace settings.
- `src/http/namespace/pages.rs` — stable namespace route exports and test-module declaration.
- `src/http/test_page/pages.rs` — stable route exports and test-module declaration.
- `src/http/auth/pages.rs` — auth page behavior and test-module declaration.
- `src/http/tree/user_management.rs` — user-management page plus stable invitation route exports.
- `src/http/tree/pages.rs` — namespace/repository/tree views plus stable data-browser route exports.
- `src/git/bare/handle.rs`, `src/git/bare/config.rs`, `src/git/repo.rs` — implementation and test-module declarations.
- `src/http/repository/editor.rs` — editor flow and stable route exports.
- `crates/twig-highlight/src/lib.rs` — highlighter implementation and test-module declaration.
- `src/db/namespaces.rs`, `src/db/invitations.rs`, `src/http/assets.rs` — implementation and test-module declarations.

### New production modules

- `src/http/repository/pages/{markdown,content,scripts,paper,presentation}.rs` — each owns its route/rendering behavior; shared repository context remains in `pages.rs`.
- `src/http/settings/pages/{account,repositories,namespaces}.rs` — settings views/actions grouped by what they manage.
- `src/http/test_page/pages/{catalog,render}.rs` — endpoint metadata and page fragments, respectively.
- `src/http/tree/pages/data.rs` — database table-browser views and handlers.
- `src/http/tree/user_management/invitations.rs` — invitation-specific views and operations.
- `src/http/repository/editor/render.rs` — editor markup and editor-only client script.

### New unit-test modules

- `src/http/repository/pages/tests/{mod,navigation,markdown,content,scripts,paper,presentation}.rs` — shared test helpers plus behavior-focused repository page tests.
- `src/http/settings/pages/tests.rs`, `src/http/namespace/pages/tests.rs`, `src/http/test_page/pages/tests.rs`, `src/http/auth/pages/tests.rs`.
- `src/http/tree/{pages,user_management}/tests.rs`.
- `src/http/repository/editor/tests.rs`, `src/git/bare/handle/edit_tests.rs`, `src/git/{repo,bare/config}/tests.rs`.
- `crates/twig-highlight/src/tests.rs`, `src/db/{namespaces,invitations}/tests.rs`, `src/http/assets/tests.rs`.

### New integration-test modules

- `src/integration_tests/tests/mod.rs` — imports common support and declares feature suites.
- `src/integration_tests/tests/support.rs` — only genuinely shared app/database/repository fixtures.
- `src/integration_tests/tests/{auth,namespace,admin,repository_editor,repository_content,repository_tabs,repository_binaries,repository_auth,test_page}.rs` — existing integration tests grouped by behavior.

The four supplied files already below 500 lines (`overview/pages.rs`, `auth/mod.rs`, `config.rs`, and `http/auth/handlers.rs`) are reviewed for a cohesive boundary. Their inline test modules provide a clear boundary, so those tests are extracted too; production behavior remains in the existing module.

## Types & signatures

No new public domain types or endpoint signatures. Existing route paths keep their current module-qualified names through re-exports. Cross-module implementation helpers stay private or `pub(super)`; do not make them crate-public just to move code.

```rust
// In src/http/repository/pages.rs
mod content;
mod markdown;
mod paper;
mod presentation;
mod scripts;

pub use content::content_handler;
pub use markdown::markdown_handler;
pub use paper::paper_handler;
pub use presentation::{present_print_handler, slide_handler};
pub use scripts::scripts_group_handler;
```

Handlers retain their current signatures, for example:

```rust
pub async fn content_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<ContentParams>,
    query: web::Query<ContentQuery>,
) -> AwResult<Markup>;

pub(super) fn render_content_view(
    namespace: &str,
    repo: &str,
    path: &str,
    entries: &[TreeEntry],
    file_bytes: Option<&[u8]>,
) -> Markup;
```

Other route modules follow the same pattern: existing public handlers are re-exported; helpers/types needed by sibling implementation modules use `pub(super)` at most. Test files are child modules and keep access to the implementation's private items via `super`.

## Call stack

- HTTP request: `http::routes::configure_routes` → existing feature module path → re-exported route handler → focused implementation module → existing DB/Git/render helpers.
- Unit test: implementation module's `#[cfg(test)]` child declaration → topic-specific test module → private implementation helpers through the parent module.
- Integration test: `integration_tests` test-only entry → feature test module → shared support fixture where needed → existing configured Actix routes.

## Test plan

- Preserve and run every existing unit test after extraction; compare test totals before and after each slice.
- Repository-page test groups continue asserting tab ordering/HTMX behavior, markdown links, content safety/rendering, scripts, paper anchors, and presentation/PDF behavior.
- Settings, namespace, auth, and tree tests continue asserting their current markup contracts, authorization, and response behavior.
- Git, database, asset, and highlighter tests continue asserting their existing persistence, path-safety, parsing, and output contracts.
- Integration modules retain the current service-level coverage for auth, namespace ownership/invitations, admin gating, repository editor/content/tabs/binaries, private repository auth, and the optional test page.
- After each slice: run focused tests, then `cargo test --all-targets`; also run `cargo fmt --check` and `git diff --check`.
- No assertions are weakened or removed; module movement must preserve test names and semantics.

## Least confident decisions

1. `repository/pages.rs` has shared private context across several views; keep the first split conservative and use only parent-scoped visibility. Avoid a general-purpose rendering framework.
2. Integration fixtures may look shared but have different filesystem/database assumptions. Move only common setup into `support.rs`; leave feature-specific fixtures with their tests.
3. Test files can themselves become large. Keep repository-page tests topic-split, and split any other extracted test file only at a coherent boundary.
4. The 500-line target is a review aid, not permission to fragment cohesive code. If a responsibility cannot be split cleanly, document the reason rather than inventing a module.
5. Preserve the existing route re-export paths and the lazy database-connection lifecycle exactly.
