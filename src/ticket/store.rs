//! Reading and writing the canonical ticket tree.
//!
//! Every writer — the web UI in the server process and the `proc-receive` hook
//! in its own process — goes through [`TicketStore::lock`], which takes an
//! exclusive `flock` on a file inside the bare repository. A `tokio::Mutex`
//! would not do: the hook is a separate OS process, so the lock has to live in
//! the filesystem.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fs2::FileExt;
use git2::{Oid, Repository};

use crate::ticket::model::{
    REPO_CONFIG_PATH, RepoConfig, TICKETS_DIR, Ticket, is_ticket_path, render_repo_config,
};
use crate::ticket::repo::{CANONICAL_REF, SCHEMA_VERSION, ticket_repo_path};

const LOCK_FILE: &str = "fig-ingest.lock";

/// A pending change to the canonical tree, keyed by repository-relative path.
#[derive(Debug, Clone)]
pub enum Change {
    Upsert(String, Box<Ticket>),
    Delete(String),
}

pub struct TicketStore {
    repo_path: PathBuf,
    namespace: String,
}

impl TicketStore {
    pub fn open(project_root: &str, namespace: &str) -> Result<Self, String> {
        let repo_path = ticket_repo_path(project_root, namespace);
        if !repo_path.join("HEAD").exists() {
            return Err(format!("No ticket repository for namespace '{namespace}'"));
        }
        Ok(Self {
            repo_path,
            namespace: namespace.to_string(),
        })
    }

    /// Opens the store for a namespace, creating the ticket repository first if
    /// it does not exist yet (namespaces created before the tracker shipped).
    pub fn open_or_create(project_root: &str, namespace: &str) -> Result<Self, String> {
        crate::ticket::repo::ensure_ticket_repo(project_root, namespace, "Fig", "fig@localhost")?;
        Self::open(project_root, namespace)
    }

    /// Opens the store directly from the bare repository path, deriving the
    /// namespace from its parent directory. Used by the `proc-receive` hook,
    /// which is started by git inside `GIT_DIR` and has no config of its own.
    ///
    /// The path is canonicalised first: git commonly sets `GIT_DIR=.`, which
    /// has no usable parent.
    pub fn from_repo_path(repo_path: &Path) -> Result<Self, String> {
        let repo_path = repo_path
            .canonicalize()
            .map_err(|e| format!("Cannot resolve {}: {e}", repo_path.display()))?;

        let namespace = repo_path
            .parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                format!(
                    "Cannot derive namespace from repository path {}",
                    repo_path.display()
                )
            })?
            .to_string();

        Ok(Self {
            repo_path,
            namespace,
        })
    }

    /// Acquires the exclusive ingest lock. Blocks until it is available.
    pub fn lock(&self) -> Result<Ingest, String> {
        let lock_path = self.repo_path.join(LOCK_FILE);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .map_err(|e| format!("Failed to open ingest lock: {e}"))?;

        file.lock_exclusive()
            .map_err(|e| format!("Failed to acquire ingest lock: {e}"))?;

        let repo = Repository::open(&self.repo_path)
            .map_err(|e| format!("Failed to open ticket repo: {e}"))?;

        Ok(Ingest {
            _lock: file,
            repo,
            namespace: self.namespace.clone(),
        })
    }

    /// Reads canonical tickets without taking the lock. Safe because git object
    /// and ref reads are atomic; callers that intend to write must use
    /// [`TicketStore::lock`] instead.
    pub fn read_tickets(&self) -> Result<Vec<Ticket>, String> {
        let repo = Repository::open(&self.repo_path)
            .map_err(|e| format!("Failed to open ticket repo: {e}"))?;
        let head = canonical_commit(&repo)?;
        let mut tickets: Vec<Ticket> = read_tickets_at(&repo, head)?.into_values().collect();
        tickets.sort_by_key(|t| std::cmp::Reverse(t.number()));
        Ok(tickets)
    }

    pub fn read_ticket(&self, number: u64) -> Result<Option<Ticket>, String> {
        Ok(self
            .read_tickets()?
            .into_iter()
            .find(|t| t.number() == number))
    }
}

