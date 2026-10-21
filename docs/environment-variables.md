# Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `BIND_ADDRESS` | IP address for the HTTP listener | `0.0.0.0` |
| `PORT` | HTTP server port | `80` |
| `LOG_LEVEL` | Logging level (error, warn, info, debug, trace) | `info` |
| `PROJECT_ROOT` | Root directory for Git repositories | `/srv/git` |
| `DB_PATH` | Path to the SQLite database file | `twig.db` |
| `API_KEY` | Secret required to generate one-time signup invites. Required in release builds. If unset in a debug build, a random key is generated but never logged; invite creation requires an explicitly configured key. | Unset in debug; required in release |
| `SESSION_KEY` | Secret used to sign/encrypt web session cookies. Set a stable value to keep sessions valid across restarts. | Random per process; sessions do not survive restart |
| `RESET_DB` | Set to exactly `true` to delete the database file and seed a development `admin` / `admin` account with automatic local login. Refused on port 80, on non-loopback binds, or in release builds. | `false` |
| `CACHE_CONTROL` | `Cache-Control` header value for versioned static assets (see [Assets](#assets)) | `public, max-age=31536000, immutable` |
| `TEST_USER` | Username allowed to access the endpoint test page and manage session PINs. Set to `true` as a shortcut for `admin`. If unset, the test page is disabled. | unset |
| `ADMIN_USER` | Username allowed to inspect database tables and values in Manage → Data and manage users/invites globally. If unset, global administration is disabled. | unset |
| `RESEND_API_KEY` | Secret API key used to send namespace invitation emails. If unset, invitations are saved and shown as links without attempting email delivery. | unset |
| `RESEND_FROM` | Sender address used for invitation emails; configure a sender/domain allowed by Resend. | `onboarding@resend.dev` |
| `PUBLIC_BASE_URL` | Canonical `http://` or `https://` origin used to build invitation links sent by email. Required for Resend delivery; do not include a path or query. | unset |

Store the real `RESEND_API_KEY` in the runtime environment or secret manager,
not in tracked files. Replace the sample `re_xxxxxxxxx` value with the real key
in your local deployment environment. `onboarding@resend.dev` is suitable for
Resend's test setup; production sending to arbitrary recipients requires a
verified sender domain.

For local development, `mise.toml` overrides several values, including
`BIND_ADDRESS=127.0.0.1`, `PORT=8080`, `PROJECT_ROOT=tests/git/srv`,
`RESET_DB=true`, and `TEST_USER=admin`.

## Request protections

Login and invite-generation attempts are limited to ten per minute per
connection peer and return `429` with `Retry-After` when throttled. Session
cookies use `SameSite=Lax`. Device approval POSTs require a one-time,
session-bound CSRF token. When a reverse proxy fronts Twig, add ingress rate
limits there: the application limiter sees the proxy's socket address unless
clients connect directly.

Twig adds `nosniff`, frame, referrer, cache, and Content Security Policy
headers. The current policy permits inline/evaluated scripts for existing
HTMX handlers; a stricter script policy requires moving those handlers into
external assets. The application does not terminate TLS or emit HSTS, so the
HTTPS edge must enforce TLS and add HSTS in the deployment.

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
