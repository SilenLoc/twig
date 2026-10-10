# Program Design: Rich text editor for repository files

## Files

- `src/http/repository/editor.rs` — editor page, request validation, auth checks, conflict-copy filename generation, response rendering, and save handler.
- `src/http/repository/mod.rs` — expose the editor module.
- `src/git/bare/mod.rs` — expose the commit outcome type to repository handlers.
- `src/http/repository/pages.rs` — add an Edit action to eligible file previews.
- `src/http/routes.rs` — register edit GET/POST routes before the Git smart-HTTP fallback.
- `src/git/bare/handle.rs` — strict blob reads, compare-and-swap replacement commits, and compare-and-swap new-file commits.
- `src/http/assets.rs` — register local, versioned editor assets and tests.
- `src/integration_tests.rs` — route-level auth, edit, commit, conflict, and rejection tests.
- `assets/twig.css` — Twig theme and responsive editor styles.
- `assets/repository-editor.js` — Quill initialization, Markdown conversion, and edit-form synchronization.
- `assets/vendor/quill/quill.min.js`, `assets/vendor/quill/quill.js.LICENSE.txt`, `assets/vendor/quill/quill.snow.css`, `assets/vendor/quill/LICENSE` — Quill 2.0.3 distribution, bundle notice, theme, and license.
- `assets/vendor/marked/marked.umd.js`, `assets/vendor/marked/LICENSE` — Marked 18.1.0 UMD distribution for Markdown-to-editor input after sanitization.
- `assets/vendor/dompurify/purify.min.js`, `assets/vendor/dompurify/LICENSE`, `assets/vendor/dompurify/LICENSE-MPL` — DOMPurify 3.4.16 sanitization and both upstream license texts before HTML enters Quill.
- `assets/vendor/turndown/turndown.js`, `assets/vendor/turndown/LICENSE` — Turndown 7.2.4 serialization back to Markdown.
- `docs/plans/rich-text-editor/00-status.md` — gate state and slice checklist.
- `docs/plans/rich-text-editor/01-product.md` — product promise and success measure.
- `docs/plans/rich-text-editor/02-architecture.md` — architecture decisions.
- `docs/plans/rich-text-editor/03-program-design.md` — implementation contract and test plan.
- `docs/plans/rich-text-editor/04-slices.md` — approved build sequence.
- `docs/plans/rich-text-editor/mockups/repository-file-editor.html` — disposable product mockup.

## Types & signatures

```rust
pub enum CommitFileOutcome {
    Committed(git2::Oid),
    Conflict,
}

impl RepoHandle {
    pub fn head_oid(&self) -> Result<Option<git2::Oid>, git2::Error>;
    pub fn file_mode(&self, path: &str) -> Result<Option<i32>, git2::Error>;
    pub fn commit_file(
        &self,
        path: &str,
        content: &[u8],
        expected_head: git2::Oid,
        author_name: &str,
        author_email: &str,
        message: &str,
    ) -> Result<CommitFileOutcome, git2::Error>;
    pub fn commit_new_file(
        &self,
        path: &str,
        content: &[u8],
        expected_head: git2::Oid,
        author_name: &str,
        author_email: &str,
        message: &str,
    ) -> Result<CommitFileOutcome, git2::Error>;
}

#[derive(serde::Deserialize)]
struct CommitFileInput {
    content: String,
    message: String,
    expected_head: String,
    save_conflict_copy: bool,
}

fn conflict_copy_path(
    handle: &RepoHandle,
    source_path: &str,
    username: &str,
    timestamp: &str,
) -> Result<String, git2::Error>;

async fn edit_file_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<EditParams>,
) -> HttpResponse;

async fn commit_file_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<EditParams>,
    input: web::Json<CommitFileInput>,
) -> HttpResponse;
```

Markdown GET content is rendered to sanitized HTML and imported into Quill, with a Markdown source toggle retaining an exact, lossless editing path. If rich content is changed, it is serialized back to Markdown by Turndown. Other UTF-8 files use the source editor and are submitted byte-for-byte as text. File bytes are bounded to 1 MiB. Only existing regular blobs may be edited; binary, invalid UTF-8, symlink, ignored, unsafe, or missing paths are refused.

## Call stack

- View: content-file renderer → `/{namespace}/{repo}/edit/{path}` → session and namespace authorization → read HEAD and target blob → editor template → local editor assets.
- Save: editor form → authenticated POST handler → repeat access/path/file validation → parse expected object ID → `RepoHandle::commit_file` → create blob, nested tree, and commit → `reference_matching` updates the branch only if expected HEAD is still current → redirect to content file.
- Conflict copy: 409 response reveals an explicit copy action → authenticated POST revalidates content and authorization → generate an absent, safe same-directory username/timestamp filename → `RepoHandle::commit_new_file` creates its blob/tree/commit parented on latest HEAD → compare-and-swap updates the ref → redirect to copied file.
- Conflict/error: normal stale writes never update the ref. A ref race during copy creation also never updates the ref; retain the browser draft and allow another copy attempt.

## Test plan

- `editor_requires_a_valid_session` — unauthenticated GET/POST cannot read or modify repository files.
- `editor_requires_namespace_access` — a signed-in user without access cannot view or commit.
- `editor_rejects_ignored_unsafe_binary_and_non_utf8_paths` — hidden or unsafe data is not exposed or changed.
- `editor_shows_markdown_rich_and_exact_source_modes` — initial content, rich/source toggle, commit form, and base commit are rendered correctly.
- `commit_file_creates_a_commit_with_the_submitted_content_and_identity` — Git history contains the exact file data and user identity.
- `commit_file_preserves_file_mode_and_other_tree_entries` — only the requested regular file changes and executable mode remains intact.
- `commit_file_detects_stale_head_without_overwriting` — racing branch update returns conflict and remains current.
- `editor_requires_message_and_rejects_oversized_payloads` — invalid saves fail without a commit.
- `editor_reloads_after_409_and_confirms_a_verified_commit` — stale saves offer a refreshed base; success confirmation is shown only for current HEAD.
- `editor_saves_stale_draft_as_unique_sibling_commit` — explicit recovery creates a username/timestamp-prefixed file on latest HEAD while preserving the original and concurrent commit.
- `commit_new_file_refuses_existing_paths_and_stale_heads` — new-file commit never replaces an existing path and compare-and-swap detects a second race.
- `editor_assets_are_local_versioned_and_served` — Quill and converter assets resolve through the existing asset handler and never require a remote host.
- `markdown_round_trip_preserves_supported_formatting` — headings, bold, italic, strike, quotes, ordered/bulleted lists, links, inline code, and fenced blocks serialize to Markdown.

## Least confident decisions

1. A username/timestamp path can approach Git/filesystem component limits; sanitize the username, truncate the basename on a UTF-8 boundary, and disambiguate collisions without replacing any entry.
2. A second branch movement while committing the conflict copy must preserve the draft and be explicit; never silently switch to an unsafe forced update.
3. Editing is authorized by existing namespace membership, matching the current Git push rule. This grants write capability to every account the namespace currently authorizes.
4. Quill plus Marked, DOMPurify, and Turndown add vendored code and maintenance. Pin exact versions and retain each upstream license.
