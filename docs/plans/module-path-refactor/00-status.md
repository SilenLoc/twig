# Status: Easier-to-navigate Twig modules

- Gate 1 — Product: APPROVED 2026-10-09
- Gate 2 — Architecture: APPROVED 2026-10-09 (HTTP boundary, then feature organization)
- Gate 3 — Program Design: APPROVED 2026-10-09 (revised for feature modules)
- Gate 4 — Slice plan: APPROVED 2026-10-09 (feature organization follow-up)

## Slices

- [x] Slice 1 — group auth handlers/pages/session helper under `http/auth/`.
- [x] Slice 2 — group the tree API and pages under `http/tree/`.
- [x] Slice 3 — group repository pages and binary handlers under `http/repository/`.
- [x] Slice 4 — organize remaining page modules by feature and keep only shared render helpers in `http/view`.

## Notes for a fresh session

- Goal stated by the user: refactor Twig's module paths so it is easy to find where something is implemented.
- Keep the product framing focused on code discoverability; implementation decisions belong in later gates.
- User clarified the desired sequence: keep the HTTP boundary first, then organize modules by feature inside it. They asked not to pause for approvals; complete the revised slices in order and prove each one.
- The initial HTTP-boundary refactor is complete; this follow-up adds feature-oriented submodules beneath that boundary.
- Initial verification: `cargo test` passed all 369 tests; `cargo fmt --check` passed; `cargo clippy --all-targets` completed with existing pedantic warnings.
- Feature-organization verification: all 369 tests pass; `cargo fmt --check` passes; `cargo clippy --all-targets` completes with the same existing pedantic warnings; no old feature-under-`http::view` paths remain.
- No endpoint, response, or database behavior was intentionally changed; database initialization remains lazy.
