# API

Fig exposes a small set of HTTP endpoints for scripts and integrations.

## Version

`GET /api/version` returns the version of the running Fig application as JSON:

```json
{"version":"0.1.104"}
```

The version is taken from the application package at build time.

## Namespace and repository tree

`GET /api/tree` returns the visible namespaces and repositories as named
MessagePack (`application/msgpack`). Public repositories are included for
anonymous requests. A signed-in user can also see private repositories they
have access to.
