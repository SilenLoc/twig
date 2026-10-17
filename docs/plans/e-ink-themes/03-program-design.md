# Program Design: E-ink-inspired themes

## Files

### Implementation

- `assets/twig.css` — replace the warm palettes with dark/light grayscale tokens; tune type, density, rules, panels, and row presentation; style the Type/Path repository listing and far-right theme control.
- `assets/fonts/ibm-plex-mono-variable.woff2` — vendor the variable IBM Plex Mono face for the technical interface.
- `assets/fonts/ibm-plex-sans-variable.woff2` — vendor the variable IBM Plex Sans face for prose and reading surfaces.
- `assets/fonts/OFL.txt` — include the fonts’ SIL Open Font License.
- `assets/theme.js` — retain the `twig-theme` browser preference and two-mode toggle; update accessible action labels for E-ink Light/Dark.
- `src/http/view.rs` — render the theme control outside the scrolling nav at the right edge of the masthead; use a visible E-ink label; remove the remote display-font request.
- `src/http/repository/pages/content.rs` — render a Type/Path header and a `DIR`, `FILE`, or `UP` label from existing tree-entry data, preserving existing routes and HTMX attributes.
- `src/http/repository/pages/presentation.rs` — remove the remote display-font request from the standalone print document so its typography uses the bundled faces.
- `src/http/assets.rs` — store embedded asset bodies as bytes so the existing `GET /assets/{filename}` route can serve WOFF2 as well as text assets. CSS will reference `/assets/ibm-plex-mono-variable.woff2` and `/assets/ibm-plex-sans-variable.woff2`; these stable URLs use the current revalidation cache policy. Responses use `Content-Type: font/woff2`.

### Tests

- `src/http/assets/tests.rs` — cover grayscale tokens, local font-face declarations, WOFF2 responses/content types, and the theme asset contract while retaining existing CSS constraints.
- `src/http/view.rs` — assert the theme control is outside the primary nav, present for signed-in and signed-out layouts, and carries the accessible theme-switch label.
- `src/http/repository/pages/tests/content.rs` — assert Type/Path headings and the correct type labels while preserving directory/file links, breadcrumbs, and HTMX navigation.
- `src/http/repository/pages/tests/paper.rs` — assert the Paper toolbar keeps the shared theme control after its other tools and exposes the visible variant label.
- `src/http/repository/pages/tests/presentation.rs` — assert the Presentation toolbar keeps the shared theme control after its other tools, exposes the visible variant label, and that the print document no longer requests a remote font.

The plan documents and approved mockups remain under `docs/plans/e-ink-themes/`; no additional application files are planned. No database or new route/handler; the existing asset registry and `GET /assets/{filename}` handler are extended for binary fonts.

## Types & signatures

Theme state remains a two-value browser preference; no Rust theme type or server-side setting is added:

```text
type Theme = "light" | "dark"
const storageKey = "twig-theme"
applyTheme(theme: Theme) -> void
```

The shared asset registry uses byte bodies for both textual assets and WOFF2 fonts:

```rust
struct Asset {
    stem: &'static str,
    extension: &'static str,
    content_type: &'static str,
    body: &'static [u8],
}
```

Existing Rust rendering signatures remain stable; the repository-row markup gains visible type information derived from `TreeEntry.is_dir`:

```rust
pub fn render_theme_toggle() -> maud::Markup;

fn render_content_row(
    href: &str,
    push: &str,
    entry: &TreeEntry,
) -> Markup;

fn render_content_dir(
    namespace: &str,
    repo: &str,
    path: &str,
    entries: &[TreeEntry],
) -> Markup;
```

The theme control continues to expose one button with the `.twig-theme-toggle` class, an accessible label naming both E-ink variants, and a visible active-variant label. The root attribute remains absent for dark and `data-twig-theme="light"` for light.

## Call stack

### Initial page render and theme restore

