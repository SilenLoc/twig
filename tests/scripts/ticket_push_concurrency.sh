#!/usr/bin/env bash
# End-to-end proof that concurrent ticket writes never surface git's plumbing.
#
# Drives a real Fig server over HTTP with real `git` clients. Every assertion is
# about what a user would actually see: exit codes, stderr text, conflict
# markers, and whether anyone's work went missing.
#
# Usage:  bash tests/scripts/ticket_push_concurrency.sh
# Exits non-zero on the first failed assertion, so it is CI-usable.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK="$(mktemp -d /tmp/fig-ticket-e2e-XXXXXX)"
# Always pick a free port. `PORT` is deliberately not consulted: it is commonly
# already exported for local development and would collide.
PORT="${FIG_TEST_PORT:-$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')}"
API_KEY="secure"
BASE="http://127.0.0.1:${PORT}"
PASSWORD="password123"

FAILURES=0
SERVER_PID=""

# ---------- harness ----------

pass() { printf '  \033[32mok\033[0m   %s\n' "$1"; }
fail() { printf '  \033[31mFAIL\033[0m %s\n' "$1"; FAILURES=$((FAILURES + 1)); }
step() { printf '\n\033[1m%s\033[0m\n' "$1"; }

assert_eq() {
    if [ "$1" = "$2" ]; then pass "$3"; else fail "$3 (expected '$2', got '$1')"; fi
}
assert_contains() {
    if printf '%s' "$1" | grep -qF -- "$2"; then pass "$3"; else fail "$3 (missing '$2' in: $1)"; fi
}
assert_not_contains() {
    if printf '%s' "$1" | grep -qF -- "$2"; then fail "$3 (unexpected '$2' in: $1)"; else pass "$3"; fi
}

cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null
    wait "$SERVER_PID" 2>/dev/null
    rm -rf "$WORK"
}
trap cleanup EXIT

# ---------- server ----------

step "Building and starting Fig"
cargo build --quiet --manifest-path "$REPO_ROOT/Cargo.toml" 2>"$WORK/build.log" \
    || { echo "build failed"; cat "$WORK/build.log"; exit 1; }

mkdir -p "$WORK/git"
PORT="$PORT" \
PROJECT_ROOT="$WORK/git" \
DB_PATH="$WORK/fig.db" \
API_KEY="$API_KEY" \
RESET_DB=true \
LOG_LEVEL=warn \
    "$REPO_ROOT/target/debug/fig" >"$WORK/server.log" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 60); do
    curl -fsS "$BASE/health" >/dev/null 2>&1 && break
    sleep 0.5
done
curl -fsS "$BASE/health" >/dev/null 2>&1 || { echo "server did not start"; cat "$WORK/server.log"; exit 1; }

# /health answers before the background database init finishes, so wait for a
# request that actually needs tables.
for _ in $(seq 1 60); do
    curl -fsS -X POST "$BASE/auth/invite" -d "api_key=$API_KEY" >/dev/null 2>&1 && break
    sleep 0.5
done
curl -fsS -X POST "$BASE/auth/invite" -d "api_key=$API_KEY" >/dev/null 2>&1 \
    || { echo "database never became ready"; cat "$WORK/server.log"; exit 1; }
pass "server is up on :$PORT"

# ---------- accounts ----------

new_user() {
    local username="$1"
    local invite
    invite="$(curl -fsS -X POST "$BASE/auth/invite" -d "api_key=$API_KEY" \
        | grep -oE '[0-9a-f]{64}' | head -1)"
    [ -n "$invite" ] || { echo "could not obtain invite"; exit 1; }
    curl -fsS -X POST "$BASE/auth/signup" \
        -d "invite=$invite" -d "username=$username" \
        -d "email=$username@example.com" -d "password=$PASSWORD" >/dev/null
    curl -fsS -c "$WORK/$username.jar" -X POST "$BASE/auth/login" \
        -d "username=$username" -d "password=$PASSWORD" >/dev/null
}

step "Creating accounts and the acme namespace"
new_user alice
new_user bob
curl -fsS -b "$WORK/alice.jar" -X POST "$BASE/auth/namespace" -d "name=acme" >/dev/null
[ -d "$WORK/git/acme/ticket" ] && pass "ticket repo created with the namespace" \
    || fail "ticket repo was not created with the namespace"

