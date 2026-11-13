# Architecture: Issues

## Fit

Issues follow the pull request design already in the code. They reuse its
numbering, permission model, and the Forgejo-compatible API layer.

- **Database (`src/db/`).** Three new tables and a `db/issues/` module, shaped like
  `db/pull_requests/`. Access goes through `Database::conn()`, which stays lazy.
  Pull request numbering (`db/pull_requests/mod.rs`, `insert_in_transaction`) changes
  to read the highest number across both pull requests and issues, so the two share one
  sequence per repository.
- **Service layer (`src/issues/`).** New rules module for issues: validation,
  permission checks, state changes, and assignee rules. Handlers call it. Pull request
  rules stay in `src/pulls/`.
- **Compatibility API (`src/http/api/compat/`).** `issues.rs` already serves
  `GET /repos/{o}/{r}/issues` for pull requests. It now also serves
  `type=issues`, and adds the issue create, read, and edit routes. `comments.rs`
  gains dispatch so `issues/{n}/comments` and `issues/comments/{id}` reach either a
  pull request or an issue. Unsupported `fj` calls get explicit `422` answers
  (see Endpoints).
- **Web UI (`src/http/repository/pages/issues/`).** A new tab module beside
  `pages/pulls/`, registered in `tabs.rs` and `routes.rs`.
- **Repository deletion.** Both deletion paths (`src/http/api/v1/repositories.rs` and
  `src/http/settings/pages/repositories.rs`) already call
  `delete_pull_requests_for_repo`. Each also calls `delete_issues_for_repo`.
- **Docs.** `docs/issues.md` (new), and `docs/pull-requests.md` and `docs/api.md`
  updated for the shared numbering and the new routes.

No change to Git storage, the Git HTTP adapter, or `twig-git`. Issues do not touch refs.

## Endpoints

All compatibility routes require a credential (`token`, `Bearer`, or Basic). Errors
use Forgejo's `{"message": "...", "url": ""}` body. Permission rules are in Data
and Flow.

Verified against `fj` 0.x with a stub server (see Notes). Each row lists the
request `fj` sends.

Issues and pull requests (`{n}` is the shared number):

- `GET /api/v1/repos/{o}/{r}/issues?type=issues&state=open|closed|all&q=&created_by=&assigned_by=&page=&limit=`
  is the issue list. `fj issue search` sends `type=issues`, with `labels=` (empty) and
  `page`/`limit`. Empty `labels`, `milestones`, and `mentioned_by` are ignored. Non-empty
  `labels` or `milestones` return `422 Labels are not supported on Twig` (or the
  milestone equivalent). `type=pulls`, and a missing `type`, keep the pull request
  list, as they do today. Returns `X-Total-Count` and `Link` when paged.
- `POST /api/v1/repos/{o}/{r}/issues` with `{title, body?, assignees?}`. `fj issue create`
  sends `labels`, `milestone`, `ref`, `due_date`, and `closed` as `null`; those are ignored.
  Returns `201` with the issue.
- `GET /api/v1/repos/{o}/{r}/issues/{n}`: an issue, or, if `{n}` is a pull request, the
  pull request in issue shape (`pull_request` marker set), as the list already shows it.
  `fj issue view` uses this.
- `PATCH /api/v1/repos/{o}/{r}/issues/{n}` with `{title?, body?, state?, assignees?}`.
  Each field is optional. `null` means unchanged. `assignees` replaces the whole set,
  so `[]` clears it. Returns `201`, because Forgejo returns `201` here and `fj`
  requires it. On a pull request number, returns `422 Use pulls/{n} to change a pull request`.
- `GET|POST /api/v1/repos/{o}/{r}/issues/{n}/comments`. For an issue, lists or adds
  issue comments. For a pull request, the existing conversation behavior is kept.
- `PATCH|DELETE /api/v1/repos/{o}/{r}/issues/comments/{id}`. Dispatch by ID range
  (see Data): the high range is an issue comment, anything else a pull request comment.
- `GET /api/v1/repos/{o}/{r}/issue_templates`: `404`. Forgejo answers `404` when a
  repository has no templates. `fj` reads that as "blank issues allowed", so
  `fj issue create` works without `--no-template`. An empty `200` list makes `fj` refuse
  ("uses issue templates"), which the slice 1 live run showed.
- `GET /api/v1/repos/{o}/{r}/issue_config`: `{"blank_issues_enabled": true, "contact_links": []}`.

Unsupported on Twig. Each returns `422` with a clear message, so `fj` shows the reason:

- `GET|POST|DELETE /api/v1/repos/{o}/{r}/issues/{n}/dependencies` (`fj issue depend`):
  `Issue dependencies are not supported on Twig`.
- `GET|POST|DELETE /api/v1/repos/{o}/{r}/issues/{n}/blocks` (`fj issue block`): `Blocking is not supported on Twig`.
- `POST|PUT|DELETE /api/v1/repos/{o}/{r}/issues/{n}/labels` (`fj issue edit labels`):
  `Labels are not supported on Twig`.
- `GET|POST|DELETE /api/v1/repos/{o}/{r}/issues/{n}/reactions` and comment reactions
  on issues: `Reactions on issues are not supported yet` (pull requests keep theirs).

Web UI (under `/{namespace}/{repo}/`, session-authenticated, form POSTs redirect back):

