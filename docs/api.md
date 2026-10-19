# API

Twig exposes a small set of HTTP endpoints for scripts and integrations.

## Version

`GET /api/version` returns the version of the running Twig application as JSON:

```json
{"version":"0.1.104"}
```

The version is taken from the application package at build time.
