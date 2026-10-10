# Vertical slices: Rich text editor for repository files

1. **Authenticated source-edit tracer:** add the file Edit entry point, an existing-file source editor, session/namespace authorization, validation, and an actual commit on successful save; reject stale repository tips instead of overwriting them.
2. **Vendored Markdown rich editing:** vendor Quill and its local conversion/sanitizing dependencies, enable rich/source toggle for Markdown, wire local versioned assets, and preserve non-Markdown text as source.
3. **Hardening and polish:** cover unsupported blobs, ignored paths, size and commit-message errors, conflict feedback, keyboard/accessibility behavior, styles, and end-to-end regression tests.
4. **Conflict-copy recovery:** after a stale save, offer an explicit action that commits the draft as a unique sibling file prefixed by username and UTC timestamp to seconds, parented on the latest branch tip; prove original and concurrent changes remain intact.

Each slice ends with focused tests and a runnable proof. The first slice is an intentionally plain UI, but its save path creates a real Git commit so the complete auth-to-history path is exercised before adding rich formatting.
