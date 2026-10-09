# Slices: Faster compile times

## Baseline captured in Slice 1

- Clean-build durations: 26.827s, 27.606s, 27.830s, 26.593s, 26.803s.
- Median: 26.827s.
- Toolchain: rustc/cargo 1.97.1, x86_64 Linux.
- Cargo reports and isolated target directories: `/tmp/twig-compile-times.lVwzqU` (8.5 GB, retained).

## Slice 1 — Tracer bullet: runnable clean-build benchmark

Add `scripts/measure-compile-times.sh`, run it end to end for five isolated clean builds, and show the five durations, median, and saved Cargo timing report locations. This produces the baseline measurement without changing application behavior.

## Slice 2 — Determine whether macro work is a justified target

Inspect the baseline Cargo timing reports and, if available, collect Rust compiler self-profile data. Determine whether the root crate is worth optimizing and whether there is evidence to support a narrow Maud-renderer experiment. If the candidate is not supported, stop and re-steer rather than making an application change.

### Result

The `twig` compile unit took 2.9–3.3 seconds across the five reports (about 11% of the 26.6–27.8 second total clean builds). `turso_core` alone took 16.2 seconds in the first report. Even removing the entire `twig` compilation could not satisfy the approved 20% clean-build reduction, so reducing macros within Twig is not a justified path to this metric. No application code was changed. Re-steer the success metric to incremental builds or broaden the optimization scope beyond Twig source before continuing.

## Slice 3 — One conditional macro-reduction experiment

Only if Slice 2 supports it, reduce macro use in the single selected renderer while preserving its existing function signature and HTML behavior. Add or update focused regression coverage for document structure and escaping, then run the relevant tests and `cargo test --workspace`.

## Slice 4 — Measure and decide

Run the same five-build benchmark with the same toolchain and conditions. Compare medians and timing reports to baseline. Keep the change only if it improves compile time without correctness regressions; claim success against the product goal only if the median clean-build time improves by at least 20%. If it does not, report the result and re-steer rather than expanding the rewrite.
