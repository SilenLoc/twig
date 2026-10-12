# Status: Users and invite tabs

- Gate 1 — Product: APPROVED 2026-10-12
- Gate 2 — Architecture: APPROVED 2026-10-12
- Gate 3 — Program Design: APPROVED 2026-10-12
- Gate 4 — Slice plan: APPROVED 2026-10-12

## Slices

- [x] Slice 1 — Return only tab content for htmx swaps and cover both tabs with regression tests.

## Notes for a fresh session

The requested fix was that the Users and Invites tabs appeared twice after an htmx request. Full-page rendering now owns the tab bar; htmx responses contain only the content swapped into `#user-management-content`. A browser-driven follow-up was considered but not added; the route-level regression test covers both tab updates without introducing new browser-test infrastructure.
