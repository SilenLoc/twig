mod docker 'docker.just'
mod hurl 'tests/hurl.just'
mod git 'tests/git.just'

export PROJECT_ROOT := "tests/git/srv"
export API_KEY := "secure"
export RESET_DB := 'true'
export PORT := "8080"

run:
    cargo run

kill:
    #!/usr/bin/env bash
    lsof -ti:8080 | xargs -r kill -9 || true

alias v := verify

up:
    just kill
    just docker stop
    just docker run
    just hurl test
    echo URL: http://localhost:8080

down:
    just docker stop

verify:
    cargo fmt -- --check
    cargo check
    cargo clippy
    cargo test
    just docker run
    just hurl test
    just docker stop

fmt:
    cargo fmt
    cargo clippy --fix --allow-dirty
