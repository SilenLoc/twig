# Product: Public binaries in the Scripts tab

## Problem

“I build binaries locally and want people to install or download the builds for their platform from the repository, without storing those binaries in the Git history. The same release may include several platform-specific files, and each file needs a direct download link that an install script can use even without opening the UI. The existing Scripts tab is where people already go to install things.”

## Success metric

For a repository with configured Scripts, every configured install script has its own copy button for the runnable command, and a visitor can directly download an asset for their platform. Each asset also has a direct HTTP URL that works independently of the UI. Each of the three highest versions can contain multiple platform assets, and no lower versions remain available.

## Announcement — the blog post before the feature

Twig's Scripts tab can now bring together install scripts and locally built binaries. Each configured installer has its own copy button, and a visitor can download the matching file directly from the repository page. Publish a version with as many platform builds as you need; every build also has a direct download URL, so install scripts and other tools can fetch it without going through the page. Binary files stay out of the Git history, keeping source checkouts lean. The tab keeps the three highest versions and all their uploaded platform builds, so recent releases remain available without growing without bound.

## Screens

- `mockups/registry-tab.html` — existing Scripts tab with its configured installer and multiple platform downloads under each version.
