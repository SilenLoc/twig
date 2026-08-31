//! Image attachments.
//!
//! Attachments live outside git: keeping them in the ticket repository would
//! make every clone carry every image ever uploaded, forever. On disk they are
//! content-addressed, so identical uploads collapse to one file and an
//! attachment path can never collide.
//!
//! The type is decided by sniffing magic bytes, never by the client-supplied
//! filename or `Content-Type`.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Largest accepted upload.
pub const MAX_ATTACHMENT_BYTES: usize = 5 * 1024 * 1024;

/// Image formats accepted for upload.
///
/// SVG is deliberately excluded: it is a document format that can carry script
/// and would be served from our own origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageKind {
    pub fn content_type(self) -> &'static str {
        match self {
            ImageKind::Png => "image/png",
            ImageKind::Jpeg => "image/jpeg",
            ImageKind::Gif => "image/gif",
            ImageKind::Webp => "image/webp",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ImageKind::Png => "png",
            ImageKind::Jpeg => "jpg",
            ImageKind::Gif => "gif",
            ImageKind::Webp => "webp",
        }
    }
}

/// Identifies an image by its leading bytes.
pub fn sniff(bytes: &[u8]) -> Option<ImageKind> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(ImageKind::Png);
    }
    if bytes.starts_with(b"\xFF\xD8\xFF") {
        return Some(ImageKind::Jpeg);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(ImageKind::Gif);
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(ImageKind::Webp);
    }
    None
}

/// Whether `digest` is a well-formed content address. Guards the serving path
/// against traversal, since the digest arrives from the URL.
pub fn is_valid_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
}

fn hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Location of an attachment. Sharded by the first byte of the digest so a
/// namespace with many attachments does not end up with one huge directory.
fn blob_path(root: &str, namespace: &str, digest: &str) -> PathBuf {
    Path::new(root)
        .join(namespace)
        .join(&digest[0..2])
        .join(digest)
}

/// Stores `bytes`, returning the content address.
///
/// Rejects anything that is not a recognised image or is over the size cap.
/// Storing the same bytes twice is a no-op that returns the same address.
pub fn store(root: &str, namespace: &str, bytes: &[u8]) -> Result<(String, ImageKind), String> {
    if bytes.is_empty() {
        return Err("The uploaded file is empty".to_string());
    }
    if bytes.len() > MAX_ATTACHMENT_BYTES {
        return Err(format!(
            "Attachments are limited to {} MB",
            MAX_ATTACHMENT_BYTES / (1024 * 1024)
        ));
    }

    let Some(kind) = sniff(bytes) else {
        return Err("Only PNG, JPEG, GIF and WebP images can be attached".to_string());
    };

    if !crate::git::bare::is_safe_component(namespace) {
        return Err("Invalid namespace".to_string());
    }

    let digest = hash(bytes);
    let path = blob_path(root, namespace, &digest);

    if path.exists() {
        return Ok((digest, kind)); // identical content already stored
    }

    let parent = path
        .parent()
        .ok_or_else(|| "Invalid attachment path".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("Failed to create attachment directory: {e}"))?;
    std::fs::write(&path, bytes).map_err(|e| format!("Failed to write attachment: {e}"))?;

    Ok((digest, kind))
}

/// Loads an attachment by content address.
pub fn load(root: &str, namespace: &str, digest: &str) -> Option<(ImageKind, Vec<u8>)> {
    if !is_valid_digest(digest) || !crate::git::bare::is_safe_component(namespace) {
        return None;
    }
    let bytes = std::fs::read(blob_path(root, namespace, digest)).ok()?;
    let kind = sniff(&bytes)?;
    Some((kind, bytes))
}

/// URL an attachment is served from, for embedding in markdown.
pub fn url(namespace: &str, digest: &str) -> String {
    format!("/{namespace}/ticket/attachment/{digest}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR fake body";
    const JPEG: &[u8] = b"\xFF\xD8\xFF\xE0 fake body";

    fn temp_root() -> String {
        let root = format!("/tmp/test_fig_attach_{}", uuid::Uuid::new_v4());
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn test_sniff_recognises_supported_formats() {
        assert_eq!(sniff(PNG), Some(ImageKind::Png));
        assert_eq!(sniff(JPEG), Some(ImageKind::Jpeg));
        assert_eq!(sniff(b"GIF89a....."), Some(ImageKind::Gif));
        assert_eq!(
            sniff(b"RIFF\x00\x00\x00\x00WEBPVP8 "),
            Some(ImageKind::Webp)
        );
    }

    #[test]
    fn test_sniff_rejects_non_images() {
        assert_eq!(sniff(b"#!/bin/sh\nrm -rf /"), None);
        assert_eq!(
            sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"),
            None
        );
        assert_eq!(sniff(b""), None);
    }

    #[test]
    fn test_store_rejects_a_disguised_non_image() {
        let root = temp_root();
        // A payload a client would happily label `evil.png`.
        let result = store(&root, "acme", b"<html><script>alert(1)</script></html>");
        assert!(
            result.is_err(),
            "content is sniffed, not trusted from the name"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_store_rejects_oversized_uploads() {
        let root = temp_root();
        let mut big = PNG.to_vec();
        big.resize(MAX_ATTACHMENT_BYTES + 1, 0);
        assert!(store(&root, "acme", &big).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_store_then_load_round_trip() {
        let root = temp_root();
        let (digest, kind) = store(&root, "acme", PNG).expect("store");

        assert!(is_valid_digest(&digest));
        assert_eq!(kind, ImageKind::Png);

        let (loaded_kind, bytes) = load(&root, "acme", &digest).expect("load");
        assert_eq!(loaded_kind, ImageKind::Png);
        assert_eq!(bytes, PNG);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_identical_uploads_deduplicate() {
        let root = temp_root();
        let (first, _) = store(&root, "acme", PNG).unwrap();
        let (second, _) = store(&root, "acme", PNG).unwrap();

        assert_eq!(first, second, "same bytes must give the same address");

        let stored = walkdir_count(&root);
        assert_eq!(stored, 1, "the same image must not be written twice");

        let _ = std::fs::remove_dir_all(&root);
    }

    fn walkdir_count(root: &str) -> usize {
        fn count(path: &Path) -> usize {
            let Ok(entries) = std::fs::read_dir(path) else {
                return 0;
            };
            entries
                .filter_map(Result::ok)
                .map(|e| {
                    if e.path().is_dir() {
                        count(&e.path())
                    } else {
                        1
                    }
                })
                .sum()
        }
        count(Path::new(root))
    }

    #[test]
    fn test_different_content_gets_different_addresses() {
        let root = temp_root();
        let (png, _) = store(&root, "acme", PNG).unwrap();
        let (jpeg, _) = store(&root, "acme", JPEG).unwrap();
        assert_ne!(png, jpeg);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_load_rejects_traversal_and_bad_digests() {
        let root = temp_root();
        store(&root, "acme", PNG).unwrap();

        assert!(load(&root, "acme", "../../etc/passwd").is_none());
        assert!(load(&root, "acme", "short").is_none());
        assert!(load(&root, "acme", &"A".repeat(64)).is_none(), "uppercase");
        assert!(load(&root, "../etc", &"a".repeat(64)).is_none());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_namespaces_are_isolated() {
        let root = temp_root();
        let (digest, _) = store(&root, "acme", PNG).unwrap();
        assert!(load(&root, "other", &digest).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_url_shape() {
        assert_eq!(url("acme", "abc"), "/acme/ticket/attachment/abc");
    }
}
