# Product: Users and invite tabs

## Problem

When I use the Users and Invites area, the tabs show up twice after the page makes an htmx request. I want one Users tab and one Invites tab, not two copies of each.

## Success metric

Zero duplicate Users or Invites tab labels after the page loads or updates; verify by checking the rendered screen in both states.

## Announcement — the blog post before the feature

The Users and Invites area now keeps its navigation clear and predictable. You’ll see one Users tab and one Invites tab, including after the page updates in the background. This small fix makes it easier to tell where you are and avoids duplicate navigation.

## Screens

- `./mockups/users-and-invites.html` — Users and Invites navigation with one of each tab.
