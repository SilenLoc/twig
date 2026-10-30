# Program Design: Pull requests

## Files

Created:

- `src/db/pull_requests/mod.rs` — PR row type, state enum, create/get/list/update/merge/delete queries. Owns number allocation.
- `src/db/pull_requests/comments.rs` — conversation and review comments (one table, one ID sequence).
- `src/db/pull_requests/reviews.rs` — review rows and the atomic "submit review with inline comments" write.
- `src/db/pull_requests/reactions.rs` — reaction toggle and listing for PRs and comments.
- `src/db/pull_requests/reviewers.rs` — requested reviewers.
- `src/db/pull_requests/tests.rs` — database behaviour tests.
- `src/git/bare/handle/pulls.rs` — repository identity (`[twig] id`), merge-base, commit ranges, diffs, PR refs, branch CAS.
- `src/git/bare/handle/pulls/merge.rs` — mergeability and building merge, squash, and rebase results in memory.
- `src/git/bare/handle/pulls/tests.rs` — Git-level behaviour tests.
- `src/pulls/mod.rs` — service layer: validation, permission rules, state transitions, merge orchestration, ref sync. Shared by API and UI.
- `src/pulls/tests.rs` — service tests with a real temporary repository and database.
- `src/http/api/compat/mod.rs` — compatibility error body, credential check, repo+permission prologue, pagination helper.
- `src/http/api/compat/json.rs` — Forgejo-shaped response structs and builders.
- `src/http/api/compat/user.rs` — `GET /api/v1/user`, `GET /api/v1/version`, `POST /api/v1/user/repos`.
- `src/http/api/compat/repos.rs` — repository, branch, statuses, labels, assignees, contents, commit read and diff.
- `src/http/api/compat/pulls.rs` — PR list, create, get, patch, diff, patch, commits, merge, merge status, update.
- `src/http/api/compat/reviews.rs` — reviews, review comments, requested reviewers.
- `src/http/api/compat/comments.rs` — conversation comments and reactions.
- `src/http/repository/pages/pulls/mod.rs` — route registration and shared frame helpers.
- `src/http/repository/pages/pulls/list.rs` — Pull requests tab (filters, rows, New pull request button).
- `src/http/repository/pages/pulls/compare.rs` — New pull request form, GET and POST.
- `src/http/repository/pages/pulls/conversation.rs` — PR page: header, timeline, comment and review composer, merge box, edit form.
- `src/http/repository/pages/pulls/files.rs` — Files changed tab and inline comment form.
- `src/http/repository/pages/pulls/actions.rs` — POST handlers for comments, reviews, inline comments, merge, state, edit.
- `src/http/repository/pages/pulls/render.rs` — maud renderers for the pieces above (kept apart so handlers stay short).
- `src/integration_tests/pull_requests_api.rs` — compatibility API end-to-end tests.
- `src/integration_tests/pull_requests_ui.rs` — web UI end-to-end tests.
- `src/integration_tests/pull_requests_git.rs` — fetch and push tests over Git HTTP.
- `docs/pull-requests.md` — user guide, T3 Code and `fj`/`tea` setup, endpoint table.

Changed:

- `src/db/migration.rs` — appends the five tables and indexes above.
- `src/db/mod.rs` — `pub mod pull_requests;`.
- `src/auth/mod.rs` — `authenticate_bearer` also accepts scheme `token` with a `twigt_` value.
- `src/http/api.rs` — `pub mod compat;`.
- `src/http/routes.rs` — registers compat routes before the Git catch-all, and PR UI routes before the repository catch-all.
- `src/http/repository/pages.rs` (module root) — `mod pulls;` and tab additions.
- `src/http/repository/pages/context.rs` — `DEFAULT_TABS` and `configured_tab` gain `"pulls"`; `TAB_IDS` gains it too.
- `src/http/repository/pages/tabs.rs` — `TAB_IDS` and `DEFAULT_TABS` entries; `tab_body` is unchanged for pulls (its own handler).
- `src/git/mod.rs` — `sync_pull_refs` before upload-pack advertisement; `refuse_pull_ref_push` before receive-pack.
- `src/http/api/v1/repositories.rs` and `src/http/settings/pages/repositories.rs` — repository delete removes PR rows for that repo ID.
- `assets/twig.css` — PR-specific classes (state chips, timeline, diff rows, merge box), in the existing token system.
- `docs/api.md`, `docs/ui.md`, `README.md` — link the new doc and describe the tab.

