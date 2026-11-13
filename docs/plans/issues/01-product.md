# Product: Issues

## Problem

A Twig repository can hold code and pull requests, but there is nowhere to record
a bug, a request, or a question that is not a change yet. Anyone who wants to track
one has to leave Twig for another host, or keep it in their head. People who work
from the terminal with the Forgejo command-line client (`fj`) are the most affected:
`fj` can already reach Twig, but its issue commands find nothing and cannot file
anything, so the terminal workflow stops at the code.

## Success metric

Within 30 days of release, at least half of the repositories that have had a push
in that window have at least one issue opened, counted from the server's admin
database page. The target (50%) is a proposal; confirm or change it at this gate.

## Announcement — the blog post before the feature

Twig repositories now have issues. Open one to record a bug, a request, or a
question, then discuss it, assign it to someone, and close it when it is done. You
can do all of this in the web UI or from the Forgejo command-line client (`fj`) you
already use. Issues and pull requests share one numbering in each repository, so
`#12` always means the same thing. Search and filter by open or closed, by text,
by author, and by assignee.

## Scope

In this release:

- Open an issue with a title and a Markdown description.
- Read an issue and its comments, the same way pull request conversations read.
- Comment; edit or delete your own comment.
- Edit the title and description; close and reopen.
- Assign people from the repository's members.
- Find issues by open, closed, or all; by text; by author; by assignee.
- Close with a final comment, from `fj` or the web.

Not in this release: labels, milestones, issue templates, dependencies and blocking,
reactions, due dates, time tracking, attachments, pinning, locking, and
notifications.

## Decisions for approval

- **Where it lives.** An **Issues** tab sits beside **Pull requests** in every
  repository.
- **Numbering.** Issues and pull requests share one number sequence per repository.
- **Who may open and comment.** Anyone who can view the repository. This matches the
  rule for pull requests and needs your confirmation.
- **Who may edit, close, or reopen.** The author, or anyone who can push.
- **Visibility.** The same as pull requests: whatever a viewer can see in the repository.
- **Labels are out.** Commands and flags that need labels, milestones, or dependencies
  will return a clear "not supported" answer rather than an empty or wrong result.

## Screens

- `mockups/issue-list.html`: the Issues tab, with open and closed counts, search, and the New issue button.
- `mockups/issue-view.html`: one issue, its conversation, the assignee panel, and Close or Reopen.
- `mockups/issue-new.html`: the new-issue form.
