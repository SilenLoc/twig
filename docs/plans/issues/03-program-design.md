# Program Design: Issues

## Files

Created:

- `src/db/issues/mod.rs`: issue rows, assignees, list, count, edit, delete. Mirrors `db/pull_requests/mod.rs`.
- `src/db/issues/comments.rs`: issue comments and the comment ID mapping.
- `src/db/issues/tests.rs`: database tests.
- `src/issues/mod.rs`: rules shared by the API and the UI (permissions, validation, state, assignees).
- `src/issues/tests.rs`: rule tests (permission matrix, assignee and author rules).
- `src/http/repository/pages/issues/mod.rs`: the Issues tab's shared helpers and module wiring.
- `src/http/repository/pages/issues/list.rs`: the Issues list page with filters.
- `src/http/repository/pages/issues/view.rs`: the issue page (conversation, assignees, close/reopen).
- `src/http/repository/pages/issues/new.rs`: the new-issue form and its POST.
- `src/http/repository/pages/issues/actions.rs`: POST handlers for comments, state, edit, and assignees.
- `src/integration_tests/issues_api.rs`: compatibility API tests, the `fj` contract.
- `src/integration_tests/issues_ui.rs`: web UI tests.
- `docs/issues.md`: user guide for the Issues tab and the `fj` commands.

Changed:

- `src/db/migration.rs`: append the `issues`, `issue_assignees`, `issue_comments` migrations.
- `src/db/mod.rs`: `pub mod issues;` and the tests module wiring.
- `src/db/pull_requests/mod.rs`: numbering reads the shared sequence across `pull_requests` and `issues`.
- `src/lib.rs` or `src/main.rs` (wherever `pulls` is declared): `pub mod issues;`.
- `src/http/api/compat/issues.rs`: list handles `type=issues`; adds create, get, patch, templates, config, and the 422 unsupported routes.
- `src/http/api/compat/comments.rs`: each handler dispatches to issues or pull requests by number or ID.
- `src/http/api/compat/mod.rs`: `from_issue_error`; registers the new routes before the catch-all.
- `src/http/repository/pages/tabs.rs`: the `issues` tab in `DEFAULT_TABS` and `configured_tab`.
- `src/http/repository/pages/tests/tabs.rs`: expected tab list includes Issues (a deliberate change, not a weakened test).
- `src/http/repository/pages.rs`: module and re-exports for the issues handlers.
- `src/http/routes.rs`: UI routes for the issues handlers.
- `src/http/api/v1/repositories.rs` and `src/http/settings/pages/repositories.rs`: call `delete_issues_for_repo` beside the pull request deletion.
- `docs/pull-requests.md`, `docs/api.md`: the shared numbering and the new routes.
- `src/integration_tests.rs`: `mod issues_api; mod issues_ui;`.

## Types & signatures

Database (`src/db/issues/mod.rs`):

```rust
pub enum IssueState { Open, Closed }
impl IssueState {
    pub fn as_str(self) -> &'static str;
    pub fn parse(value: &str) -> Option<Self>;
}

pub struct Issue {
    pub id: String,
    pub repo_id: String,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub state: IssueState,
    pub author_user_id: String,
    pub assignee_user_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
}

pub struct NewIssue<'a> {
    pub repo_id: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub author_user_id: &'a str,
    pub assignee_user_ids: &'a [String],
}

pub struct IssueFilter {
    pub state: Option<IssueState>,
    /// Case-insensitive substring of the title or the body.
    pub query: Option<String>,
    pub author_user_id: Option<String>,
    pub assignee_user_id: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

pub struct IssueEdit {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<IssueState>,
}

impl Database {
    pub async fn create_issue(&self, new: NewIssue<'_>) -> Result<Issue, String>;
    pub async fn get_issue(&self, repo_id: &str, number: i64) -> Result<Option<Issue>, String>;
    pub async fn get_issue_by_id(&self, id: &str) -> Result<Option<Issue>, String>;
    pub async fn list_issues(&self, repo_id: &str, filter: &IssueFilter) -> Result<(Vec<Issue>, bool), String>;
    pub async fn count_issues(&self, repo_id: &str, filter: &IssueFilter) -> Result<u64, String>;
    pub async fn count_issues_by_state(&self, repo_id: &str) -> Result<(u64, u64), String>;
    pub async fn edit_issue(&self, issue_id: &str, edit: &IssueEdit, now: &str) -> Result<(), String>;
    pub async fn replace_issue_assignees(&self, issue_id: &str, user_ids: &[String], now: &str) -> Result<(), String>;
    pub async fn delete_issues_for_repo(&self, repo_id: &str) -> Result<(), String>;
}
```

