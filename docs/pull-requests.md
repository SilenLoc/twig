# Pull Requests

Twig repositories have pull requests in the Forgejo and Gitea style. A request
proposes that a branch be merged into another branch of the same repository. It
holds the title, the description, a conversation, reviews with inline comments,
and a merge.

Requests are available in the web UI on the repository's **Pull requests** tab,
and through a Forgejo/Gitea-compatible REST API. The API is what lets T3 Code and
the Forgejo and Gitea command-line tools (`fj`, `tea`) work with Twig as a
Forgejo server.

## Using pull requests in the web UI

1. Push a branch to the repository, then open **Pull requests** and choose
   **New pull request**. Pick the base and head branches. The page lists the
   commits the head adds and writes the title and description, pre-filled from a
   single commit.
2. The request opens at `/{namespace}/{repo}/pulls/{number}`. Its three views are
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

## Connecting T3 Code and the Forgejo/Gitea CLIs

T3 Code finds a Forgejo or Gitea server by a matching login in `fj` or `tea`.
For a Twig server:

1. Create a personal access token in **Settings → Access tokens**. Grant it the
   namespaces and permissions you want T3 Code to use. Read access is enough to
   browse requests; push access is needed to open, review, or merge them.
2. Log in with the token, using the server's address:

   ```sh
   fj --host https://twig.example auth add-token
   ```

   or, with `tea`, run `tea login add` and enter the same address and token when
   it asks.

3. Clone the repository over HTTPS, so the remote is
   `https://twig.example/{namespace}/{repo}.git`. T3 Code matches the remote's host
   against the login.

T3 Code then lists the repository's open requests, checks one out by fetching
`refs/pull/{number}/head`, and opens or merges requests through the API below.

## Git refs

Each open request has a read-only ref, `refs/pull/{number}/head`, that names its
head commit. It updates as the head branch is pushed. Clients fetch it by number:

```sh
git fetch origin refs/pull/1/head
```

Pushes to `refs/pull/*` are refused. Twig maintains these refs itself.

## API

All paths are under `/api/v1`. Every request needs a credential: a
personal access token as `Authorization: token <token>` or
`Authorization: Bearer <token>`, or HTTP Basic credentials. Browser sessions are
not accepted. Errors use Forgejo's shape, `{"message": "...", "url": ""}`.

Repositories are addressed as `/repos/{owner}/{repo}`. The owner is the Twig
namespace. Identifiers are integers: a request's `number` is its index within the
repository, and user and repository `id` values are stable integers derived from
Twig's internal IDs.

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/user` | The caller, for `whoami` |
| `GET` | `/version` | The Twig version |
| `POST` | `/user/repos` | Create a repository in the namespace named after your login |
| `GET` | `/repos/{o}/{r}` | The repository and the caller's permissions |
| `GET` | `/repos/{o}/{r}/branches/{branch}` | A branch and its commit |
| `GET` | `/repos/{o}/{r}/contents/{path}?ref=` | A file's bytes, base64 |
| `GET` | `/repos/{o}/{r}/git/commits/{sha}` | A commit |
| `GET` | `/repos/{o}/{r}/git/commits/{sha}.diff` | A commit's diff |
| `GET` | `/repos/{o}/{r}/compare/{base}...{head}` | The commits `head` adds to `base` |
| `GET` | `/repos/{o}/{r}/raw/{path}?ref=` | A file's raw bytes |
| `GET` | `/repos/{o}/{r}/commits/{sha}/status` | Combined status (`pending`: Twig runs no CI) |
| `GET` | `/repos/{o}/{r}/issues?type=pulls` | Requests in the issue shape, for searching (`type=issues` is empty) |
| `GET` | `/repos/{o}/{r}/pulls/{n}/files` | The files a request changes |
| `GET` | `/repos/{o}/{r}/pulls` | Requests (`state`, `head`, `sort`, `page`, `limit`) |
| `POST` | `/repos/{o}/{r}/pulls` | Open a request (`base`, `head`, `title`, `body`) |
| `GET` | `/repos/{o}/{r}/pulls/{n}` | One request, with `mergeable` and change size |
| `PATCH` | `/repos/{o}/{r}/pulls/{n}` | Change `title`, `body`, or `state` |
| `GET` | `/repos/{o}/{r}/pulls/{n}.diff` | The request's diff |
| `GET` | `/repos/{o}/{r}/pulls/{n}/commits` | The request's commits |
| `POST` | `/repos/{o}/{r}/pulls/{n}/merge` | Merge (`Do`: `merge`, `squash`, or `rebase`) |
| `GET` | `/repos/{o}/{r}/pulls/{n}/merge` | `204` once merged, `404` before |
| `POST` | `/repos/{o}/{r}/pulls/{n}/update?style=merge` | Merge the base into the head |
| `GET`, `POST` | `/repos/{o}/{r}/pulls/{n}/reviews` | List reviews, or submit one (`event`, `body`, `comments`) |
| `GET` | `/repos/{o}/{r}/pulls/{n}/reviews/{id}` | One review |
| `GET` | `/repos/{o}/{r}/pulls/{n}/reviews/{id}/comments` | A review's inline comments |
| `GET`, `POST`, `DELETE` | `/repos/{o}/{r}/pulls/{n}/requested_reviewers` | Reviewers (`reviewers`: logins) |
| `GET`, `POST` | `/repos/{o}/{r}/issues/{n}/comments` | Conversation comments |
| `PATCH`, `DELETE` | `/repos/{o}/{r}/issues/comments/{id}` | Edit or delete a comment |
| `GET`, `POST`, `DELETE` | `/repos/{o}/{r}/issues/{n}/reactions` | Reactions on the request (`content`) |
| `GET`, `POST`, `DELETE` | `/repos/{o}/{r}/issues/comments/{id}/reactions` | Reactions on a comment |
| `GET` | `/repos/{o}/{r}/statuses/{sha}` | Always empty: Twig runs no CI |
| `GET` | `/repos/{o}/{r}/labels` | Always empty: Twig has no labels |
| `GET` | `/repos/{o}/{r}/assignees` | Namespace members |

List endpoints accept `page` and `limit` (at most 50). A `Link` header with
`rel="next"` appears when another page exists, and `X-Total-Count` gives the total.
Without `page` or `limit`, a list returns every row.

### What is not supported

- Pull requests from another repository (forks). The head must be a branch of the
  same repository. Use `owner:branch` only when the owner is the same namespace.
- Labels, milestones, issues, and projects.
- Rebase updates (`update?style=rebase`). Only merge updates are offered, so
  clients do not show rebase updates.
- Resolving review threads and replying to them.
- Commit statuses and CI checks.
- Creating a *private* repository through the API. Privacy is set in `.twig.toml`.
- Renaming and moving repositories keeps their requests, because requests follow the
  repository's internal identity, not its name.
