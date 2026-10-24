# Web UI

Twig provides a web interface for browsing namespaces and repositories,
managing your account, and (when enabled) using admin tools.

## Authentication

The UI uses an encrypted, server-side session cookie for authentication. After
logging in, the browser sends the cookie with subsequent requests. Sessions
last 30 days; configure `SESSION_KEY` to keep them valid across restarts.

| Action | Access |
|--------|--------|
| Browse public namespaces/repositories | Public |
| View a private repository | Logged in |
| Create namespace | Logged in; the new namespace belongs to you |
| Create repository | Logged in as namespace owner; account must have an email |
| Generate signup invite | Enter `API_KEY` on the invite page |
| Sign up | Unused one-time invite; username, email, and password |
| Inspect database data | Logged-in user named by `ADMIN_USER` |
| Use endpoint test suite | `TEST_USER` account or a valid session PIN |

### Account flow

1. Generate an invite at `/auth/invite` using the server's `API_KEY`.
2. Create an account at `/auth/signup` with the invite, username, email, and
   password.
3. Log in at `/auth/login`.
4. Create a namespace at `/auth/namespace`.
5. Set or update an email in Settings before creating repositories in the UI.

## Pages

### Home (`/`)

Lists public namespaces and their owners.

- Search namespaces by name; the same query also matches repository names
  across every namespace, listed under a Repository section with their
  namespace named. Results update as you type. Private repositories stay hidden
  from anonymous visitors.
- Signed-in users can create namespaces.
- Signed-in users pin namespaces from the namespace settings page
  (`/manage/namespaces/{namespace}/settings`), not from the home list. Pinned
  namespaces are listed first under a Pinned heading; the others stay hidden
  behind a "Show N more" toggle until expanded or a search is active. Pins are
  personal to each account and unpinning returns a namespace to the full list.
- Select a namespace to view its repositories.

### Namespace (`/{namespace}`)

Lists public repositories in a namespace. Signed-in users can also see private
repositories; only the namespace owner can create repositories.

- Search repositories by name.
- Repository rows show the last commit time.
- The owner can create a repository with a selected default branch.

### Repository (`/{namespace}/{repo}`)

The repository page offers tabs according to available content and `.twig.toml`
configuration:

- **Documentation** — rendered Markdown, including README files and links
  between repository Markdown files.
- **Paper** — read every Markdown page under `[paper].dir` as one continuous
  document. Pages load as they scroll into view, and the pinned toolbar offers
  the same A−/A+ zoom as presentations plus a Sans/Serif/Mono reading font.
  Every page and heading carries an anchor (`#paper-01.md`,
  `#paper-01.md--introduction`) so any position is directly linkable.
- **Content** — browse files and folders; open file content. Markdown files also
  offer compact **Raw** and **Copy source** actions here.
- **Commits** — recent commit history with short hash, author, date, and message.
- **Config** — view `.twig.toml` when present.
- **Present** — navigate configured Markdown slides when `[present].files` is
  set. The toolbar's Download PDF button opens `/{namespace}/{repo}/present/print`,
  a chrome-free view that lays every slide out on its own A4 landscape page
  with the license as the final page, then opens the print dialog automatically; choose
  "Save as PDF" there to download the deck.
- **Scripts** — the configured script groups when `[scripts]` is set. Each
  group becomes a sub-tab listing its scripts with a copyable
  `curl … | bash` command; the files are served from `/{namespace}/{repo}/raw/…`
  so the command runs as printed.
- **License** — display the repository license (`LICENSE.md`, `LICENSE`, or the
  `license` field of `Cargo.toml`), or a fallback non-commercial license when
  none is present. The fallback covers the author's own content — code,
  documentation, papers, slides, data — and excludes third-party material.

The config can choose which tabs to display and which repository files to omit
from the browser. Repositories with `private = true` require a logged-in user
for the web UI. The full `.twig.toml` reference — ignore patterns, tab
selection, privacy, presentations, paper, and script groups — lives in the
Repository Config docs.

When `.twig.toml` cannot be parsed, the repository page shows a compiler-style
diagnostic with the offending line instead of silently ignoring the file; the
rest of the UI keeps working with default settings.

### Settings (`/settings`)

Available to signed-in users:

- Update the account email.
- Access tokens (`/settings/tokens`): create a token on the "Create token" tab,
  review tokens in a table on the "Existing tokens" tab, revoke from that table,
  and open a token's details page (`/settings/tokens/{id}`) for its namespace
  access.

### Manage repositories and namespaces

- **Repository** (`/manage/repositories`) — search the repositories you manage,
  rename them, and delete those explicitly marked `deletable = true` in
  `.twig.toml`.
- **Namespace** (`/manage/namespaces`) — search the namespaces you manage. Each
  row opens its settings page.
- **Namespace settings** (`/manage/namespaces/{namespace}/settings`) — rename the
  namespace, move its repositories to other namespaces you manage repositories
  in, and delete the namespace once it holds no repositories.

### Manage and admin tools

The Manage area links to account settings and namespace management. Optional
tools may also be available:

#### Database data (`/manage/data`)

Enabled only when `ADMIN_USER` names a Twig account. It shows read-only database
tables and their rows in pages of 50.

#### Endpoint test suite (`/_test`)

Enabled only when `TEST_USER` is configured. The configured user can run the
endpoint checks and create/remove a six-digit session PIN so other users can
join. Set `TEST_USER=true` to use the username `admin`.

### Docs (`/_info`)

Contains the About and Documentation tabs. The Documentation tab embeds the
Markdown pages from the repository's `docs/` directory into the running
application. Every page heads with a **Copy page** button that puts that
page's Markdown source on the clipboard.

## Authentication pages

- **Invite** (`/auth/invite`) — generate a one-time signup invite using `API_KEY`.
- **Signup** (`/auth/signup`) — create an account; usernames need at least 3
  characters and passwords at least 8.
- **Login** (`/auth/login`) — sign in with username and password. Users with no
  namespaces are shown the namespace creation form after login.
- **Create namespace** (`/auth/namespace`) — create a namespace owned by the
  signed-in user.
- **Logout** (`POST /auth/logout`) — end the browser session.
