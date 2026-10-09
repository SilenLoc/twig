# Program Design: Faster compile times

## Files

- `docs/plans/compile-times/00-status.md` — track gate approvals and slice completion.
- `docs/plans/compile-times/03-program-design.md` — record this implementation design.
- `scripts/measure-compile-times.sh` — new repeatable benchmark: run five clean builds in isolated target directories, collect wall-clock durations and Cargo timing reports, and print the median plus report locations. Keep output under a unique temporary directory and do not remove existing build artifacts.
- `src/view/repo.rs` — conditional candidate only: if compiler profiling or a controlled experiment identifies Maud expansion in the PDF renderer as worthwhile, change `render_pdf_document` without changing its signature and add a focused regression test. If evidence points elsewhere, stop and revise this design before touching application code.

No route, data, dependency, or configuration changes are planned.

## Types & signatures

Existing renderer boundary to preserve if the candidate is validated:

```rust
fn render_pdf_document(
    namespace: &str,
    repo: &str,
    slides: &[PresentSlide],
    license: Option<&str>,
) -> Markup;
```

Benchmark script interface:

```text
measure-compile-times.sh
  -> five clean `cargo build --timings` runs
  -> per-run elapsed seconds, median elapsed seconds, retained report paths
```

The benchmark creates a distinct target directory per run so the runs measure clean builds. It uses the same command and toolchain for baseline and comparison.

## Call stack

### Baseline and comparison

1. Contributor runs `scripts/measure-compile-times.sh` from the repository.
2. The script creates a uniquely named temporary output directory and five fresh Cargo target directories beneath it.
3. Each run invokes `cargo build --timings` with its own target directory and records elapsed wall time.
4. The script prints all five durations, their median, and the Cargo timing report paths.
5. Contributor compares baseline and post-change medians and inspects crate-level timing reports. Use Rust compiler self-profile data if available to investigate macro expansion; Cargo timing reports alone are not module- or macro-level attribution.

### Conditional PDF renderer change

1. Existing `present_print_handler` calls `render_pdf_document`.
2. `render_pdf_document` renders the standalone document and delegates slide/license-page markup to existing helpers.
3. Existing focused unit tests assert document structure and behavior; add an escaping/trusted-markup regression assertion if the implementation changes the rendering mechanism.

## Test plan

- `benchmark_script_passes_shell_syntax_check` — `bash -n` accepts the script.
- `benchmark_runs_five_isolated_clean_builds` — an end-to-end run produces five duration values and five timing reports, with a distinct fresh target directory for each run.
- `benchmark_reports_the_median_correctly` — the displayed median matches the middle duration after sorting the five values.
- `test_print_export_renders_one_page_per_slide_with_the_license_last` — existing behavior remains intact after any renderer change.
- `test_print_export_opens_the_dialog_once_loaded` — font/image wait and print hint remain intact.
- `test_print_export_is_a_chrome_free_document` — standalone document shape, empty-deck behavior, and no-print-on-empty behavior remain intact.
- `test_print_export_escapes_dynamic_title_and_preserves_trusted_license_markup` — if rendering changes, dynamic title input remains escaped while intentionally pre-escaped license HTML remains markup.
- `cargo test --workspace` — all existing application and `twig-highlight` tests pass.
- `benchmark_meets_approved_compile_time_target` — after the change, the median of five clean builds is at least 20% below the baseline under the same conditions; otherwise report that the target was not met and do not claim a win.

## Least confident decisions

1. The root Twig crate may not be a large part of clean-build time; dependency compilation may dominate.
2. The compiler toolchain available in the environment may not support useful self-profile attribution, leaving a controlled A/B build as the practical evidence.
3. `render_pdf_document` may not be a significant compile-time hotspot despite being a sizeable Maud template; it must remain conditional on evidence.
4. Replacing its Maud rendering with a non-macro approach may make escaping and markup maintenance harder than the compile-time benefit warrants.
5. Five-run medians may still be noisy; runs must be comparable, and a small improvement must not be mistaken for the approved 20% target.