## Types & signatures

No implementation bodies. Names are final unless a slice notes otherwise.

```rust
// src/db/pull_requests/mod.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestState { Open, Closed }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    pub id: String,
    pub repo_id: String,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub state: PullRequestState,
    pub author_user_id: String,
    pub head_branch: String,
    pub base_branch: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub merged_at: Option<String>,
    pub merged_by_user_id: Option<String>,
    pub merge_commit_oid: Option<String>,
}

pub struct NewPullRequest<'a> { pub repo_id: &'a str, pub title: &'a str, pub body: &'a str,
    pub author_user_id: &'a str, pub head_branch: &'a str, pub base_branch: &'a str }

pub struct PullRequestFilter { pub state: Option<PullRequestState>, pub head_branch: Option<String>,
    pub query: Option<String>, pub limit: u32, pub offset: u32 }

pub struct PullRequestEdit { pub title: Option<String>, pub body: Option<String>,
    pub state: Option<PullRequestState> }

impl Database {
    pub async fn create_pull_request(&self, new: NewPullRequest<'_>) -> Result<PullRequest, String>;
    pub async fn get_pull_request(&self, repo_id: &str, number: i64) -> Result<Option<PullRequest>, String>;
    pub async fn list_pull_requests(&self, repo_id: &str, filter: &PullRequestFilter) -> Result<(Vec<PullRequest>, bool), String>; // rows, has_more
    pub async fn find_open_pull_request(&self, repo_id: &str, head: &str, base: &str) -> Result<Option<PullRequest>, String>;
    pub async fn count_pull_requests_by_state(&self, repo_id: &str) -> Result<(u64, u64), String>;
    pub async fn edit_pull_request(&self, pull_request_id: &str, edit: &PullRequestEdit, now: &str) -> Result<(), String>;
    pub async fn mark_pull_request_merged(&self, pull_request_id: &str, merged_by: &str, merge_commit: &str, now: &str) -> Result<(), String>;
    pub async fn open_pull_request_heads(&self, repo_id: &str) -> Result<Vec<(i64, String)>, String>;
    pub async fn delete_pull_requests_for_repo(&self, repo_id: &str) -> Result<(), String>;
}

// src/db/pull_requests/comments.rs
pub struct PullRequestComment { pub id: i64, pub pull_request_id: String, pub review_id: Option<i64>,
    pub author_user_id: String, pub body: String, pub path: Option<String>,
    pub old_position: Option<i64>, pub new_position: Option<i64>, pub commit_oid: Option<String>,
    pub created_at: String, pub updated_at: Option<String> }
impl Database {
    pub async fn add_pull_request_comment(&self, pull_request_id: &str, author_user_id: &str, body: &str, now: &str) -> Result<PullRequestComment, String>;
    pub async fn list_pull_request_comments(&self, pull_request_id: &str) -> Result<Vec<PullRequestComment>, String>; // conversation + review comments
    pub async fn get_pull_request_comment(&self, id: i64) -> Result<Option<PullRequestComment>, String>;
    pub async fn update_pull_request_comment(&self, id: i64, body: &str, now: &str) -> Result<(), String>;
    pub async fn delete_pull_request_comment(&self, id: i64) -> Result<(), String>;
}

// src/db/pull_requests/reviews.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewState { Approved, RequestChanges, Comment }
pub struct PullRequestReview { pub id: i64, pub pull_request_id: String, pub author_user_id: String,
    pub state: ReviewState, pub body: String, pub commit_oid: String, pub submitted_at: String }
pub struct NewInlineComment<'a> { pub path: &'a str, pub body: &'a str, pub old_position: Option<i64>, pub new_position: Option<i64>, pub commit_oid: &'a str }
impl Database {
    pub async fn submit_pull_request_review(&self, pull_request_id: &str, author_user_id: &str,
        state: ReviewState, body: &str, commit_oid: &str, inline: &[NewInlineComment<'_>], now: &str)
        -> Result<PullRequestReview, String>; // single transaction
    pub async fn list_pull_request_reviews(&self, pull_request_id: &str) -> Result<Vec<PullRequestReview>, String>;
    pub async fn get_pull_request_review(&self, pull_request_id: &str, review_id: i64) -> Result<Option<PullRequestReview>, String>;
}

// src/db/pull_requests/reactions.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionContent { PlusOne, MinusOne, Laugh, Hooray, Confused, Heart, Rocket, Eyes }
impl ReactionContent { pub fn parse(api_value: &str) -> Option<Self>; pub fn api_value(self) -> &'static str; }
impl Database {
    pub async fn set_reaction(&self, target_key: &str, user_id: &str, content: ReactionContent, on: bool, now: &str) -> Result<(), String>;
    pub async fn list_reactions(&self, target_key: &str) -> Result<Vec<(String, ReactionContent)>, String>; // (username, content)
}

// src/db/pull_requests/reviewers.rs
impl Database {
    pub async fn set_requested_reviewer(&self, pull_request_id: &str, user_id: &str, requested: bool, now: &str) -> Result<(), String>;
    pub async fn list_requested_reviewers(&self, pull_request_id: &str) -> Result<Vec<String>, String>; // usernames
}

// src/git/bare/handle/pulls.rs
pub struct CommitSummary { pub oid: git2::Oid, pub author_name: String, pub author_email: String,
    pub committed_at: i64, pub message: String, pub parents: Vec<git2::Oid> }
pub struct DiffFile { pub old_path: Option<String>, pub new_path: Option<String>, pub status: FileStatus,
    pub additions: usize, pub deletions: usize, pub hunks: Vec<DiffHunk>, pub binary: bool }
pub enum FileStatus { Added, Deleted, Modified, Renamed }
pub struct DiffHunk { pub header: String, pub lines: Vec<DiffLine> }
pub struct DiffLine { pub kind: LineKind, pub old_line: Option<u32>, pub new_line: Option<u32>, pub text: String }
pub enum LineKind { Context, Added, Removed }

impl RepoHandle {
    pub fn repo_id(&self) -> Result<String, git2::Error>;              // reads or creates [twig] id
    pub fn merge_base(&self, a: git2::Oid, b: git2::Oid) -> Result<Option<git2::Oid>, git2::Error>;
    pub fn commits_in_range(&self, base: git2::Oid, head: git2::Oid, limit: usize) -> Result<Vec<CommitSummary>, git2::Error>; // oldest first
    pub fn count_commits_in_range(&self, base: git2::Oid, head: git2::Oid) -> Result<usize, git2::Error>;
    pub fn is_ancestor(&self, ancestor: git2::Oid, descendant: git2::Oid) -> Result<bool, git2::Error>;
    pub fn diff_patch(&self, from: git2::Oid, to: git2::Oid) -> Result<String, git2::Error>; // tree-to-tree, unified
    pub fn diff_files(&self, from: git2::Oid, to: git2::Oid) -> Result<Vec<DiffFile>, git2::Error>;
    pub fn commit_patch(&self, oid: git2::Oid) -> Result<String, git2::Error>;
    pub fn commit_summary(&self, oid: git2::Oid) -> Result<CommitSummary, git2::Error>;
    pub fn set_pull_ref(&self, number: i64, oid: git2::Oid) -> Result<(), git2::Error>; // refs/pull/N/head
    pub fn pull_ref(&self, number: i64) -> Result<Option<git2::Oid>, git2::Error>;
}

// src/git/bare/handle/pulls/merge.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy { Merge, Squash, Rebase }

pub enum MergeBuild { Ready { tip: git2::Oid }, Conflict { paths: Vec<String> } }

impl RepoHandle {
    pub fn mergeability(&self, base: git2::Oid, head: git2::Oid) -> Result<MergeBuild, git2::Error>; // no ref writes
    pub fn build_merge(&self, strategy: MergeStrategy, base: git2::Oid, head: git2::Oid,
        author: &git2::Signature<'_>, title: &str, message: &str) -> Result<MergeBuild, git2::Error>;
    pub fn advance_branch(&self, branch: &str, expected_old: git2::Oid, new: git2::Oid, reason: &str)
        -> Result<bool, git2::Error>; // false when the branch moved (compare-and-swap)
}

// src/pulls/mod.rs
pub struct RepoRef<'a> { pub namespace: &'a str, pub repo: &'a str, pub repo_id: &'a str }
pub struct Actor<'a> { pub user_id: &'a str, pub username: &'a str, pub email: Option<&'a str> }
pub struct Permissions<'a>(pub &'a crate::auth::PermissionSet);

#[derive(Debug)]
pub enum PullError { NotFound, Forbidden, Invalid(String), Conflict(String), Internal(String) }

pub struct CreateInput<'a> { pub base: &'a str, pub head: &'a str, pub title: &'a str, pub body: &'a str }

pub async fn create(db: &Database, handle: &RepoHandle, repo: RepoRef<'_>, actor: Actor<'_>,
    perms: &PermissionSet, input: CreateInput<'_>, now: &str) -> Result<PullRequest, PullError>;
pub async fn list(db: &Database, repo: RepoRef<'_>, filter: PullRequestFilter) -> Result<(Vec<PullRequest>, bool), PullError>;
pub async fn get(db: &Database, repo: RepoRef<'_>, number: i64) -> Result<PullRequest, PullError>;
pub async fn edit(db: &Database, repo: RepoRef<'_>, actor: Actor<'_>, perms: &PermissionSet,
    pr: &PullRequest, edit: PullRequestEdit, now: &str) -> Result<PullRequest, PullError>;
pub async fn merge(db: &Database, handle: &RepoHandle, repo: RepoRef<'_>, actor: Actor<'_>,
    perms: &PermissionSet, pr: &PullRequest, strategy: MergeStrategy, title: Option<&str>,
    message: Option<&str>, now: &str) -> Result<MergeOutcome, PullError>;
pub async fn sync_refs(db: &Database, handle: &RepoHandle, repo_id: &str) -> Result<(), String>;
pub async fn submit_review(...) -> Result<PullRequestReview, PullError>;
pub async fn add_comment(...) -> Result<PullRequestComment, PullError>;
pub async fn mergeability(handle: &RepoHandle, pr: &PullRequest) -> Result<Option<bool>, PullError>;

pub struct MergeOutcome { pub commit: String, pub state: PullRequestState }

// src/http/api/compat/mod.rs
pub struct PageQuery { pub page: Option<u32>, pub limit: Option<u32> }
pub fn numeric_id(key: &str) -> i64;
pub fn compat_error(status: StatusCode, message: impl Into<String>) -> HttpResponse;
pub async fn authorized_repo(req: &HttpRequest, server: &config::Server, auth: &TwigContext,
    owner: &str, repo: &str, permission: NamespacePermission)
    -> Result<CompatRepo, HttpResponse>;
pub struct CompatRepo { pub principal: AuthenticatedPrincipal, pub permissions: PermissionSet,
    pub handle: RepoHandle, pub repo_id: String }
pub fn paged<T: Serialize>(rows: Vec<T>, has_more: bool, base: &str, query: &PageQuery) -> HttpResponse;
```

