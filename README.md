# Fig

Fig serves Git repositories over HTTP and provides a web interface for browsing repositories.

## What This Is

Fig is a self-hosted Git backend that provides:

- **Git HTTP Backend**: Host Git repositories over HTTP with support for clone, fetch, and push operations
- **Web UI**: Browse namespaces, repositories, commits, and README files through a clean web interface

## Features

### Git Backend
- Clone and fetch repositories (public read access)
- Push to repositories (authenticated write access)
- Per-namespace access control

[Git Backend Documentation](docs/git-backend.md)

### Web UI
- Browse all namespaces with search
- View repositories within a namespace with last commit info
- View repository details including commit history and rendered README
- Create repositories through web forms
- User authentication with session-based login

[UI Documentation](docs/ui.md)

### Tickets
- Per-namespace issue tracker stored as TOML in a reserved `ticket` git repo
- Markdown bodies and comments, statuses, labels, assignees, `@mentions`
- Editable from the web UI *and* from a plain `git clone` — concurrent writes are
  merged server-side, so a push is never rejected and a pull never conflicts
- Image attachments stored outside git, content-addressed

[Tickets Documentation](docs/tickets.md)

## Quick Start

Deploy with the [once project] (https://github.com/basecamp/once)
Be sure to set follwoing environment variables:

DB_PATH=/storage/<your_choice>
PROJECT_ROOT=/storage/<your_choice>

# Optional: Sentry instrumentation (errors, traces, logs)
# SENTRY_DSN=https://<key>@o<orgId>.ingest.sentry.io/<projectId>
# SENTRY_TRACES_SAMPLE_RATE=1.0

## Documentation

| Document | Description |
|----------|-------------|
| [Git Backend](docs/git-backend.md) | Git HTTP backend usage and workflows |
| [UI Documentation](docs/ui.md)

### Tickets
- Per-namespace issue tracker stored as TOML in a reserved `ticket` git repo
- Markdown bodies and comments, statuses, labels, assignees, `@mentions`
- Editable from the web UI *and* from a plain `git clone` — concurrent writes are
  merged server-side, so a push is never rejected and a pull never conflicts
- Image attachments stored outside git, content-addressed

[Tickets Documentation](docs/tickets.md) | Web interface guide and page descriptions |
| [Tickets](docs/tickets.md) | Issue tracker: storage format, merge rules, CLI workflow |
| [Environment Variables](docs/environment-variables.md) | Configuration options reference |

## Authentication Overview

Fig uses multiple authentication methods depending on the action:

| Action | Authentication Method |
|--------|----------------------|
| Generate signup invite | API Key (admin only) |
| Create account | Single-use invite |
| Log in (UI) | Form submission → sets session cookie |
| Create namespace | session cookie (UI) |
| Create repository | Basic Auth |
| Git clone/fetch | None (public read) |
| Git push | Basic Auth via git |
| Log out |session cookie |

### Registration Flow

1. **Admin** generates a signup invite using the API Key
2. **User** creates an account with the invite (single-use, consumed on signup)
3. **User** logs in
4. **User** can now create namespaces and repositories

See the individual documentation files for detailed authentication flows.
