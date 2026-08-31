#!/usr/bin/env bash
# Authorization matrix for ticket writes.
#
# Proves that only members of a namespace can change its ticket content, over
# every write path: the web UI endpoints, attachment upload, and git push.
# Includes positive controls, so a wall of 403s cannot be mistaken for success
# when the endpoints are simply broken.
#
# Usage:  bash tests/scripts/ticket_authorization.sh

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d /tmp/fig-ticket-authz-XXXXXX)"
# `PORT` is deliberately not consulted: it is commonly already exported.
PORT="${FIG_TEST_PORT:-$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')}"
API_KEY="secure"
BASE="http://127.0.0.1:${PORT}"
PASSWORD="password123"

FAILURES=0
SERVER_PID=""

pass() { printf '  \033[32mok\033[0m   %s\n' "$1"; }
fail() { printf '  \033[31mFAIL\033[0m %s\n' "$1"; FAILURES=$((FAILURES + 1)); }
step() { printf '\n\033[1m%s\033[0m\n' "$1"; }

cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null
    wait "$SERVER_PID" 2>/dev/null
    rm -rf "$WORK"
}
trap cleanup EXIT

# ---------- server ----------

cargo build --quiet --manifest-path "$REPO_ROOT/Cargo.toml" 2>"$WORK/build.log" \
    || { echo "build failed"; cat "$WORK/build.log"; exit 1; }

mkdir -p "$WORK/git" "$WORK/attach"
PORT="$PORT" PROJECT_ROOT="$WORK/git" DB_PATH="$WORK/fig.db" ATTACHMENT_ROOT="$WORK/attach" \
API_KEY="$API_KEY" RESET_DB=true LOG_LEVEL=warn \
    "$REPO_ROOT/target/debug/fig" >"$WORK/server.log" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 60); do
    curl -fsS --max-time 2 -X POST "$BASE/auth/invite" -d "api_key=$API_KEY" >/dev/null 2>&1 && break
    sleep 0.5
done
curl -fsS --max-time 2 -X POST "$BASE/auth/invite" -d "api_key=$API_KEY" >/dev/null 2>&1 \
    || { echo "server never became ready"; cat "$WORK/server.log"; exit 1; }

new_user() {
    local username="$1" invite
    invite="$(curl -fsS -X POST "$BASE/auth/invite" -d "api_key=$API_KEY" | grep -oE '[0-9a-f]{64}' | head -1)"
    curl -fsS -X POST "$BASE/auth/signup" -d "invite=$invite" -d "username=$username" \
        -d "email=$username@example.com" -d "password=$PASSWORD" >/dev/null
    curl -fsS -c "$WORK/$username.jar" -X POST "$BASE/auth/login" \
        -d "username=$username" -d "password=$PASSWORD" >/dev/null
}

step "Setup: alice owns 'acme', bob owns 'bobcorp'"
new_user alice
new_user bob
curl -fsS -b "$WORK/alice.jar" -X POST "$BASE/auth/namespace" -d "name=acme" >/dev/null
curl -fsS -b "$WORK/bob.jar" -X POST "$BASE/auth/namespace" -d "name=bobcorp" >/dev/null
# a ticket to aim the comment/status endpoints at
curl -fsS -b "$WORK/alice.jar" -X POST "$BASE/acme/tickets" \
    -d "title=Owner ticket" -d "markdown=body" >/dev/null
curl -fsS -b "$WORK/bob.jar" -X POST "$BASE/bobcorp/tickets" \
    -d "title=Bob ticket" -d "markdown=body" >/dev/null
pass "two namespaces, each with one ticket"

printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89' > "$WORK/shot.png"

# ---------- helpers ----------

# status_of <jar-or-"none"> <method-args...>
status_of() {
    local jar="$1"; shift
    if [ "$jar" = "none" ]; then
        curl -s -o /dev/null -w '%{http_code}' "$@"
    else
        curl -s -o /dev/null -w '%{http_code}' -b "$jar" "$@"
    fi
}

# expect <expected> <actual> <label>
expect() {
    if [ "$2" = "$1" ]; then pass "$3 -> $2"; else fail "$3 (expected $1, got $2)"; fi
}

# Runs the four UI write endpoints against namespace $2 as identity $1.
# Prints the four status codes.
write_attempts() { # <jar|none> <namespace>
    local jar="$1" ns="$2"
    echo "$(status_of "$jar" -X POST "$BASE/$ns/tickets" -d "title=x" -d "markdown=y")" \
         "$(status_of "$jar" -X POST "$BASE/$ns/tickets/1/comment" -d "markdown=x")" \
         "$(status_of "$jar" -X POST "$BASE/$ns/tickets/1/status" -d "status=closed")" \
         "$(status_of "$jar" -F "file=@$WORK/shot.png" "$BASE/$ns/ticket/attachment")"
}

# ---------- anonymous ----------

step "Anonymous users cannot write"
read -r C1 C2 C3 C4 <<<"$(write_attempts none acme)"
expect 401 "$C1" "anonymous create ticket"
expect 401 "$C2" "anonymous comment"
expect 401 "$C3" "anonymous status change"
expect 401 "$C4" "anonymous attachment upload"

step "A forged or expired session is rejected"
echo "session=$(printf 'a%.0s' $(seq 1 64))" > "$WORK/forged.jar.tmp"
printf '127.0.0.1\tFALSE\t/\tFALSE\t0\tsession\t%s\n' "$(printf 'a%.0s' $(seq 1 64))" > "$WORK/forged.jar"
read -r C1 C2 C3 C4 <<<"$(write_attempts "$WORK/forged.jar" acme)"
expect 401 "$C1" "forged cookie create ticket"
expect 401 "$C2" "forged cookie comment"
expect 401 "$C3" "forged cookie status change"
expect 401 "$C4" "forged cookie attachment upload"

