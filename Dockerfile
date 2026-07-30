# Base builder stage with cargo-chef for dependency caching
FROM rust:slim-bookworm AS base-builder

WORKDIR /app

# Install cargo-chef for dependency caching
RUN cargo install cargo-chef

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

# Planner stage - analyzes dependencies
FROM base-builder AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage - builds with cached dependencies
FROM base-builder AS builder

# Copy dependency recipe (cached layer if dependencies haven't changed)
COPY --from=planner /app/recipe.json recipe.json

# Copy vendored path dependencies and lockfile required by cargo-chef cook.
# These must be present because Cargo.toml references `vendor/uuid` as a
# path dependency, so cargo resolves it before cooking can begin.
COPY vendor ./vendor
COPY Cargo.toml Cargo.lock ./

# Build dependencies (cached if recipe.json is unchanged)
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source files (invalidates cache only when source changes)
COPY assets ./assets
COPY src ./src

# Build the project (only compiles project code, not dependencies)
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/release/fig /app/

# Expose port 80
EXPOSE 80

# Run the binary
ENTRYPOINT ["./fig"]