AUTH_URL="http://alice:$PASSWORD@127.0.0.1:$PORT/acme/ticket"
SRV="$WORK/git/acme/ticket"

server_main() { git -C "$SRV" rev-parse refs/heads/main; }
server_files() { git -C "$SRV" ls-tree --name-only -r refs/heads/main; }

# ---------- clones ----------

step "1. Two clones, each configured once with the documented push refspec"
for clone in a1 a2; do
    git -C "$WORK" clone -q "$AUTH_URL" "$clone" 2>/dev/null || { fail "clone $clone"; exit 1; }
    git -C "$WORK/$clone" config remote.origin.push HEAD:refs/for/main
    git -C "$WORK/$clone" config user.name "alice"
    git -C "$WORK/$clone" config user.email "alice@example.com"
done
pass "both clones configured"

write_ticket() { # <clone> <file> <title>
    mkdir -p "$WORK/$1/tickets"
    printf 'title = "%s"\nstatus = "open"\n\n[body]\nmarkdown = "from %s"\n' "$3" "$1" \
        > "$WORK/$1/tickets/$2"
}

# Runs `git push` in a clone, setting PUSH_OUT and PUSH_RC in the CURRENT
# shell. A command substitution would run in a subshell and lose PUSH_RC.
PUSH_OUT=""
PUSH_RC=0
do_push() { # <clone>
    PUSH_OUT="$(git -C "$WORK/$1" push 2>&1)"
    PUSH_RC=$?
}

step "2-4. Stale pushes must succeed silently, pulls must fast-forward"
for round in 1 2 3 4 5; do
    write_ticket a1 "new-a1-$round.toml" "a1 round $round"
    git -C "$WORK/a1" add -A && git -C "$WORK/a1" commit -qm "a1 $round"
    do_push a1; OUT_A1="$PUSH_OUT"; RC_A1=$PUSH_RC

    # a2 has not pulled since before a1's push: this is the stale case that
    # plain git refuses client-side.
    write_ticket a2 "new-a2-$round.toml" "a2 round $round"
    git -C "$WORK/a2" add -A && git -C "$WORK/a2" commit -qm "a2 $round"
    do_push a2; OUT_A2="$PUSH_OUT"; RC_A2=$PUSH_RC

    assert_eq "$RC_A1" "0" "round $round: up-to-date push exits 0"
    assert_eq "$RC_A2" "0" "round $round: STALE push exits 0"
    for out in "$OUT_A1" "$OUT_A2"; do
        assert_not_contains "$out" "rejected" "round $round: no rejection shown"
        assert_not_contains "$out" "fetch first" "round $round: no 'fetch first' hint"
        assert_not_contains "$out" "error:" "round $round: no error line"
    done

    PULL_A1="$(git -C "$WORK/a1" pull 2>&1)"; RC=$?
    assert_eq "$RC" "0" "round $round: a1 pull exits 0"
    assert_contains "$PULL_A1" "Fast-forward" "round $round: a1 pull fast-forwards"
    PULL_A2="$(git -C "$WORK/a2" pull 2>&1)"; RC=$?
    assert_eq "$RC" "0" "round $round: a2 pull exits 0"

    MARKERS="$(grep -rl '<<<<<<<' "$WORK/a1" "$WORK/a2" 2>/dev/null | wc -l)"
    assert_eq "$MARKERS" "0" "round $round: zero conflict markers in either clone"
done

step "5. No data loss: every ticket from both clones survived"
FILES="$(server_files)"
COUNT="$(printf '%s\n' "$FILES" | grep -c '^tickets/[0-9]*\.toml$')"
assert_eq "$COUNT" "10" "all 10 tickets present on the server"
assert_not_contains "$FILES" "new-a1-" "draft files were consumed"
assert_not_contains "$FILES" "new-a2-" "draft files were consumed"

step "10. Sequential numbering, no collisions, and idempotent re-push"
NUMBERS="$(printf '%s\n' "$FILES" | grep -oE 'tickets/[0-9]+\.toml' | grep -oE '[0-9]+' | sort -n | tr '\n' ' ')"
assert_eq "$NUMBERS" "1 2 3 4 5 6 7 8 9 10 " "numbers are distinct and sequential"

