//! Helpers shared by the GitHub-compatible REST routes (`/api/v3`) and the GraphQL
//! endpoint: the public origin, the `Link` header for paged lists, stable numeric
//! identifiers, and the size of a pull request's change.

use std::fmt::Write;

use actix_web::{HttpRequest, web};
use sha2::{Digest, Sha256};

use crate::{config, db::pull_requests::PullRequest, git::bare::RepoHandle, pulls};

/// Derives the integer identifier REST clients expect from a stable Twig key.
/// The first 8 bytes of SHA-256 are masked to 53 bits so JavaScript clients can
/// represent the value exactly. It is deterministic, so it never changes for a key.
pub fn numeric_id(key: &str) -> i64 {
    let digest = Sha256::digest(key.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    i64::try_from(u64::from_be_bytes(bytes) & ((1u64 << 53) - 1)).unwrap_or_default()
}

/// GitHub titles a request as work in progress with a `WIP:` or `[WIP]` prefix when
/// it has no draft flag; clients read `draft` from this.
pub fn is_work_in_progress(title: &str) -> bool {
    let lower = title.to_ascii_lowercase();
    lower.starts_with("wip:") || lower.starts_with("[wip]")
}

/// Lines added, lines removed, and files changed by a request, from its merge base.
pub fn change_size(handle: &RepoHandle, pull_request: &PullRequest) -> (i64, i64, i64) {
    let range = pulls::change_range(handle, pull_request)
        .and_then(|(from, to)| handle.diff_files(from, to).ok());
    let files = range.unwrap_or_default();
    let additions: usize = files.iter().map(|file| file.additions).sum();
    let deletions: usize = files.iter().map(|file| file.deletions).sum();
    (
        i64::try_from(additions).unwrap_or_default(),
        i64::try_from(deletions).unwrap_or_default(),
        i64::try_from(files.len()).unwrap_or_default(),
    )
}

/// The `Link` header for `rel="next"`, keeping the caller's other query parameters
/// and replacing `page`, so a client can follow it without rebuilding the URL.
pub fn next_page_link(req: &HttpRequest, next_page: u32) -> String {
    let kept = req
        .query_string()
        .split('&')
        .filter(|pair| !pair.is_empty() && pair.split('=').next() != Some("page"))
        .collect::<Vec<_>>();
    let mut query = kept.join("&");
    if !query.is_empty() {
        query.push('&');
    }
    let _ = write!(query, "page={next_page}");
    format!("<{}{}?{query}>; rel=\"next\"", origin(req), req.path())
}

/// Origin used for `html_url`, clone URLs, and `Link` headers. `PUBLIC_BASE_URL` wins
/// when configured; otherwise the request's scheme and host are used.
pub fn origin(req: &HttpRequest) -> String {
    if let Some(base) = req
        .app_data::<web::Data<config::Server>>()
        .and_then(|server| server.public_base_url())
    {
        return base.trim_end_matches('/').to_string();
    }
    let info = req.connection_info();
    format!("{}://{}", info.scheme(), info.host())
}
