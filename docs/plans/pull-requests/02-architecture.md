# Architecture: Pull requests

## Fit

Four existing areas are touched, and one new service layer sits between them.

- **Git layer (`src/git/bare/`).** `RepoHandle` already opens a bare repository and
  owns commit writes with compare-and-swap on refs (`handle/commit.rs`). Pull
  request Git work (merge-base, commit ranges, diffs, in-memory merges, merge
  commits, `refs/pull/N/head` maintenance) is added as `handle/pulls.rs` and
  `handle/pulls/merge.rs`, so private fields stay private. The API layer already
  uses this module (`contents.rs`, `repositories.rs`), so the new code joins it.
  `crates/twig-git` is not touched: it duplicates this surface mid-extraction and
  the API does not use it for contents.
- **Database (`src/db/`).** New tables and a `db/pull_requests/` module (with
  submodules for comments, reviews, reactions, reviewers) follow the style of
  `db/binaries.rs`. Access goes through `Database::conn()`, which stays lazy.
- **Auth (`src/auth/`).** `authenticate_bearer` also accepts the
  `Authorization: token <PAT>` scheme that Forgejo/Gitea clients send. Permission
  checks reuse `permissions::for_principal` (token scope intersected with user role).
- **HTTP (`src/http/`).** Three additions:
  1. `src/http/api/compat/` for the Forgejo/Gitea-shaped JSON API.
  2. `src/http/repository/pages/pulls/` for the web UI, registered as tabs and
     routes beside the existing repository pages.
  3. A `refs/pull/` guard and PR-ref sync inside `src/git/mod.rs` (the Git HTTP adapter).

New service layer: `src/pulls/` holds the rules used by both the API and the UI
(validation, permission checks, state transitions, merge orchestration). The
handlers only parse input and render output. Both front ends call the same functions.

## Endpoints

All compatibility endpoints require credentials (Basic, `Bearer`, or `token`);
browser cookies are not accepted, matching the existing API. Errors use
Forgejo's body shape `{"message": "...", "url": "..."}`.

Identity and server:

- `GET /api/v1/user`: current user, Forgejo `User` shape (`login`, `id`, `full_name`, `avatar_url`, `email`).
- `GET /api/v1/version`: `{"version": "<app version>"}`.

Repository metadata (what T3 reads to classify and display a repository):

- `GET /api/v1/repos/{owner}/{repo}`: repository with `permissions`, merge-style flags, `clone_url`, `ssh_url`, `default_branch`.
- `GET /api/v1/repos/{owner}/{repo}/branches/{branch}`: branch with `commit.id`; `enable_status_check: false`, `status_check_contexts: []`.
- `GET /api/v1/repos/{owner}/{repo}/statuses/{sha}`: `[]` (no CI).
- `GET /api/v1/repos/{owner}/{repo}/labels`: `[]` (labels are not part of v1).
- `GET /api/v1/repos/{owner}/{repo}/assignees`: namespace members with push rights.
- `GET /api/v1/repos/{owner}/{repo}/contents/{path}?ref=`: base64 file content, Forgejo shape.
- `GET /api/v1/repos/{owner}/{repo}/git/commits/{sha}` and `.../{sha}.diff`: commit read and commit diff.
- `POST /api/v1/user/repos`: creates a repository in the caller's namespace (`name`, `private`).

Pull requests (`{index}` is the per-repository PR number):

- `GET /api/v1/repos/{owner}/{repo}/pulls?state=open|closed|all&head=&sort=&page=&limit=`: paged, `Link: rel="next"` when more.
- `POST /api/v1/repos/{owner}/{repo}/pulls` with `{base, head, title, body}`; `head` may be `owner:branch` where owner is the same namespace.
- `GET /api/v1/repos/{owner}/{repo}/pulls/{index}`: includes `mergeable` (computed) and `merge_base`.
- `PATCH /api/v1/repos/{owner}/{repo}/pulls/{index}` with `{title?, body?, state?}`.
- `GET /api/v1/repos/{owner}/{repo}/pulls/{index}.diff` and `.patch`: `text/plain`.
- `GET /api/v1/repos/{owner}/{repo}/pulls/{index}/commits?files=&verification=&page=&limit=`.
- `POST /api/v1/repos/{owner}/{repo}/pulls/{index}/merge` with `{Do: merge|squash|rebase, MergeTitleField?, MergeMessageField?}`: 200 on success.
- `GET /api/v1/repos/{owner}/{repo}/pulls/{index}/merge`: 204 if merged, else 404.
- `POST /api/v1/repos/{owner}/{repo}/pulls/{index}/update?style=merge|rebase`: merges or rebases base into head (slice 8 only if time allows, see Program Design).
- `GET|POST /api/v1/repos/{owner}/{repo}/pulls/{index}/reviews`; `GET /reviews/{id}`; `GET /reviews/{id}/comments`.
- `POST /api/v1/repos/{owner}/{repo}/pulls/{index}/reviews` with `{event: APPROVED|REQUEST_CHANGES|COMMENT, body, commit_id, comments: [{path, body, old_position, new_position}]}`.
- `GET|POST|DELETE /api/v1/repos/{owner}/{repo}/pulls/{index}/requested_reviewers` with `{reviewers: [login]}`.

