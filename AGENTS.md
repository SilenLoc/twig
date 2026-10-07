# AGENTS.md

Twig is a Git server and web UI: Rust, Actix-web, maud templates, Turso/libSQL over a local SQLite-compatible file. The shipped app includes namespace/repository browsing and management, Git smart HTTP, auth, admin database inspection, an optional endpoint test page, and a MessagePack tree endpoint.

Never try refactor to connect to the sql lite file eagerly.