## Call stack

Create PR (API): `pulls::create_handler` → `compat::authorized_repo(PushChanges)` → `RepoHandle::open` → `RepoHandle::repo_id` → `pulls::create` → `handle.commits_in_range`/`merge_base` (validation) → `Database::find_open_pull_request` → `Database::create_pull_request` → `handle.set_pull_ref` → `json::pull(...)` → `201`.

Create PR (UI): `compare::post_handler` → same service path. On success, 303 to `/{ns}/{repo}/pulls/{n}`.

Fetch with PRs: `git::git_handler` (GET, upload-pack advertisement) → `authenticate_git_request` → `RepoHandle::open` → `pulls::sync_refs` → `Database::open_pull_request_heads` → `handle.pull_ref` / `set_pull_ref` → `twig_git::backend::run_with_config`.

Push guard: `git::git_handler` (POST, receive-pack) → `refuse_pull_ref_push(body)` → 403 or continue.

Merge (API): `pulls::merge_handler` → `authorized_repo(PushChanges)` → `Database::get_pull_request` → `pulls::merge` → `handle.branch_tip(base)`, `handle.branch_tip(head)`, `handle.is_ancestor` → `handle.build_merge` → `handle.advance_branch` → `Database::mark_pull_request_merged` → `handle.set_pull_ref` (unchanged, keeps ref) → `200 {"merged": true, "message": "..."}`.

