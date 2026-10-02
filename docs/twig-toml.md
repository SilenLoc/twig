# Repository Config (`.twig.toml`)

Every repository can carry a `.twig.toml` file at its root that configures how
Twig presents it. Twig reads the file from the repository's current commit, so
changing it is an ordinary commit — no server-side settings involved.

New repositories created through the web UI start with a commented template
listing every option at its default, and editors can validate the file against
the JSON Schema served at `/assets/twig.schema.json` via a `#:schema` comment.

When the file cannot be parsed, the repository page shows a compiler-style
diagnostic naming the offending line; every other setting falls back to its
default so the rest of the UI keeps working.

Twig also reads `.twig`, `.fig.toml` and `fig.toml` as fallbacks for
repositories created before the rename. A present `.twig.toml` always wins.

## Top-level options

```toml
# Files or folders to hide from the file browser and Markdown listings.
# A trailing slash matches a folder; a bare name matches any path component.
ignore_for_view = ["secrets/", "notes.txt", "scratch"]

# Which tabs to display. When empty or absent, every available tab is shown.
tabs = ["markdown", "content", "commits"]

# Whether the repository can be deleted from the Settings page.
deleteable = false

# Whether reads (web UI, raw files, Git fetch) require authentication.
private = false
```

`private = true` affects more than the web UI: Git clones need credentials and
the Scripts tab's raw file route rejects anonymous requests. See the Git
Backend docs for the credential matrix.

## `[present]` — slide presentations

Lists the Markdown files rendered as a presentation deck:

```toml
[present]
files = ["slides/01-intro.md", "slides/02-demo.md"]
# Any additional string key becomes a {{mustache}} template variable.
author = "Jane Doe"
```

## `[paper]` — long-form reading

Renders every Markdown page under one directory as a single, lazily loaded
document:

```toml
[paper]
dir = "paper"   # default
```

## `[scripts]` — runnable script groups

The Scripts tab lists repository scripts a reader can run with one copied
command. The section is optional: without it (or without a group holding at
least one script) the tab does not appear.

Each key inside `[scripts]` names a **group**; nesting group tables is what
builds the hierarchy. Every group shows up as a sub-tab labelled with its
name, and nested groups are reachable at
`/{namespace}/{repo}/scripts/<group>/<subgroup>`:

```toml
[scripts.linux]
name = "Linux"
scripts = [
  { name = "Install", path = "scripts/linux/install.sh" },
]

# Nested inside `linux`: its own sub-tab, at /scripts/linux/maintenance.
[scripts.linux.maintenance]
scripts = [
  { name = "Cleanup", path = "scripts/cleanup.sh" },
  { path = "scripts/prune.sh", shell = "sh" },
]
```

Each script entry takes:

| Key | Required | Meaning |
|-----|----------|---------|
| `path` | yes | Repository-relative path of the script file |
| `name` | no | Display name; falls back to the file's base name |
| `shell` | no | Interpreter the run command pipes into; defaults to `bash` |

A group's `name` is optional too and falls back to the group's key. Groups
with neither scripts nor usable children are pruned from the tab bar, and an
entry whose `path` is missing from the repository HEAD is marked `MISSING`
instead of silently skipped.

Each entry in the tab shows a copyable download-and-run command. The file is
served raw from `/{namespace}/{repo}/raw/<path>`, so the command runs exactly
as printed:

```sh
curl -fsSL 'https://your-twig-server/namespace/repo/raw/scripts/linux/install.sh' | bash
```

Notes:

- Paths matched by `ignore_for_view` are refused by the raw route, so
  hiding a file also takes it out of script reach.
- On private repositories the raw route needs authentication; add credentials
  to the command, e.g. `curl -u user:password -fsSL …`.
