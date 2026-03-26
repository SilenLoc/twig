# Tachyons v4.12.0 — Complete Documentation

> Tachyons is a functional CSS toolkit for designing in the browser.
> Source: https://tachyons.io/docs/

---

## Table of Contents

- [Overview](#overview)
- [Typography](#typography)
  - [Type Scale](#type-scale)
  - [Measure](#measure)
  - [Line Height / Leading](#line-height--leading)
  - [Tracking / Letter Spacing](#tracking--letter-spacing)
  - [Font Weights](#font-weights)
  - [Font Style](#font-style)
  - [Vertical Align](#vertical-align)
  - [Text Align](#text-align)
  - [Text Transform](#text-transform)
  - [Text Decoration](#text-decoration)
  - [White Space](#white-space)
  - [Font Families](#font-families)
- [Layout](#layout)
  - [Debugging](#debugging)
  - [Debug with a Grid](#debug-with-a-grid)
  - [Basic Grid](#basic-grid)
  - [Flexbox](#flexbox)
  - [Box Sizing](#box-sizing)
  - [Spacing](#spacing)
  - [Floats](#floats)
  - [Clearfix](#clearfix)
  - [Display](#display)
  - [Widths](#widths)
  - [Max Widths](#max-widths)
  - [Heights](#heights)
  - [Position](#position)
- [Theming](#theming)
  - [Skins (Colors)](#skins-colors)
  - [Skins Pseudo / Animations](#skins-pseudo--animations)
  - [Hovers](#hovers)
  - [Background Size](#background-size)
  - [Borders](#borders)
  - [Border Radius](#border-radius)
  - [Box Shadow](#box-shadow)
  - [Opacity](#opacity)
- [Elements](#elements)
  - [Images](#images)
  - [Links](#links)
  - [Lists](#lists)
  - [Forms](#forms)
  - [Tables](#tables)

---

## Overview

Tachyons is a functional/atomic CSS framework built around the idea that CSS classes should do one thing and do it well. Instead of writing complex custom CSS, you compose pre-defined utility classes directly in HTML.

**Key concepts:**

- **Single-purpose classes** — each class sets exactly one CSS property
- **Responsive suffixes** — append `-ns` (not-small), `-m` (medium), or `-l` (large) to any class for breakpoint-specific overrides
- **No cascade conflicts** — because each class is single-purpose, specificity issues are minimal
- **Ratio-based scales** — spacing, type sizes, and widths follow mathematical scales (powers of 2)

**Media Query Breakpoints:**

| Suffix | Breakpoint |
|--------|-----------|
| *(none)* | All screen sizes (mobile-first) |
| `-ns` | Not small — `@media screen and (min-width: 30em)` |
| `-m` | Medium — `@media screen and (min-width: 30em) and (max-width: 60em)` |
| `-l` | Large — `@media screen and (min-width: 60em)` |

---

## Typography

### Type Scale

> "The simplest scale is a single note, and sticking with a single note draws more attention to other parameters, such as rhythm and inflection… In the sixteenth century, a series of common sizes developed among European typographers, and the series survived with little change and few additions for 400 years… This is the typographic equivalent of the diatonic scale."
> — Robert Bringhurst, *The Elements of Typographic Style*

Tachyons provides a simple 8-step type scale. Rather than declaring many arbitrary font-sizes in your CSS, use these utilities to adjust text size directly in HTML.

**Base class:** `f` (font-size)
**Modifiers:** `1`–`7` (step in scale), plus `f-headline` and `f-subheadline` for hero sizes
**Responsive suffixes:** `-ns`, `-m`, `-l`

| Class | Size | Pixels (approx) |
|-------|------|-----------------|
| `.f-6` / `.f-headline` | `6rem` | 96px |
| `.f-5` / `.f-subheadline` | `5rem` | 80px |
| `.f1` | `3rem` | 48px |
| `.f2` | `2.25rem` | 36px |
| `.f3` | `1.5rem` | 24px |
| `.f4` | `1.25rem` | 20px |
| `.f5` | `1rem` | 16px |
| `.f6` | `0.875rem` | 14px |
| `.f7` | `0.75rem` | 12px *(use with caution — hard to read)* |

**Source CSS (`src/_type-scale.css`):**

```css
/*
   TYPE SCALE
   Docs: http://tachyons.io/docs/typography/scale/

   Base:
    f = font-size

   Modifiers
     1 = 1st step in size scale
     2 = 2nd step in size scale
     ...
     7 = 7th step in size scale

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

/* For Hero/Marketing Titles */
.f-6, .f-headline  { font-size: 6rem; }
.f-5, .f-subheadline { font-size: 5rem; }

/* Type Scale */
.f1 { font-size: 3rem; }
.f2 { font-size: 2.25rem; }
.f3 { font-size: 1.5rem; }
.f4 { font-size: 1.25rem; }
.f5 { font-size: 1rem; }
.f6 { font-size: .875rem; }
.f7 { font-size: .75rem; }

@media (--breakpoint-not-small) {
  .f-6-ns, .f-headline-ns { font-size: 6rem; }
  .f-5-ns, .f-subheadline-ns { font-size: 5rem; }
  .f1-ns { font-size: 3rem; }
  .f2-ns { font-size: 2.25rem; }
  .f3-ns { font-size: 1.5rem; }
  .f4-ns { font-size: 1.25rem; }
  .f5-ns { font-size: 1rem; }
  .f6-ns { font-size: .875rem; }
  .f7-ns { font-size: .75rem; }
}
/* -m and -l variants follow the same pattern */
```

**Examples:**

```html
<h1 class="f-headline lh-solid">Hero Title</h1>
<h1 class="f1 lh-title">Section Title</h1>
<h2 class="f2 lh-copy">Subtitle</h2>
<p class="f5 lh-copy">Body text</p>
<small class="f6">Small caption</small>
```

---

### Measure

Measure refers to the length of a line of text — one of the most important factors in readability.

> "Anything from 45 to 75 characters is widely regarded as a satisfactory length of line for a single-column page… the 66-character line (counting both letters and spaces) is widely regarded as ideal."
> — Robert Bringhurst, *The Elements of Typographic Style*

| Class | Max Width | ~Characters |
|-------|-----------|-------------|
| `.measure` | `30em` | ~66 characters |
| `.measure-wide` | `34em` | ~80 characters |
| `.measure-narrow` | `20em` | ~45 characters |

**Additional utilities:**

| Class | Description |
|-------|-------------|
| `.indent` | `text-indent: 1em` — book-style paragraph indent |
| `.small-caps` | `font-variant: small-caps` |
| `.truncate` | Truncate overflowing text with ellipsis |

**Source CSS (`src/_typography.css`):**

```css
/*
   TYPOGRAPHY
   http://tachyons.io/docs/typography/measure/

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

/* Measure is limited to ~66 characters */
.measure        { max-width: 30em; }

/* Measure is limited to ~80 characters */
.measure-wide   { max-width: 34em; }

/* Measure is limited to ~45 characters */
.measure-narrow { max-width: 20em; }

/* Book paragraph style */
.indent {
  text-indent: 1em;
  margin-top: 0;
  margin-bottom: 0;
}

.small-caps { font-variant: small-caps; }

/* Truncate text with ellipsis */
.truncate {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
/* -ns, -m, -l responsive variants follow the same pattern */
```

**Example:**

```html
<p class="measure lh-copy f5">
  Readable paragraph text that stays at an ideal line length.
</p>

<h2 class="f2 measure-narrow">Narrow headline for visual impact</h2>

<p class="mw4 truncate">Very long text that gets cut off...</p>
```

---

### Line Height / Leading

> *lead* [rhyming with red]: Originally a strip of soft metal used for vertical spacing between lines of type. Now meaning the vertical distance from the baseline of one line to the baseline of the next. Also called **leading**.

> "Many people with cognitive disabilities have trouble tracking lines of text when a block of text is single spaced. Providing spacing between 1.5 to 2 allows them to start a new line more easily once they have finished the previous one."
> — WCAG 2.0 Compliance Techniques

| Class | Value | Use case |
|-------|-------|----------|
| `.lh-solid` | `line-height: 1` | Headings, display text |
| `.lh-title` | `line-height: 1.25` | Titles |
| `.lh-copy` | `line-height: 1.5` | Body copy |

**Source CSS (`src/_line-height.css`):**

```css
/*
   LINE HEIGHT / LEADING
   Docs: http://tachyons.io/docs/typography/line-height

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.lh-solid { line-height: 1; }
.lh-title { line-height: 1.25; }
.lh-copy  { line-height: 1.5; }

@media (--breakpoint-not-small) {
  .lh-solid-ns { line-height: 1; }
  .lh-title-ns { line-height: 1.25; }
  .lh-copy-ns  { line-height: 1.5; }
}
@media (--breakpoint-medium) {
  .lh-solid-m { line-height: 1; }
  .lh-title-m { line-height: 1.25; }
  .lh-copy-m  { line-height: 1.5; }
}
@media (--breakpoint-large) {
  .lh-solid-l { line-height: 1; }
  .lh-title-l { line-height: 1.25; }
  .lh-copy-l  { line-height: 1.5; }
}
```

---

### Tracking / Letter Spacing

Letter-spacing is the consistent white-space between letters in a piece of text. In typography this is known as **tracking**.

- **Uppercase text should always be tracked** to improve readability.
- **Lowercase text should never be tracked.**
- For large text, a negative tracking may be desirable depending on the typeface.

| Class | Value | Use |
|-------|-------|-----|
| `.tracked` | `letter-spacing: .1em` | Standard uppercase tracking |
| `.tracked-tight` | `letter-spacing: -.05em` | Tight/negative tracking |
| `.tracked-mega` | `letter-spacing: .25em` | Wide/mega tracking |

**Source CSS (`src/_letter-spacing.css`):**

```css
/*
   LETTER SPACING
   Docs: http://tachyons.io/docs/typography/tracking/

   Base:
     tracked = letter-spacing

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.tracked       { letter-spacing:  .1em; }
.tracked-tight { letter-spacing: -.05em; }
.tracked-mega  { letter-spacing:  .25em; }

@media (--breakpoint-not-small) {
  .tracked-ns       { letter-spacing:  .1em; }
  .tracked-tight-ns { letter-spacing: -.05em; }
  .tracked-mega-ns  { letter-spacing:  .25em; }
}
/* -m and -l variants follow the same pattern */
```

**Example:**

```html
<h2 class="ttu tracked f4">Uppercase Heading</h2>
<h1 class="tracked-mega ttu f2">Mega Tracked Display</h1>
```

---

### Font Weights

> Robert Bringhurst suggests two rules for font-weight:
> 1. Use bold weights sparingly.
> 2. Font-weight should decrease as font-size increases.

**Base class:** `fw` (font-weight)
**Modifiers:** `1`–`9` (literal CSS weight values 100–900), plus `.normal` and `.b`

| Class | Value |
|-------|-------|
| `.normal` | `font-weight: normal` |
| `.b` | `font-weight: bold` |
| `.fw1` | `font-weight: 100` |
| `.fw2` | `font-weight: 200` |
| `.fw3` | `font-weight: 300` |
| `.fw4` | `font-weight: 400` |
| `.fw5` | `font-weight: 500` |
| `.fw6` | `font-weight: 600` |
| `.fw7` | `font-weight: 700` |
| `.fw8` | `font-weight: 800` |
| `.fw9` | `font-weight: 900` |

**Source CSS (`src/_font-weight.css`):**

```css
/*
   FONT WEIGHT
   Docs: http://tachyons.io/docs/typography/font-weight/

   Base fw = font-weight
   Modifiers: 1–9 = literal values 100–900

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.normal { font-weight: normal; }
.b      { font-weight: bold; }
.fw1    { font-weight: 100; }
.fw2    { font-weight: 200; }
.fw3    { font-weight: 300; }
.fw4    { font-weight: 400; }
.fw5    { font-weight: 500; }
.fw6    { font-weight: 600; }
.fw7    { font-weight: 700; }
.fw8    { font-weight: 800; }
.fw9    { font-weight: 900; }
/* Responsive variants follow the same pattern */
```

---

### Font Style

Single-purpose classes to set `font-style`. Italics are used to emphasize content. Common uses include titles, vehicle names, scientific terms, and quotes.

| Class | Value |
|-------|-------|
| `.i` | `font-style: italic` |
| `.fs-normal` | `font-style: normal` |

**Source CSS (`src/_font-style.css`):**

```css
/*
   FONT STYLE
   Docs: http://tachyons.io/docs/typography/font-style/

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.i         { font-style: italic; }
.fs-normal { font-style: normal; }

@media (--breakpoint-not-small) {
  .i-ns         { font-style: italic; }
  .fs-normal-ns { font-style: normal; }
}
/* -m and -l variants follow the same pattern */
```

---

### Vertical Align

Use vertical align to set typography just right. Vertical align works on **inline-level elements** (`display: inline` and `display: inline-block`) and on **table cells**.

| Class | Value |
|-------|-------|
| `.v-base` | `vertical-align: baseline` |
| `.v-mid` | `vertical-align: middle` |
| `.v-top` | `vertical-align: top` |
| `.v-btm` | `vertical-align: bottom` |

**Source CSS (`src/_vertical-align.css`):**

```css
/*
   VERTICAL ALIGN
   Docs: http://tachyons.io/docs/typography/vertical-align/

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.v-base { vertical-align: baseline; }
.v-mid  { vertical-align: middle; }
.v-top  { vertical-align: top; }
.v-btm  { vertical-align: bottom; }
/* Responsive variants follow the same pattern */
```

**Example:**

```html
<p class="dib v-top">Content</p>
<p class="dib v-top">Content</p>

<!-- Table cells -->
<div class="dt">
  <div class="dtc v-mid">Vertically centered</div>
</div>
```

---

### Text Align

Simple utilities for setting text-alignment. Designing for readability across infinite screen sizes often requires different text alignments at different breakpoints.

| Class | Value |
|-------|-------|
| `.tl` | `text-align: left` |
| `.tr` | `text-align: right` |
| `.tc` | `text-align: center` |
| `.tj` | `text-align: justify` |

**Source CSS (`src/_text-align.css`):**

```css
/*
   TEXT ALIGN
   Docs: http://tachyons.io/docs/typography/text-align/

   Base t = text-align
   Modifiers:
     l = left
     r = right
     c = center
     j = justify

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.tl { text-align: left; }
.tr { text-align: right; }
.tc { text-align: center; }
.tj { text-align: justify; }

@media (--breakpoint-not-small) {
  .tl-ns { text-align: left; }
  .tr-ns { text-align: right; }
  .tc-ns { text-align: center; }
  .tj-ns { text-align: justify; }
}
/* -m and -l variants follow the same pattern */
```

---

### Text Transform

Classes to set the capitalization of text. If using uppercase, combine with a tracking utility for best readability.

| Class | Value |
|-------|-------|
| `.ttc` | `text-transform: capitalize` |
| `.ttl` | `text-transform: lowercase` |
| `.ttu` | `text-transform: uppercase` |
| `.ttn` | `text-transform: none` |

**Source CSS (`src/_text-transform.css`):**

```css
/*
   TEXT TRANSFORM
   Docs: http://tachyons.io/docs/typography/text-transform/

   Base: tt = text-transform
   Modifiers:
     c = capitalize
     l = lowercase
     u = uppercase
     n = none

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.ttc { text-transform: capitalize; }
.ttl { text-transform: lowercase; }
.ttu { text-transform: uppercase; }
.ttn { text-transform: none; }
/* Responsive variants follow the same pattern */
```

---

### Text Decoration

| Class | Value |
|-------|-------|
| `.strike` | `text-decoration: line-through` |
| `.underline` | `text-decoration: underline` |
| `.no-underline` | `text-decoration: none` |

**Source CSS (`src/_text-decoration.css`):**

```css
/*
   TEXT DECORATION
   Docs: http://tachyons.io/docs/typography/text-decoration/

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.strike      { text-decoration: line-through; }
.underline   { text-decoration: underline; }
.no-underline { text-decoration: none; }

@media (--breakpoint-not-small) {
  .strike-ns       { text-decoration: line-through; }
  .underline-ns    { text-decoration: underline; }
  .no-underline-ns { text-decoration: none; }
}
/* -m and -l variants follow the same pattern */
```

---

### White Space

Controls how whitespace is rendered.

| Class | Value | Description |
|-------|-------|-------------|
| `.ws-normal` | `white-space: normal` | Suppress whitespace, break lines on width |
| `.nowrap` | `white-space: nowrap` | Keep text on one line (use with truncation) |
| `.pre` | `white-space: pre` | Preserve all whitespace and line breaks |

**Source CSS (`src/_white-space.css`):**

```css
/*
   WHITE SPACE
   Docs: http://tachyons.io/docs/typography/white-space/

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.ws-normal { white-space: normal; }
.nowrap    { white-space: nowrap; }
.pre       { white-space: pre; }
/* Responsive variants follow the same pattern */
```

---

### Font Families

Tachyons includes pre-defined font stacks using attractive system fonts with fallbacks. Relying on system fonts improves page performance and helps blend in with the user's OS.

**Source CSS (`src/_font-family.css`):**

```css
/*
   FONT FAMILY GROUPS
   Docs: http://tachyons.io/docs/typography/font-family/
*/

/* System font stacks */
.sans-serif {
  font-family: -apple-system, BlinkMacSystemFont, 'avenir next', avenir,
               'helvetica neue', helvetica, ubuntu, roboto, noto, 'segoe ui',
               arial, sans-serif;
}
.serif { font-family: georgia, times, serif; }
.system-sans-serif { font-family: sans-serif; }
.system-serif      { font-family: serif; }

/* Monospaced (for code) */
code, .code { font-family: Consolas, monaco, monospace; }
.courier    { font-family: 'Courier Next', courier, monospace; }

/* Named sans-serif typefaces */
.helvetica  { font-family: 'helvetica neue', helvetica, sans-serif; }
.avenir     { font-family: 'avenir next', avenir, sans-serif; }

/* Named serif typefaces */
.athelas  { font-family: athelas, georgia, serif; }
.georgia  { font-family: georgia, serif; }
.times    { font-family: times, serif; }
.bodoni   { font-family: "Bodoni MT", serif; }
.calisto  { font-family: "Calisto MT", serif; }
.garamond { font-family: garamond, serif; }
.baskerville { font-family: baskerville, serif; }
```

**Available font family classes:**

- `.sans-serif` — full system sans-serif stack
- `.serif` — georgia/times serif
- `.system-sans-serif` — bare `sans-serif`
- `.system-serif` — bare `serif`
- `.code` / `code` — monospace code font
- `.courier` — courier monospace
- `.helvetica` — Helvetica Neue
- `.avenir` — Avenir Next
- `.athelas` — Athelas (serif)
- `.georgia` — Georgia
- `.times` — Times
- `.bodoni` — Bodoni MT
- `.calisto` — Calisto MT
- `.garamond` — Garamond
- `.baskerville` — Baskerville

---

## Layout

### Debugging

The `debug` module adds outlines to every child element, making it easy to see how elements are positioned and sized.

| Class | Effect |
|-------|--------|
| `.debug *` | `outline: 1px solid gold` on all children |
| `.debug-white *` | `outline: 1px solid white` on all children |
| `.debug-black *` | `outline: 1px solid black` on all children |

```css
/* DEBUG CHILDREN
   Docs: http://tachyons.io/docs/debug/ */

.debug *       { outline: 1px solid gold; }
.debug-white * { outline: 1px solid white; }
.debug-black * { outline: 1px solid black; }
```

**Usage:**

```html
<div class="debug">
  <!-- All children will show gold outlines -->
</div>
```

---

### Debug with a Grid

The debug grid module puts a background grid on any element to help align things vertically and horizontally.

The background grid comes in both **8px** and **16px** column sizes.

```css
/* DEBUG GRID
   Docs: http://tachyons.io/docs/debug-grid/

   Can be useful for debugging layout issues or making sure
   things line up perfectly. */

.debug-grid {
  background-image: url('data:image/png;...');
  background-size: 8px 8px;
}
.debug-grid-16 {
  background-image: url('data:image/png;...');
  background-size: 16px 16px;
}
```

> **Note:** The debug module is commented out at the bottom of `src/tachyons.css` by default. Uncomment it to add 1px outlines to every element on the page. Each element gets a different color outline to help distinguish closely positioned elements.

---

### Basic Grid

Tachyons grids are built by combining `display`, `float`, `padding`, and `width` utilities. There is no special "grid" module — just composition.

**Example: Simple 3-column grid**

```html
<div class="cf">
  <div class="fl w-third pa2">Column 1</div>
  <div class="fl w-third pa2">Column 2</div>
  <div class="fl w-third pa2">Column 3</div>
</div>
```

**Example: 2-column layout**

```html
<div class="cf">
  <div class="fl w-50 pa3">Left</div>
  <div class="fl w-50 pa3">Right</div>
</div>
```

You can combine display, float, padding, and widths to construct a wide variety of grids. See the [Widths](#widths), [Floats](#floats), and [Clearfix](#clearfix) sections for available classes.

---

### Flexbox

Flex can be used to achieve powerful horizontal or vertical layouts without JavaScript.

**Container classes:**

| Class | Description |
|-------|-------------|
| `.flex` | `display: flex` |
| `.inline-flex` | `display: inline-flex` |
| `.flex-auto` | `flex: 1 1 auto; min-width: 0; min-height: 0` |
| `.flex-none` | `flex: none` |

**Direction:**

| Class | Description |
|-------|-------------|
| `.flex-column` | `flex-direction: column` |
| `.flex-row` | `flex-direction: row` |
| `.flex-column-reverse` | `flex-direction: column-reverse` |
| `.flex-row-reverse` | `flex-direction: row-reverse` |

**Wrapping:**

| Class | Description |
|-------|-------------|
| `.flex-wrap` | `flex-wrap: wrap` |
| `.flex-nowrap` | `flex-wrap: nowrap` |
| `.flex-wrap-reverse` | `flex-wrap: wrap-reverse` |

**Align items (cross-axis):**

| Class | Description |
|-------|-------------|
| `.items-start` | `align-items: flex-start` |
| `.items-end` | `align-items: flex-end` |
| `.items-center` | `align-items: center` |
| `.items-baseline` | `align-items: baseline` |
| `.items-stretch` | `align-items: stretch` |

**Align self:**

| Class | Description |
|-------|-------------|
| `.self-start` | `align-self: flex-start` |
| `.self-end` | `align-self: flex-end` |
| `.self-center` | `align-self: center` |
| `.self-baseline` | `align-self: baseline` |
| `.self-stretch` | `align-self: stretch` |

**Justify content (main axis):**

| Class | Description |
|-------|-------------|
| `.justify-start` | `justify-content: flex-start` |
| `.justify-end` | `justify-content: flex-end` |
| `.justify-center` | `justify-content: center` |
| `.justify-between` | `justify-content: space-between` |
| `.justify-around` | `justify-content: space-around` |

**Align content (multi-line):**

| Class | Description |
|-------|-------------|
| `.content-start` | `align-content: flex-start` |
| `.content-end` | `align-content: flex-end` |
| `.content-center` | `align-content: center` |
| `.content-between` | `align-content: space-between` |
| `.content-around` | `align-content: space-around` |
| `.content-stretch` | `align-content: stretch` |

**Order:**

| Class | Value |
|-------|-------|
| `.order-0` – `.order-8` | `order: 0` – `order: 8` |
| `.order-last` | `order: 99999` |

All flexbox classes support responsive suffixes `-ns`, `-m`, `-l`.

**Example:**

```html
<!-- Horizontal centered layout -->
<div class="flex items-center justify-center">
  <div class="pa3">Item 1</div>
  <div class="pa3">Item 2</div>
  <div class="pa3">Item 3</div>
</div>

<!-- Column layout on mobile, row on larger screens -->
<div class="flex flex-column flex-row-ns">
  <div class="pa3 w-100 w-50-ns">Left</div>
  <div class="pa3 w-100 w-50-ns">Right</div>
</div>

<!-- Space between items -->
<div class="flex justify-between">
  <div>Logo</div>
  <nav>Navigation</nav>
</div>
```

---

### Box Sizing

The box model has been known to trip up developers. This module applies the `border-box` model to specific elements (and provides a class for manual application).

With `border-box`, the declared `width` includes padding and border — making sizing much more predictable.

```css
/*
   BOX SIZING
   Docs: http://tachyons.io/docs/layout/box-sizing/
*/

html,
body,
div,
article,
section,
main,
footer,
header,
form,
fieldset,
legend,
pre,
code,
a,
h1, h2, h3, h4, h5, h6,
p,
ul, ol, li,
dl, dt, dd,
textarea,
table,
td,
th,
select {
  box-sizing: border-box;
}

.border-box { box-sizing: border-box; }
```

| Class | Value |
|-------|-------|
| `.border-box` | `box-sizing: border-box` |

---

### Spacing

Tachyons uses a ratio-based spacing scale built on powers of two, starting at `0.25rem` (≈4px). This ensures consistent, mathematically sound spacing across the design.

**Spacing scale variables:**

| Step | Variable | Value |
|------|----------|-------|
| 0 | `--spacing-none` | `0` |
| 1 | `--spacing-extra-small` | `0.25rem` |
| 2 | `--spacing-small` | `0.5rem` |
| 3 | `--spacing-medium` | `1rem` |
| 4 | `--spacing-large` | `2rem` |
| 5 | `--spacing-extra-large` | `4rem` |
| 6 | `--spacing-extra-extra-large` | `8rem` |
| 7 | `--spacing-extra-extra-extra-large` | `16rem` |

**Class naming convention:**

```
[property][direction][step]

Property:
  p = padding
  m = margin

Direction:
  a = all
  h = horizontal (left + right)
  v = vertical (top + bottom)
  t = top
  r = right
  b = bottom
  l = left

Step: 0–7

Examples:
  pa3  = padding: 1rem (all sides)
  ph2  = padding: 0.5rem (horizontal)
  mv4  = margin: 2rem (vertical)
  ml1  = margin-left: 0.25rem
```

**Padding classes (all steps 0–7, all directions):**

`pa0`–`pa7`, `ph0`–`ph7`, `pv0`–`pv7`, `pt0`–`pt7`, `pr0`–`pr7`, `pb0`–`pb7`, `pl0`–`pl7`

**Margin classes (all steps 0–7, all directions):**

`ma0`–`ma7`, `mh0`–`mh7`, `mv0`–`mv7`, `mt0`–`mt7`, `mr0`–`mr7`, `mb0`–`mb7`, `ml0`–`ml7`

All spacing classes support responsive suffixes `-ns`, `-m`, `-l`.

**Special margin class:**

| Class | Description |
|-------|-------------|
| `.mw-100` | `max-width: 100%` |
| `.center` | `margin-left: auto; margin-right: auto` |

**Example:**

```html
<!-- Card with padding and margin -->
<div class="pa3 ma2 ba b--light-gray br2">
  Card content
</div>

<!-- Responsive padding -->
<section class="ph2 ph4-ns pv4">
  Content with larger horizontal padding on non-small screens
</section>
```

---

### Floats

> 1. Floated elements are automatically rendered as block level elements. Setting floats to `display: inline` fixes the double margin bug in IE6.
> 2. Don't forget to clearfix your floats with `.cf`

**Base class:** `f` (float)
**Modifiers:** `l` = left, `r` = right, `n` = none

| Class | Value |
|-------|-------|
| `.fl` | `float: left; _display: inline` |
| `.fr` | `float: right; _display: inline` |
| `.fn` | `float: none` |

All float classes support responsive suffixes `-ns`, `-m`, `-l`.

**Source CSS (`src/_floats.css`):**

```css
/*
   FLOATS
   http://tachyons.io/docs/layout/floats/

   Base:
     f = float

   Modifiers:
     l = left
     r = right
     n = none

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.fl { float: left;  _display: inline; }
.fr { float: right; _display: inline; }
.fn { float: none; }
/* Responsive variants follow the same pattern */
```

---

### Clearfix

The clearfix hack is used to clear floated children. Add the `.cf` class to a parent containing floated children.

| Class | Effect |
|-------|--------|
| `.cf` | Clearfix (clears floated children) |

**Source CSS (`src/_clearfix.css`):**

```css
/*
   CLEARFIX
   http://tachyons.io/docs/layout/clearfix/
*/

.cf:before,
.cf:after { content: " "; display: table; }
.cf:after { clear: both; }
.cf       { *zoom: 1; }
```

---

### Display

Single-purpose classes for setting the `display` property at any breakpoint.

| Class | Value | Notes |
|-------|-------|-------|
| `.dn` | `display: none` | |
| `.di` | `display: inline` | Does not respect width/height |
| `.db` | `display: block` | Sets width to 100% of parent |
| `.dib` | `display: inline-block` | Respects width/height; renders whitespace |
| `.dit` | `display: inline-table` | |
| `.dt` | `display: table` | Use with `.dtc` for table layout |
| `.dtc` | `display: table-cell` | Use inside `.dt` |
| `.dt--fixed` | `table-layout: fixed` | Makes table cells equal width |
| `.flex` | `display: flex` | *(See Flexbox section)* |
| `.inline-flex` | `display: inline-flex` | *(See Flexbox section)* |

**Source CSS (`src/_display.css`):**

```css
/*
   DISPLAY
   Docs: http://tachyons.io/docs/layout/display/

   Base:
     d = display

   Modifiers:
     n = none
     i = inline
     b = block
     ib = inline-block
     it = inline-table
     t = table
     tc = table-cell
     t-row = table-row
     t-columm = table-column
     t-column-group = table-column-group

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.dn        { display: none; }
.di        { display: inline; }
.db        { display: block; }
.dib       { display: inline-block; }
.dit       { display: inline-table; }
.dt        { display: table; }
.dtc       { display: table-cell; }
.dt-row    { display: table-row; }
.dt-row-group  { display: table-row-group; }
.dt-column { display: table-column; }
.dt-column-group { display: table-column-group; }
.dt--fixed { table-layout: fixed; width: 100%; }
/* Responsive variants follow the same pattern */
```

**Table display example:**

```html
<!-- Equal-width table cells without markup -->
<div class="dt dt--fixed w-100">
  <div class="dtc pa2">Column 1</div>
  <div class="dtc pa2">Column 2</div>
  <div class="dtc pa2">Column 3</div>
</div>

<!-- Vertically center content -->
<div class="dt vh-100 w-100">
  <div class="dtc v-mid tc">
    Vertically and horizontally centered
  </div>
</div>
```

---

### Widths

The widths module provides a five-step width scale based on powers of two, plus percentage values for building responsive grids.

**Percentage-based widths:**

| Class | Value |
|-------|-------|
| `.w-10` | `width: 10%` |
| `.w-20` | `width: 20%` |
| `.w-25` | `width: 25%` |
| `.w-30` | `width: 30%` |
| `.w-33` | `width: 33%` |
| `.w-34` | `width: 34%` |
| `.w-40` | `width: 40%` |
| `.w-50` | `width: 50%` |
| `.w-60` | `width: 60%` |
| `.w-70` | `width: 70%` |
| `.w-75` | `width: 75%` |
| `.w-80` | `width: 80%` |
| `.w-90` | `width: 90%` |
| `.w-100` | `width: 100%` |
| `.w-third` | `width: calc(100% / 3)` |
| `.w-two-thirds` | `width: calc(100% / 1.5)` |
| `.w-auto` | `width: auto` |

**Rem-based width scale:**

| Class | Value |
|-------|-------|
| `.w1` | `width: 1rem` |
| `.w2` | `width: 2rem` |
| `.w3` | `width: 4rem` |
| `.w4` | `width: 8rem` |
| `.w5` | `width: 16rem` |

All width classes support responsive suffixes `-ns`, `-m`, `-l`.

**Example:**

```html
<!-- Responsive grid: stack on mobile, side-by-side on medium+ -->
<div class="cf">
  <div class="fl w-100 w-50-m w-third-l pa2">Column</div>
  <div class="fl w-100 w-50-m w-third-l pa2">Column</div>
  <div class="fl w-100 w-100-m w-third-l pa2">Column</div>
</div>
```

---

### Max Widths

The max-widths module provides a ten-step scale based on powers of two, plus `100%` and `none`.

| Class | Value |
|-------|-------|
| `.mw1` | `max-width: 1rem` |
| `.mw2` | `max-width: 2rem` |
| `.mw3` | `max-width: 4rem` |
| `.mw4` | `max-width: 8rem` |
| `.mw5` | `max-width: 16rem` |
| `.mw6` | `max-width: 32rem` |
| `.mw7` | `max-width: 48rem` |
| `.mw8` | `max-width: 64rem` |
| `.mw9` | `max-width: 96rem` |
| `.mw-100` | `max-width: 100%` |
| `.mw-none` | `max-width: none` |

All max-width classes support responsive suffixes `-ns`, `-m`, `-l`.

**Example:**

```html
<!-- Constrain article width for readability -->
<article class="mw7 center pa4">
  <p class="measure lh-copy">Article content...</p>
</article>
```

---

### Heights

**Fixed height scale:**

| Class | Value |
|-------|-------|
| `.h1` | `height: 1rem` |
| `.h2` | `height: 2rem` |
| `.h3` | `height: 4rem` |
| `.h4` | `height: 8rem` |
| `.h5` | `height: 16rem` |

**Percentage heights:**

| Class | Value |
|-------|-------|
| `.h-25` | `height: 25%` |
| `.h-50` | `height: 50%` |
| `.h-75` | `height: 75%` |
| `.h-100` | `height: 100%` |
| `.h-auto` | `height: auto` |
| `.h-inherit` | `height: inherit` |

**Viewport heights:**

| Class | Value |
|-------|-------|
| `.vh-25` | `height: 25vh` |
| `.vh-50` | `height: 50vh` |
| `.vh-75` | `height: 75vh` |
| `.vh-100` | `height: 100vh` |
| `.min-vh-100` | `min-height: 100vh` |

All height classes support responsive suffixes `-ns`, `-m`, `-l`.

---

### Position

HTML elements are initially set to `position: static`. Tachyons provides classes for `relative` and `absolute` positioning.

- **Absolute elements** are positioned inside a relative parent.
- **Relative elements** allow offset positioning without affecting surrounding elements.

| Class | Value |
|-------|-------|
| `.static` | `position: static` |
| `.relative` | `position: relative` |
| `.absolute` | `position: absolute` |
| `.fixed` | `position: fixed` |

**Coordinate classes (used with absolute/relative):**

| Class | Value |
|-------|-------|
| `.top-0` | `top: 0` |
| `.right-0` | `right: 0` |
| `.bottom-0` | `bottom: 0` |
| `.left-0` | `left: 0` |
| `.top-1` | `top: 1rem` |
| `.right-1` | `right: 1rem` |
| `.bottom-1` | `bottom: 1rem` |
| `.left-1` | `left: 1rem` |
| `.top-2` | `top: 2rem` |
| `.right-2` | `right: 2rem` |
| `.bottom-2` | `bottom: 2rem` |
| `.left-2` | `left: 2rem` |
| `.top--1` | `top: -1rem` |
| `.right--1` | `right: -1rem` |
| `.bottom--1` | `bottom: -1rem` |
| `.left--1` | `left: -1rem` |
| `.top--2` | `top: -2rem` |
| `.right--2` | `right: -2rem` |
| `.bottom--2` | `bottom: -2rem` |
| `.left--2` | `left: -2rem` |

**Overflow:**

| Class | Value |
|-------|-------|
| `.overflow-visible` | `overflow: visible` |
| `.overflow-hidden` | `overflow: hidden` |
| `.overflow-scroll` | `overflow: scroll` |
| `.overflow-auto` | `overflow: auto` |
| `.overflow-x-scroll` | `overflow-x: scroll` |
| `.overflow-y-scroll` | `overflow-y: scroll` |
| `.overflow-x-auto` | `overflow-x: auto` |
| `.overflow-y-auto` | `overflow-y: auto` |

**Z-index:**

| Class | Value |
|-------|-------|
| `.z-0` | `z-index: 0` |
| `.z-1` | `z-index: 1` |
| `.z-2` | `z-index: 2` |
| `.z-3` | `z-index: 3` |
| `.z-4` | `z-index: 4` |
| `.z-5` | `z-index: 5` |
| `.z-999` | `z-index: 999` |
| `.z-9999` | `z-index: 9999` |
| `.z-max` | `z-index: 2147483647` |
| `.z-inherit` | `z-index: inherit` |
| `.z-initial` | `z-index: initial` |
| `.z-unset` | `z-index: unset` |

**Example:**

```html
<!-- Full-cover overlay -->
<div class="relative">
  <img src="photo.jpg" />
  <div class="absolute top-0 left-0 w-100 h-100 bg-black-50">
    <p class="white tc v-mid">Overlay text</p>
  </div>
</div>
```

---

## Theming

### Skins (Colors)

Tachyons comes with a collection of classes for setting text color and background color. All combinations listed are accessibility-friendly (WCAG AA compliant).

**Text color classes** — prefix `.{color-name}`:

```css
.black       { color: #000000; }
.near-black  { color: #111111; }
.dark-gray   { color: #333333; }
.mid-gray    { color: #555555; }
.gray        { color: #777777; }
.silver      { color: #999999; }
.light-silver { color: #AAAAAA; }
.moon-gray   { color: #CCCCCC; }
.light-gray  { color: #EEEEEE; }
.near-white  { color: #F4F4F4; }
.white       { color: #FFFFFF; }
.dark-red    { color: #E7040F; }
.red         { color: #FF4136; }
.light-red   { color: #FF725C; }
.orange      { color: #FF6300; }
.gold        { color: #FFB700; }
.yellow      { color: #FFD700; }
.light-yellow { color: #FBF1A9; }
.purple      { color: #5E2CA5; }
.light-purple { color: #A463F2; }
.dark-pink   { color: #D5008F; }
.hot-pink    { color: #FF41B4; }
.pink        { color: #FF80CC; }
.light-pink  { color: #FFA3D7; }
.dark-green  { color: #137752; }
.green       { color: #19A974; }
.light-green { color: #9EEBCF; }
.navy        { color: #001B44; }
.dark-blue   { color: #00449E; }
.blue        { color: #357EDD; }
.light-blue  { color: #96CCFF; }
.lightest-blue { color: #CDECFF; }
.washed-blue { color: #F6FFFE; }
.washed-green { color: #E8FDF5; }
.washed-yellow { color: #FFFCEB; }
.washed-red  { color: #FFDFDF; }
.transparent { color: transparent; }
.inherit     { color: inherit; }
```

**Background color classes** — prefix `.bg-{color-name}`:

All color names above are also available with the `bg-` prefix:

```css
.bg-black       { background-color: #000000; }
.bg-near-black  { background-color: #111111; }
.bg-dark-gray   { background-color: #333333; }
/* ...etc for all colors above... */
.bg-transparent { background-color: transparent; }
.bg-inherit     { background-color: inherit; }
```

**Color Palette:**

| Name | Hex |
|------|-----|
| black | `#000000` |
| near-black | `#111111` |
| dark-gray | `#333333` |
| mid-gray | `#555555` |
| gray | `#777777` |
| silver | `#999999` |
| light-silver | `#AAAAAA` |
| moon-gray | `#CCCCCC` |
| light-gray | `#EEEEEE` |
| near-white | `#F4F4F4` |
| white | `#FFFFFF` |
| dark-red | `#E7040F` |
| red | `#FF4136` |
| light-red | `#FF725C` |
| orange | `#FF6300` |
| gold | `#FFB700` |
| yellow | `#FFD700` |
| light-yellow | `#FBF1A9` |
| purple | `#5E2CA5` |
| light-purple | `#A463F2` |
| dark-pink | `#D5008F` |
| hot-pink | `#FF41B4` |
| pink | `#FF80CC` |
| light-pink | `#FFA3D7` |
| dark-green | `#137752` |
| green | `#19A974` |
| light-green | `#9EEBCF` |
| navy | `#001B44` |
| dark-blue | `#00449E` |
| blue | `#357EDD` |
| light-blue | `#96CCFF` |
| lightest-blue | `#CDECFF` |
| washed-blue | `#F6FFFE` |
| washed-green | `#E8FDF5` |
| washed-yellow | `#FFFCEB` |
| washed-red | `#FFDFDF` |

**Accessible Combinations (out of all possible pairings):**

- AA Large (contrast ≥ 3.0): **518** combinations
- AA (contrast ≥ 4.5): **318** combinations
- AAA (contrast ≥ 7.0): **194** combinations

**Example:**

```html
<div class="bg-navy white pa4">
  <h1 class="f2 white">Navy background, white text</h1>
</div>

<div class="bg-light-yellow dark-gray pa3">
  Light yellow background, dark gray text
</div>
```

---

### Skins Pseudo / Animations

Hover and focus state classes for text color and background color. Use `.bg-animate` with a `hover-bg-*` class to animate background color transitions.

**Background animate:**

```css
.bg-animate,
.bg-animate:hover,
.bg-animate:focus {
  transition: background-color .15s ease-in-out;
}
```

**Hover background color classes** — format: `.hover-bg-{color-name}`:

```css
.hover-bg-black:hover,  .hover-bg-black:focus  { background-color: #000000; }
.hover-bg-dark-red:hover, ...
.hover-bg-red:hover, ...
/* ...all color names available... */
```

**Hover text color classes** — format: `.hover-{color-name}`:

```css
.hover-black:hover,  .hover-black:focus  { color: #000000; }
.hover-white:hover,  .hover-white:focus  { color: #FFFFFF; }
/* ...all color names available... */
```

**Example:**

```html
<!-- Animated background on hover -->
<a href="#" class="link b black pv2 db bg-animate hover-bg-near-black hover-white">
  Hover me
</a>

<!-- Animated nav links -->
<nav>
  <a href="#" class="link dim white dib pa2 bg-animate hover-bg-dark-blue">Home</a>
  <a href="#" class="link dim white dib pa2 bg-animate hover-bg-dark-blue">About</a>
</nav>
```

---

### Hovers

Utility classes for common hover effects.

| Class | Effect |
|-------|--------|
| `.dim` | Fades to 50% opacity on hover |
| `.glow` | Fades to 100% opacity on hover (for partially transparent elements) |
| `.hide-child` | Hides nested `.child` elements, reveals on hover/focus |
| `.underline-hover` | Adds underline on hover/focus |
| `.grow` | Scales up to 1.05 on hover |
| `.grow-large` | Scales up to 1.2 on hover |
| `.pointer` | Shows pointer cursor on hover |
| `.shadow-hover` | Adds box shadow on hover |

**Source CSS (`src/_hovers.css`):**

```css
/*
   HOVER EFFECTS
   Docs: http://tachyons.io/docs/themes/hovers/
*/

/* Dim element on hover */
.dim { opacity: 1; transition: opacity .15s ease-in; }
.dim:hover, .dim:focus { opacity: .5; transition: opacity .15s ease-in; }
.dim:active { opacity: .8; transition: opacity .15s ease-out; }

/* Animate to full opacity on hover */
.glow { transition: opacity .15s ease-in; }
.glow:hover, .glow:focus { opacity: 1; transition: opacity .15s ease-in; }

/* Hide child & reveal on hover */
.hide-child .child { opacity: 0; transition: opacity .15s ease-in; }
.hide-child:hover .child,
.hide-child:focus .child,
.hide-child:active .child {
  opacity: 1;
  transition: opacity .15s ease-in;
}

/* Underline on hover */
.underline-hover:hover,
.underline-hover:focus { text-decoration: underline; }

/* Grow on hover — combine with overflow-hidden for background image grow effect */
.grow {
  -moz-osx-font-smoothing: grayscale;
  backface-visibility: hidden;
  transform: translateZ(0);
  transition: transform 0.25s ease-out;
}
.grow:hover, .grow:focus { transform: scale(1.05); }
.grow:active { transform: scale(.90); }

.grow-large {
  -moz-osx-font-smoothing: grayscale;
  backface-visibility: hidden;
  transform: translateZ(0);
  transition: transform .25s ease-in-out;
}
.grow-large:hover, .grow-large:focus { transform: scale(1.2); }
.grow-large:active { transform: scale(.95); }

/* Pointer cursor */
.pointer:hover { cursor: pointer; }

/* Box shadow on hover */
.shadow-hover { cursor: pointer; position: relative; }
.shadow-hover:after {
  content: '';
  position: absolute;
  top: 0; left: 0; bottom: 0; right: 0;
  box-shadow: 0px 0px 8px 2px rgba(0,0,0,.2);
  opacity: 0;
  transition: opacity .25s ease-in-out;
}
.shadow-hover:hover:after { opacity: 1; }
```

**Example:**

```html
<!-- Dim on hover -->
<a href="#" class="dim link black">Dim link</a>

<!-- Grow card on hover -->
<div class="grow mw5 ba b--black-10 pa3 br2">
  Card content
</div>

<!-- Reveal child on hover -->
<div class="hide-child relative">
  <img src="photo.jpg" class="db" />
  <div class="child absolute top-0 left-0 w-100 h-100 bg-black-50 flex items-center justify-center">
    <p class="white f4">View</p>
  </div>
</div>
```

---

### Background Size

| Class | Value |
|-------|-------|
| `.cover` | `background-size: cover` |
| `.contain` | `background-size: contain` |

**Source CSS (`src/_background-size.css`):**

```css
/*
   BACKGROUND SIZE
   Docs: http://tachyons.io/docs/themes/background-size/

   Media Query Extensions:
     -ns = not-small
     -m  = medium
     -l  = large
*/

.cover   { background-size: cover; }
.contain { background-size: contain; }
/* Responsive variants follow the same pattern */
```

**Background position:**

| Class | Value |
|-------|-------|
| `.bg-center` | `background-position: center center` |
| `.bg-top` | `background-position: top center` |
| `.bg-right` | `background-position: center right` |
| `.bg-bottom` | `background-position: bottom center` |
| `.bg-left` | `background-position: center left` |

**Example:**

```html
<div class="cover bg-center vh-100"
     style="background-image: url('hero.jpg')">
  Hero content
</div>
```

---

### Borders

**Border style:**

| Class | Value |
|-------|-------|
| `.ba` | `border-style: solid; border-width: 1px` |
| `.bt` | `border-top-style: solid; border-top-width: 1px` |
| `.br` | `border-right-style: solid; border-right-width: 1px` |
| `.bb` | `border-bottom-style: solid; border-bottom-width: 1px` |
| `.bl` | `border-left-style: solid; border-left-width: 1px` |
| `.bn` | `border-style: none; border-width: 0` |

**Border widths:**

| Class | Value |
|-------|-------|
| `.bw0` | `border-width: 0` |
| `.bw1` | `border-width: .125rem` |
| `.bw2` | `border-width: .25rem` |
| `.bw3` | `border-width: .5rem` |
| `.bw4` | `border-width: 1rem` |
| `.bw5` | `border-width: 2rem` |

**Border colors** — format: `.b--{color-name}`:

All color names from the palette are available as border color classes with the `b--` prefix:

```css
.b--black       { border-color: #000000; }
.b--near-black  { border-color: #111111; }
.b--dark-gray   { border-color: #333333; }
/* ...all palette colors available... */
.b--white       { border-color: #FFFFFF; }
.b--transparent { border-color: transparent; }
```

All border classes support responsive suffixes `-ns`, `-m`, `-l`.

**Example:**

```html
<!-- Card with border -->
<div class="ba b--light-gray br2 pa3">
  Card with border
</div>

<!-- Bottom border only -->
<nav class="bb b--black-10 pb2 mb4">
  Navigation
</nav>

<!-- Thick left accent border -->
<div class="bl bw3 b--blue pl3">
  Quoted text
</div>
```

---

### Border Radius

| Class | Value |
|-------|-------|
| `.br0` | `border-radius: 0` |
| `.br1` | `border-radius: .125rem` |
| `.br2` | `border-radius: .25rem` |
| `.br3` | `border-radius: .5rem` |
| `.br4` | `border-radius: 1rem` |
| `.br-100` | `border-radius: 100%` |
| `.br-pill` | `border-radius: 9999px` |
| `.br--bottom` | Rounds bottom corners only |
| `.br--top` | Rounds top corners only |
| `.br--right` | Rounds right corners only |
| `.br--left` | Rounds left corners only |

**Source CSS (`src/_border-radius.css`):**

```css
/*
   BORDER RADIUS
   Docs: http://tachyons.io/docs/themes/border-radius/
*/

.br0  { border-radius: 0; }
.br1  { border-radius: .125rem; }
.br2  { border-radius: .25rem; }
.br3  { border-radius: .5rem; }
.br4  { border-radius: 1rem; }
.br-100 { border-radius: 100%; }
.br-pill { border-radius: 9999px; }

.br--bottom {
  border-top-left-radius: 0;
  border-top-right-radius: 0;
}
.br--top {
  border-bottom-left-radius: 0;
  border-bottom-right-radius: 0;
}
.br--right {
  border-top-left-radius: 0;
  border-bottom-left-radius: 0;
}
.br--left {
  border-top-right-radius: 0;
  border-bottom-right-radius: 0;
}
```

**Example:**

```html
<!-- Avatar circle -->
<img class="br-100 h3 w3 dib" src="avatar.jpg" alt="Avatar" />

<!-- Pill button -->
<button class="br-pill ph4 pv2 bg-blue white bn">
  Click Me
</button>

<!-- Rounded card -->
<div class="br3 pa3 bg-near-white">Card content</div>
```

---

### Box Shadow

| Class | Value |
|-------|-------|
| `.shadow-1` | Small shadow |
| `.shadow-2` | Medium shadow |
| `.shadow-3` | Large shadow |
| `.shadow-4` | Very large shadow |
| `.shadow-5` | Largest shadow |

**Source CSS (`src/_box-shadow.css`):**

```css
/*
   BOX SHADOW
   Docs: http://tachyons.io/docs/themes/box-shadow/
*/

.shadow-1 { box-shadow: 0px 0px 4px 2px rgba(0, 0, 0, .2); }
.shadow-2 { box-shadow: 0px 0px 8px 2px rgba(0, 0, 0, .2); }
.shadow-3 { box-shadow: 2px 2px 4px 2px rgba(0, 0, 0, .2); }
.shadow-4 { box-shadow: 2px 2px 8px 0px rgba(0, 0, 0, .2); }
.shadow-5 { box-shadow: 4px 4px 8px 0px rgba(0, 0, 0, .2); }
```

---

### Opacity

| Class | Value |
|-------|-------|
| `.o-100` | `opacity: 1` |
| `.o-90` | `opacity: .9` |
| `.o-80` | `opacity: .8` |
| `.o-70` | `opacity: .7` |
| `.o-60` | `opacity: .6` |
| `.o-50` | `opacity: .5` |
| `.o-40` | `opacity: .4` |
| `.o-30` | `opacity: .3` |
| `.o-20` | `opacity: .2` |
| `.o-10` | `opacity: .1` |
| `.o-05` | `opacity: .05` |
| `.o-025` | `opacity: .025` |
| `.o-0` | `opacity: 0` |

**Transparent color utilities** (black/white with opacity via CSS variables):

```css
/* Black with varying opacity */
.black-90 { color: rgba(0,0,0,.9); }
.black-80 { color: rgba(0,0,0,.8); }
.black-70 { color: rgba(0,0,0,.7); }
.black-60 { color: rgba(0,0,0,.6); }
.black-50 { color: rgba(0,0,0,.5); }
.black-40 { color: rgba(0,0,0,.4); }
.black-30 { color: rgba(0,0,0,.3); }
.black-20 { color: rgba(0,0,0,.2); }
.black-10 { color: rgba(0,0,0,.1); }
.black-05 { color: rgba(0,0,0,.05); }

/* White with varying opacity */
.white-90 { color: rgba(255,255,255,.9); }
.white-80 { color: rgba(255,255,255,.8); }
/* ...etc... */

/* Background versions */
.bg-black-90 { background-color: rgba(0,0,0,.9); }
.bg-black-80 { background-color: rgba(0,0,0,.8); }
/* ...etc... */
```

---

## Elements

### Images

```css
/*
   IMAGES
   Docs: http://tachyons.io/docs/elements/images/
*/

/* Remove the space between inline block elements */
img { vertical-align: middle; }

/* Make all images fully responsive by default */
.img-responsive { max-width: 100%; height: auto; }
```

**Example:**

```html
<!-- Responsive image -->
<img class="db mw-100" src="photo.jpg" alt="Description" />

<!-- Circular image -->
<img class="br-100 h4 w4 dib cover" src="avatar.jpg" alt="Avatar" />

<!-- Image with shadow -->
<img class="db shadow-2 br2" src="product.jpg" alt="Product" />
```

---

### Links

```css
/*
   LINKS
   Docs: http://tachyons.io/docs/elements/links/
*/

.link {
  text-decoration: none;
  transition: color .15s ease-in;
}

.link:link, .link:visited {
  transition: color .15s ease-in;
}

.link:hover {
  transition: color .15s ease-in;
}

.link:active {
  transition: color .15s ease-in;
}

.link:focus {
  transition: color .15s ease-in;
  outline: 1px dotted currentColor;
}
```

The `.link` class removes text decoration and adds smooth color transitions. Combine with skin and hover classes to style links:

```html
<!-- Styled link -->
<a href="#" class="link dim black b">Dimming link</a>

<!-- Colored link -->
<a href="#" class="link blue hover-dark-blue">Blue link</a>

<!-- No-underline link -->
<a href="#" class="link no-underline underline-hover black">
  Underline on hover only
</a>
```

---

### Lists

```css
/*
   LISTS
   Docs: http://tachyons.io/docs/elements/lists/
*/

.list { list-style-type: none; }
```

| Class | Value |
|-------|-------|
| `.list` | `list-style-type: none; padding: 0; margin: 0` |

**Example:**

```html
<!-- Unstyled list -->
<ul class="list pl0 ma0">
  <li class="pv2 bb b--light-gray">Item 1</li>
  <li class="pv2 bb b--light-gray">Item 2</li>
  <li class="pv2">Item 3</li>
</ul>

<!-- Horizontal nav list -->
<ul class="list pl0 ma0 flex">
  <li class="mr3"><a href="#" class="link black dim">Home</a></li>
  <li class="mr3"><a href="#" class="link black dim">About</a></li>
  <li><a href="#" class="link black dim">Contact</a></li>
</ul>
```

---

### Forms

Tachyons doesn't provide heavy form-specific styles, but the general utilities work well for styling form elements. Remove default browser styles and apply tachyons classes:

```css
/*
   FORMS
   Docs: http://tachyons.io/docs/elements/forms/
*/

.input-reset { -webkit-appearance: none; -moz-appearance: none; }

/* Applies to <button>, <select>, etc. */
.button-reset {
  -webkit-appearance: none;
  -moz-appearance: none;
  cursor: pointer;
}
```

**Example:**

```html
<!-- Simple text input -->
<input
  class="input-reset ba b--black-20 pa2 w-100 br2 f5"
  type="text"
  placeholder="Your email"
/>

<!-- Select dropdown -->
<select class="input-reset ba b--black-20 pa2 br2 f5">
  <option>Option 1</option>
  <option>Option 2</option>
</select>

<!-- Submit button -->
<button class="button-reset bn br2 bg-blue white pa3 f5 pointer w-100">
  Submit
</button>

<!-- Full form example -->
<form class="pa4 black-80 mw6">
  <fieldset class="bn pa0">
    <legend class="f4 fw6 mb3">Sign Up</legend>

    <div class="mb3">
      <label class="db fw6 lh-copy f6 mb1" for="name">Full Name</label>
      <input class="input-reset ba b--black-20 pa2 w-100 br2" type="text" id="name" />
    </div>

    <div class="mb3">
      <label class="db fw6 lh-copy f6 mb1" for="email">Email</label>
      <input class="input-reset ba b--black-20 pa2 w-100 br2" type="email" id="email" />
    </div>

    <button class="button-reset bn bg-blue white br2 pa3 w-100 f5 pointer">
      Create Account
    </button>
  </fieldset>
</form>
```

---

### Tables

```css
/*
   TABLES
   Docs: http://tachyons.io/docs/elements/tables/
*/

.collapse {
  border-collapse: collapse;
  border-spacing: 0;
}

/* Striped rows using nth-child */
.striped--light-silver:nth-child(odd) { background-color: var(--light-silver); }
.striped--moon-gray:nth-child(odd)    { background-color: var(--moon-gray); }
.striped--light-gray:nth-child(odd)   { background-color: var(--light-gray); }
.striped--near-white:nth-child(odd)   { background-color: var(--near-white); }
.stripe-light:nth-child(odd)          { background-color: var(--white-10); }
.stripe-dark:nth-child(odd)           { background-color: var(--black-10); }
```

**Example:**

```html
<!-- Basic styled table -->
<table class="collapse w-100 f6 ba b--black-10">
  <thead>
    <tr class="bg-near-white">
      <th class="pa3 tl fw6 bb b--black-20">Name</th>
      <th class="pa3 tl fw6 bb b--black-20">Role</th>
      <th class="pa3 tl fw6 bb b--black-20">Location</th>
    </tr>
  </thead>
  <tbody>
    <tr class="stripe-dark">
      <td class="pa3">Alice</td>
      <td class="pa3">Engineer</td>
      <td class="pa3">San Francisco</td>
    </tr>
    <tr class="stripe-dark">
      <td class="pa3">Bob</td>
      <td class="pa3">Designer</td>
      <td class="pa3">New York</td>
    </tr>
  </tbody>
</table>
```

---

## Responsive Design Cheat Sheet

Every utility in Tachyons can be applied at specific breakpoints by appending a suffix:

| Suffix | Media Query |
|--------|------------|
| *(none)* | All sizes (mobile-first default) |
| `-ns` | `@media screen and (min-width: 30em)` — not small (≥480px) |
| `-m` | `@media screen and (min-width: 30em) and (max-width: 60em)` — medium only |
| `-l` | `@media screen and (min-width: 60em)` — large (≥960px) |

**Example — responsive typography:**

```html
<h1 class="f4 f3-m f1-l lh-title">
  Small on mobile, medium on tablets, large on desktop
</h1>
```

**Example — responsive layout:**

```html
<div class="flex flex-column flex-row-ns">
  <aside class="w-100 w-25-ns pa3 bg-near-white">Sidebar</aside>
  <main class="w-100 w-75-ns pa3">Main content</main>
</div>
```

**Example — responsive spacing:**

```html
<section class="pa2 pa4-ns pa5-l">
  Content with increasing padding at larger breakpoints
</section>
```

---

## Common Patterns

### Card Component

```html
<article class="br2 ba b--black-10 shadow-1 mw5 center">
  <img class="db br2 br--top w-100" src="image.jpg" alt="Card image" />
  <div class="pa3">
    <h2 class="f5 fw6 mt0 mb2">Card Title</h2>
    <p class="f6 lh-copy measure gray mt0">
      Card description text goes here.
    </p>
    <a href="#" class="link dim br2 ph3 pv2 dib white bg-dark-blue f6 fw6">
      Read More
    </a>
  </div>
</article>
```

### Navigation Bar

```html
<nav class="flex items-center justify-between pa3 bg-dark-blue white">
  <a href="/" class="link white fw7 f4 no-underline">Brand</a>
  <ul class="list flex pl0 ma0 f6">
    <li class="ml4"><a href="#" class="link white dim">Home</a></li>
    <li class="ml4"><a href="#" class="link white dim">About</a></li>
    <li class="ml4"><a href="#" class="link white dim">Contact</a></li>
  </ul>
</nav>
```

### Hero Section

```html
<header class="cover bg-center vh-100 flex items-center justify-center"
        style="background-image: url('hero.jpg')">
  <div class="tc white pa4">
    <h1 class="f-headline fw9 ttu tracked mb2">Hero Title</h1>
    <p class="f3 fw3 lh-copy measure center">
      A compelling hero subtitle that describes your product.
    </p>
    <a href="#" class="link br-pill ph4 pv3 dib white bg-blue f4 fw6 mt4">
      Get Started
    </a>
  </div>
</header>
```

### Media Object

```html
<div class="flex items-start pa3">
  <img class="br-100 h3 w3 mr3 flex-none" src="avatar.jpg" alt="Avatar" />
  <div class="flex-auto">
    <h3 class="f5 fw6 mt0 mb1">Person Name</h3>
    <p class="f6 gray lh-copy mt0">
      Content text describing the media object item.
    </p>
  </div>
</div>
```

---

## Installation

```html
<!-- Via CDN (unpkg) -->
<link rel="stylesheet" href="https://unpkg.com/tachyons@4.12.0/css/tachyons.min.css" />
```

```bash
# Via npm
npm install tachyons

# Via yarn
yarn add tachyons
```

**GitHub:** https://github.com/tachyons-css/tachyons/
**Version:** 4.12.0

---

*Documentation compiled from https://tachyons.io/docs/ — Tachyons v4.12.0*