Database (`src/db/issues/comments.rs`):

```rust
/// Issue comment IDs start here, so they never collide with pull request comment IDs.
pub const ISSUE_COMMENT_ID_BASE: i64 = 1 << 40;

pub fn issue_comment_api_id(row_id: i64) -> i64;
pub fn issue_comment_row_id(api_id: i64) -> Option<i64>; // None below the base

pub struct IssueComment {
    pub row_id: i64,
    pub issue_id: String,
    pub author_user_id: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: Option<String>,
}

impl Database {
    pub async fn add_issue_comment(&self, issue_id: &str, author_user_id: &str, body: &str) -> Result<IssueComment, String>;
    pub async fn list_issue_comments(&self, issue_id: &str) -> Result<Vec<IssueComment>, String>;
    pub async fn get_issue_comment(&self, row_id: i64) -> Result<Option<IssueComment>, String>;
    pub async fn update_issue_comment(&self, row_id: i64, body: &str, now: &str) -> Result<(), String>;
    pub async fn delete_issue_comment(&self, row_id: i64) -> Result<(), String>;
}
```

Rules (`src/issues/mod.rs`):

```rust
pub struct RepoRef<'a> {
    pub repo_id: &'a str,
    pub namespace: &'a str,
}

pub struct Actor<'a> {
    pub user_id: &'a str,
}

pub enum IssueError { NotFound, Forbidden, Invalid(String), Conflict(String), Internal(String) }

pub struct CreateInput<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub assignees: &'a [String], // usernames
}

pub struct IssueChange {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<IssueState>,
    pub assignees: Option<Vec<String>>, // usernames; replaces the set
}

pub struct IssuePage {
    pub items: Vec<Issue>,
    pub has_more: bool,
    pub total: u64,
}

pub async fn create(db: &Database, scope: &RepoRef<'_>, actor: &Actor<'_>, perms: &PermissionSet, input: CreateInput<'_>) -> Result<Issue, IssueError>;
pub async fn get(db: &Database, scope: &RepoRef<'_>, number: i64) -> Result<Issue, IssueError>;
pub async fn list(db: &Database, scope: &RepoRef<'_>, filter: &IssueFilter) -> Result<IssuePage, IssueError>;
pub async fn change(db: &Database, scope: &RepoRef<'_>, issue: &Issue, actor: &Actor<'_>, perms: &PermissionSet, change: IssueChange) -> Result<Issue, IssueError>;
pub async fn add_comment(db: &Database, issue: &Issue, actor: &Actor<'_>, perms: &PermissionSet, body: &str) -> Result<IssueComment, IssueError>;
pub async fn edit_comment(db: &Database, issue: &Issue, row_id: i64, actor: &Actor<'_>, perms: &PermissionSet, body: &str) -> Result<IssueComment, IssueError>;
pub async fn delete_comment(db: &Database, issue: &Issue, row_id: i64, actor: &Actor<'_>, perms: &PermissionSet) -> Result<(), IssueError>;
pub async fn resolve_assignees(db: &Database, scope: &RepoRef<'_>, usernames: &[String]) -> Result<Vec<String>, IssueError>;
```

Compatibility (`src/http/api/compat/issues.rs`):

