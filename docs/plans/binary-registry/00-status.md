# Status: Public binary registry

- Gate 1 — Product: APPROVED 2026-10-09
- Gate 2 — Architecture: APPROVED 2026-10-09
- Gate 3 — Program Design: APPROVED 2026-10-09
- Gate 4 — Slice plan: APPROVED 2026-10-09

## Slices

- [x] Slice 1 — tracer bullet: show the existing Scripts tab with a stub release and working direct download response. Verified with the focused integration test and full `cargo test` (362 passed).
- [x] Slice 2 — replace the stub with authenticated HTTP upload and SQLite BLOB download, including public/private access. Verified with focused DB/HTTP tests and full `cargo test` (364 passed).
- [x] Slice 3 — support many assets per SemVer version, retain the top three versions, and resolve latest-download URLs. Verified with SemVer/retention integration and database tests; full `cargo test` passed (368 tests).
- [x] Slice 4 — finish input/error handling, size limits, and compatibility regression coverage. Verified with the full `cargo test` suite (369 passed), `cargo fmt --check`, and `git diff --check`.

## Notes for a fresh session

- The registry is intended for binaries built locally by the repository owner, not built by Twig.
- A public repository's registry is public.
- Keep the three highest versions by semantic version ordering, retaining all uploaded assets in those versions.
- Normal Git smart pushes remain unchanged for source code. The user explicitly ruled out a Git-push option for binaries: binary publishing and retrieval use HTTP upload/download routes only; do not add Git commit/path interception or hooks.
- Gate 1 was previously reopened after the user clarified that binaries should appear within the existing Scripts tab and each version should contain multiple platform binaries; the revised Gate 1 is approved.
- The existing `.twig.toml` `[scripts]` configuration is the visibility control; do not add a separate `[binaries]` opt-in section.
- Every uploaded asset must have a direct HTTP download URL usable outside the UI, including by installer scripts; the download endpoint must not depend on the Scripts tab being visible.
- Every configured install script in the Scripts tab needs its own copy button for its runnable command.
- Do not start implementation before Gate 4 approval.
- Slice 1's tracer is complete; its hardcoded fixture has been replaced by Slice 2's real storage and routes.
- Slice 2 is complete: authenticated HTTP PUT stores SQLite BLOBs; versioned/latest GET returns the exact bytes. Public downloads work without a Scripts tab; private downloads require a session. Full `cargo test` passed (364 tests).
- Slice 3 is complete: versions normalize to SemVer, every asset for the highest three versions is retained transactionally, uploads below the retained window return 409, and latest URLs select the newest retained version that contains that filename. Full `cargo test` passed (368 tests).
- Slice 4 is complete: invalid versions and filenames are rejected, the 512 MiB upload ceiling is explicit, response headers are safe, uploads require namespace membership, private downloads require a session, and the Git receive-pack advertisement remains available with credentials. Full `cargo test` passed (369 tests).