/// An exclusive write session. The lock is released when this is dropped.
pub struct Ingest {
    _lock: std::fs::File,
    repo: Repository,
    namespace: String,
}

impl Ingest {
    pub fn repo(&self) -> &Repository {
        &self.repo
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Current canonical commit, re-read inside the lock.
    pub fn head(&self) -> Result<Option<Oid>, String> {
        canonical_commit(&self.repo)
    }

    /// Tickets in the canonical tree, keyed by repository-relative path.
    pub fn canonical_tickets(&self) -> Result<BTreeMap<String, Ticket>, String> {
        read_tickets_at(&self.repo, self.head()?)
    }

    /// Tickets in an arbitrary commit, used for the merge base and the
    /// incoming submission.
    pub fn tickets_at(&self, commit: Option<Oid>) -> Result<BTreeMap<String, Ticket>, String> {
        read_tickets_at(&self.repo, commit)
    }

    pub fn config(&self) -> Result<RepoConfig, String> {
        let head = self.head()?;
        let text = read_blob(&self.repo, head, REPO_CONFIG_PATH)?;
        match text {
            Some(text) => RepoConfig::parse(&text),
            None => Ok(RepoConfig {
                schema_version: SCHEMA_VERSION,
                namespace: self.namespace.clone(),
                next_number: 1,
            }),
        }
    }

    /// The number to assign to the next ticket.
    ///
    /// Takes the maximum of the stored counter and one past the highest number
    /// in the tree, so numbering stays monotonic even if a client deletes or
    /// rewrites the config file. Numbers are never reused.
    pub fn next_number(&self) -> Result<u64, String> {
        let stored = self.config()?.next_number;
        let highest = self
            .canonical_tickets()?
            .values()
            .filter_map(|t| t.number)
            .max()
            .unwrap_or(0);
        Ok(stored.max(highest + 1))
    }

    /// Applies `changes`, writes the repository config, and advances the
    /// canonical ref.
    ///
    /// `extra_parent` is the accepted client tip. Recording it as a parent is
    /// what makes the client's next `git pull` a fast-forward, which is why
    /// users never see a merge.
    pub fn commit(
        &self,
        changes: &[Change],
        next_number: u64,
        extra_parent: Option<Oid>,
        author: (&str, &str),
        message: &str,
    ) -> Result<Oid, String> {
        let head = self.head()?;
        let base_tree = match head {
            Some(oid) => Some(
                self.repo
                    .find_commit(oid)
                    .and_then(|c| c.tree())
                    .map_err(|e| format!("Failed to read canonical tree: {e}"))?,
            ),
            None => None,
        };

        let mut builder = git2::build::TreeUpdateBuilder::new();

        for change in changes {
            match change {
                Change::Upsert(path, ticket) => {
                    let text = ticket.to_toml()?;
                    let blob = self
                        .repo
                        .blob(text.as_bytes())
                        .map_err(|e| format!("Failed to write ticket blob: {e}"))?;
                    builder.upsert(path.as_str(), blob, git2::FileMode::Blob);
                }
                Change::Delete(path) => {
                    builder.remove(path.as_str());
                }
            }
        }

        let config_text = render_repo_config(&self.namespace, SCHEMA_VERSION, next_number);
        let config_blob = self
            .repo
            .blob(config_text.as_bytes())
            .map_err(|e| format!("Failed to write config blob: {e}"))?;
        builder.upsert(REPO_CONFIG_PATH, config_blob, git2::FileMode::Blob);

        let base_tree = if let Some(tree) = base_tree {
            tree
        } else {
            let oid = self
                .repo
                .treebuilder(None)
                .and_then(|b| b.write())
                .map_err(|e| format!("Failed to create empty tree: {e}"))?;
            self.repo
                .find_tree(oid)
                .map_err(|e| format!("Failed to read empty tree: {e}"))?
        };

        let tree_oid = builder
            .create_updated(&self.repo, &base_tree)
            .map_err(|e| format!("Failed to build tree: {e}"))?;

        // Nothing changed and there is no client tip to record: skip the commit
        // so an idempotent re-push does not pile up empty commits.
        if tree_oid == base_tree.id()
            && extra_parent.is_none()
            && let Some(head) = head
        {
            return Ok(head);
        }

        let tree = self
            .repo
            .find_tree(tree_oid)
            .map_err(|e| format!("Failed to read new tree: {e}"))?;

        let signature = git2::Signature::now(author.0, author.1)
            .map_err(|e| format!("Failed to build signature: {e}"))?;

        let mut parent_oids = Vec::new();
        if let Some(head) = head {
            parent_oids.push(head);
        }
        if let Some(extra) = extra_parent
            && Some(extra) != head
        {
            parent_oids.push(extra);
        }

        let parent_commits = parent_oids
            .iter()
            .map(|oid| self.repo.find_commit(*oid))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to read parent commit: {e}"))?;
        let parents: Vec<&git2::Commit> = parent_commits.iter().collect();

        let commit = self
            .repo
            .commit(None, &signature, &signature, message, &tree, &parents)
            .map_err(|e| format!("Failed to create commit: {e}"))?;

        // Compare-and-swap against the value observed at the start of this
        // session. Under the flock this cannot fail, so a failure means the
        // lock was bypassed and must not be papered over.
        let current = canonical_commit(&self.repo)?;
        if current != head {
            return Err(
                "Canonical ref moved during an ingest session; refusing to overwrite".to_string(),
            );
        }

        self.repo
            .reference(CANONICAL_REF, commit, true, message)
            .map_err(|e| format!("Failed to advance canonical ref: {e}"))?;

        Ok(commit)
    }
}

fn canonical_commit(repo: &Repository) -> Result<Option<Oid>, String> {
    match repo.refname_to_id(CANONICAL_REF) {
        Ok(oid) => Ok(Some(oid)),
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(e) => Err(format!("Failed to resolve {CANONICAL_REF}: {e}")),
    }
}

/// Reads and parses every ticket file in `commit`, keyed by path.
fn read_tickets_at(
    repo: &Repository,
    commit: Option<Oid>,
) -> Result<BTreeMap<String, Ticket>, String> {
    let mut tickets = BTreeMap::new();
    let Some(commit) = commit else {
        return Ok(tickets);
    };

    let tree = repo
        .find_commit(commit)
        .and_then(|c| c.tree())
        .map_err(|e| format!("Failed to read tree: {e}"))?;

    let Ok(entry) = tree.get_path(Path::new(TICKETS_DIR)) else {
        return Ok(tickets); // no tickets yet
    };
    let Ok(dir) = entry.to_object(repo).and_then(|o| o.peel_to_tree()) else {
        return Ok(tickets);
    };

    for item in &dir {
        let Some(name) = item.name() else { continue };
        let path = format!("{TICKETS_DIR}/{name}");
        if !is_ticket_path(&path) {
            continue;
        }
        let Ok(blob) = item.to_object(repo).and_then(|o| o.peel_to_blob()) else {
            continue;
        };
        let text = std::str::from_utf8(blob.content())
            .map_err(|_| format!("{path} is not valid UTF-8"))?;
        let ticket = Ticket::parse(text).map_err(|e| format!("{path}: {e}"))?;
        tickets.insert(path, ticket);
    }

    Ok(tickets)
}

fn read_blob(repo: &Repository, commit: Option<Oid>, path: &str) -> Result<Option<String>, String> {
    let Some(commit) = commit else {
        return Ok(None);
    };
    let tree = repo
        .find_commit(commit)
        .and_then(|c| c.tree())
        .map_err(|e| format!("Failed to read tree: {e}"))?;
    let Ok(entry) = tree.get_path(Path::new(path)) else {
        return Ok(None);
    };
    let Ok(blob) = entry.to_object(repo).and_then(|o| o.peel_to_blob()) else {
        return Ok(None);
    };
    Ok(String::from_utf8(blob.content().to_vec()).ok())
}
