# Program Design: Public binaries in the Scripts tab

## Files

- `Cargo.toml` — add SemVer parsing/comparison for normalized version ordering.
- `src/main.rs` — declare the binary HTTP module and register its routes before the Git catch-all.
- `src/binaries.rs` — validate versions and asset names, authenticate uploads, enforce repository visibility, and handle HTTP upload/download responses.
- `src/db/mod.rs` — register the binary data-access module.
- `src/db/migration.rs` — add the artifact BLOB table and version/asset lookup index.
- `src/db/binaries.rs` — define binary metadata types and SQLite-compatible BLOB insert, list, lookup, and retention methods.
- `src/view/repo.rs` — show assets for the three retained versions in the existing Scripts tab; continue to show one Copy button per configured install script.
- `src/integration_tests.rs` — prove HTTP upload/download, visibility, and retention through the application routes.

## Types & signatures

```rust
// src/db/binaries.rs
pub struct BinaryAssetInfo {
    pub filename: String,
    pub size_bytes: u64,
}

pub struct BinaryRelease {
    /// Canonical SemVer string, without an optional input `v` prefix.
    pub version: String,
    pub assets: Vec<BinaryAssetInfo>,
}

pub struct BinaryBlob {
    pub filename: String,
    pub bytes: Vec<u8>,
}

impl Database {
    pub async fn put_binary(
        &self, namespace: &str, repo: &str, version: &semver::Version,
        filename: &str, bytes: &[u8],
    ) -> Result<PutBinaryOutcome, String>;
    pub async fn list_binary_releases(
        &self, namespace: &str, repo: &str,
    ) -> Result<Vec<BinaryRelease>, String>;
    pub async fn get_binary(
        &self, namespace: &str, repo: &str, version: &semver::Version,
        filename: &str,
    ) -> Result<Option<BinaryBlob>, String>;
    pub async fn get_latest_binary(
        &self, namespace: &str, repo: &str, filename: &str,
    ) -> Result<Option<BinaryBlob>, String>;
}

pub enum PutBinaryOutcome { Stored, Replaced, TooOld }
```

```rust
// src/binaries.rs
#[derive(Deserialize)]
struct BinaryParams { namespace: String, repo: String, version: String, filename: String }

#[put("/{namespace}/{repo}/binaries/{version}/{filename}")]
async fn put_binary(
    req: HttpRequest, body: web::Bytes, params: web::Path<BinaryParams>,
    server: web::Data<config::Server>, auth: web::Data<TwigContext>,
) -> HttpResponse;

#[get("/{namespace}/{repo}/binaries/{version}/{filename}")]
async fn get_binary(
    req: HttpRequest, params: web::Path<BinaryParams>,
    server: web::Data<config::Server>, auth: web::Data<TwigContext>,
) -> HttpResponse;

async fn require_namespace_writer(
    req: &HttpRequest, auth: &TwigContext, namespace: &str,
) -> Result<User, HttpResponse>;
fn parse_release_version(input: &str) -> Result<semver::Version, HttpResponse>;
fn validate_asset_filename(input: &str) -> Result<(), HttpResponse>;
```

```rust
// src/view/repo.rs — changes to the existing Scripts rendering path
async fn load_script_tab_binaries(
    db: &Database, namespace: &str, repo: &str,
) -> Result<Vec<BinaryRelease>, String>;
fn render_scripts_view(
    ctx: &RepoContext, node: &ScriptGroupNode, releases: &[BinaryRelease],
) -> Markup;
```

The upload API is HTTP `PUT` with the artifact as the raw request body. A request version may start with `v`; normalize it before persistence and comparison. The reserved version segment `latest` is download-only. Asset filenames are single safe path components; the filename also serves as the visible platform/build label.

## Call stack

### Upload a locally built asset

1. `PUT /{namespace}/{repo}/binaries/{version}/{filename}` → `binaries::put_binary`.
2. Validate repository components, SemVer (allow optional `v`), filename, and the existing 512 MiB request limit.
3. Open the repository and require Basic credentials for a user with namespace access; do not create a namespace or repository as a side effect.
4. `Database::put_binary` starts an immediate transaction, upserts the BLOB, computes the three highest distinct SemVer versions, prunes all assets in older versions, and commits.
5. Return success for a stored/replaced asset; return `409 Conflict` if the submitted version is lower than the retained three-version window and therefore is not stored.

### Show the Scripts tab and fetch an asset

1. The existing repository route opens `.twig.toml` and discovers its current `[scripts]` groups. No new `[binaries]` configuration is added; those existing groups and the optional `tabs` allow-list control Scripts-tab visibility.
2. Only when rendering Scripts, fetch metadata with `Database::list_binary_releases`; do not query the binary table while rendering unrelated tabs.
3. Render the existing script groups, preserving an individual Copy button per script command, then list all assets in each retained version with version-pinned download links.
4. A direct `GET /{namespace}/{repo}/binaries/{version}/{filename}` fetches a pinned version. `GET /{namespace}/{repo}/binaries/latest/{filename}` resolves the highest retained version containing that filename. Both routes work independently of whether the Scripts tab is visible.
5. Download handling opens the repository, applies its `private` setting (anonymous for public repositories; session-authenticated for private ones), then reads the exact BLOB from SQLite and responds with attachment and `nosniff` headers.

## Test plan

- `binary_versions_accept_optional_v_and_normalize` — `v1.2.3` and `1.2.3` normalize to the same version.
- `binary_version_rejects_invalid_semver_and_latest_on_upload` — malformed versions and the reserved `latest` segment are rejected.
- `binary_filename_rejects_path_traversal_and_header_characters` — slashes, dot segments, and unsafe header characters cannot escape or inject response headers.
- `binary_database_round_trips_arbitrary_blob_bytes` — insert and retrieve bytes including NUL and non-UTF-8 values without modification.
- `binary_database_replaces_same_version_filename` — reuploading the same version/filename replaces bytes rather than duplicating a row.
- `binary_database_keeps_multiple_assets_in_one_version` — distinct filenames under one version remain listed together.
- `binary_database_retains_top_three_semver_versions` — values such as `1.10.0` sort above `1.9.0`; all assets in older versions are pruned.
- `binary_upload_below_retention_window_returns_conflict` — an older fourth version is rejected and does not disturb retained releases.
- `binary_upload_requires_namespace_writer_basic_auth` — missing, invalid, or unrelated credentials cannot store an artifact; a member can.
- `public_binary_download_works_without_scripts_tab` — a public asset URL returns exact bytes even when no `[scripts]` section exists.
- `private_binary_download_requires_session` — anonymous access is denied and an authenticated repository session can download.
- `latest_binary_url_uses_highest_version_with_matching_asset` — when the highest release lacks a platform file, `latest` falls back to the highest retained release that has it.
- `scripts_tab_lists_every_asset_and_preserves_script_copy_commands` — the tab renders all platform assets for each retained version and each configured installer command still has a Copy button.
- `git_smart_push_routes_are_unchanged` — existing Git push integration tests continue to pass; no binary-upload behavior is added to Git routes.

## Least confident decisions

1. Resolved in Slice 3: an upload below the retained three-version window returns `409 Conflict` and does not change stored releases.
2. Use the artifact filename as its platform/build label instead of adding separate upload metadata. This keeps upload calls simple but means owners should choose descriptive filenames.
3. Private downloads use Twig's existing session authentication. Public-repository downloads, including installer-script requests, remain anonymous.
4. Resolved in Slice 4: uploads are explicitly capped at the existing 512 MiB request limit and buffered; larger binaries would need a streaming design.