# Log bob out, then reuse his now-invalid cookie.
cp "$WORK/bob.jar" "$WORK/bob-stale.jar"
curl -fsS -b "$WORK/bob.jar" -X POST "$BASE/auth/logout" >/dev/null 2>&1
read -r C1 _ _ _ <<<"$(write_attempts "$WORK/bob-stale.jar" bobcorp)"
expect 401 "$C1" "cookie from a logged-out session"
# log bob back in for the remaining checks
curl -fsS -c "$WORK/bob.jar" -X POST "$BASE/auth/login" \
    -d "username=bob" -d "password=$PASSWORD" >/dev/null

# ---------- authenticated non-member ----------

step "An authenticated non-member cannot write to someone else's namespace"
read -r C1 C2 C3 C4 <<<"$(write_attempts "$WORK/bob.jar" acme)"
expect 403 "$C1" "bob create ticket in acme"
expect 403 "$C2" "bob comment in acme"
expect 403 "$C3" "bob status change in acme"
expect 403 "$C4" "bob attachment upload to acme"

step "…and the reverse direction is equally closed"
read -r C1 C2 C3 C4 <<<"$(write_attempts "$WORK/alice.jar" bobcorp)"
expect 403 "$C1" "alice create ticket in bobcorp"
expect 403 "$C2" "alice comment in bobcorp"
expect 403 "$C3" "alice status change in bobcorp"
expect 403 "$C4" "alice attachment upload to bobcorp"

step "Writing to a namespace that does not exist is refused"
read -r C1 _ _ _ <<<"$(write_attempts "$WORK/bob.jar" nosuchns)"
expect 403 "$C1" "create ticket in a non-existent namespace"

# ---------- positive controls ----------

step "Positive control: members CAN write to their own namespace"
read -r C1 C2 C3 C4 <<<"$(write_attempts "$WORK/alice.jar" acme)"
expect 200 "$C1" "alice create ticket in acme"
expect 200 "$C2" "alice comment in acme"
expect 200 "$C3" "alice status change in acme"
expect 200 "$C4" "alice attachment upload to acme"

read -r C1 _ _ _ <<<"$(write_attempts "$WORK/bob.jar" bobcorp)"
expect 200 "$C1" "bob create ticket in bobcorp"

# ---------- nothing actually changed ----------

step "No refused write left a trace in acme's ticket repository"
BODIES="$(git -C "$WORK/git/acme/ticket" show refs/heads/main:tickets/1.toml 2>/dev/null)"
if printf '%s' "$BODIES" | grep -q 'author = "bob"'; then
    fail "a refused write by bob reached acme's canonical state"
else
    pass "acme's ticket carries no content authored by a non-member"
fi

# ---------- git push ----------

step "Git push is gated the same way"
git -C "$WORK" clone -q "http://bob:$PASSWORD@127.0.0.1:$PORT/acme/ticket" bobclone 2>/dev/null
if [ -d "$WORK/bobclone" ]; then
    git -C "$WORK/bobclone" config remote.origin.push HEAD:refs/for/main
    git -C "$WORK/bobclone" config user.name bob
    git -C "$WORK/bobclone" config user.email bob@example.com
    mkdir -p "$WORK/bobclone/tickets"
    printf 'title = "intruder"\n' > "$WORK/bobclone/tickets/new-bob.toml"
    git -C "$WORK/bobclone" add -A && git -C "$WORK/bobclone" commit -qm "bob"
    git -C "$WORK/bobclone" push >/dev/null 2>&1
    [ $? -ne 0 ] && pass "non-member push is refused" || fail "non-member push must be refused"

    # Anonymous push: no credentials at all.
    git -C "$WORK/bobclone" remote set-url origin "http://127.0.0.1:$PORT/acme/ticket"
    GIT_TERMINAL_PROMPT=0 git -C "$WORK/bobclone" push >/dev/null 2>&1
    [ $? -ne 0 ] && pass "anonymous push is refused" || fail "anonymous push must be refused"

    # Positive control: a member's push succeeds.
    git -C "$WORK" clone -q "http://alice:$PASSWORD@127.0.0.1:$PORT/acme/ticket" aliceclone 2>/dev/null
    git -C "$WORK/aliceclone" config remote.origin.push HEAD:refs/for/main
    git -C "$WORK/aliceclone" config user.name alice
    git -C "$WORK/aliceclone" config user.email alice@example.com
    mkdir -p "$WORK/aliceclone/tickets"
    printf 'title = "member ticket"\n' > "$WORK/aliceclone/tickets/new-alice.toml"
    git -C "$WORK/aliceclone" add -A && git -C "$WORK/aliceclone" commit -qm "alice"
    git -C "$WORK/aliceclone" push >/dev/null 2>&1
    [ $? -eq 0 ] && pass "member push succeeds" || fail "member push should succeed"
else
    fail "could not clone acme/ticket for the push checks"
fi

# ---------- read side, stated explicitly ----------

step "Read access is public, by design (same as repositories)"
READ_STATUS="$(status_of none "$BASE/acme/tickets")"
expect 200 "$READ_STATUS" "anonymous can READ the ticket list"
printf '  \033[33mnote\033[0m Ticket content is world-readable to anyone who can reach the\n'
printf '       server, exactly like repository content. Writes are gated; reads are not.\n'

printf '\n'
if [ "$FAILURES" -eq 0 ]; then
    printf '\033[32mAll authorization assertions passed.\033[0m\n'
    exit 0
fi
printf '\033[31m%d assertion(s) failed.\033[0m\n' "$FAILURES"
exit 1