Merge (UI): `actions::merge_handler` → same service path → 303 to the conversation.

Mergeability (GET one): `pulls::get_handler` → `pulls::mergeability` → `handle.mergeability(base_tip, head_tip)`.

Diff: `pulls::diff_handler` → `pulls::get` → `handle.merge_base` → `handle.diff_patch(merge_base, head)`.

Review with inline comments: `reviews::submit_handler` → `pulls::submit_review` → `Database::submit_pull_request_review` (one transaction).

Conversation timeline: `conversation::handler` → `pulls::get` → `Database::list_pull_request_comments` + `list_pull_request_reviews` + `list_requested_reviewers` + `list_reactions` → render.

## Test plan

Database (`src/db/pull_requests/tests.rs`):
- `create_pull_request_allocates_sequential_numbers_per_repo` — numbers 1,2 for repo A; 1 for repo B.
- `concurrent_creates_never_share_a_number` — 8 parallel creates in one repo yield 8 distinct numbers.
- `list_filters_by_state_and_head_branch` — open filter excludes closed; head filter matches exactly.
- `closing_sets_closed_at_and_reopening_clears_it` — timestamps as specified.
- `mark_merged_sets_merge_fields_and_closes` — state closed, merged_at, merged_by, merge commit stored.
- `submit_review_is_atomic_with_inline_comments` — inline insert forced to fail rolls back the review row.
- `reactions_toggle_and_are_unique_per_user_and_content` — second add is a no-op; remove deletes.
- `delete_pull_requests_for_repo_removes_only_that_repo` — other repo's rows remain.
- `migrations_create_pull_request_tables` — extends the existing migration test list.

