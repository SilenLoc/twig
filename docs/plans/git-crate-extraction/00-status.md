# Status: Git crate extraction

- Gate 1 — Product: APPROVED 2026-10-17
- Gate 2 — Architecture: APPROVED 2026-10-17
- Gate 3 — Program Design: APPROVED 2026-10-17 (re-approved with public repository API additions)
- Gate 4 — Slice plan: APPROVED 2026-10-17

## Slices

- [x] Slice 1 — Add `twig-git` and wire repository path validation through it end to end.
- [x] Slice 2 — Move repository storage, config, history, initialization, and management behind the crate API.
- [x] Slice 3 — Move the smart-HTTP backend, finish app adapter integration, and remove the in-app Git module.

## Notes for a fresh session

The requested crate name is `twig-git`, located under `crates/`. Do not eagerly connect to the SQLite-compatible database as part of this refactor. Slice 1 passed its crate and editor tests. Slice 2 moved repository access/config/history/listing, validation, initialization/hooks, and management operations into `twig-git`. The app's Actix/auth/database Git adapter remains at `src/git/mod.rs`; smart HTTP is handled by `twig-git`'s CGI adapter, which invokes the installed `git http-backend` command. `crates/twig-http` remains a separate in-process implementation and is not used by the app. Git implementation dependencies are owned by the relevant workspace crates. Verification on 2026-10-17: `cargo test --workspace` passed (414 Twig, 63 twig-git, 11 twig-highlight, 8 twig-http unit tests, plus 1 twig-http integration test). `cargo tree -p twig --edges normal -i git2` shows `git2` under `twig-git`, not as a direct Twig dependency.
