# Product: Rich text editor for repository files

## Problem

When I notice a typo or need to make a small change to a file in a repository, I want to edit it on the site and commit it there, instead of cloning the repository and switching tools.

## Success metric

In a usability check, at least 9 out of 10 first-time users can edit a repository file and produce a commit containing the intended change within two minutes, without losing existing file content.

## Announcement — the blog post before the feature

You can now make small changes to repository files right from Twig. Open a file, edit its contents, and save your change as a commit with a message—no local checkout required. Markdown files get a rich editing experience, while other text files keep their source contents intact. If the repository changes while you edit, save your draft as a separate conflict copy instead of losing it. Every saved change becomes part of the repository's normal history, so it can be reviewed, pulled, or reverted like any other commit.

## Screens

- `./mockups/repository-file-editor.html` — an opened repository text file with an editor, commit message, and save action.