Conversation (PR comments are issue comments in Forgejo, and share one ID sequence with review comments):

- `GET|POST /api/v1/repos/{owner}/{repo}/issues/{index}/comments`.
- `PATCH|DELETE /api/v1/repos/{owner}/{repo}/issues/comments/{id}`.
- `GET|POST /api/v1/repos/{owner}/{repo}/issues/{index}/reactions`; `DELETE` with body.
- `GET|POST|DELETE /api/v1/repos/{owner}/{repo}/issues/comments/{id}/reactions`.

Web UI (all under `/{namespace}/{repo}/`, session-authenticated, form POSTs redirect back):

- `GET pulls` list (`?state=open|closed|all`, `?q=`); `GET pulls/new?base=&head=` compare form; `POST pulls/new`.
- `GET pulls/{n}` conversation; `GET pulls/{n}/commits`; `GET pulls/{n}/files`.
- `POST pulls/{n}/comments`; `POST pulls/{n}/reviews`; `POST pulls/{n}/files/comments` (inline).
- `POST pulls/{n}/merge` with `method`; `POST pulls/{n}/state` with `action=close|reopen`.
- `GET|POST pulls/{n}/edit`.

## Data

Repository identity. A repository is keyed by a UUID stored in its bare Git
config as `[twig] id`. It is generated on first pull request access if absent
(`git config --local twig.id <uuid>`). Keys follow the repo, so renames and moves
do not orphan pull requests. Deleting a repository leaves its rows orphaned and
unreachable. They are removed only when the repository is deleted through the API
or settings code path (slice 2). A new repository always gets a new UUID.

New tables (migrations appended to `db/migration.rs`):

```sql
pull_requests (
  id TEXT PRIMARY KEY,                -- uuid
  repo_id TEXT NOT NULL,
  number INTEGER NOT NULL,            -- per-repository, 1-based
  title TEXT NOT NULL,
  body TEXT NOT NULL DEFAULT '',
  state TEXT NOT NULL,                -- 'open' | 'closed'
  author_user_id TEXT NOT NULL,
  head_branch TEXT NOT NULL,
  base_branch TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  closed_at TEXT,
  merged_at TEXT,
  merged_by_user_id TEXT,
  merge_commit_oid TEXT,
  UNIQUE (repo_id, number)
)
CREATE INDEX idx_pull_requests_repo_state ON pull_requests(repo_id, state, updated_at);

pull_request_comments (                -- conversation and review comments, one ID sequence
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  pull_request_id TEXT NOT NULL,
  review_id INTEGER,                   -- NULL for conversation comments
  author_user_id TEXT NOT NULL,
  body TEXT NOT NULL,
  path TEXT, old_position INTEGER, new_position INTEGER, commit_oid TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT
)
CREATE INDEX idx_pull_request_comments_pr ON pull_request_comments(pull_request_id, created_at);

pull_request_reviews (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  pull_request_id TEXT NOT NULL,
  author_user_id TEXT NOT NULL,
  state TEXT NOT NULL,                 -- APPROVED | REQUEST_CHANGES | COMMENT
  body TEXT NOT NULL DEFAULT '',
  commit_oid TEXT NOT NULL,
  submitted_at TEXT NOT NULL
)

pull_request_reactions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  target_key TEXT NOT NULL,            -- 'pull:<pr id>' or 'comment:<comment id>'
  user_id TEXT NOT NULL,
  content TEXT NOT NULL,               -- +1 -1 laugh hooray confused heart rocket eyes
  created_at TEXT NOT NULL,
  UNIQUE (target_key, user_id, content)
)

pull_request_reviewers (
  pull_request_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  requested_at TEXT NOT NULL,
  PRIMARY KEY (pull_request_id, user_id)
)
```

