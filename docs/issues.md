# Issues

Twig repositories have issues: a place to record a bug, a request, or a question
that is not a change yet. Issues are available in the web UI on the repository's
**Issues** tab, and through the GitHub-compatible REST API for `gh` (see
[API](api.md#github-compatible-api)).

## Using issues in the web UI

1. Open **Issues** and choose **New issue**. Give it a title and a description in
   Markdown. Anyone who can view the repository may open an issue.
2. The issue opens at `/{namespace}/{repo}/issues/{number}`. It shows the
   description and the conversation. Signed-in viewers can comment.
3. The author, and anyone who can push, can edit the title and description, and
   close or reopen the issue. The author can edit or delete their own comments.
4. Anyone who can push can assign an owner of the namespace from the **Assignees**
   dropdown, and remove assignees. The dropdown lists owners who are not assigned yet.
5. The list filters by **Open**, **Closed**, or **All**. The search box matches the
   title and the description. The author and assignee boxes take a username.

Issues and pull requests share one number sequence in each repository. `#12` is
one or the other, never both. An issue's page redirects to its pull request when
the number is a pull request.

## Using issues from the command line

`gh issue` works with a Twig server through the GitHub-compatible REST API. The
routes are listed under [API](api.md#github-compatible-api). Conversation comments
and reactions are only on the web page.

### What Twig does not do

Twig has no labels, milestones, issue templates, dependencies, blocking, reactions
on issues, due dates, time tracking, attachments, pinning, locking, or
notifications. Requests that name them are refused with `422` and a reason, instead
of returning a wrong result. For example, creating an issue with a label answers
`Labels are not supported on Twig`.

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

The GitHub-compatible routes are under `/api/v3/repos/{owner}/{repo}/issues`:

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `issues?state=&creator=&assignee=&page=&per_page=` | Search issues. `X-Total-Count` and `Link` are set. |
| `POST` | `issues` | Open an issue (`title`, `body`, `assignees`). Answers `201`. |
| `GET` | `issues/{number}` | One issue |
| `PATCH` | `issues/{number}` | Change `title`, `body`, `state`, or `assignees` (replaces the set) |

Errors use GitHub's shape, `{"message": "...", "documentation_url": "..."}`.