BEFORE="$(server_main)"
do_push a2
assert_eq "$PUSH_RC" "0" "re-pushing an already-ingested state exits 0"
COUNT_AFTER="$(server_files | grep -c '^tickets/[0-9]*\.toml$')"
assert_eq "$COUNT_AFTER" "10" "re-push created no duplicate tickets"

step "6. A direct push to the server-owned branch is refused with guidance"
# The clone must be up to date first. A stale direct push is rejected by git
# *client-side* ("fetch first") and never reaches the server, which is exactly
# the behaviour the refs/for pseudo-ref exists to avoid.
git -C "$WORK/a1" pull -q
git -C "$WORK/a1" commit -q --allow-empty -m "direct"
OUT="$(git -C "$WORK/a1" push origin HEAD:refs/heads/main 2>&1)"; RC=$?
[ "$RC" -ne 0 ] && pass "direct push to refs/heads/main is refused" \
    || fail "direct push to refs/heads/main should be refused"
assert_contains "$OUT" "git config remote.origin.push HEAD:refs/for/main" \
    "refusal names the exact config line to run"
assert_contains "$OUT" "managed by the server" "refusal explains why"
git -C "$WORK/a1" reset -q --hard origin/main

step "7. Malformed TOML is refused and canonical state is untouched"
git -C "$WORK/a1" pull -q
BEFORE="$(server_main)"
printf 'title = \n' > "$WORK/a1/tickets/new-broken.toml"
git -C "$WORK/a1" add -A && git -C "$WORK/a1" commit -qm "broken"
do_push a1
[ "$PUSH_RC" -ne 0 ] && pass "malformed submission is refused" || fail "malformed submission should be refused"
assert_eq "$(server_main)" "$BEFORE" "canonical ref unchanged after a refused push"
git -C "$WORK/a1" reset -q --hard origin/main

step "8. A forged author is replaced by the authenticated user"
git -C "$WORK/a1" pull -q
printf 'title = "forged"\nauthor = "mallory"\n\n[body]\nmarkdown = "x"\n' \
    > "$WORK/a1/tickets/new-forged.toml"
git -C "$WORK/a1" add -A && git -C "$WORK/a1" commit -qm "forged"
do_push a1
assert_eq "$PUSH_RC" "0" "push with a forged author is accepted"
git -C "$WORK/a1" pull -q
FORGED_FILE="$(grep -rl 'title = "forged"' "$WORK/a1/tickets/" | head -1)"
assert_contains "$(cat "$FORGED_FILE")" 'author = "alice"' "author is the authenticated pusher"
assert_not_contains "$(cat "$FORGED_FILE")" 'mallory' "claimed author is discarded"

step "9. Disallowed tree content is refused (git trees cannot encode '..')"
reject_push() { # <label>
    do_push a1
    [ "$PUSH_RC" -ne 0 ] && pass "$1 is refused" || fail "$1 should be refused"
    assert_eq "$(server_main)" "$BEFORE" "$1: canonical ref unchanged"
    git -C "$WORK/a1" reset -q --hard origin/main
    git -C "$WORK/a1" clean -qfd
}

git -C "$WORK/a1" pull -q; BEFORE="$(server_main)"
echo 'x = 1' > "$WORK/a1/escape.toml"
git -C "$WORK/a1" add -A && git -C "$WORK/a1" commit -qm "root file"
reject_push "a file at the repository root"

mkdir -p "$WORK/a1/config"; echo 'x = 1' > "$WORK/a1/config/evil.toml"
git -C "$WORK/a1" add -A && git -C "$WORK/a1" commit -qm "unexpected dir"
reject_push "a file in an unexpected directory"

ln -sf /etc/passwd "$WORK/a1/tickets/link.toml"
git -C "$WORK/a1" add -A && git -C "$WORK/a1" commit -qm "symlink"
reject_push "a symlink inside tickets/"

head -c 1000000 /dev/zero | tr '\0' 'a' > "$WORK/a1/tickets/99.toml"
git -C "$WORK/a1" add -A && git -C "$WORK/a1" commit -qm "oversized"
reject_push "an oversized blob"

