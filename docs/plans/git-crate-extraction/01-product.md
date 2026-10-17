# Product: Git crate extraction

## Problem

People maintaining Twig need Git-related functionality to have a clear, independent home instead of being mixed in with the web application. This makes Git behavior easier to find, reason about, and change without obscuring unrelated application code.

## Success metric

100% of Git-specific functionality is grouped behind one clearly defined boundary, while all existing automated checks for the project pass.

## Announcement — the blog post before the feature

Twig's Git functionality now has a clear home of its own. This makes it easier for maintainers to understand where Git behavior lives and to make focused changes without wading through unrelated web-app code. The change preserves the existing user-facing behavior of Twig. It also gives future Git improvements a cleaner foundation.

## Screens

no UI
