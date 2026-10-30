# Slices: Pull requests

Build order. Each slice ends runnable, with its tests passing. Each slice's
proof is recorded in `00-status.md` under the slice line.

1. **Tracer bullet.** `token` auth, `GET /api/v1/user`, `GET /api/v1/repos/{o}/{r}/pulls` returning one fixed PR, and a Pull requests tab showing a stub row. Proof: curl with `Authorization: token twigt_…` gets the JSON; the browser tab renders.
2. **Real storage.** Repository identity, PR tables, `POST pulls`, list, get, the compare/new page creating a real PR, and Forgejo-style error bodies. Proof: create via curl and UI; list shows it; unit and integration tests pass.
3. **T3 metadata and PR refs.** Repository, branch, statuses, labels, assignees; `refs/pull/N/head` maintained and advertised. Proof: `git ls-remote` shows `refs/pull/N/head`; `git fetch origin refs/pull/N/head` yields the head commit; new push to the head branch moves the ref after the next fetch.
4. **Conversation.** Issue comments, reactions, PATCH title/body/state, close and reopen, conversation page with comment composer and edit form. Proof: curl roundtrips; UI shows the timeline and close button.
5. **Reviews.** Verdict reviews, inline review comments, requested reviewers, review list and detail, review UI. Proof: submit approve and request-changes via curl and UI; the timeline shows them.
6. **Code review surfaces.** Commits list, `.diff`/`.patch`, commit read and diff, contents, Files changed page with line numbers and inline comments. Proof: diff text matches `git diff` for the same range; Files page shows hunks.
7. **Merge.** Mergeability, merge/squash/rebase with compare-and-swap, merge status endpoint, merge box. Proof: merge via API and UI; base branch has the new commit; conflicting merge returns 409 and base unchanged.
8. **T3 contract and hardening.** Replay T3's exact request sequence in tests, `POST user/repos`, `Link` pagination check, `refs/pull/*` push guard, docs (`docs/pull-requests.md`, `api.md`, `ui.md`, README). Proof: the replay test passes; a real push to `refs/pull/9/head` is refused.
9. **Real client.** Run `fj` or `tea` against a local Twig server if installable; record the outcome either way.

## Deviations and additions, recorded during the build

These were decided while building. Each one changes or extends what the earlier gate
documents say, so they are written down here for the next session.

- **Client compatibility routes beyond Gate 2.** The real `fj` client needed more than T3's
  provider does: `GET repos/{o}/{r}/issues?type=pulls` (search), `GET repos/{o}/{r}/compare/{base}...{head}`
  (fj suggests a title before creating), `GET repos/{o}/{r}/raw/{path}` (fj looks for a PR
  template), `GET repos/{o}/{r}/commits/{sha}/status` (fj pr status), and
  `GET repos/{o}/{r}/pulls/{n}/files` (fj pr view). `POST user/repos` was added for T3's
  createRepository. All are in `docs/pull-requests.md`.
- **`X-Total-Count`.** Every list sends it, because clients compute page counts from it.
- **Unknown `/api/v1/` paths** answer with Forgejo's JSON 404 instead of an empty body,
  because clients decode error bodies.
- **Clicking a diff line to comment is not built.** The Files changed page has a form that
  takes file, side, and line. Inline comments appear under their line.
- **`.patch` is served like `.diff`** (a unified diff), not as an mbox patch.
- **Titles.** A merge or squash title the client supplies is used as given. The default
  title for a squash is the request title with `(#n)`.
- **Updating a branch** is merge-style only. `allow_rebase_update` is `false`, so T3 offers
  only the merge update.
- **Commit authors** are linked to Twig users by email when one matches. Otherwise `author` is null.
- **User repository creation** (`POST user/repos`) creates in the namespace named after the
  caller's login. That namespace must exist, and the token must be scoped to it. `private`
  is refused, because privacy is set in `.twig.toml`.
- **Permissions adjusted to match T3's capability model.** Requesting a reviewer is allowed
  to the author or to anyone who can push. Reviews (approve, request changes, comment) need
  push rights, as Gate 3 decided, and the author cannot approve or request changes on their own request.
- **Git HTTP refusal.** A refused `refs/pull/*` push gets HTTP 403. `git` reports it as "remote
  end hung up", because git expects a receive-pack report. The server logs a warning.
- **Pre-existing test flake.** `hooks::tests::ref_updates_are_capped_and_marked_in_the_run_record`
  in `twig-git` fails with "Text file busy" when tests run in parallel. It predates this work.
