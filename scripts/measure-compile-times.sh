#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
output_dir="$(mktemp -d "${TMPDIR:-/tmp}/twig-compile-times.XXXXXX")"
durations=()

printf 'Repository: %s\n' "$repo_root"
printf 'Build reports and isolated targets: %s\n' "$output_dir"

for run in 1 2 3 4 5; do
    target_dir="$output_dir/run-$run/target"
    mkdir -p "$target_dir"

    printf '\nRun %s/5\n' "$run"
    start_ns="$(date +%s%N)"
    CARGO_TARGET_DIR="$target_dir" cargo build --timings --manifest-path "$repo_root/Cargo.toml"
    end_ns="$(date +%s%N)"

    duration="$(awk -v start="$start_ns" -v end="$end_ns" \
        'BEGIN { printf "%.3f", (end - start) / 1000000000 }')"
    durations+=("$duration")
    printf 'Elapsed: %s seconds\n' "$duration"

    report_dir="$target_dir/cargo-timings"
    if [[ -d "$report_dir" ]]; then
        find "$report_dir" -maxdepth 1 -type f -print
    else
        printf 'Warning: Cargo did not create a timing report in %s\n' "$report_dir" >&2
    fi
done

median="$(printf '%s\n' "${durations[@]}" | LC_ALL=C sort -n | awk 'NR == 3 { print; exit }')"

printf '\nDurations (seconds): %s\n' "${durations[*]}"
printf 'Median clean-build time: %s seconds\n' "$median"
printf 'Reports and target directories retained at: %s\n' "$output_dir"