```rust
pub struct IssueJson { /* Forgejo issue shape; pull_request marker null for issues */ }
impl IssueJson {
    pub fn from_issue(issue: &Issue, user: Option<UserJson>, assignees: Vec<UserJson>, comments: i64, origin: &str, owner: &str, repo: &str) -> Self;
    fn from_pull(pull: PullRequestJson) -> Self; // existing
}

#[get("/api/v1/repos/{owner}/{repo}/issues")] pub async fn list_issues(...) -> HttpResponse;
#[post("/api/v1/repos/{owner}/{repo}/issues")] pub async fn create_issue(...) -> HttpResponse;
#[get("/api/v1/repos/{owner}/{repo}/issues/{index}")] pub async fn get_issue(...) -> HttpResponse;
#[patch("/api/v1/repos/{owner}/{repo}/issues/{index}")] pub async fn edit_issue(...) -> HttpResponse;
#[get("/api/v1/repos/{owner}/{repo}/issue_templates")] pub async fn issue_templates(...) -> HttpResponse;
#[get("/api/v1/repos/{owner}/{repo}/issue_config")] pub async fn issue_config(...) -> HttpResponse;
// 422 answers for unsupported features:
pub async fn unsupported_dependencies(...) -> HttpResponse;
pub async fn unsupported_blocks(...) -> HttpResponse;
pub async fn unsupported_labels(...) -> HttpResponse;
pub async fn unsupported_issue_reactions(...) -> HttpResponse;
```

Compatibility, shared (`src/http/api/compat/mod.rs`):

```rust
pub fn from_issue_error(error: IssueError) -> HttpResponse;
```

Web UI (`src/http/repository/pages/issues/`):

```rust
pub(super) async fn load(ctx: &RepoContext, auth_state: &web::Data<TwigContext>, filter: IssueListQuery) -> Result<Markup, String>;
#[get("/{namespace}/{repo}/issues")] pub async fn issues_tab_handler(...) -> AwResult<Markup>;
#[get("/{namespace}/{repo}/issues/new")] pub async fn new_issue_page_handler(...) -> AwResult<Markup>;
#[post("/{namespace}/{repo}/issues/new")] pub async fn create_issue_handler(...) -> AwResult<HttpResponse>;
#[get("/{namespace}/{repo}/issues/{number}")] pub async fn issue_page_handler(...) -> AwResult<HttpResponse>;
#[post("/{namespace}/{repo}/issues/{number}/comments")] pub async fn comment_handler(...);
#[post("/{namespace}/{repo}/issues/{number}/comments/{id}/edit")] pub async fn edit_comment_handler(...);
#[post("/{namespace}/{repo}/issues/{number}/comments/{id}/delete")] pub async fn delete_comment_handler(...);
#[post("/{namespace}/{repo}/issues/{number}/state")] pub async fn state_handler(...);
#[post("/{namespace}/{repo}/issues/{number}/edit")] pub async fn edit_handler(...);
#[post("/{namespace}/{repo}/issues/{number}/assignees")] pub async fn assignees_handler(...);
```

## Call stack

Create via `fj issue create` (API):

```
POST /api/v1/repos/{o}/{r}/issues
  compat::issues::create_issue
    compat::authorized_repo(ViewNamespace)
    compat::issues  parse CreateBody (title, body, assignees)
    issues::resolve_assignees(db, scope, usernames)
      db.list_namespace_members(namespace)
    issues::create(db, scope, actor, perms, input)
      db.create_issue(NewIssue)          // BEGIN IMMEDIATE, shared number
    IssueJson::from_issue → 201
```

Search via `fj issue search` (API):

```
GET /api/v1/repos/{o}/{r}/issues?type=issues&...
  compat::issues::list_issues
    authorized_repo(ViewNamespace)
    reject non-empty labels/milestones → 422
    issues::list(db, scope, filter)
      db.list_issues + db.count_issues
    per row: db.get_user_by_id (author), assignees → IssueJson
    X-Total-Count, Link
```

Close with comment (`fj issue close -w`):

```
POST /api/v1/repos/{o}/{r}/issues/{n}/comments         → compat::comments::create_comment
  if issue(n): issues::add_comment → 201
  else: existing pulls::add_comment
PATCH /api/v1/repos/{o}/{r}/issues/{n}  {state: closed} → compat::issues::edit_issue
  issues::get; issues::change (author or PushChanges) → 201
```

Comment edit or delete (API): `compat::comments::edit_comment` with an ID.

```
if issue_comment_row_id(id) is Some(row) → issues::edit_comment / delete_comment
else → existing PR path
```

UI conversation (`GET /{ns}/{repo}/issues/{n}`): `open_repo` → `issues::get` (or redirect to `pulls/{n}` if only a PR has that number) → load comments and assignees → render.

UI close (`POST issues/{n}/state`): signed-in check (`signed_in`) → `issues::change` → 303 back to the issue.

