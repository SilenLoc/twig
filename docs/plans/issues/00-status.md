# Status: Issues (fj-compatible)

- Gate 1 — Product: APPROVED 2026-11-13 (user said "continue"; proposed defaults kept: viewers may open and comment, 50% metric target)
- Gate 2 — Architecture: APPROVED 2026-11-13 (user: "approve all")
- Gate 3 — Program Design: APPROVED 2026-11-13 (user: "approve all")
- Gate 4 — Slice plan: APPROVED 2026-11-13 (user: "approve all, all slices, no review from me needed")

## Slices

- [x] Slice 1 — tracer bullet: Issues tab stub; `GET/POST issues` stubs; templates and config; live `fj issue search` and `fj issue create`. Proof (live server, `fj` 0.x, token auth): `fj issue search` printed `1 issue / #1: Tracer issue (by admin)`; `fj issue create --no-template "Tracer title"` printed `created issue #1`; `fj issue create` with no flags printed `created issue #1` once `issue_templates` answered 404 (an empty 200 list made it refuse). Tests: `issues_api` (3), `issues_ui` (1), tab-order tests updated; `pages::tests::tabs` 17 pass.
- [x] Slice 2 — real storage, happy path: migrations, `db/issues`, shared numbering, create/list/get API, Issues list/new/view UI; live fj create/search/view. Proof (live server, real `fj`): `fj issue create` → `created issue #1`, `#2`; `fj issue search` lists both newest first; `fj issue view 1` shows title, body, `0 comments`; `git push -u` of a branch then `fj pr create` → `created pull request #3` (shared numbering after two issues); web list shows both; `/admin/tooling/issues/3` 303s to `pulls/3`; `/issues/99` 404; anonymous new page shows the sign-in notice. Tests: `db::issues` (7), `issues_api` (7), `issues_ui` (3); `cargo test pull` 43 pass.
- [x] Slice 3 — conversation and state: comments with ID offset and dispatch, close/reopen, UI comments and state; live fj comment, close -w, view comments. Proof (live, real `fj`): `fj issue comment 1 ...` posted; `fj issue view 1 comments` lists them; `fj issue close 2 -w "Fixed..."` closed #2 and the message appears as its last comment; `fj issue edit admin/tooling#1 comment 0 "..."` edits (note: `fj` takes the comment's 0-based list position, then `PATCH`es the real ID; a PATCH with the real ID 1099511627777 returned 200); reopen by `PATCH` returned 201, `closed_at` null, and the web list shows it open. Found and fixed: `assets` missing from comment JSON (`fj issue view comments` failed), and no single-comment `GET`. Tests: `issues_api` (+4 incl. PR comments and author-only), `issues_ui` (+2), 27 issue tests pass.
- [x] Slice 4 — edit and assignees: PATCH title/body, assignee replace with membership checks, UI edit and assignee panel; live fj edit, assign, unassign. Proof (live, real `fj`): `fj issue edit 1 title "..."` and `fj issue edit 1 body "..."` applied; `fj issue assign 1 admin` → `assigned 1 user to admin/tooling#1`; `fj issue view 1 assignees` lists `admin`; `fj issue assign 1 nobody` → 422 (`fj` prints only `unexpected status code 422`, not our message); `fj issue unassign 1 admin` → `unassigned 1 user`. Tests: DB replace (1), `issues_api` (+2), `issues_ui` (+1); 31 issue tests pass.
- [x] Slice 5 — search and honest refusals: filters, paging headers, 422 for labels/milestones/deps/blocks/reactions; live fj search filters and refusals. Proof (live, real `fj`): `fj issue search -s all` lists both; `-c admin` lists both (created by admin); `-a admin` lists none while unassigned, and after `fj issue assign 1 admin` lists `#1`; `-c nobody` lists none; `fj issue search -l bug` → `Error: validation failed: Labels are not supported on Twig`; `fj issue depend add 1 2` and `fj issue block add 1 2` → 422 (fj shows only the status). Tests: `fj_search_filters_by_state_text_and_people`, `features_twig_lacks_answer_422_with_a_reason`, `issue_list_searches_and_filters_by_people`; 34 issue tests pass. Issue reactions and labels are refused in the same way (see the API test).
- [x] Slice 6 — cleanup and docs: repository deletion, docs, full test run, final live fj flow. Proof: repository deletion removes issues (API test; mutation-checked: the test fails with the call removed). `docs/issues.md` added; `docs/api.md` and `docs/pull-requests.md` updated. `cargo fmt --check` clean; `cargo test` 605 passed. Final live run (real `fj`): `fj issue create` → #4; `fj issue comment 4`; `fj issue edit 4 title`; `fj issue assign 4 admin`; `fj issue close 4 -w "All checks pass."` → `Closed issue #4`; `fj issue search -s closed` lists #4; `fj issue view 4 comments` shows both comments; PATCH reopen → 201; `fj issue search -s open` lists #4, #2, #1.

## Notes for a fresh session

- Twig has no native issues today. `src/http/api/compat/issues.rs` only lists pull requests in issue shape (`type=pulls`); `type=issues` returns an empty list.
- Pull requests keep their conversations under `/repos/{o}/{r}/issues/{n}/comments`, so issue and pull request numbers must come from one sequence per repository.
- Verified `fj` (installed at `~/.cargo/bin/fj`) traffic against a throwaway stub server in `/tmp/fj-probe` (not in the repo). Routes and bodies are recorded in `02-architecture.md` once Gate 2 is written.
- `fj issue create` (no flags) needs `GET issue_templates` to answer `404` when the repo has no templates. An empty `200` list makes `fj` refuse with "uses issue templates" (found in the slice 1 live run).
- Forgejo returns `201` (not `200`) for `PATCH /issues/{n}`; `fj issue close` fails on `200`.

### Built versus the approved design (for a fresh session)

- `GET issue_templates` answers `404` (the design said `[]`). A `200` empty list makes `fj issue create` refuse; found in the live run.
- Comment JSON carries `assets: []` (Forgejo's shape, which `fj issue view comments` needs). This also applies to pull request comments.
- Added `GET /issues/comments/{id}` for issue comments (Forgejo route; `fj issue edit ... comment` does not call it, but it is tested).
- `fj issue edit <issue> comment <IDX>` takes a 0-based position in the comment list, then `PATCH`es the real ID. That is `fj`'s behavior, not Twig's.
- `GET .../issues/{n}/labels` answers `[]` (true: Twig has no labels). `POST|PUT|DELETE` labels answer `422`.
- Reactions on an issue or issue comment answer `422` with a reason (the design listed them).
- `issues::change` takes the repository scope (for assignee membership) and the title, body, state, and assignees fields.
- The list sorts by number, newest first (the design left the order open; the mockup says "newest").
- The UI list keeps search and account filters in tab links; there is no paging control (50 rows per view).
- `fj` prints only `unexpected status code 422` for refusals. The reason is in the body; `fj` does not show it.
- `Cargo.lock` changed (`twig` 0.4.1 → 0.4.2, cargo syncing the release commit's version). Not part of this feature; left as cargo wrote it.
- `twig-git` test `custom_hook_run_records_metadata_and_replays_both_output_streams_and_exit_status` failed once in a parallel `cargo test --workspace` run and passed on rerun (79/79). Not touched by this work.
