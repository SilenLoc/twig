# Architecture: Public binaries in the Scripts tab

## Fit

- Keep normal Git smart HTTP pushes (`git http-backend` in `src/git_backend.rs` and `src/git/mod.rs`) unchanged for source code and commits. Binary publishing and retrieval use HTTP upload/download routes only; do not provide a Git-push option for binaries, intercept Git commits, inspect Git paths for binary uploads, or install Git hooks for this feature.
- Add repository-scoped HTTP upload/download routes alongside the existing Scripts tab. This avoids transforming Git history and gives local build scripts a direct publication interface.
- Do not add a `[binaries]` section or a separate registry opt-in. Reuse the existing `.twig.toml` `[scripts]` configuration and existing Scripts tab: when configured script groups make that tab available, it also displays retained version groups and their assets. Existing configured script entries remain the source of copyable install commands and repository script locations. If `tabs` is explicitly set, its existing allow-list/order still applies to the `scripts` tab. This controls UI visibility only; direct download URLs work independently of whether the Scripts tab is visible.
- Extend the existing Scripts tab frame/rendering in `src/view/repo.rs`, preserving its grouped install scripts and a separate copy button for each configured script command while adding a releases section.
- Extend `src/db/migration.rs` and `src/db/` database methods to store and query artifacts as SQLite-compatible BLOBs. Preserve `Database`'s lazy connection behavior; do not open the SQLite file eagerly.
- Public/private visibility should follow the repository's existing `private` setting: listing and downloading are anonymous for public repositories and require the existing authenticated session for private repositories. Uploads always require valid Git Basic credentials and namespace write access, matching push authorization.

## Endpoints

- `GET /{namespace}/{repo}/scripts` — extend the existing page with configured install scripts and retained version groups with multiple asset downloads.
- `PUT /{namespace}/{repo}/binaries/{version}/{filename}` — upload or replace one platform binary; authenticated namespace writer only.
- `GET /{namespace}/{repo}/binaries/{version}/{filename}` — stable, direct download URL for one pinned binary; usable by browser links and scripts, public for public repositories and authenticated for private ones. It works even when the Scripts tab is hidden.
- `GET /{namespace}/{repo}/binaries/latest/{filename}` — direct script-friendly URL resolving the filename in the highest retained SemVer version; the same public/private access rule applies.

## Data

- Add a `repository_binaries` table with repository namespace/name, SemVer version, filename/platform label, BLOB content, byte size, and upload timestamp. A unique key on `(namespace, repo, version, filename)` makes re-uploading the same asset replace it. One version can contain any number of distinct platform assets. Accept versions with or without a leading `v`, normalize them for storage/comparison, and display them with a leading `v` in the UI.
- Listing query: select repository assets grouped by version, ordered by parsed semantic version descending, then filename; show all assets for the three highest distinct versions. The `latest` download URL resolves the highest retained version that contains the requested filename.
- After a successful upload, transactionally keep the three highest distinct SemVer versions for that repository and delete every asset belonging to lower versions. Uploading another filename for a retained version keeps the other assets in that version.
- Download query: look up one repository/version/filename row and return its bytes with attachment headers and a safe content type.
- The current Actix payload limit is 512 MiB. The first implementation should use that existing ceiling for an individual upload; request bodies and BLOB values are materialized in memory, so deployments expecting larger artifacts need a streaming/file-backed design instead.

## Flow

1. A visitor opens the repository's Scripts tab; Twig reads `.twig.toml`, discovers its existing script groups, applies repository visibility, and renders the configured copyable installer commands.
2. The same tab reads the three highest version groups from SQLite and shows every uploaded platform asset in each group with a direct download link.
3. The owner uploads an artifact with `PUT` and Git Basic credentials. Twig validates the repository/version/filename, authenticates namespace write access, then atomically upserts the BLOB and prunes versions below the newest three.
4. A direct versioned download request applies repository visibility, reads the BLOB from SQLite, and returns it as a downloadable response. The `latest` form first resolves the highest retained version containing that filename, then returns the same downloadable response. Neither URL requires the Scripts page to be visible.

## External

None. Versions are proposed to follow Semantic Versioning; the existing project has no SemVer dependency, so implementation would need to add one or define an equivalent validated comparator.
