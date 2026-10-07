# Twig

Twig is a self-hosted Git server with a web interface for browsing and managing
repositories. It is a single Rust/Actix-web application backed by a local
SQLite-compatible database (Turso/libSQL) and Git's smart-HTTP backend.

## Features

- Public namespace and repository browsing, with search, Markdown rendering,
  Mermaid diagrams, file browsing, commit history, license display, optional
  slide presentations, and an optional long-form Paper reader configured in
  `.twig.toml`.
- A configurable Scripts tab: group repository scripts in `.twig.toml` and
  readers run them with one copied `curl … | bash` command.
- Git clone/fetch over HTTP; pushes use HTTP Basic Auth. Repositories can be
  made private in `.twig.toml`.
- Account signup by one-time invite, session-based web login, owned namespaces,
  and repository creation from the UI or Git push.
- Settings for updating email, moving repositories between your namespaces, and
  deleting repositories/namespaces.
- Optional endpoint test runner (`TEST_USER`) and read-only database browser
  (`ADMIN_USER`).
- API endpoints for the application version (`GET /api/version`) and a
  MessagePack namespace/repository tree (`GET /api/tree`).

## Quick start

Run locally with [mise](https://mise.jdx.dev/):

```sh
mise run run
```

The development configuration sets `PROJECT_ROOT=tests/git/srv`,
`RESET_DB=true`, `PORT=8080`, and `TEST_USER=admin`. With `RESET_DB=true`, Twig
resets its local database and seeds an `admin` / `admin` account for local
browsing. Do not use this mode for persistent data.

The container listens on port 80. Persist both the Git root and database file
when deploying, and set `SESSION_KEY` to a stable secret so browser sessions
survive restarts. For example:

```sh
docker run --rm -p 8080:80 \
  -e PROJECT_ROOT=/data/git \
  -e DB_PATH=/data/twig.db \
  -e SESSION_KEY='replace-with-a-long-random-secret' \
  -e API_KEY='replace-with-a-secret' \
  -v twig-data:/data \
  silenloc/twig
```

See [Environment Variables](docs/environment-variables.md) for the complete
configuration reference.

## Documentation

| Document | Description |
|----------|-------------|
| [API](docs/api.md) | Version and namespace/repository tree endpoints |
| [Git Backend](docs/git-backend.md) | Clone, fetch, push, authentication, and repository creation |
| [Web UI](docs/ui.md) | Pages, account access, settings, and optional admin tools |
| [Environment Variables](docs/environment-variables.md) | Runtime configuration |

These Markdown files are also embedded in the application and shown under
Docs at `/_info`, where each page can be copied as Markdown.

## Authentication overview

| Action | Authentication |
|--------|----------------|
| Browse public namespaces/repositories | None |
| Read a private repository | HTTP Basic Auth for Git; web login for the UI |
| Generate a signup invite | `API_KEY` entered on the invite page |
| Sign up | Unused one-time invite |
| Create namespace/repository in the UI | Web session; repository creation also requires an email on the account |
| Push over Git HTTP | HTTP Basic Auth and access to the namespace |
| Read database tables | Logged-in user named by `ADMIN_USER` |
| Use the endpoint test page | Logged-in user named by `TEST_USER`, or a valid session PIN |

Git reads are public for public repositories. An authenticated push can create
a missing namespace and repository; the new namespace belongs to the pushing
user. Namespaces and repositories created in the web UI belong to the logged-in
user.
