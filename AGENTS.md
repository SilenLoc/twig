# AGENTS.md

This file contains guidelines for AI coding agents working on this repository.

## Project Overview

Fig is a Git server and web UI built with Rust (Actix-web). It serves Git repositories over HTTP and provides a web interface for browsing repositories.

## Build/Lint/Test Commands

### Task runner (mise)

The project uses `mise` (see `mise.toml`) for project tasks. There is no `justfile`.

```bash
# Full verification (fmt check, check, clippy, test)
mise run verify

# Format code and auto-fix clippy
mise run fmt

# Run the application
mise run run

# Run the Docker image (publishes port 8080 -> 8080)
mise run docker
```

### Direct Cargo Commands

```bash
# Check code without building
cargo check

# Run linter
cargo clippy

# Format code
cargo fmt

# Run unit tests
cargo test

# Run a single test
cargo test <test_name>

# Build release
cargo build --release
```

### Docker Commands

There is no `just docker` recipe; build with `docker build` directly. The Dockerfile
uses `cargo-chef` for dependency caching and exposes port `80` in the runtime stage.

```bash
# Build Docker image
docker build -t silenloc/fig .

# Run Docker container (map host port to container port 80)
docker run -p 8080:80 silenloc/fig
```

## Code Style Guidelines

### Imports

- Group imports: std library first, then external crates, then local modules
- Use `use crate::` for local module imports
- Example:
  ```rust
  use std::path::Path;
  
  use actix_web::{HttpRequest, get, web};
  use serde::Deserialize;
  
  use crate::{config, git};
  ```

### Formatting

- Run `cargo fmt` before committing
- Run `cargo clippy` and fix all warnings
- 4 spaces for indentation (Rust default)

### Types and Naming

- **Structs/Enums**: PascalCase (e.g., `GitRequest`, `Server`)
- **Functions/Variables**: snake_case (e.g., `get_commits`, `project_root`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `TCSS`, `HTMX`)
- **Modules**: snake_case (e.g., `repo.rs`, `bare.rs`)
- Use descriptive names; avoid abbreviations except common ones (`req`, `ctx`)

### Error Handling

- Use `Result<T, E>` for fallible operations
- Propagate errors with `?` operator when appropriate
- Log errors using `log::error!()` before returning HTTP error responses
- Handle `Option` with `if let` or `match` rather than `.unwrap()` in production code

### HTML/Views

- **Use the bundled `h.js` (htmx) and `t.css` (Tachyons) for creating views** - do not change CSS without reason
  - Static assets live in `assets/` and are embedded via `include_str!` in `src/assets.rs`
  - Served as `h.js`, `hx-response-targets.js`, `t.css`, `fig.svg` under `/assets/...`
  - `h.js` is htmx; `hx-response-targets.js` is the htmx response-targets extension
- Use `maud` for HTML templating
- Use `maud::html!` macro for markup
- Use `maud::DOCTYPE` for doctype declaration
- Check for `HX-Request` header to determine if rendering full layout or partial content
- Structure:
  ```rust
  if req.headers().get("HX-Request").is_some() {
      Ok(content)
  } else {
      Ok(super::render_layout(&content))
  }
  ```

### Request Handlers

- Use Actix-web macros: `#[get("/path")]`, `#[post("/path")]`
- Return `impl Responder` or specific types like `AwResult<maud::Markup>`
- Extract path params with `web::Path<Params>` using a `#[derive(Deserialize)]` struct
- Access config via `web::Data<config::Server>`
- Access auth context via `web::Data<auth::FigContext>`

### Testing

- **Tests are native Actix-web tests** colocated in `#[cfg(test)] mod tests` blocks within each module (see `src/main.rs`, `src/auth/mod.rs`, `src/auth/handlers.rs`, `src/config.rs`).
- Use `actix_web::test::TestRequest` and `test::init_service` / `test::call_service` to exercise the app service tree.
- Run tests with `cargo test` (or `mise run verify` for the full fmt+check+clippy+test pipeline).
- Run a single test with `cargo test <test_name>`.
- Tests use temporary databases under `/tmp` (e.g. `/tmp/test_fig_*.db`) and call `Database::init_tables()` before assertions.

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | Server port (mapped to bind address `0.0.0.0`) | `80` |
| `LOG_LEVEL` | Logging level (error, warn, info, debug, trace); also dampens `libsql`, `turso`, and `tracing::span` to `warn` | `info` |
| `PROJECT_ROOT` | Git repositories root path | `/srv/git` |
| `DB_PATH` | Path to the SQLite database file | `fig.db` |
| `API_KEY` | API key for ticket generation (auto-generated if not set or empty) | Auto-generated |
| `RESET_DB` | Set to `true` to delete the database file on startup | `false` |
| `SENTRY_DSN` | Sentry project DSN. If unset, Sentry instrumentation is disabled | unset |
| `SENTRY_TRACES_SAMPLE_RATE` | Fraction of transactions sent to Sentry (0.0–1.0) | `1.0` |

> Note: `mise.toml` overrides these for local development (`PORT=8080`, `API_KEY=secure`, `RESET_DB=true`).

