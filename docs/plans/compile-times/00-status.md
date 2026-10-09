# Status: Faster compile times

- Gate 1 — Product: APPROVED 2026-10-09
- Gate 2 — Architecture: APPROVED 2026-10-09
- Gate 3 — Program Design: APPROVED 2026-10-09
- Gate 4 — Slice plan: APPROVED 2026-10-09

## Slices

- [x] Slice 1 — tracer bullet: five-run clean-build benchmark is runnable and baseline captured.
- [x] Slice 2 — Cargo timing data shows a macro-focused source change cannot meet the approved 20% clean-build target; no application change made.

## Notes for a fresh session

- The user wants to explore restructuring Twig to improve compile times, starting with using fewer macros.
- Keep macro usage as a candidate approach to assess in Gate 2, not a predetermined solution.
- No application implementation before Gate 4 approval; Slice 1 is limited to the measurement script.
- Baseline with `scripts/measure-compile-times.sh`: 26.827s median across 26.827, 27.606, 27.830, 26.593, and 26.803 seconds.
- Toolchain: rustc/cargo 1.97.1 on x86_64 Linux. Baseline Cargo reports and isolated targets are at `/tmp/twig-compile-times.lVwzqU` (8.5 GB; retained as planned).
- Cargo reports show the root `twig` compile unit at 2.9–3.3s across the five runs (about 11% of a build); `turso_core` alone takes 16.2s in the first report. Even eliminating all root-crate compile time would not reach the approved 20% clean-build target, and macros are only part of that crate's compilation.
- Slice 2 stopped before an application-code change. Re-steer the success metric to incremental builds or broaden the investigation beyond Twig source before continuing.