step "11. Delete versus modify follows the documented rule"
# (a) contested delete: modification wins, the ticket survives
git -C "$WORK/a1" pull -q && git -C "$WORK/a2" pull -q
git -C "$WORK/a2" rm -q "tickets/1.toml"
git -C "$WORK/a2" commit -qm "delete 1"
sed -i 's/^title = .*/title = "still needed"/' "$WORK/a1/tickets/1.toml"
git -C "$WORK/a1" commit -qam "retitle 1"
do_push a1; RC_MOD=$PUSH_RC
do_push a2; RC_DEL=$PUSH_RC
assert_eq "$RC_MOD" "0" "contested modify push exits 0"
assert_eq "$RC_DEL" "0" "contested delete push exits 0"
assert_contains "$(server_files)" "tickets/1.toml" "contested ticket is retained"
assert_contains "$(git -C "$SRV" show refs/heads/main:tickets/1.toml)" 'still needed' \
    "retained ticket carries the concurrent modification"

# (b) uncontested delete actually deletes
git -C "$WORK/a1" pull -q
git -C "$WORK/a1" rm -q "tickets/2.toml"
git -C "$WORK/a1" commit -qm "delete 2"
do_push a1
assert_eq "$PUSH_RC" "0" "uncontested delete push exits 0"
assert_not_contains "$(server_files)" "tickets/2.toml" "uncontested delete removes the ticket"

step "12. A UI write racing a CLI push: both land"
# Proves the flock is genuinely shared across processes: the web server writes
# in-process while the proc-receive hook writes from a child of git.
git -C "$WORK/a1" pull -q
python3 - "$WORK/a1/tickets/1.toml" <<'PY'
import pathlib, sys
path = pathlib.Path(sys.argv[1])
# A hand-written comment, deliberately claiming someone else as the author.
path.write_text(path.read_text() + '\n[[comment]]\nmarkdown = "from the command line"\nauthor = "mallory"\n')
PY
git -C "$WORK/a1" commit -qam "cli comment"

( do_push a1 >/dev/null 2>&1 ) &
PUSH_JOB=$!
curl -fsS -o /dev/null -b "$WORK/alice.jar" -X POST \
    "$BASE/acme/tickets/1/comment" -d "markdown=from the web ui" &
UI_JOB=$!
# Wait only on these two: a bare `wait` would also block on the server process.
wait "$PUSH_JOB" "$UI_JOB"

git -C "$WORK/a1" pull -q
RACED="$(cat "$WORK/a1/tickets/1.toml")"
assert_contains "$RACED" "from the command line" "the CLI comment survived the race"
assert_contains "$RACED" "from the web ui" "the UI comment survived the race"
assert_not_contains "$RACED" "mallory" "a forged comment author is replaced"
MARKERS="$(grep -rlc '<<<<<<<' "$WORK/a1" 2>/dev/null | wc -l)"
assert_eq "$MARKERS" "0" "the race produced no conflict markers"

step "Authorisation: a user without namespace access cannot push"
git -C "$WORK" clone -q "http://bob:$PASSWORD@127.0.0.1:$PORT/acme/ticket" bobclone 2>/dev/null
git -C "$WORK/bobclone" config remote.origin.push HEAD:refs/for/main
git -C "$WORK/bobclone" config user.name bob
git -C "$WORK/bobclone" config user.email bob@example.com
mkdir -p "$WORK/bobclone/tickets"
printf 'title = "intruder"\n' > "$WORK/bobclone/tickets/new-bob.toml"
git -C "$WORK/bobclone" add -A && git -C "$WORK/bobclone" commit -qm "bob"
git -C "$WORK/bobclone" push >/dev/null 2>&1
[ $? -ne 0 ] && pass "a non-member push is refused" || fail "a non-member must not be able to push"

# ---------- summary ----------

printf '\n'
if [ "$FAILURES" -eq 0 ]; then
    printf '\033[32mAll assertions passed.\033[0m\n'
    exit 0
fi
printf '\033[31m%d assertion(s) failed.\033[0m\n' "$FAILURES"
exit 1