Numeric identifiers for clients. Forgejo clients expect integers for users,
repositories, and comments. Users and repositories get a 53-bit integer derived
from SHA-256 of their stable key (first 8 bytes of the digest, masked to 53 bits),
so the value is deterministic and safe for JavaScript clients. PR numbers and
comment and review IDs are the SQLite integers above.

Queries that hit these tables:

- Open PRs for sync: `SELECT number, head_branch FROM pull_requests WHERE repo_id = ?1 AND state = 'open'`.
- Next number: `SELECT COALESCE(MAX(number), 0) + 1 FROM pull_requests WHERE repo_id = ?1`, inside `BEGIN IMMEDIATE`.
- Branch match for `head` filter: `WHERE repo_id = ?1 AND head_branch = ?2 AND state = ?3`.
- Timeline: comments and reviews for one PR, ordered by `created_at`, plus reactions by `target_key IN (...)`.

## Flow

Create a pull request (API `POST pulls`, UI `POST pulls/new`):

1. `authenticate` → principal (PAT scope applied).
2. `require_namespace_permission(PushChanges)`.
3. `RepoHandle::open`; `repo_id()` (create if missing).
4. Validate `base` and `head` exist as `refs/heads/*`, differ, and have commits ahead of base.
5. Reject if an open PR already exists for the same `(head, base)`.
6. Allocate number in a `BEGIN IMMEDIATE` transaction and insert the row.
7. Write `refs/pull/N/head` to the head tip (see Git HTTP sync).

Fetch (`GET info/refs` for `upload-pack`):

1. Existing auth.
2. `pulls::sync_refs(db, repo_handle)`: for each open PR, set `refs/pull/N/head` to the head branch tip when it differs. Failures are logged and do not fail the fetch.
3. Hand off to `twig_git` CGI as before.

Merge (`POST pulls/{n}/merge`, UI `POST pulls/{n}/merge`):

1. `authenticate`; load PR; reject if not open (409 `already merged` or `closed`).
2. Permission: `PushChanges` (token-scoped).
3. Read `base` tip (`old_base`) and `head` tip.
4. Compute `merge-base`. If head is an ancestor of base, return 409 "nothing to merge".
5. Build the result in memory:
   - `merge`: `merge_trees`; if conflicts, return 409 with the conflicted paths; otherwise commit with parents `[old_base, head]`.
   - `squash`: one commit on `old_base` with the head tree; message from `MergeTitleField` and `MergeMessageField`, defaulting to the PR title and body.
   - `rebase`: replay commits from `merge-base..head` onto `old_base` with the cherry-pick path in memory; a conflict returns 409.
6. Move `refs/heads/base` from `old_base` to the new commit with `reference_matching` (compare-and-swap). A `Modified` error returns 409 "base branch moved, retry".
7. In one transaction, mark the PR closed, `merged_at`, `merged_by`, `merge_commit_oid`.

Mergeability (`GET pulls/{n}` only, for cost reasons): an in-memory merge without writing
objects, returning `mergeable: true|false`, and `null` in list responses.

Diff (`.diff`, `files`): `merge_base(base_tip, head_tip)` → `diff_tree_to_tree(base_tree_of_merge_base, head_tree)` → patch text, plus a parsed file list for the UI.

Web UI: each handler calls the same `pulls::` service functions as the API,
using the session user in place of a PAT. Form POSTs follow
POST-redirect-GET to the conversation page, and show an error banner on failure.

## External

None. No third-party service is called.

Client contract, for T3 Code and the Forgejo/Gitea CLIs:

- T3 Code detects the provider by hostname (`codeberg.org`, DNS labels `forgejo` or
  `gitea`) or by a matching login in `fj` or `tea`. For Twig, a matching login is required.
- Tokens: `fj --host <url> auth add-token` or `tea login add`, using a Twig personal
  access token. The CLI sends `Authorization: token <PAT>` to `${url}/api/v1/...`.
- Remote URLs must be `https://<host>/<namespace>/<repo>(.git)`. SSH remotes are
  supported by T3 but the CLI uses HTTPS API calls for everything except `git` operations.

Environment variables:

- `PUBLIC_BASE_URL` (existing, optional): when set, used as the origin for `html_url`
  and `clone_url`. Otherwise the request's scheme and host are used. No new variables.

Git push protection (not a client-visible API): `receive-pack` requests whose ref
commands target `refs/pull/*` are refused with HTTP 403 before reaching
`git http-backend`. The check parses the pkt-line ref commands at the start of the
request body. Compressed request bodies (`Content-Encoding: gzip`) fail closed. The
existing managed pre-receive hook is not changed, because the installer does not
rewrite hooks that already carry the V1 marker, so the hook would not reach existing
repositories.