Git (`src/git/bare/handle/pulls/tests.rs`, temp bare repo built with commits):
- `repo_id_is_generated_once_and_persisted` — second call returns the same UUID.
- `repo_id_survives_directory_move` — moving the bare repo directory keeps the ID.
- `commits_in_range_excludes_base_history` — only head-only commits, oldest first.
- `diff_patch_contains_only_head_changes_from_merge_base` — patch names the changed file, not base-only changes.
- `diff_files_reports_line_numbers_and_counts` — additions/deletions and old/new line numbers in hunks.
- `mergeability_reports_conflicting_paths` — both sides change one line → conflict lists the path.
- `build_merge_creates_commit_with_both_parents` — parents are [base, head].
- `build_squash_creates_single_parent_commit_with_head_tree` — one parent; tree equals merged content.
- `build_rebase_replays_commits_in_order` — new commits have the base as parent; contents match.
- `advance_branch_is_compare_and_swap` — stale expected old returns false and leaves the branch.
- `set_pull_ref_points_refs_pull_number_head_at_oid` — `git rev-parse refs/pull/1/head` agrees.

Service (`src/pulls/tests.rs`):
- `create_requires_push_changes` — viewer gets `Forbidden`.
- `create_rejects_identical_branches_and_branches_without_commits` — `Invalid`.
- `create_rejects_duplicate_open_request_for_same_branches` — `Conflict`.
- `author_can_close_own_request_but_other_viewers_cannot` — author Ok; other user `Forbidden`.
- `merge_rejects_closed_request` — `Conflict`.
- `merge_conflict_leaves_base_unchanged` — base tip identical after failure.
- `merge_squash_default_message_uses_request_title` — message begins with the title.

Auth (`src/auth/tests.rs`):
- `token_scheme_authenticates_personal_access_token`; `bearer_still_authenticates`; `token_scheme_rejects_session_value`.