### Git Workflow

- Do not run `git commit`, `git push`, or destructive git operations unless explicitly asked
- The project uses semantic-release for automated versioning

## Authentication System

Fig includes a complete authentication system based on **session cookies and HTTP Basic Auth**.
There is no separate `/api/*` surface and no Bearer-token API; the `tokens` table stores
session tokens that are exchanged via the `session` cookie.

Ticket-based signup is the only admin-gated action and is submitted via the `/auth/ticket`
form with the `API_KEY` provided as a form field (not an HTTP header).

### Flow Overview

1. **Get Ticket** (requires API Key, submitted as a form field on `/auth/ticket`) → Returns one-time signup ticket
2. **Signup** (`/auth/signup`, requires ticket) → Creates account
3. **Login** (`/auth/login`) → Sets `session` cookie
4. **Create Namespace** (`/auth/namespace`, requires `session` cookie) → Creates namespace + directory under `PROJECT_ROOT`
5. **Git Operations** (`/{namespace}/{repo}/...`) → Public read; pushes require Basic Auth (username:password)

### Authentication by Endpoint Type

| Endpoint Type | Auth Method |
|--------------|-------------|
| Git clone/fetch | None (public read) |
| Git push | Basic Auth (username:password) |
| Init repo (`POST /init`) | Basic Auth |
| Web UI pages (`/`, `/auth/*`, `/settings`, `/{namespace}`) | Session cookie (`session`) optional; login required for mutating actions |
| Ticket generation (`POST /auth/ticket`) | API Key (form field `api_key`) |
| Signup (`POST /auth/signup`) | Single-use ticket (form field `ticket`) |

### Database Schema

The system uses `turso` (the libSQL client) over a local SQLite file for storing auth data.
Schema is defined with the `migs` crate in `src/db/migration.rs` and applied on startup by
`Database::create_tables()` (runs `PRAGMA journal_mode = WAL;` then each migration in order).

Tables:

- **users**: User accounts with hashed passwords
- **namespaces**: Namespace definitions with owners
- **namespace_members**: Many-to-many relationship for namespace access
- **tickets**: Single-use tickets for signup
- **tokens**: Session tokens for cookie-based UI authentication

### Security

- Passwords are hashed using Argon2 (memory-hard password hashing)
- API key required for ticket generation to prevent unauthorized account creation; if `API_KEY` is unset/empty a random 64-character hex key is generated at startup
- Tickets are single-use for signup only
- Per-namespace access control
- Sessions are stored as 64-character hex tokens; a token is considered valid for **30 days from creation** (`created_at`), not from last activity

## Documentation

| Document | Description |
|----------|-------------|
| [Git Backend](docs/git-backend.md) | Git HTTP backend usage and workflows |
| [UI Documentation](docs/ui.md) | Web interface guide and page descriptions |
| [Environment Variables](docs/environment-variables.md) | Configuration options reference |

## Source Layout

```
src/
├── assets.rs        # Static asset embedding (t.css, h.js, hx-response-targets.js, fig.svg)
├── config.rs        # Server config from env (Server struct, from_env, maybe_reset_database)
├── git_backend.rs   # Git smart-HTTP handler
├── main.rs          # App wiring, routes, native Actix-web tests
├── auth/            # Auth types, hashing, FigContext, UI form handlers
│   ├── mod.rs
│   └── handlers.rs
├── db/              # turso/libSQL Database wrapper + per-table ops + migs migrations
│   ├── mod.rs
│   ├── migration.rs
│   ├── namespaces.rs
│   ├── tickets.rs
│   ├── tokens.rs
│   └── users.rs
├── git/             # Git repo operations (init, bare repo, repo browsing)
│   ├── mod.rs
│   ├── bare.rs
│   └── repo.rs
├── md/              # Markdown rendering (pulldown-cmark)
│   └── mod.rs
└── view/            # Maud HTML views (layout, overview, namespace, repo, settings, auth, session_auth)
    ├── mod.rs
    ├── overview.rs
    ├── namespace.rs
    ├── repo.rs
    ├── settings.rs
    ├── auth.rs
    └── session_auth.rs
```

## Dependencies

Key crates used:

- `actix-web`: Web framework (v4, features: macros, cookies, http2)
- `actix-multipart`: Multipart form support
- `maud`: HTML templating (v0.27, with `actix-web` feature)
- `git2`: Git operations (v0.18)
- `xshell`: Shell command execution (used by `git::repo::init`)
- `turso`: libSQL client for local SQLite database (auth data)
- `migs`: Compile-time SQL migrations macro (used in `src/db/migration.rs`)
- `argon2`: Password hashing
- `base64`: Base64 encoding/decoding (Basic Auth)
- `chrono`: Date/time handling
- `pulldown-cmark`: Markdown rendering (`src/md/mod.rs`)
- `uuid`: v4 IDs for users/namespaces
- `rand`: Cryptographic RNG for tokens
- `hex`: Hex-encoding for session tokens
- `toml`: TOML parsing
- `serde`: Serialization
- `tokio`: Async runtime
- `env_logger`: Logging
- `reqwest`: HTTP client (rustls-tls)