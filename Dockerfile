# syntax=docker/dockerfile:1
#
# Build strategy: BuildKit cache mounts instead of cargo-chef.
#
# The cargo registry and the `target/` directory are persisted in BuildKit
# cache mounts, so a release build only recompiles the `fig` crate itself —
# all 340+ dependency artifacts are reused from the previous build. That makes
# cargo-chef (and the `cargo install cargo-chef` + full `chef cook` passes it
# needs) redundant on a machine with a persistent BuildKit cache, which is how
# `mise run release` builds.
#
# Note: cache mounts are local to the BuildKit daemon. A cold machine (fresh
# CI runner) pays one full dependency build; every build after that is warm.

FROM rust:slim-bookworm AS builder

WORKDIR /app

# mold links the final binary several times faster than GNU ld.
# gcc 12 (bookworm) drives it via -fuse-ld=mold, so no clang needed.
# No pkg-config/libssl-dev: git2 is built with default-features off and
# vendors libgit2, and nothing in the tree links OpenSSL.
RUN apt-get update \
    && apt-get install -y --no-install-recommends mold \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold" \
    CARGO_INCREMENTAL=0 \
    CARGO_NET_RETRY=10 \
    CARGO_TERM_COLOR=always

COPY Cargo.toml Cargo.lock ./
COPY assets ./assets
COPY src ./src

# `target/` lives in the cache mount and is therefore not part of the layer —
# the binary has to be copied out before the mount is unmounted.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    cargo build --release \
    && cp target/release/fig /usr/local/bin/fig

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# `git` is required at runtime for the smart-HTTP backend.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/fig /app/fig

# Expose port 80
EXPOSE 80

# Run the binary
ENTRYPOINT ["./fig"]
