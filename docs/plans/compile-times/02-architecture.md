# Architecture: Faster compile times

## Fit

Twig is a Cargo workspace with the server application at the root and one library crate, `crates/twig-highlight`. The root `src/main.rs` declares the server modules and registers routes. The server-side Maud templates live under `src/view/`; `src/view/repo.rs` contains the largest concentration of `maud::html!` invocations found in the initial scan (32 non-test invocations, with additional test invocations).

The work should first use Cargo's existing build timing reports to establish the root crate's share of clean-build time and whether optimizing Twig's code is likely to matter. Cargo timings are crate-level: they do not attribute time to an individual module or macro. If the root crate is a meaningful cost, use Rust compiler self-profiling where the available toolchain supports it, or validate a narrowly scoped macro change with a controlled before/after build comparison. The Maud-heavy `src/view/repo.rs` is a candidate to investigate, not a conclusion. Do not remove macros across the application based on counts alone. A new crate or broad module split is not justified unless measured results show it can improve the approved clean-build metric; an extra crate boundary can add compilation work and is not automatically faster.

Compare before and after using the same toolchain, machine, build command, and clean-build conditions, with five runs for each and the median as the result. Retain the approved target of at least a 20% median clean-build reduction. Use Cargo timings for crate-level attribution and compiler profiling or a controlled code-change comparison for macro attribution; source-level macro counts alone are insufficient.

## Endpoints

None.

## Data

None.

## Flow

1. Capture five clean-build durations and Cargo timing reports for the current workspace.
2. Check whether the root crate's compilation time is large enough to target. If needed and supported by the toolchain, collect compiler self-profile data; otherwise use a controlled, narrowly scoped experiment to assess a candidate macro change.
3. If evidence points to a macro-heavy area, choose one small change that preserves the rendered output and behavior; otherwise, propose a different measured optimization.
4. Run the existing validation for the change and repeat the same five clean builds.
5. Compare medians and timing attribution against the baseline; keep the change only if it provides a real improvement without regressions.

## External

None. Use the installed Rust toolchain and local Cargo timing/profiling reports; no external service or environment variables are required. Compiler self-profiling may require a nightly toolchain; if so, treat it as diagnostic only and keep the approved build-time comparison consistent on one toolchain.
