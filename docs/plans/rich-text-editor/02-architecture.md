# Architecture: Rich text editor for repository files

## Fit

- `http::repository::editor` owns editor page rendering and the authenticated edit/commit handlers. It follows the existing repository path conventions and is registered alongside the content routes.
- `git::bare::RepoHandle` owns reading strict UTF-8 file data, replacing a file, or adding a new file, with compare-and-swap branch updates.
- Existing `TwigContext` session lookup and namespace membership checks authorize writes. User profile email is the commit author email; users without an email are sent to account settings.
- Quill, its Snow theme, and a Markdown serializer are served from versioned local assets. No CDN or database additions are needed.

## Endpoints

- `GET /{namespace}/{repo}/edit/{path:.*}` — show the editor for an existing, visible, UTF-8 text file.
- `POST /{namespace}/{repo}/edit/{path:.*}` — validate submitted text/message/base commit and create a Git commit; an explicit `save_conflict_copy` flag creates a new file after a stale-revision conflict.

Both routes require a valid session and namespace access. They reject ignored paths, unsafe paths, binary/non-UTF-8 content, symlinks, missing files, and oversized content. Private repository read rules remain in force.

## Data

- No database schema changes. For each request, load the current account and namespace access through the existing database API.
- Git reads the current HEAD commit, requested blob, and tree. A normal save writes a replacement blob; conflict recovery writes a new blob at a separate path. Each commit uses the revision it was built on as its parent.
- Normal saves compare the submitted base commit with the locked current branch tip. A mismatch returns a conflict without changing the ref. Choosing “Save draft as a copy” makes a second authenticated request; the server chooses a unique same-directory path prefixed with a sanitized username and UTC `YYYYMMDD-HHMMSS`, then compare-and-swaps against the latest HEAD. The original and concurrent changes remain untouched.
- A collision at the generated path adds a numeric disambiguator before the original filename. If HEAD changes again during copy creation, return another conflict and keep the draft available for retry.
- Commit author and committer are the signed-in username and configured account email. A non-empty commit message is required.

## Flow

1. Content file view links to the edit route.
2. GET verifies session, namespace access, repository privacy, visible safe path, regular blob type, file size, and strict UTF-8; then renders a source editor for non-Markdown or a Quill rich/source toggle for Markdown.
3. POST repeats authorization and file checks, validates the expected HEAD and commit metadata, then asks `RepoHandle` to construct the new tree and commit under a Git ref lock.
4. If the branch moved, return a conflict without moving the ref and offer either reload or “Save draft as a copy.” The copy action commits the draft as a unique sibling file atop latest HEAD. On success, redirect to the saved file with a verified commit confirmation; the new commit appears in normal history.

## External

- None. Quill, its styles, and the Markdown serialization dependency are vendored and served locally; no external API, CDN, webhook, or new environment variable is used.
