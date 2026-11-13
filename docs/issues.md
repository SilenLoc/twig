# Issues

Twig repositories have issues: a place to record a bug, a request, or a question
that is not a change yet. Issues are available in the web UI on the repository's
**Issues** tab, and through the Forgejo/Gitea-compatible REST API, so the Forgejo
command-line client (`fj`) works with them.

## Using issues in the web UI

1. Open **Issues** and choose **New issue**. Give it a title and a description in
   Markdown. Anyone who can view the repository may open an issue.
2. The issue opens at `/{namespace}/{repo}/issues/{number}`. It shows the
   description and the conversation. Signed-in viewers can comment.
3. The author, and anyone who can push, can edit the title and description, and
   close or reopen the issue. The author can edit or delete their own comments.
4. Anyone who can push can assign members of the namespace from the **Assignees**
   panel, and remove them.
5. The list filters by **Open**, **Closed**, or **All**. The search box matches the
   title and the description. The author and assignee boxes take a username.

Issues and pull requests share one number sequence in each repository. `#12` is
one or the other, never both. An issue's page redirects to its pull request when
the number is a pull request.

## Using issues with `fj`

Log in with a personal access token (see [Pull Requests](pull-requests.md#connecting-t3-code-and-the-forgejogitea-clis)),
then use the repository's remote:

```sh
fj issue create "Uploader drops the last chunk" --body "Reproduced on a slow link."
fj issue search                       # open issues
fj issue search -s all -c alice       # every issue alice opened
fj issue view 12                      # title and description
fj issue view 12 comments
fj issue comment 12 "Looking into it."
fj issue edit 12 title "New title"
fj issue edit 12 body "New description"
fj issue assign 12 bob
fj issue unassign 12 bob
fj issue close 12 -w "Fixed in 0.4.3."
```

`fj` has no reopen command. Reopen an issue in the web UI, or with a `PATCH` to
`/api/v1/repos/{owner}/{repo}/issues/{number}` with `{"state": "open"}`.

`fj issue edit <issue> comment <index>` takes the comment's position in the list,
starting at 0, as `fj issue view <issue> comments` shows them.

### What Twig does not do

Twig has no labels, milestones, issue templates, dependencies, blocking, reactions
on issues, due dates, time tracking, attachments, pinning, locking, or
notifications. Commands for these answer with a refusal instead of a wrong result:

| Command | Result |
| --- | --- |
| `fj issue search -l <label>`, `-m <milestone>` | `Labels are not supported on Twig` (or milestones) |
| `fj issue depend ...` | `422`: issue dependencies are not supported |
| `fj issue block ...` | `422`: blocking is not supported |
| Reactions on an issue or issue comment | `422`: reactions on issues are not supported yet |

`fj` prints only the status for some of these (`unexpected status code 422`). The
server's message is in the response body.

## Permissions

| Action | Who may do it |
| --- | --- |
| See issues and comments | Anyone with `view_namespace` |
| Open an issue, comment | Anyone with `view_namespace` |
| Edit the title or description, close, or reopen | The author, or anyone with `push_changes` |
| Assign or unassign members | Anyone with `push_changes` |
| Edit or delete a comment | Its author only |

Assignees must be members of the namespace. Naming anyone else is refused with
`422`. API tokens are scoped the same way as pull requests: a token's grant is
intersected with its owner's current rights.

## API

The routes are under `/api/v1/repos/{owner}/{repo}/issues`. They use Forgejo's
shapes and its error body. The authoritative route list is in
[Pull Requests](pull-requests.md#api), which covers the shared endpoints.

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `issues?type=issues&state=&q=&created_by=&assigned_by=&page=&limit=` | Search issues. `X-Total-Count` and `Link` are set. |
| `POST` | `issues` | Open an issue (`title`, `body`, `assignees`) |
| `GET` | `issues/{number}` | An issue, or a pull request in issue shape |
| `PATCH` | `issues/{number}` | Change `title`, `body`, `state`, or `assignees` (replaces the set). Answers `201`. |
| `GET`, `POST` | `issues/{number}/comments` | The conversation |
| `GET`, `PATCH`, `DELETE` | `issues/comments/{id}` | One comment |
| `GET` | `issue_templates` | `404`: Twig has no templates |
| `GET` | `issue_config` | `{"blank_issues_enabled": true, "contact_links": []}` |

Issue comment IDs are 2⁴⁰ (`1099511627776`) plus the comment's row number, so the
first is `1099511627777`. They never collide with pull request comment IDs in the
shared routes.
