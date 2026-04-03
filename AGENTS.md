# AGENTS.md

This file contains guidelines for AI coding agents working on this repository.

## Project Overview

Fig is a Git server and web UI built with Rust (Actix-web). It serves Git repositories over HTTP and provides a web interface for browsing repositories.

## Build/Lint/Test Commands

### Essential Commands (use these via `just`)

```bash
# Full verification (runs all checks, tests, and hurl tests)
just verify

# Format code
just fmt

# Run the application
just run

# Run hurl acceptance tests (requires server running)
just hurl test

# Run a single hurl test file
just hurl test health.hurl

# Start/stop Docker environment
just up      # kill, docker stop, docker run, hurl test
just down    # docker stop
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

# Run a specific test
cargo test <test_name>

# Build release
cargo build --release
```

### Docker Commands

```bash
# Build Docker image
just docker build

# Run Docker container
just docker run

# Stop Docker container
just docker stop

# Build and push release
just docker release <version>
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

- **Always use `htm` and `t.css` for creating views** - do not change CSS without reason
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

### Project Structure

```
src/
├── main.rs           # Application entry, route setup
├── config.rs         # Configuration and env vars
├── assets.rs         # Static assets (CSS, JS)
├── git/              # Git-related modules
│   ├── mod.rs        # Git HTTP handlers
│   ├── repo.rs       # Repository management
│   └── bare.rs       # Bare repo operations
└── view/             # Web UI views
    ├── mod.rs        # Layout and index
    ├── repo.rs       # Repository view
    └── namespace.rs  # Namespace view
```

### Testing

- **Always use hurl commands/recipes to run format and test**
- Hurl tests are in `tests/*.hurl`
- Variables in `tests/variables`
- Run `just hurl test` for acceptance tests
- To run a single test: `just hurl test <file.hurl>`

### Environment Variables

- `PORT`: Server port (default: 8080)
- `LOG_LEVEL`: Logging level (default: info)
- `PROJECT_ROOT`: Git repositories root path (default: /srv/git)

### Git Workflow

- Do not run `git commit`, `git push`, or destructive git operations unless explicitly asked
- The project uses semantic-release for automated versioning

## Dependencies

Key crates used:
- `actix-web`: Web framework
- `maud`: HTML templating
- `git2`: Git operations
- `xshell`: Shell command execution
- `serde`: Serialization
- `chrono`: Date/time handling
