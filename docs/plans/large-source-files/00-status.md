# Status: Smaller, clearer source modules

- Gate 1 — Product: APPROVED 2026-10-17
- Gate 2 — Architecture: APPROVED 2026-10-17
- Gate 3 — Program Design: APPROVED 2026-10-17
- Gate 4 — Slice plan: APPROVED 2026-10-17

## Slices

- [x] Slice 1 — tracer: move Git repository tests into a child test module.
- [x] Slice 2 — extract database unit tests.
- [x] Slice 3 — extract Git and syntax-highlighter unit tests.
- [x] Slice 4 — extract auth and namespace page tests.
- [x] Slice 5 — extract remaining oversized HTTP module tests.
- [x] Slice 6 — split integration tests by feature with shared support.
- [x] Slice 7 — split settings, test-page, tree, user-management, and editor production modules.
- [x] Slice 8 — split repository page behavior and tests by view responsibility.
- [x] Slice 9 — review near-threshold files and run final verification/audit.

## Notes for a fresh session

- The user asked to reduce the size of the files in their Marg inventory by moving tests or splitting code meaningfully.
- This is about module contents and discoverability, distinct from the completed `module-path-refactor` plan.
- Preserve existing worktree changes; do not eagerly connect to the SQLite-compatible database.
- All four gates are approved. The user explicitly approved all slices and asked to proceed without pausing for per-slice approval.
- Final verification: `cargo test --workspace --all-targets` passed (413 app tests + 11 highlighter tests); formatting and diff checks passed.
- Final Marg audit: CLEAR, 114 Rust files measured, zero files above 500 lines, zero suppression blockers. The estimated complexity maximum remains 27; complexity is a review signal, not a target to minimize mechanically.
- Clippy completes with pre-existing warnings in database, Git, and invitation code; this refactor adds no Clippy suppression.
- No routes, endpoint behavior, schemas, or database initialization timing were intentionally changed.
