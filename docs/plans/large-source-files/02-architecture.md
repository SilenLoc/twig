# Architecture: Smaller, clearer source modules

## Fit

Keep the current feature-oriented module paths and public route registration. Split internals beneath those modules, re-exporting route handlers from their current paths so `http/routes.rs` and endpoint behavior do not need to change. Move inline unit tests into child test files (`#[path = "…/tests.rs"] mod tests`) so they remain descendants of the implementation module and can continue testing private helpers.

For the largest production modules, split by behavior rather than line ranges:

- `src/http/repository/pages.rs`: retain shared repository context, tab shell, and stable handler exports; move the cohesive Markdown/link, content-browser, Scripts, Paper, and presentation/PDF implementations into `pages/` submodules. Keep each route handler next to the behavior it serves and re-export handlers at `repository::pages`.
- `src/http/settings/pages.rs`: split account settings, repository operations, and namespace operations into sibling modules; preserve route exports.
- `src/http/test_page/pages.rs`: separate endpoint catalog and page rendering from request handlers; preserve current exports.
- `src/http/tree/pages.rs`: separate the admin database table browser from namespace/repository views; preserve route exports.
- `src/http/tree/user_management.rs`: separate invitation creation/resend and invitation rendering/delivery from the user-management page and its shared scope.
- `src/http/repository/editor.rs`: separate editor markup from edit/commit flow and conflict/path helpers.

For other listed files, first extract their inline unit tests: auth pages, namespace pages, tree pages and user management, repository pages/editor, bare-repository handle/config, Git repo, highlighter, DB namespaces/invitations, assets, overview pages, auth, config, and auth handlers. The test-only integration suite becomes feature-grouped child modules under `integration_tests/`, with common service/repository setup in `support.rs`.

The four listed files already below 500 lines are review-only. Split one only if inspection finds a cohesive boundary; do not create modules solely to hit a number. No database lifecycle or initialization changes are in scope; database connections must remain lazy.

## Endpoints

None added, removed, or changed. Existing route paths, methods, ordering, and handler exports remain stable.

## Data

No schema or query changes. Existing persistence behavior and connection timing remain unchanged.

## Flow

Requests continue through `http/routes.rs` to the same feature handler paths. The handler delegates to the same rendering, repository, and database logic, now housed in focused child modules. Unit tests are loaded from child test files; integration tests are dispatched to feature-grouped modules that share the current test-service helpers.

## External

None. No new dependencies, environment variables, or external services.
