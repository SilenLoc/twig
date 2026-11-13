# Slices: Issues

Build order. Each slice ends in a working, testable state. Each one ends with a
live proof against a running Twig server and the real `fj` binary, recorded in
`00-status.md`.

1. **Tracer bullet.** The Issues tab appears (stub list), and `GET issues?type=issues`
   returns one fixed issue. `POST issues` returns a fixed `201`. `issue_templates` and
   `issue_config` answer. Proof: `fj issue search` lists the stub and `fj issue create`
   succeeds through the live server. Tab-list test updated.
2. **Real storage, the happy path.** Migrations and `db/issues/`. Shared numbering in the
   pull request insert. Create, list, and get on the API, and the Issues tab list, the
   new-issue form, and the read-only issue page. Proof: `fj issue create`,
   `fj issue search`, and `fj issue view` on the live server; a pull request created
   after an issue gets the next shared number.
3. **Conversation and state.** Issue comments with the ID offset; dispatch in the comment
   routes (pull request regression tested). Close and reopen through `PATCH`. UI comment
   composer, comment edit and delete for the author, close and reopen. Proof:
   `fj issue comment`, `fj issue close -w`, `fj issue view comments`. `fj` has no reopen
   command, so reopen is proven through the web UI and a raw `PATCH`.
4. **Edit and assignees.** `PATCH` title and body; `assignees` replace (with membership
   checks); the UI edit form and the assignee panel for people who can push. Proof:
   `fj issue edit title`, `fj issue edit body`, `fj issue assign`, `fj issue unassign`.
5. **Search and honest refusals.** Filters on `state=all|closed`, `q`, `created_by`,
   `assigned_by`, paging headers; UI filters. `422` answers for labels, milestones,
   dependencies, blocking, issue labels, and issue reactions. Proof: `fj issue search -s all -c <user> -a <user>`,
   `fj issue search -l bug` (422 message), `fj issue depend add` and `fj issue block add` (422 message).
6. **Cleanup and docs.** Repository deletion removes issues. `docs/issues.md`, `docs/api.md`,
   `docs/pull-requests.md`. Full `cargo test` run. Final live proof of the whole fj flow.
