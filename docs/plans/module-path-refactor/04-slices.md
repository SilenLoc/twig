# Slices: Easier-to-navigate Twig modules

1. **Tracer bullet — auth feature:** group auth handlers, auth pages, and the shared request-session helper under `http/auth/`; update the route module and tests, then run the auth and full test suites.
2. **Tree feature:** group the MessagePack tree endpoint and tree UI under `http/tree/`; use `http/view` only for shared rendering and run tree plus full tests.
3. **Repository feature:** group repository pages and binary upload/download handlers under `http/repository/`; update routes/callers and run repository/binary plus full tests.
4. **Remaining page features:** group namespace, overview, settings, docs, and optional test pages in their feature directories; flatten the old generic view layer to shared render helpers, then run full tests, formatting, and Clippy.
