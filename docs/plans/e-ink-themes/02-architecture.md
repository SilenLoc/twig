# Architecture: E-ink-inspired themes

## Fit

- `assets/twig.css` is Twig’s shared design system. Replace the current warm dark/light palettes with the complete e-ink grayscale pair, and adjust shared typography, density, panel, rule, and row presentation so the result is not a recolor-only theme. Load locally vendored IBM Plex Mono for the technical interface and IBM Plex Sans for prose through local `@font-face` rules.
- `assets/theme.js` already restores and toggles the browser’s two appearance modes. Keep the existing `twig-theme` local-storage preference and `data-twig-theme="light"` selector contract; the two modes now mean E-ink Light and E-ink Dark, with no legacy theme choices.
- `src/http/view.rs` renders the shared masthead and theme button. Put the button at the far right, outside the horizontally scrollable primary nav. Keep the corresponding control at the far right of the Paper and Presentation toolbars rendered in `src/http/repository/pages/paper.rs` and `src/http/repository/pages/presentation.rs`.
- `src/http/repository/pages/content.rs` renders the repository directory listing. Add a compact Type/Path presentation using the existing `TreeEntry.is_dir` and `TreeEntry.name`; preserve its current links, breadcrumbs, and partial-page navigation. No commit metadata or preview behavior is added.
- `src/http/assets.rs` embeds and versions the existing CSS and JavaScript assets. Extend the existing asset registry to serve binary WOFF2 font files through the same asset route; no new route is needed.

## Endpoints

No new endpoints. The existing `GET /assets/{filename}` route also serves the bundled WOFF2 font files; repository content requests remain unchanged.

## Data

No analytics, telemetry, server-side preference data, tables, or queries. The existing browser-local `twig-theme` value remains `light` or `dark`; those values select the replacement E-ink variants. IBM Plex Mono and IBM Plex Sans WOFF2 files are static application assets served through the existing asset route. The repository listing’s Type and Path are derived from the already-loaded `TreeEntry` values.

## Flow

1. The shared page shell (or Paper/Presentation toolbar) renders the theme control at the far right.
2. `theme.js` restores the saved light/dark choice before styles paint and applies it to the root element.
3. `twig.css` selects the matching E-ink token set and shared presentation rules and loads the bundled IBM Plex faces through the existing asset route; selecting the control toggles and persists the other variant.
4. On a repository directory view, the existing handler renders Type and Path from each `TreeEntry`; the same compact, ruled presentation applies through the shared stylesheet.

## External

No runtime third-party APIs, environment variables, webhooks, or external font requests. Vendor IBM Plex Mono and IBM Plex Sans under the SIL Open Font License and include the license with the font assets.
