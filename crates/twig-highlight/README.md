# twig-highlight

`twig-highlight` is Twig's dependency-free, server-side syntax highlighter. It
scans each source string once and writes escaped HTML directly into one output
buffer. Unknown languages return `None`, so callers can preserve a plain-text
fallback.

## Add a language

1. Add a grammar module under `src/` and implement `Grammar`.
2. Use `HtmlWriter::text` for unclassified text and `HtmlWriter::token` for
   recognized tokens. Both escape HTML-sensitive input.
3. Register the grammar in `highlight` and map its file extension in
   `highlight_path`.
4. Add token roles to `TokenKind` only if the existing semantic roles do not
   fit; define their dark and light colors in `assets/twig.css`.

The Rust grammar is in `src/rust.rs` and is intentionally a small, linear
scanner rather than a regex pipeline. It handles comments (including nested
block comments), attributes, strings, raw strings, lifetimes, common Rust
keywords/types, function names, macros, and numbers. It is a lightweight viewer
grammar, not a Rust parser.

## Embedded languages

Grammars can delegate a source region to another grammar using
`highlight(language, source)` or `highlight_with(grammar, source)`. The app's
Markdown event adapter uses this for fenced `rust`/`rs` blocks, without parsing
Markdown again or affecting other fences. That adapter is in `src/md/mod.rs`.