Git HTTP (`src/integration_tests/pull_requests_git.rs`):
- `refuse_pull_ref_push_detects_ref_commands` (pure unit test in `git/mod.rs`) — `refs/pull/1/head` in pkt-lines → true; `refs/heads/main` → false; gzip → fail closed.
- `receive_pack_to_refs_pull_is_forbidden` — real `git push origin HEAD:refs/pull/9/head` is rejected.
- `fetch_advertises_pull_ref_after_request_is_created` — `git ls-remote` lists `refs/pull/N/head`.
- `pull_ref_follows_head_branch_after_new_push` — after pushing to the head branch, a fetch shows the new SHA.

API (`src/integration_tests/pull_requests_api.rs`):
- `user_endpoint_returns_login_and_numeric_id` — `token` scheme; `login`, integer `id`.
- `repository_endpoint_reports_permissions_per_role` — contributor push true, viewer push false.
- `create_pull_request_returns_201_with_forgejo_shape` — fields from the T3 decoder all present.
- `list_pull_requests_paginates_with_link_header` — `limit=1`, `Link` has `rel="next"` on page 1 only.
- `get_unknown_pull_request_returns_404_message_body` — `{"message": ...}`.
- `patch_closes_and_reopens` — `state` toggles, `closed_at` set and cleared.
- `issue_comments_roundtrip_with_reactions` — create/list/edit/delete; reaction count.
- `reviews_roundtrip_with_inline_comments_and_requested_reviewers`.
- `diff_endpoint_returns_text_plain_patch`.
- `merge_endpoint_merges_and_status_endpoint_reports_204_then_404` — merge then 204; before merge 404.
- `token_scoped_to_other_namespace_is_forbidden` — a PAT for namespace A cannot read B's PRs.

UI (`src/integration_tests/pull_requests_ui.rs`):
- `pulls_tab_lists_open_requests_and_state_filter` — counts and rows.
- `compare_page_creates_request_and_redirects_to_it`.
- `conversation_shows_merge_box_only_to_pushers`.
- `files_page_renders_hunks_with_line_numbers`.
- `merge_form_merges_and_shows_merged_state`.
- `anonymous_read_of_public_repo_pulls_is_allowed_without_forms`.

Manual / external (slice 9): run `fj` or `tea` against a local Twig server if installable, plus `git fetch origin refs/pull/N/head`.

## Least confident decisions

1. **Repository identity in Git config (`[twig] id`).** Keeps PRs through renames and moves without touching rename code, but it is new state written from a read path (lazy creation). Alternative: a `repositories` table with a foreign key. Revisit if the Git-crate extraction moves config ownership.
2. **Hash-derived 53-bit integer IDs for users and repositories.** Deterministic, no migration. Collisions are negligible at this scale but not impossible. If an instance grows large, store a real integer per user.
3. **Syncing `refs/pull/N/head` on every fetch advertisement.** Costs one ref read per open PR per fetch. Fine for dozens of open PRs per repository; a hook or a cache would be needed for hundreds.
4. **Merge always creates a merge commit when head is not an ancestor**; "already merged" (head ancestor of base) returns 409 instead of silently closing. Forgejo's default is also a merge commit.
5. **Permission mapping:** read everything with `ViewNamespace`; create PR, submit review, merge, requested reviewers with `PushChanges`; comments and reactions with `ViewNamespace` (any reader may discuss, matching Forgejo); close, reopen, and edit with author or `PushChanges`.
6. **Rebase merge is implemented in memory** through cherry-picks, not by shelling out to `git`. Conflict behaviour (409, no partial writes) is the same as merge. Edge cases with merge commits inside the range are rejected as unsupported.
7. **Two error shapes.** Compatibility routes return Forgejo's `{"message","url"}`; Twig's own `/api/v1/namespaces` keeps `{"error":{...}}`. Two shapes by design, so existing clients do not change.
8. **Compressed receive-pack bodies fail closed** for the `refs/pull/*` guard. A rare legitimate push may be refused. Revisit if users report it.
9. **Deleting a repository deletes its PR rows** in both the API and the settings delete paths. A rename or move keeps them because the ID travels with the repository.
