# Vertical Slices: Git crate extraction

## Slice 1 — Crate tracer bullet

Create the `twig-git` workspace crate with its initial public API for repository path safety, then route the existing repository editor validation through that crate. This is the smallest real app-to-crate path: no mock endpoint or UI is needed because the refactor does not add a user-facing capability. Add/keep crate-level path-safety tests and run them alongside the editor's existing behavior tests. The application must build with the new crate dependency.

**Proof:** `cargo test -p twig-git` and the targeted Twig repository editor tests pass; invalid traversal paths remain rejected by the existing editor flow.

## Slice 2 — Repository operations and read APIs

Move bare-repository access, config, history, listings, name/path policy, repository initialization, hooks, and safe filesystem management operations into `twig-git`. Update Twig UI and integration-test callers to use crate APIs and public types; remove direct production `git2` usage from the app. Keep all authorization and UI response decisions in Twig.

**Proof:** crate unit tests and the targeted repository browsing, editing, namespace/settings, and private-repository tests pass; move/delete/rename operations preserve the existing paths, conflict handling, and repository contents.

## Slice 3 — Smart HTTP and workspace completion

Move Git request classification and CGI backend execution into `twig-git`; rename the remaining app-side handler module to an explicit HTTP/auth adapter; wire the existing routes through it; remove `src/git/` after all its domain code and hook assets have moved. Finalize dependency ownership and verify no behavior or endpoint changes.

**Proof:** Git smart-HTTP auth/fetch/push integration tests pass, `cargo test --workspace` passes, and `cargo tree -p twig --edges normal` shows Git implementation dependencies only through `twig-git`.
