# Architecture: Easier-to-navigate Twig modules

## Fit

- Keep `main.rs` focused on process startup and shared middleware setup.
- Keep `http/` as the outer transport boundary, with shared route registration, assets, and health checks there.
- Organize HTTP implementations inside that boundary by feature rather than by handler/view layer: `auth/` (handlers, pages, session request helpers), `tree/` (MessagePack endpoint and page), `repository/` (repository pages and binary endpoints), plus focused `namespace/`, `overview/`, `settings/`, `info/`, and `test_page/` modules.
- Keep shared page layout/render helpers in `http/view.rs`; feature page implementations should live beside their feature's other HTTP code.
- Keep domain logic in the existing `auth/`, `db/`, and `git/` areas. Keep the Git CGI/backend integration in `git/backend.rs`, alongside the Git request handling that calls it.
- Keep configuration, Markdown helpers, and integration tests in their existing top-level areas.
- This is a source-organization change only: preserve route behavior/order, response formats, and lazy database initialization.

## Endpoints

None added, removed, or changed. Existing routes and methods remain as registered today; registration stays in `http/routes.rs` and points to feature-oriented HTTP modules, preserving order, especially static/UI routes before dynamic namespace/repository routes.

## Data

No data changes. Existing database tables and queries remain unchanged. `Database` must continue to initialize its connection lazily on first use.

## Flow

- Startup: `main` loads configuration and constructs shared state, then delegates route registration to the HTTP routes module.
- Web request: Actix route configuration selects the relevant feature handler; feature pages use shared layout helpers and business operations continue to call the existing auth/database/Git domain code.
- Git smart HTTP: the HTTP route delegates to `git` request handling, which authenticates and invokes the Git backend integration in the Git area.

## External

None. No new third-party APIs, environment variables, or webhooks.
