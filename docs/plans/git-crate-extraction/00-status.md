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

The requested crate name is `twig-git`, located under `crates/`. Do not eagerly connect to the SQLite-compatible database as part of this refactor. Slice 1 passed its crate and editor tests. Slice 2 moved repository access/config/history/listing, validation, initialization/hooks, and management operations into `twig-git`. Slice 3 moved the CGI smart-HTTP backend into `twig-git`, kept only the Actix/auth/database adapter in `src/git_http.rs`, removed the in-app `src/git/` module, and placed Git implementation dependencies in the new crate. Final verification: `cargo test --workspace` passed (353 Twig, 63 twig-git, 11 twig-highlight tests); `cargo check -p twig --bin twig`, formatting, and diff checks passed. `cargo tree -p twig --edges normal -i git2` shows `git2` under `twig-git`, not as a direct Twig dependency.