1. A normal page calls `render_layout` in `src/http/view.rs`; Paper and Presentation render their existing toolbar using the shared `render_theme_toggle` helper.
2. `theme.js` reads `twig-theme` and sets the root theme attribute before `twig.css` paints. Missing, invalid, or unavailable storage keeps the dark default.
3. `twig.css` selects the matching grayscale tokens and shared presentation rules and loads the bundled IBM Plex faces through the existing asset route. The masthead control is outside the scrollable nav; Paper and Presentation keep it at the end of their right-aligned tool groups.

### Theme switch

1. The delegated click handler finds `.twig-theme-toggle` and selects the other of the two modes.
2. `applyTheme` updates the root attribute and control’s accessible action label, then the handler stores `light` or `dark` under `twig-theme` when storage is available.

### Repository directory view

1. The existing content handler obtains `TreeEntry` values and calls `render_content_dir`.
2. The renderer emits the Type/Path heading and one linked row per entry; type is `DIR` or `FILE`, derived from `is_dir`, and path uses the existing entry name. A parent row is labeled `UP`.
3. Existing path URLs and HTMX target/push behavior remain unchanged; `twig.css` supplies the compact ruled layout.

## Test plan

- `test_twig_css_has_grayscale_dark_and_light_palettes` — both dark-default and light token blocks exist, and their palette values are grayscale.
- `test_twig_css_confines_raw_colour_values_to_the_token_block` — all component colors still reference tokens.
- `test_twig_css_honours_the_depth_and_motion_bans` — no shadows or rounded corners; reduced-motion and visible-focus rules remain.
- `test_twig_css_uses_only_the_two_authoritative_breakpoints` — only the established 48rem and 80rem breakpoints remain.
- `test_twig_css_uses_local_plex_font_faces` — the CSS declares IBM Plex Mono and IBM Plex Sans from local WOFF2 URLs and contains no remote font URL.
- `test_theme_asset_uses_existing_two_mode_preference_contract` — the script reads/writes `twig-theme`, applies the root attribute, retains a dark fallback, and names the E-ink target variant in its action labels.
- `test_assets_handler_serves_bundled_font_bytes` — both versioned WOFF2 assets return the expected MIME type and exact embedded bytes.
- `test_font_assets_include_open_font_license` — the bundled license identifies SIL OFL 1.1.
- `test_layout_places_theme_control_at_masthead_end` — for signed-in and signed-out layouts, the toggle is a masthead sibling after the primary nav, not inside it; it has the visible variant label and accessible action label, and the page no longer requests a remote font.
- `test_content_rows_show_type_and_path` — the listing has Type/Path headings, directories render `DIR`, files render `FILE`, and a parent row renders `UP`.
- Existing content-navigation assertions continue to pass — directory and file hrefs, breadcrumbs, HTMX targets, and push URLs are unchanged.
- `test_paper_toolbar_keeps_theme_toggle_last` and `test_presentation_toolbar_keeps_theme_toggle_last` — each toolbar retains the shared toggle after its other controls and exposes the visible variant label.
- `test_print_document_uses_local_font_assets` — the separate print document includes the e-ink stylesheet but no Google Fonts links.
- Manual browser check — switch both ways, reload to verify preference restoration, and check the far-right control plus compact listing at desktop and narrow viewport widths. This is the behavior check for the current JavaScript, which has no standalone JS test runner.
- Full Rust test suite — run after the implementation slices to catch regressions in shared page styling and rendering.

## Least confident decisions

1. **Grayscale syntax and status cues:** replace hue-based distinctions with contrast, weight, and explicit labels. This best fits the e-ink direction but could make code/status scanning slower.
2. **IBM Plex font choice and variable WOFF2 format:** use IBM Plex Mono for interface/navigation and IBM Plex Sans for longer prose; bundle both under SIL OFL. Their letterforms will noticeably shape the E-ink identity.
3. **Visible Type/Path columns:** make the directory view more structured without adding data. Validate that the type column remains useful rather than redundant with trailing slashes.
4. **Control width on small screens:** the explicit E-ink label improves clarity but consumes masthead space; verify the layout does not hide navigation or force horizontal scrolling.
