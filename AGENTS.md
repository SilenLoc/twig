# AGENTS.md

Twig is a Git server and web UI: Rust, Actix-web, maud templates, SQLite (bundled `rusqlite`) over a local database file. The shipped app includes namespace/repository browsing and management, Git smart HTTP, auth, admin database inspection, and an optional endpoint test page.

Never try refactor to connect to the sql lite file eagerly.
