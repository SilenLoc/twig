# Vertical Slices: E-ink-inspired themes

1. **Tracer bullet — working E-ink switch:** replace the old warm palettes with E-ink Light/Dark grayscale tokens, show the labeled theme control at the far right on standard, Paper, and Presentation views, and verify toggling plus browser-local persistence end to end.
2. **E-ink presentation system:** bundle IBM Plex Mono and IBM Plex Sans WOFF2 files with the SIL Open Font License, serve them through the existing asset registry/route, and reshape shared typography, spacing, panels, rules, and grayscale syntax/status cues; remove remote font requests and verify desktop and narrow layouts.
3. **Structured repository listing:** render Type/Path headings with DIR/FILE/UP labels from existing tree entries, keep current breadcrumbs and HTMX navigation intact, and verify directory and file browsing.

Each slice will add its targeted regression tests, run the relevant Rust test suite, and be manually checked in the browser before being marked complete. No analytics, telemetry, new routes, or server-side theme data are included.
