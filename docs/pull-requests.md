# Pull Requests

Twig repositories have pull requests. A request proposes that a branch be merged
into another branch of the same repository. It holds the title, the description,
a conversation, reviews with inline comments, and a merge.

Requests are available in the web UI on the repository's **Pull requests** tab,
and through the GitHub-compatible REST and GraphQL APIs that `gh` uses. See
[API](api.md#github-compatible-api).

## Using pull requests in the web UI

1. Push a branch to the repository, then open **Pull requests** and choose
   **New pull request**. Pick the base and head branches. The page lists the
   commits the head adds and writes the title and description, pre-filled from a
   single commit.
2. The request opens at `/{namespace}/{repo}/pulls/{number}`. GitHub's address for
   the same page, `/{namespace}/{repo}/pull/{number}`, opens it too, with its
   `/files`, `/commits`, and `/edit` sub-pages and `.diff`. Its three views are
   **Conversation**, **Commits**, and **Files changed**.
3. **Conversation** holds the description and the timeline of comments and
   reviews. Anyone signed in with access can comment and react. The author and
   anyone who can push can edit or delete a comment, edit the title and
   description, close, or reopen.
4. **Files changed** shows each file's hunks with old and new line numbers. Add an
   inline comment with the form at the top: choose the file, the side (new or old),
   and the line. Inline comments appear under their line.
5. Reviewers are listed in the side panel. The author, or anyone who can push, can
   request a review from a namespace member. Anyone who can push and is not the
   author can **Approve** or **Request changes**.
6. Anyone who can push can merge from the **Merge** box, choosing one of three
   styles: a merge commit, a squash, or a rebase. The merge box shows whether the
   branch merges cleanly with its base.

### Permissions

| Action | Who may do it |
| --- | --- |
| See requests, commits, diffs, and reviews | Anyone with `view_namespace` |
| Comment, react | Anyone with `view_namespace` |
| Open a request | Anyone with `push_changes` (the head is a branch in the same repository) |
| Edit the title or description, close, or reopen | The author, or anyone with `push_changes` |
| Edit or delete a comment | Its author; deleting also works for anyone with `push_changes` |
| Approve or request changes | Anyone with `push_changes` who is not the author |
| Request or withdraw a reviewer | The author, or anyone with `push_changes` |
| Merge, or update the head from the base | Anyone with `push_changes` |

API tokens are scoped the same way: a token's grant is intersected with its
owner's current rights, so a token cannot do more than its owner can.

## Git refs

Each open request has a read-only ref, `refs/pull/{number}/head`, that names its
head commit. It updates as the head branch is pushed. Clients fetch it by number:

```sh
git fetch origin refs/pull/1/head
```

Pushes to `refs/pull/*` are refused. Twig maintains these refs itself.

## Command line and API

`gh` works with a Twig server through the GitHub-compatible API. Log in with a
personal access token and the server's host, as described in
[API](api.md#github-compatible-api), then use the usual commands, for example:

```sh
gh pr list -R twig.example/namespace/repository
```

The routes that serve requests are `/api/v3/repos/{owner}/{repo}/pulls`, with
`/{number}`, `/merge`, `/files`, `/commits`, `/reviews`, and
`/requested_reviewers` beneath it, and the `pullRequest`, `pullRequests`,
`createPullRequest`, `addPullRequestReview`, `mergePullRequest`,
`closePullRequest`, and `reopenPullRequest` operations on `/api/graphql`. Errors
use GitHub's shape, `{"message": "...", "documentation_url": "..."}`.

Requests are numbered within the repository and share their numbers with issues
(see [Issues](issues.md)).

### What is not supported

- Pull requests from another repository (forks). The head must be a branch of the
  same repository.
- Labels, milestones, and projects. Issues are supported; see [Issues](issues.md).
- Resolving review threads and replying to them. Inline comments are listed as
  threads of one comment each, and every thread stays unresolved.
- Auto-merge, updating a request's branch from its base, reverting a request,
  deleting its branch through the API, and marking files as viewed. The API answers
  these with an `UNSUPPORTED` error, and reports auto-merge as not allowed.
- Stacked requests. Requests never belong to a stack.
- Required status checks: CI results do not gate a merge. See [CI](ci.md).
- Creating a *private* repository through the API. Privacy is set in `.twig.toml`.
- Renaming and moving repositories keeps their requests, because requests follow the
  repository's internal identity, not its name.
