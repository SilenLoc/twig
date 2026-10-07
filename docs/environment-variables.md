# Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | HTTP server port (binds on `0.0.0.0`) | `80` |
| `LOG_LEVEL` | Logging level (error, warn, info, debug, trace) | `info` |
| `PROJECT_ROOT` | Root directory for Git repositories | `/srv/git` |
| `DB_PATH` | Path to the SQLite database file | `twig.db` |
| `API_KEY` | Secret required to generate one-time signup invites. If unset or empty, a random key is generated and logged at startup. | Generated per process |
| `SESSION_KEY` | Secret used to sign/encrypt web session cookies. Set a stable value to keep sessions valid across restarts. | Random per process; sessions do not survive restart |
| `RESET_DB` | Set to exactly `true` to delete the database file at startup and seed a development `admin` / `admin` account with automatic local login. | `false` |
| `CACHE_CONTROL` | `Cache-Control` header value for versioned static assets (see [Assets](#assets)) | `public, max-age=31536000, immutable` |
| `TEST_USER` | Username allowed to access the endpoint test page and manage session PINs. Set to `true` as a shortcut for `admin`. If unset, the test page is disabled. | unset |
| `ADMIN_USER` | Username allowed to inspect database tables and values in Tree → Data. If unset, database browsing is disabled. | unset |

For local development, `mise.toml` overrides several values, including
`PORT=8080`, `PROJECT_ROOT=tests/git/srv`, `RESET_DB=true`, and
`TEST_USER=admin`.

## Assets

Pages link assets under their version, e.g. `/assets/twig-0.1.123.css`, so a
browser only reuses its cached copy while that version still matches: a new
release requests a new URL and fetches fresh files, while every release keeps
its own cache entry. Versioned URLs receive the configured `CACHE_CONTROL`
(`immutable` by default); the unversioned form (`/assets/twig.css`) still works
for external references such as `twig.schema.json`, but is served with
`Cache-Control: no-cache` because one URL would otherwise have to change content
between releases. Asset URLs from a different version return 404.

Bumping the version in `Cargo.toml` is what invalidates browsers' asset caches;
run `mise run release` to publish one.
