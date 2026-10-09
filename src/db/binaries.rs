use std::collections::HashMap;

use semver::Version;

use crate::db::Database;

const RETAINED_VERSION_COUNT: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryAssetInfo {
    pub filename: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryRelease {
    /// Canonical SemVer string, without an optional input `v` prefix.
    pub version: String,
    pub assets: Vec<BinaryAssetInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryBlob {
    pub filename: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutBinaryOutcome {
    Stored,
    Replaced,
    TooOld,
}

fn version_order_desc(a: &str, b: &str) -> std::cmp::Ordering {
    let a_version = Version::parse(a).expect("only validated SemVer versions are stored");
    let b_version = Version::parse(b).expect("only validated SemVer versions are stored");
    b_version.cmp(&a_version).then_with(|| a.cmp(b))
}

impl Database {
    /// Stores or replaces an asset and atomically prunes releases outside the
    /// three highest SemVer versions. A too-old upload leaves the database
    /// untouched and returns `TooOld`.
    pub async fn put_binary(
        &self,
        namespace: &str,
        repo: &str,
        version: &Version,
        filename: &str,
        bytes: &[u8],
    ) -> Result<PutBinaryOutcome, String> {
        let version = version.to_string();
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|e| e.to_string())?;

        let result: Result<PutBinaryOutcome, String> = async {
            let mut existing = conn
                .query(
                    "SELECT 1 FROM repository_binaries
                     WHERE namespace = ?1 AND repo = ?2 AND version = ?3 AND filename = ?4",
                    turso::params![namespace, repo, version.clone(), filename],
                )
                .await
                .map_err(|e| e.to_string())?;
            let outcome = if existing.next().await.map_err(|e| e.to_string())?.is_some() {
                PutBinaryOutcome::Replaced
            } else {
                PutBinaryOutcome::Stored
            };

            let mut rows = conn
                .query(
                    "SELECT DISTINCT version FROM repository_binaries
                     WHERE namespace = ?1 AND repo = ?2",
                    turso::params![namespace, repo],
                )
                .await
                .map_err(|e| e.to_string())?;
            let mut versions = Vec::new();
            while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
                versions.push(row.get::<String>(0).map_err(|e| e.to_string())?);
            }
            if !versions.contains(&version) {
                versions.push(version.clone());
            }
            for stored_version in &versions {
                Version::parse(stored_version).map_err(|error| {
                    format!("Invalid SemVer version stored in binary registry: {error}")
                })?;
            }
            versions.sort_by(|a, b| version_order_desc(a, b));

            let retained: Vec<String> = versions
                .iter()
                .take(RETAINED_VERSION_COUNT)
                .cloned()
                .collect();
            if !retained.contains(&version) {
                return Ok(PutBinaryOutcome::TooOld);
            }

            conn.execute(
                "INSERT INTO repository_binaries
                     (namespace, repo, version, filename, content, size_bytes, uploaded_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(namespace, repo, version, filename) DO UPDATE SET
                     content = excluded.content,
                     size_bytes = excluded.size_bytes,
                     uploaded_at = excluded.uploaded_at",
                turso::params![
                    namespace,
                    repo,
                    version,
                    filename,
                    turso::Value::Blob(bytes.to_vec()),
                    bytes.len() as i64,
                    chrono::Utc::now().to_rfc3339()
                ],
            )
            .await
            .map_err(|e| e.to_string())?;

            for old_version in versions.iter().skip(RETAINED_VERSION_COUNT) {
                conn.execute(
                    "DELETE FROM repository_binaries
                     WHERE namespace = ?1 AND repo = ?2 AND version = ?3",
                    turso::params![namespace, repo, old_version.as_str()],
                )
                .await
                .map_err(|e| e.to_string())?;
            }

            Ok(outcome)
        }
        .await;

        match result {
            Ok(PutBinaryOutcome::TooOld) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|error| format!("Upload was too old (rollback failed: {error})"))?;
                Ok(PutBinaryOutcome::TooOld)
            }
            Ok(outcome) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| format!("Failed to commit binary upload: {e}"))?;
                Ok(outcome)
            }
            Err(error) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|rollback| format!("{error} (rollback also failed: {rollback})"))?;
                Err(error)
            }
        }
    }

    /// Lists metadata for all retained releases without loading BLOB content.
    pub async fn list_binary_releases(
        &self,
        namespace: &str,
        repo: &str,
    ) -> Result<Vec<BinaryRelease>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT version, filename, size_bytes FROM repository_binaries
                 WHERE namespace = ?1 AND repo = ?2
                 ORDER BY filename ASC",
                turso::params![namespace, repo],
            )
            .await
            .map_err(|e| e.to_string())?;

        let mut assets_by_version: HashMap<String, Vec<BinaryAssetInfo>> = HashMap::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            let version: String = row.get(0).map_err(|e| e.to_string())?;
            Version::parse(&version)
                .map_err(|error| format!("Invalid SemVer version in registry: {error}"))?;
            let asset = BinaryAssetInfo {
                filename: row.get(1).map_err(|e| e.to_string())?,
                size_bytes: row
                    .get::<i64>(2)
                    .map_err(|e| e.to_string())?
                    .try_into()
                    .map_err(|_| "Invalid negative binary size in database".to_string())?,
            };
            assets_by_version.entry(version).or_default().push(asset);
        }

        let mut releases: Vec<BinaryRelease> = assets_by_version
            .into_iter()
            .map(|(version, mut assets)| {
                assets.sort_by(|a, b| a.filename.cmp(&b.filename));
                BinaryRelease { version, assets }
            })
            .collect();
        releases.sort_by(|a, b| version_order_desc(&a.version, &b.version));
        releases.truncate(RETAINED_VERSION_COUNT);
        Ok(releases)
    }

    pub async fn get_binary(
        &self,
        namespace: &str,
        repo: &str,
        version: &Version,
        filename: &str,
    ) -> Result<Option<BinaryBlob>, String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT filename, content FROM repository_binaries
                 WHERE namespace = ?1 AND repo = ?2 AND version = ?3 AND filename = ?4",
                turso::params![namespace, repo, version.to_string(), filename],
            )
            .await
            .map_err(|e| e.to_string())?;

        let Some(row) = rows.next().await.map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        let content = match row.get_value(1).map_err(|e| e.to_string())? {
            turso::Value::Blob(bytes) => bytes,
            _ => return Err("Binary content is not stored as a BLOB".to_string()),
        };

        Ok(Some(BinaryBlob {
            filename: row.get(0).map_err(|e| e.to_string())?,
            bytes: content,
        }))
    }

    /// Resolves a filename in the highest retained SemVer release that has it.
    pub async fn get_latest_binary(
        &self,
        namespace: &str,
        repo: &str,
        filename: &str,
    ) -> Result<Option<BinaryBlob>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                "SELECT DISTINCT version FROM repository_binaries
                 WHERE namespace = ?1 AND repo = ?2 AND filename = ?3",
                turso::params![namespace, repo, filename],
            )
            .await
            .map_err(|e| e.to_string())?;
        let mut versions = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            versions.push(row.get::<String>(0).map_err(|e| e.to_string())?);
        }
        for version in &versions {
            Version::parse(version)
                .map_err(|error| format!("Invalid SemVer version in registry: {error}"))?;
        }
        versions.sort_by(|a, b| version_order_desc(a, b));

        match versions.first() {
            Some(version) => {
                let version = Version::parse(version).map_err(|e| e.to_string())?;
                self.get_binary(namespace, repo, &version, filename).await
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db_path() -> String {
        format!("/tmp/test_twig_binaries_{}.db", uuid::Uuid::new_v4())
    }

    fn version(input: &str) -> Version {
        Version::parse(input).expect("valid test SemVer")
    }

    #[tokio::test]
    async fn binary_database_round_trips_arbitrary_blob_bytes() {
        let path = test_db_path();
        let db = Database::new(&path);
        db.init_tables().await.expect("initialize database");
        let bytes = [0, 255, 1, 0, 128];

        assert_eq!(
            db.put_binary("ns", "repo", &version("1.0.0"), "program", &bytes)
                .await
                .expect("store binary"),
            PutBinaryOutcome::Stored
        );
        let binary = db
            .get_binary("ns", "repo", &version("1.0.0"), "program")
            .await
            .expect("read binary")
            .expect("binary exists");
        assert_eq!(binary.bytes, bytes);
        assert_eq!(binary.filename, "program");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn binary_database_replaces_and_lists_asset_metadata() {
        let path = test_db_path();
        let db = Database::new(&path);
        db.init_tables().await.expect("initialize database");
        db.put_binary("ns", "repo", &version("1.0.0"), "linux", b"old")
            .await
            .expect("store first binary");
        assert_eq!(
            db.put_binary("ns", "repo", &version("1.0.0"), "linux", b"new bytes")
                .await
                .expect("replace binary"),
            PutBinaryOutcome::Replaced
        );
        db.put_binary("ns", "repo", &version("1.0.0"), "macos", b"arm")
            .await
            .expect("store second asset");

        let releases = db
            .list_binary_releases("ns", "repo")
            .await
            .expect("list releases");
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].version, "1.0.0");
        assert_eq!(releases[0].assets.len(), 2);
        assert_eq!(releases[0].assets[0].size_bytes, 9);
        assert_eq!(
            db.get_binary("ns", "repo", &version("1.0.0"), "linux")
                .await
                .expect("read replaced binary")
                .unwrap()
                .bytes,
            b"new bytes"
        );

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn binary_database_orders_semver_and_retains_the_top_three_versions() {
        let path = test_db_path();
        let db = Database::new(&path);
        db.init_tables().await.expect("initialize database");
        for release in ["1.0.0", "1.9.0", "1.10.0"] {
            db.put_binary("ns", "repo", &version(release), "linux", release.as_bytes())
                .await
                .expect("store release");
        }

        // The fourth upload is newer than 1.0.0, so it replaces that release
        // in the retained window while preserving all assets in 1.8.0.
        db.put_binary("ns", "repo", &version("1.8.0"), "linux", b"1.8 linux")
            .await
            .expect("store fourth release");
        db.put_binary("ns", "repo", &version("1.8.0"), "macos", b"1.8 mac")
            .await
            .expect("store another asset in retained release");
        assert_eq!(
            db.put_binary("ns", "repo", &version("1.7.0"), "linux", b"too old")
                .await
                .expect("too-old upload returns an outcome"),
            PutBinaryOutcome::TooOld
        );

        let releases = db
            .list_binary_releases("ns", "repo")
            .await
            .expect("list retained releases");
        assert_eq!(
            releases
                .iter()
                .map(|release| release.version.as_str())
                .collect::<Vec<_>>(),
            ["1.10.0", "1.9.0", "1.8.0"]
        );
        assert_eq!(releases[2].assets.len(), 2);
        assert!(
            db.get_binary("ns", "repo", &version("1.0.0"), "linux")
                .await
                .expect("query old release")
                .is_none()
        );
        assert!(
            db.get_binary("ns", "repo", &version("1.7.0"), "linux")
                .await
                .expect("query too-old release")
                .is_none()
        );

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn latest_binary_resolves_highest_release_containing_filename() {
        let path = test_db_path();
        let db = Database::new(&path);
        db.init_tables().await.expect("initialize database");
        db.put_binary("ns", "repo", &version("1.9.0"), "linux", b"linux 1.9")
            .await
            .expect("store linux release");
        db.put_binary("ns", "repo", &version("1.10.0"), "macos", b"mac 1.10")
            .await
            .expect("store newer macOS release");

        let latest_linux = db
            .get_latest_binary("ns", "repo", "linux")
            .await
            .expect("resolve latest Linux asset")
            .expect("Linux asset exists");
        assert_eq!(latest_linux.bytes, b"linux 1.9");

        let _ = std::fs::remove_file(path);
    }
}
