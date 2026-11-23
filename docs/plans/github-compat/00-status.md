# Status: GitHub and gh compatibility

Decisions from the user:
- Serve REST at `/api/v3` and GraphQL at `/api/graphql`, the paths `gh` uses for a custom host (`GH_HOST=<twig host>`).
- Implement both REST and GraphQL; `gh pr` and `gh issue` use GraphQL, so both are required.
- Existing issues and pull requests are discarded; no data migration is needed.
- Auth tokens must work for `gh` and for `gh auth`.

## Done

- [x] REST issues on `/api/v3`: list (state, creator, assignee, paging), create, get, PATCH. GitHub shape and error body. Tests: `github_issues_api`.
- [x] REST pull requests on `/api/v3`: list, create (`draft` as `WIP:`), get (with `Accept: application/vnd.github.v3.diff`), PATCH, merge status, `PUT` merge (`merge`/`squash`/`rebase`, `sha` guard), files, commits, reviews (`APPROVE`/`REQUEST_CHANGES`/`COMMENT`), requested reviewers (`POST`/`DELETE`). Merged requests keep their files and commits, from the merge commit. Tests: `github_pull_request_lifecycle_over_rest`.
- [x] GraphQL at `/api/graphql`: `viewer`, `repository` (`issue`, `issues`, `issueOrPullRequest`, `pullRequest`, `pullRequests`), `search` (repo-scoped, with `is:`, `author:`, `assignee:`, `review-requested:`), and the mutations `createIssue`, `updateIssue`, `closeIssue`, `reopenIssue`, `addComment`, `createPullRequest`, `addPullRequestReview`, `mergePullRequest`, `closePullRequest`, `reopenPullRequest`. Tests: `graphql_reads_and_creates_issues_and_pull_requests`.
- [x] Tokens: `Authorization: token <PAT>` and `Bearer` work on REST and GraphQL; anonymous calls get `401`. `GET /api/v3/` is public, as on GitHub. Tests: `token_works_in_both_github_schemes_and_is_refused_without_one`.
- [x] Live check with the real `gh` 2.105 against a running server (TLS front end, `GH_HOST=127.0.0.1:18443`, `GH_ENTERPRISE_TOKEN`):
  - `gh auth status`, `gh api user`, `gh repo view`
  - `gh issue create | list | view | comment | edit | close | reopen`
  - `gh pr create | list | view | diff | status | comment | review | merge`
  - `gh api` for the REST pull request routes

## Not done or open

- [x] `gh auth login --with-token` works. It needs a hostname with no port (`gh` refuses ports in `--hostname`); `GH_HOST` with a port is not a login target. Verified with `gh` 2.105 against a LAN address through an HTTPS CONNECT proxy: the login stores the token, and `gh api user`, `gh repo view`, `gh pr list|view|create`, and `gh issue list|create` then use it.

- Forgejo routes (`/api/v1/...` compat, `fj`) are still mounted and still have tests. Removing them, and moving repository, actions and code routes to GitHub shapes, is not part of this slice.
- Not implemented: labels, milestones, projects, teams, reactions on issues, auto-merge, check runs, `gh pr checkout`, and `gh pr edit` beyond title and body.
- Known gap: the merged-PR commit list shows whatever `commit_ids_in_range` returns for the merge commit's parents; it is correct for merge commits and single-commit for squash or rebase.