- `GET issues`: the Issues tab. `?state=open|closed|all`, `?q=`, `?author=`, `?assignee=`.
- `GET issues/new` and `POST issues/new`.
- `GET issues/{n}`: the conversation. A pull request number redirects to `pulls/{n}`.
- `POST issues/{n}/comments`; `POST issues/{n}/comments/{id}/edit`; `POST issues/{n}/comments/{id}/delete`.
- `POST issues/{n}/state` with `action=close|reopen`.
- `POST issues/{n}/edit` with `title`, `body`.
- `POST issues/{n}/assignees` with `action=add|remove` and `username`.

## Data

New tables, appended to `db/migration.rs`:

```sql
issues (
  id TEXT PRIMARY KEY,                 -- uuid
  repo_id TEXT NOT NULL,
  number INTEGER NOT NULL,             -- shared with pull requests, per repository
  title TEXT NOT NULL,
  body TEXT NOT NULL DEFAULT '',
  state TEXT NOT NULL,                 -- 'open' | 'closed'
  author_user_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  closed_at TEXT,
  UNIQUE (repo_id, number)
)
CREATE INDEX idx_issues_repo_state ON issues(repo_id, state, updated_at);

issue_assignees (
  issue_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  assigned_at TEXT NOT NULL,
  PRIMARY KEY (issue_id, user_id)
)

issue_comments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  issue_id TEXT NOT NULL,
  author_user_id TEXT NOT NULL,
  body TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT
)
CREATE INDEX idx_issue_comments_issue ON issue_comments(issue_id, created_at);
```

Shared numbering. `SELECT COALESCE(MAX(n), 0) + 1 FROM (SELECT number AS n FROM pull_requests
WHERE repo_id = ?1 UNION ALL SELECT number FROM issues WHERE repo_id = ?1)`, inside the
same `BEGIN IMMEDIATE` as the insert. Both issue and pull request creation use it.

Comment IDs. Pull request comments keep their `AUTOINCREMENT` IDs. Issue comments are
returned as `ISSUE_COMMENT_ID_BASE + rowid`, with `ISSUE_COMMENT_ID_BASE = 1 << 40`,
so the two spaces cannot collide. A route with an ID at or above the base is an issue
comment. Pull request comment IDs stay far below it.

Numeric identifiers. Users use `numeric_id(user.id)`, as today. Issues use
`numeric_id(issue.id)` for their `id`, and the number for `number`.

Queries that hit these tables:

- Issue by number: `SELECT ... FROM issues WHERE repo_id = ?1 AND number = ?2`.
- List: `WHERE repo_id = ?1 AND (?2 IS NULL OR state = ?2) AND (?3 IS NULL OR lower(title) LIKE ?3 OR lower(body) LIKE ?3) AND (?4 IS NULL OR author_user_id = ?4) AND (?5 IS NULL OR EXISTS (SELECT 1 FROM issue_assignees a WHERE a.issue_id = issues.id AND a.user_id = ?5)) ORDER BY updated_at DESC, number DESC LIMIT ?6 OFFSET ?7`.
- Count: the same predicate, `COUNT(*)`.
- Open and closed counts: `SELECT state, COUNT(*) FROM issues WHERE repo_id = ?1 GROUP BY state`.
- Assignees of a set of issues: `SELECT issue_id, user_id FROM issue_assignees WHERE issue_id IN (...)`.
- Comments: `SELECT ... FROM issue_comments WHERE issue_id = ?1 ORDER BY created_at, id`.

## Flow

Create an issue (`fj issue create`, `POST issues`, UI `POST issues/new`):

1. Authenticate; require `ViewNamespace` (anyone who can view may open an issue, as Gate 1 decided).
2. `RepoHandle::open`; `repo_id()`.
3. Validate `title` (non-empty after trim); validate `assignees` (each a namespace member; needs `PushChanges` to set any).
4. In `BEGIN IMMEDIATE`: allocate the shared number, insert the row and assignees.
5. Return the issue (`201`, or redirect in the UI).

Read (`fj issue view`, `fj issue search`): authenticate; `ViewNamespace`; look up by number,
or fall back to the pull request with the same number for `GET issues/{n}`.

Comment: `ViewNamespace`; the body is non-empty; insert; return `201`.

Edit comment or delete comment: only the comment's author. Anyone else gets `403`.
The comment must belong to an issue in the repository, so a comment ID never reaches
another repository.

Change state (`PATCH issues/{n}` with `state`, `fj issue close`/`reopen`):

1. The author, or anyone with `PushChanges`, may change state, title, or body.
2. Closing sets `closed_at`; reopening clears it. Both bump `updated_at`.

Assignees (`PATCH` with `assignees`, `fj issue assign`/`unassign`): `PushChanges` only.
Each name must be a namespace member. Replace the set in one transaction.

## External

None. No environment variables and no third-party services. `fj` and the Forgejo
client behavior are described above.

## Notes

- The `fj` contract was checked by running `fj` against a stub server (`/tmp/fj-probe`,
  not in the repo). Create, search, view, comment, close with a message, edit, assign,
  unassign, and the dependency and block commands were all exercised. Each request matches
  the list above.
- Two calls `fj` makes need real answers: `GET repos/{o}/{r}` (repository lookup) and
  `GET repos/search` (used by some flows). Slice 2 confirms both against the real server.
- Pull requests keep their behavior. The only change to them is the shared numbering.
