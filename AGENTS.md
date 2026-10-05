# AGENTS.md

Twig is a Git server and web UI: Rust, Actix-web, maud templates, Turso/libSQL over a local SQLite-compatible file. The shipped app includes namespace/repository browsing and management, Git smart HTTP, auth, admin database inspection, an optional endpoint test page, and a MessagePack tree endpoint. There is no ticket UI or `twig-ticket` CLI in this repository.

## Commands

- `mise run verify` — `cargo fmt --check`, `cargo clippy --all-targets`, `cargo nextest run`. Run this before you finish.
- `mise run test` (or `cargo nextest run <name>` for one test); `mise run fmt` formats and applies clippy fixes; `mise run run` / `mise run watch` serves on `:8080`.
- `mise run release` bumps the patch version, builds, and pushes `silenloc/twig:<v>`. Never run it, or any git write, unless asked.

## Layout

`src/main.rs` wires routes. `src/view/*` holds maud pages (overview `/`, namespace, repo, settings, auth, info `/_info`, Tree/Data, optional Test); `src/auth/*` handles sessions and Argon2 hashing; `src/db/*` is the Turso/libSQL wrapper plus a hand-rolled `MIGRATIONS` array in `migration.rs`; `src/git/*` and `src/git_backend.rs` implement repo operations and smart HTTP; `src/api.rs` serves `GET /api/tree` as MessagePack. `src/info/` compiles the registered Markdown docs via `include_str!` into the Docs page at `/_info` — when adding a doc file, register it there too.

## Rules

- Tests are `#[cfg(test)]` modules colocated in the file they cover, driven by `actix_web::test`. Keep new tests there rather than in `tests/`.
- Views use `maud::html!`. Check the `HX-Request` header: present → return the bare partial, absent → wrap it in `view::render_layout`.
- All styling flows through the `--twig-*` tokens in the `:root` block of `assets/twig.css`; that block is the only place raw colour values may appear. Assets (`t.css`, `h.js` htmx, `twig.svg`) are embedded via `include_str!` in `src/assets.rs`. Do not add a CSS framework, inline styles, or a second stylesheet. Reference assets in views through `crate::assets::url("twig.css")`, never a literal `/assets/...` path, so every URL carries the version and stays immutable-cacheable only for its own release.
- Clippy runs at `pedantic`. Prefer `?` over `.unwrap()`, and `log::error!` before returning an HTTP error.

## Config and auth

Env vars are read in `src/config.rs`: `PORT` (80), `LOG_LEVEL` (info), `PROJECT_ROOT` (/srv/git), `DB_PATH` (twig.db), `API_KEY` (random and logged when unset), `SESSION_KEY` (random per process; sessions do not survive restart unless set), `TEST_USER` (optional test-page user; `true` maps to `admin`), `ADMIN_USER` (optional database viewer), `RESET_DB`, `CACHE_CONTROL`, `SENTRY_TRACES_SAMPLE_RATE`; `SENTRY_DSN` is read in `main.rs`. `mise.toml` overrides them for local development; with `RESET_DB=true` it resets the database and seeds/logs in the development `admin`/`admin` user.

Public Git reads need no credentials; private Git reads and all pushes need Basic Auth, and writes require namespace ownership. A push can create a missing namespace and repository. `POST /init` creates a repo only in an existing namespace the authenticated user owns. The web UI uses a signed/encrypted Actix session cookie backed by the database with a 30-day lifetime (not a raw 64-hex token cookie). UI repo creation requires a session, namespace ownership, and an account email. `POST /auth/invite` requires `api_key` as a form field, and signup consumes a single-use invite.
