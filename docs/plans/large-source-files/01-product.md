# Product: Smaller, clearer source modules

## Problem

Twig's large source files make related behavior harder to find and review. Tests mixed into production-heavy files add to the scroll and make it less obvious where a change belongs. Developers want code grouped by meaningful responsibility, not just moved around to satisfy a line-count target.

## Success metric

Reduce the 16 files in the supplied Marg inventory that exceed 500 physical lines to fewer than 500 lines each, using cohesive test or responsibility boundaries. Review the four near-threshold files in that inventory, but split them only where a clear boundary improves discoverability. All existing tests must continue to pass, with no intended behavior change.

## Announcement — the blog post before the feature

Twig's internals are getting easier to navigate. We are separating focused tests and distinct responsibilities where that makes the code clearer to work on. This is a maintenance-only change: Twig's behavior and user-facing pages will stay the same. The result should be smaller, more focused places to make future changes.

## Screens

No UI.
