# AGENTS.md

Fig is a Git server and web UI: Rust, Actix-web, maud templates, turso/libSQL over a local SQLite file.

## Commands

- `mise run verify` — `cargo fmt --check`, `cargo clippy --all-targets`, `cargo nextest run`. Run this before you finish.
- `mise run test` (or `cargo nextest run <name>` for one test); `mise run fmt` formats and applies clippy fixes; `mise run run` / `mise run watch` serves on `:8080`.
- `mise run release` bumps the patch version, builds, and pushes `silenloc/fig:<v>`. Never run it, or any git write, unless asked.

## Layout

`src/main.rs` wires all routes. `src/view/*` holds the maud pages (overview `/`, namespace, repo, settings, auth, info `/_info`); `src/auth/*` sessions and Argon2 hashing; `src/db/*` a turso wrapper plus a hand-rolled `MIGRATIONS` array in `migration.rs`; `src/git/*` and `src/git_backend.rs` repo operations and smart-HTTP; `src/info/` the Information section, which compiles `docs/*.md` in via `include_str!` — so editing those files changes the running UI.

## Rules

- Tests are `#[cfg(test)]` modules colocated in the file they cover, driven by `actix_web::test`. Keep new tests there rather than in `tests/`.
- Views use `maud::html!`. Check the `HX-Request` header: present → return the bare partial, absent → wrap it in `view::render_layout`.
- All styling flows through the `--fig-*` tokens in the `:root` block of `assets/fig.css`; that block is the only place raw colour values may appear. Assets (`t.css`, `h.js` htmx, `fig.svg`) are embedded via `include_str!` in `src/assets.rs`. Do not add a CSS framework, inline styles, or a second stylesheet.
- Clippy runs at `pedantic`. Prefer `?` over `.unwrap()`, and `log::error!` before returning an HTTP error.

## Config and auth

Env vars are read in `src/config.rs`: `PORT` (80), `LOG_LEVEL` (info), `PROJECT_ROOT` (/srv/git), `DB_PATH` (fig.db), `API_KEY` (random when unset), `SESSION_KEY` (random when unset), `ADMIN_USER` (database viewer user), `RESET_DB`, `CACHE_CONTROL`, `SENTRY_TRACES_SAMPLE_RATE`; `SENTRY_DSN` is read in `main.rs`. `mise.toml` overrides them for local dev.

Git reads are public; pushes and `POST /init` need Basic Auth. The web UI uses a `session` cookie holding a 64-hex token valid 30 days from `created_at`. `POST /auth/invite` requires `api_key` as a form field, and signup consumes a single-use invite.
