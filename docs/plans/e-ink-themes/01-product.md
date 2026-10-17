# Product: E-ink-inspired themes

## Problem

Twig’s current theme choices are expressive and warm, but people who prefer a restrained, e-paper-like workspace have no way to make the interface feel quiet, crisp, and focused on repository content. They want a coherent monochrome look in both bright and dim environments without losing clear hierarchy or readable code. The experience should feel purpose-built for reading and navigating code—not like the existing view with different colors.

## Success metric

Within 30 days of release, at least 20% of active users select one of the e-ink variants at least once, measured by theme-selection events among active users.

## Announcement — the blog post before the feature

Meet Twig’s new E-ink look, built for people who want the code to take center stage. Choose a bright, paper-like grayscale or a dark, ink-like version, both built around crisp boundaries, calm surfaces, and clear text. The new pair replaces the previous theme choices with a focused, consistent experience for browsing repositories and reading source. Switch between light and dark whenever your surroundings change.

## Design intent

The light and dark variants are the complete theme set and replace the previous themes. Both change the presentation as well as the palette: use a compact, structured repository listing that foregrounds entry type and path, clear ruled divisions, restrained controls, and a wider focus on readable source content. Keep one theme control visible at the far right of the top bar for switching between the two variants. The theme does not add repository data or new preview behavior.

## Screens

- `mockups/e-ink-light.html` — repository directory listing in the bright e-ink variant, structured around entry type and path.
- `mockups/e-ink-dark.html` — the same repository directory listing in the dark e-ink variant.
