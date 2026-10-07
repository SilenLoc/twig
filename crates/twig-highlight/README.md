# twig-highlight

`twig-highlight` is Twig's server-side syntax highlighter. It scans source and
writes escaped HTML directly into one output buffer. Unknown languages return
`None`, so callers can preserve a plain-text fallback. Built-in TOML grammars
currently cover Rust, JavaScript, and shell scripts.

## Add a language

Add a `.toml` file under `grammars/`. The crate's build script includes every
TOML file in that directory, so no Rust registry or module change is needed.
For example, a small Python grammar could start like this:

```toml
name = "python"
aliases = ["py"]
extensions = ["py"]

[[rules]]
kind = "comment"
pattern = '#[^\n]*'

[[rules]]
kind = "keyword"
pattern = '\b(?:def|class|if|else|elif|for|while|return|import|from|as|pass|True|False|None)\b'
```

Rules are tried top-to-bottom at each source position. `pattern` uses Rust's
[`regex` syntax](https://docs.rs/regex/latest/regex/#syntax), anchored at the
current position. `kind` can be `keyword`, `type`, `function`, `macro`,
`string`, `lifetime`, `number`, `comment`, or `attribute`; use `plain` to
consume a match without highlighting it. A `capture = N` rule highlights only
capture group `N` and leaves the rest of that match as plain text, which is
useful for recognizing function names before `(` or macro names before `!`.

For constructs that regular expressions cannot conveniently match, rules may
use `matcher = "delimited"` with `start` and `end`; set `nested = true` for
nested delimiters. `matcher = "hash_raw_string"` recognizes raw strings with a
configurable `prefixes` array. See `grammars/rust.toml` for a complete example.

Both `HtmlWriter::text` and `HtmlWriter::token` escape HTML-sensitive input.
The semantic token roles map to the `twig-syn-*` CSS variables in
`assets/twig.css`. The built-in Rust grammar is intentionally lightweight; it
is not a Rust parser.

## Embedded languages

Call `highlight(language, source)` or `highlight_with(grammar, source)` when a
source region should be highlighted separately. The app's Markdown event
adapter uses this for fenced `rust`/`rs` blocks, without parsing Markdown again
or affecting other fences; that adapter is in `src/md/mod.rs`.
