# Tickets

Fig has a built-in issue tracker. Every namespace gets one reserved git
repository named `ticket`, served at `/{namespace}/ticket`, holding every ticket
in that namespace as a readable TOML file.

Tickets do not belong to a repository. They can be *linked* to any number of
repositories and branches.

There are two ways to work with them, and they are equals: the web UI at
`/{namespace}/tickets`, and an ordinary `git clone` of the ticket repository.

## Working from the web UI

`/{namespace}/tickets` lists tickets with status filters. From there you can
create tickets, comment, change status, apply labels, and link repositories.

Bodies and comments are markdown. `@username` is highlighted when it resolves to
a real account. Raw HTML in ticket text is escaped, never rendered.

## Working from the command line

```sh
git clone http://your-fig-host/acme/ticket
cd ticket
git config remote.origin.push HEAD:refs/for/main
```

That third line is required, once per clone. After it, `git push` and `git pull`
behave completely normally.

### Editing a ticket

Edit the file and push:

```sh
$EDITOR tickets/42.toml
git commit -am "close #42"
git push
```

### Creating a ticket

Add a file named `tickets/new-<anything>.toml` with at least a title:

```toml
title = "Login fails on Safari"
status = "open"

[body]
markdown = "Clicking login does nothing."
```

Push it. The server assigns the number, uuid, author and timestamps, then
renames the file to `tickets/<number>.toml`. Pull afterwards to see it.

### Deleting a ticket

```sh
git rm tickets/42.toml && git commit -m "drop 42" && git push
```

## The ticket file

```toml
uuid       = "0195e0c9-..."
number     = 42
namespace  = "acme"
title      = "Login fails on Safari"
status     = "open"           # open | in_progress | blocked | closed
author     = "alice"
created_at = "2026-08-31T10:00:00Z"
updated_at = "2026-08-31T12:30:00Z"
labels     = ["bug", "auth"]
assignees  = ["bob"]

[body]
markdown = """
Clicking login does nothing on Safari 18.
![screenshot](/acme/ticket/attachment/ab12cd34...)
"""

[[link]]
repo   = "fig"
branch = "main"

[[comment]]
id         = "0195e0d1-..."
author     = "bob"
created_at = "2026-08-31T11:00:00Z"
markdown   = "Reproduced. cc @alice"
```

`uuid`, `number`, `namespace`, `author` and `created_at` are owned by the server.
Editing them in the file has no effect — the server restores them on ingest.

## Why you never see a merge conflict

Concurrent edits are normal here: the UI and any number of clones all write to
the same repository. You will never be asked to resolve a conflict, and a push
is never rejected because someone else got there first.

That works as follows.

**Clients push to `refs/for/main`, not to `main`.** From git's point of view
that ref does not exist yet, so a push to it is always a *ref creation* and never
trips the client-side fast-forward check. This is the reason for the one-time
config line: git refuses a stale push locally, before the server is ever
contacted, so no amount of server-side cleverness can rescue a plain
`git push origin main`.

**The server merges field by field.** A `proc-receive` hook reconciles the
submission against canonical state using knowledge of the ticket schema, rather
than by diffing text:

| Field | Resolution |
|---|---|
| `uuid`, `number`, `namespace`, `author`, `created_at` | Server-owned; client values discarded |
| `title`, `status`, `body` | An incoming change wins; otherwise canonical stands |
| `labels`, `assignees`, `link` | Set union of additions, minus removals |
| `comment` | Union by `id`; an edit is last-write-wins; a missing one is a deletion |

Whole-file cases:

| Case | Rule |
|---|---|
| New file in the push | Ticket is created and numbered |
| Deleted, nothing changed canonically | Deletion succeeds |
| Deleted, but modified concurrently | **Ticket is retained** with the modification |
| Deleted on both sides | No-op |

**Your commit becomes a parent of the result.** The canonical commit lists the
commit you pushed as one of its parents, so your next `git pull` is always a
fast-forward. There is nothing to merge locally, which is why no conflict marker
can appear.

## What is still rejected

Refusing invalid input is not the same as exposing merge complexity. A push is
rejected, with a message, when it:

- is malformed TOML;
- writes outside `tickets/`, or adds a symlink or submodule;
- contains a file over 512 KB, or more than 1000 commits;
- has history unrelated to the repository;
- comes from a user without access to the namespace.

Canonical state is left untouched in every one of those cases.

## Who can change what

Reads are public. Writes require membership of the namespace, on every path:

| Path | Requirement |
|---|---|
| Reading tickets (UI or `git clone`) | None — public, like repository content |
| UI: create, comment, status, labels, links | Session cookie **and** namespace access |
| UI: attachment upload | Session cookie **and** namespace access |
| `git push` | HTTP Basic Auth **and** namespace access |

An authenticated user who is not a member of the namespace is refused with
`403`, in both directions — owning one namespace grants nothing in another.
Anonymous requests, forged cookies and cookies from a logged-out session are
refused with `401`. `tests/scripts/ticket_authorization.sh` asserts this matrix,
with positive controls so a broken endpoint cannot masquerade as a secure one.

> Ticket content is world-readable to anyone who can reach the server, exactly
> like repository content. If a namespace holds sensitive tickets, it needs
> network-level restriction; Fig has no private-read mode.

## Identity

The author of a ticket or a comment is always the authenticated user who pushed
it. Git commit authorship is self-attested, so writing `author = "someone-else"`
in a file achieves nothing: the server overwrites it, along with all timestamps.

The pusher's identity reaches the ingest hook through `REMOTE_USER`, which Fig
sets from HTTP Basic Auth. It is never derived from a request header, and it is
set explicitly to empty when nobody is authenticated so a value inherited from
the server's own environment cannot pass for a user. An ingest with no
authenticated actor is refused outright.

## Attachments

Images are uploaded through the UI and stored **outside** git, content-addressed
by SHA-256 under `ATTACHMENT_ROOT`. Keeping them out of the repository means a
clone stays small no matter how many screenshots a ticket accumulates.

- Accepted: PNG, JPEG, GIF, WebP, up to 5 MB.
- The format is decided by sniffing magic bytes, not the filename, so an
  executable renamed `photo.png` is refused.
- SVG is deliberately not accepted: it can carry script and would be served from
  Fig's own origin.
- Identical uploads collapse to one file and one URL.

Embed one with ordinary markdown:

```markdown
![screenshot](/acme/ticket/attachment/<sha256>)
```

## Reserved names

`ticket` and `tickets` cannot be used as repository names in any namespace: the
first is this repository, the second is the UI path that would shadow it.

## Known limitations

- The one-time `git config` line is unavoidable. Without it, a stale push is
  refused by git on your own machine, with a message naming the fix.
- Competing edits to the *same field* resolve by server arrival order. The
  losing edit is discarded rather than merged.
- Deletion loses to a concurrent modification, so deleting a busy ticket may
  take two attempts.
- Deleting a ticket removes it from the current tree only. Git history, existing
  clones and backups still contain it, so this is not a way to erase data.
- Git history retains every version of every ticket, so repository size grows
  monotonically. Attachments living outside git is the main mitigation.
