# Status: Rich text editor for repository files

- Gate 1 — Product: APPROVED 2026-10-10
- Gate 2 — Architecture: APPROVED 2026-10-10
- Gate 3 — Program Design: APPROVED 2026-10-10
- Gate 4 — Slice plan: APPROVED 2026-10-10

## Slices

- [x] Slice 1 — Open a repository file in an editor and save a real Git commit.
- [x] Slice 2 — Add Quill formatting for Markdown while preserving Markdown files.
- [x] Slice 3 — Handle file validation, commit conflicts, and error feedback.
- [x] Slice 4 — Preserve a conflicted draft as a uniquely named, committed file on the latest branch tip.

## Notes for a fresh session

- The user asked for a Quill-based rich text editor for files in Git repositories, asked that edits be committed (so authentication and Git conflicts matter), explicitly requested the Software Factory gates, and explicitly said not to ask questions.
- Therefore, document each gate and proceed autonomously without approval prompts. Record assumptions and any decisions that need future review in the gate documents.
- Do not eagerly connect to the SQLite-compatible database (project-level constraint; this feature should not need it).
- `Cargo.lock` was already modified before this task. Preserve that unrelated change.
- Product interpretation to validate against the existing code: allow editing all UTF-8 text files; render rich text as Markdown for Markdown files, and preserve source text for other text formats. Binary files remain uneditable.
- Slice 1 proved auth-gated source editing, exact content/author commit creation, and stale-HEAD conflict refusal with `cargo test --locked repository_editor -- --nocapture` (2 integration tests); Git commit builder tests also pass (2 tests).
- Slice 2 vendors Quill 2.0.3, Marked 18.1.0, DOMPurify 3.4.16, and Turndown 7.2.4 from their npm distributions with upstream licenses. Assets are served by Twig's local versioned asset handler; no runtime CDN requests are used.
- Slice 2 verification: a temporary Happy DOM smoke test verified Quill mounting, lossless untouched source-mode toggles, Markdown serialization, DOMPurify filtering, and base-HEAD submission. A follow-up headless Chromium Playwright run exercised every toolbar control and fixed/verified Quill-specific list, code-block, and strikethrough Markdown serialization.
- Slice 3 verification: `cargo test --locked` passed all 379 Rust tests, covering auth, namespace access, Markdown/source editing, real commits, stale-ref conflict safety, file type/path/size validation, success confirmation, and no-op/message validation. `cargo clippy --locked --all-targets` completed with only pre-existing warnings in `src/db/binaries.rs` and `src/http/repository/binaries.rs`.
- Follow-up requirement from the user: after a stale-revision conflict, provide an explicit second save action that commits the unchanged draft as a new same-directory file prefixed by sanitized username and UTC timestamp to seconds; do not overwrite the original or another concurrent edit. The user requested no questions, so update gates and proceed autonomously.
- Slice 4 verification: `cargo test --locked` passed all 383 tests. Integration coverage proves an edit draft can be saved as a prefixed sibling commit parented on the latest tip, retains concurrent modifications, and still saves if the original file was deleted. Filename tests cover sanitization, collisions, and UTF-8-safe component truncation. Headless Chromium Playwright verified the draft remains editable after 409, the copy action saves the exact draft and navigates to the new file, and the reload action intentionally replaces it with the latest source.