## Test plan

Database (`src/db/issues/tests.rs`):

- `create_issue_shares_numbers_with_pull_requests`: create an issue, then a PR, then an issue in one repo; numbers are 1, 2, 3. A second repo starts at 1.
- `pull_request_after_issue_takes_next_shared_number`: the PR numbering reads issues (regression on PR creation).
- `list_filters_by_state_text_author_and_assignee`: each filter narrows the list; `query` matches body text, not just title.
- `list_reports_has_more_and_count`: paging flag and total.
- `count_issues_by_state_counts_open_and_closed`.
- `edit_issue_close_sets_closed_at_and_reopen_clears_it`.
- `replace_assignees_replaces_the_whole_set`.
- `issue_comment_ids_are_offset_and_round_trip`: `issue_comment_row_id(issue_comment_api_id(n)) == Some(n)`; a pull request id is `None`.
- `add_list_update_delete_comment`.
- `delete_issues_for_repo_leaves_other_repos`.

Rules (`src/issues/tests.rs`):

- `viewers_may_open_and_comment`; `viewers_may_not_edit_title`.
- `author_may_close_without_push_rights`; `non_author_viewer_may_not_close`.
- `only_push_rights_may_assign`.
- `assignee_must_be_a_namespace_member` (422 for an unknown name).
- `only_the_comment_author_may_edit_or_delete` (even a pusher is refused for someone else's comment, as Gate 1 decided).
- `title_is_required` (trimmed empty → invalid).

API (`src/integration_tests/issues_api.rs`), each one fails before its slice:

- `fj_search_returns_issues_with_total_header` (`type=issues`, `X-Total-Count`, `Link` when paged; `labels=` empty ignored).
- `create_issue_returns_201_in_forgejo_shape` (fields `number`, `title`, `state`, `user`, `assignees`, `pull_request: null`).
- `get_issue_by_pull_request_number_returns_pull_shape`.
- `patch_close_then_reopen_returns_201_each_time`.
- `assignees_replace_and_clear_with_fj_shapes`.
- `comment_round_trip_uses_issue_comment_ids` (POST → 201; list; PATCH; DELETE → 204).
- `pull_request_comments_still_work_through_issue_paths` (regression on the PR conversation).
- `unsupported_features_answer_422_with_reason` (labels param, dependencies, blocks, labels endpoint, reactions).
- `issue_templates_and_config_allow_blank_issues`.
- `non_member_assignee_is_rejected`.

UI (`src/integration_tests/issues_ui.rs`):

- `issues_tab_is_listed_and_renders_empty_state`.
- `new_issue_form_creates_and_redirects_to_issue_page`.
- `issue_page_shows_comments_and_state`.
- `comment_then_close_then_reopen_from_the_page`.
- `pull_request_number_redirects_to_pull_request`.
- `issue_list_filters_by_state_and_text`.

Tabs (`src/http/repository/pages/tests/tabs.rs`): the default tab list includes `issues` in its expected order (updated on purpose).

Live proof per slice: a running Twig server and the real `fj` binary, with a token. Each slice's status records the commands and output.

## Least confident decisions

1. **Comment IDs with a base offset (`1 << 40`).** Issue comments are stored in their own table, and their API IDs are offset to keep them apart from pull request comment IDs in the shared routes. The alternative is a single shared ID counter across both tables, which needs changes to the pull request insert paths. Challenge this if you prefer the second.
2. **Shared numbering.** Changes the pull request insert path. Existing pull request numbers do not change; new issues take the next free number, and new pull requests skip numbers taken by issues.
3. **`GET issues/{n}` on a pull request number** returns the pull request in issue shape, as the list already does. `PATCH issues/{n}` on a pull request number answers 422 and points to `pulls/{n}`.
4. **No `type` on the list** keeps returning pull requests, as it does today. Forgejo would return both; `fj` always sends `type`.
5. **Assigning requires push rights**, as Gate 1 left open. Viewers can open and comment.
6. **Labels and milestones are a 422, not empty.** `fj issue search -l bug` shows an error instead of a wrong empty result.
7. **Only the comment author edits or deletes it**, which is narrower than pull request comments (where pushers may delete). This follows Gate 1 exactly.
