| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | Server port | `8080` |
| `LOG_LEVEL` | Logging level (error, warn, info, debug, trace) | `info` |
| `PROJECT_ROOT` | Root directory for Git repositories | `/srv/git` |
| `DB_PATH` | Path to the SQLite database file | `fig.db` |
| `ATTACHMENT_ROOT` | Root directory for ticket image attachments (stored outside Git) | `/srv/attachments` |
| `API_KEY` | API key for user signup endpoint | Auto-generated |
| `SESSION_KEY` | Secret used to sign and encrypt identity session cookies; set a strong value and keep it stable across restarts | Random per process |
| `RESET_DB` | Set to `true` to delete the database on startup | `false` |
| `CACHE_CONTROL` | `Cache-Control` header value for static assets | `public, max-age=31536000, immutable` |
| `TEST_USER` | Admin username allowed to access the rapid endpoint test page. If unset, the test page is disabled | unset |
| `ADMIN_USER` | Username allowed to inspect database tables and values in Tree → Data. If unset, the page is disabled | unset |
| `SENTRY_DSN` | Sentry project DSN. If unset, Sentry instrumentation is disabled | unset |
| `SENTRY_TRACES_SAMPLE_RATE` | Fraction of transactions sent to Sentry (0.0–1.0) | `1.0` |
