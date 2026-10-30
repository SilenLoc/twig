# Product: Pull requests

## Problem

I host my code on Twig. When I want a change reviewed before it lands on the
main line, I have no way to ask for that inside Twig: I push a branch, then
review and merge it by hand, or I leave the Twig web page and use another forge.
I also already use T3 Code as my coding environment, and T3 Code can work with
Forgejo and Gitea servers, so I want my Twig repositories to show up there the
same way, with no special setup beyond logging in once.

## Success metric

- A repository owner can take a pushed branch from "I pushed it" to "reviewed
  and merged" entirely inside Twig, in under five minutes, measured by a
  scripted walkthrough run in the test suite.
- Twig works as a Forgejo/Gitea provider in T3 Code: a repository whose remote
  points at Twig shows its open pull requests, opens a pull request's branch
  locally, and creates a pull request, without any Twig-specific configuration
  in T3 Code. Measured by a manual check against a real T3 Code install before
  release (see slice 6 in `04-slices.md`).

## Announcement — the blog post before the feature

Twig repositories now have pull requests. Push a branch, open a pull request
against the branch you want to change, and the people who can see the
repository will see your title, description, commits, and the exact file
changes side by side. Comments, reviews with inline notes, and three merge
styles (merge commit, squash, or rebase) live on the same page, and closing a
request without merging is one click. Pull requests belong to the repository,
so they keep working when the repository is renamed or moved to another
namespace. If you use T3 Code or the Forgejo and Gitea command-line tools, they
read Twig's pull requests as they read any Forgejo server, so you can review
and merge without leaving your editor.

## Screens

- `mockups/pull-request-list.html` — the repository's Pull requests tab: open and closed filters, a New pull request button, one row per request.
- `mockups/pull-request-new.html` — the New pull request page: pick base and head branches, see the commit count and merge status, write a title and description.
- `mockups/pull-request-view.html` — one pull request: the Conversation (description, comments, reviews, merge box) and the Commits and Files changed tabs.
