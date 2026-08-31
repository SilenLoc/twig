| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | Server port | `8080` |
| `LOG_LEVEL` | Logging level (error, warn, info, debug, trace) | `info` |
| `PROJECT_ROOT` | Root directory for Git repositories | `/srv/git` |
| `DB_PATH` | Path to the SQLite database file | `fig.db` |
| `ATTACHMENT_ROOT` | Root directory for ticket image attachments (stored outside Git) | `/srv/attachments` |
| `API_KEY` | API key for user signup endpoint | Auto-generated |
| `RESET_DB` | Set to `true` to delete the database on startup | `false` |
| `SENTRY_DSN` | Sentry project DSN. If unset, Sentry instrumentation is disabled | unset |
| `SENTRY_TRACES_SAMPLE_RATE` | Fraction of transactions sent to Sentry (0.0–1.0) | `1.0` |
