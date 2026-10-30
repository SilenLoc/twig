//! Git-side support for pull requests: a stable repository identity, the
//! merge-base and commit range that a request covers, and the fixed refs that
//! expose each request's head to clients.

pub use merge::{MergeBuild, MergeStrategy};
mod merge;

use std::cell::RefCell;

use git2::{ConfigLevel, Delta, DiffFormat, DiffOptions, ErrorCode, Oid, Sort};

use super::RepoHandle;

/// The parts of a commit that request pages show: enough to identify it and
/// to pre-fill a request's title and description.
#[derive(Debug, Clone)]
pub struct CommitSummary {
    pub id: Oid,
    pub short_id: String,
    pub summary: String,
    pub body: String,
    pub author_name: String,
    pub author_email: String,
    pub committed_at: i64,
    pub parents: Vec<Oid>,
}

/// How a file changed between two trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
}

/// One line of a diff hunk. `old_line` and `new_line` are 1-based and absent on
/// the side the line does not exist on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFile {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: FileStatus,
    pub additions: usize,
    pub deletions: usize,
    pub binary: bool,
    pub hunks: Vec<DiffHunk>,
}

impl DiffFile {
    /// The path a reader thinks of as "the file": the new path, or the old one for deletions.
    pub fn path(&self) -> &str {
        self.new_path
            .as_deref()
            .or(self.old_path.as_deref())
            .unwrap_or_default()
    }
}

/// Git config key holding the repository's stable identifier, written under `[twig]`.
const REPOSITORY_ID_KEY: &str = "twig.id";

impl RepoHandle {
    /// The repository's stable identifier. Pull requests are keyed by it, so a
    /// renamed or moved repository keeps its requests. Created on first use.
    pub fn repo_id(&self) -> Result<String, git2::Error> {
        if let Ok(existing) = self.repo.config()?.get_string(REPOSITORY_ID_KEY)
            && !existing.is_empty()
        {
            return Ok(existing);
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.repo
            .config()?
            .open_level(ConfigLevel::Local)?
            .set_str(REPOSITORY_ID_KEY, &id)?;
        // Re-read so that two first callers agree on the value that was kept.
        self.repo.config()?.get_string(REPOSITORY_ID_KEY)
    }

    /// The best common ancestor of two commits, or `None` if they are unrelated.
    pub fn merge_base(&self, one: Oid, other: Oid) -> Result<Option<Oid>, git2::Error> {
        match self.repo.merge_base(one, other) {
            Ok(base) => Ok(Some(base)),
            Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Commits reachable from `head` but not from `base`, oldest first. This is
    /// the set a request proposes to bring into its base.
    pub fn commit_ids_in_range(&self, base: Oid, head: Oid) -> Result<Vec<Oid>, git2::Error> {
        let mut walk = self.repo.revwalk()?;
        walk.push(head)?;
        walk.hide(base)?;
        walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
        walk.collect()
    }

    /// Reads the summary of one commit.
    pub fn commit_summary(&self, oid: Oid) -> Result<CommitSummary, git2::Error> {
        let commit = self.repo.find_commit(oid)?;
        let message = commit.message().unwrap_or_default();
        let (summary, body) = match message.split_once('\n') {
            Some((first, rest)) => (first.trim().to_string(), rest.trim().to_string()),
            None => (message.trim().to_string(), String::new()),
        };
        let id = oid.to_string();
        let author = commit.author();
        Ok(CommitSummary {
            id: oid,
            short_id: id[..id.len().min(7)].to_string(),
            summary,
            body,
            author_name: author.name().unwrap_or_default().to_string(),
            author_email: author.email().unwrap_or_default().to_string(),
            committed_at: commit.time().seconds(),
            parents: commit.parent_ids().collect(),
        })
    }

    /// The unified diff from `from` to `to`, as `git diff` prints it.
    pub fn diff_patch(&self, from: Oid, to: Oid) -> Result<String, git2::Error> {
        let diff = self.tree_diff(from, to)?;
        let mut patch = Vec::new();
        diff.print(DiffFormat::Patch, |_, _, line| {
            if matches!(line.origin(), '+' | '-' | ' ') {
                patch.push(line.origin() as u8);
            }
            patch.extend_from_slice(line.content());
            true
        })?;
        Ok(String::from_utf8_lossy(&patch).into_owned())
    }

    /// The changed files between `from` and `to`, with their hunks and line numbers.
    pub fn diff_files(&self, from: Oid, to: Oid) -> Result<Vec<DiffFile>, git2::Error> {
        let diff = self.tree_diff(from, to)?;
        let files: RefCell<Vec<DiffFile>> = RefCell::new(Vec::new());
        diff.foreach(
            &mut |delta, _| {
                let status = match delta.status() {
                    Delta::Added => FileStatus::Added,
                    Delta::Deleted => FileStatus::Deleted,
                    Delta::Renamed => FileStatus::Renamed,
                    _ => FileStatus::Modified,
                };
                let path = |file: git2::DiffFile<'_>| {
                    file.path().map(|path| path.to_string_lossy().into_owned())
                };
                files.borrow_mut().push(DiffFile {
                    old_path: path(delta.old_file()),
                    new_path: path(delta.new_file()),
                    status,
                    additions: 0,
                    deletions: 0,
                    binary: delta.old_file().is_binary() || delta.new_file().is_binary(),
                    hunks: Vec::new(),
                });
                true
            },
            None,
            Some(&mut |_, hunk: git2::DiffHunk<'_>| {
                if let Some(file) = files.borrow_mut().last_mut() {
                    file.hunks.push(DiffHunk {
                        header: String::from_utf8_lossy(hunk.header())
                            .trim_end()
                            .to_string(),
                        lines: Vec::new(),
                    });
                }
                true
            }),
            Some(&mut |_, _, line: git2::DiffLine<'_>| {
                let kind = match line.origin() {
                    '+' => LineKind::Added,
                    '-' => LineKind::Removed,
                    _ => LineKind::Context,
                };
                let mut files = files.borrow_mut();
                if let Some(file) = files.last_mut() {
                    match kind {
                        LineKind::Added => file.additions += 1,
                        LineKind::Removed => file.deletions += 1,
                        LineKind::Context => {}
                    }
                    if let Some(hunk) = file.hunks.last_mut() {
                        hunk.lines.push(DiffLine {
                            kind,
                            old_line: line.old_lineno(),
                            new_line: line.new_lineno(),
                            text: String::from_utf8_lossy(line.content())
                                .trim_end_matches('\n')
                                .to_string(),
                        });
                    }
                }
                true
            }),
        )?;
        Ok(files.into_inner())
    }

    /// The patch a single commit introduces, against its first parent (or the empty tree).
    pub fn commit_patch(&self, oid: Oid) -> Result<String, git2::Error> {
        let commit = self.repo.find_commit(oid)?;
        let parent = commit.parent(0).ok().map(|parent| parent.id());
        match parent {
            Some(parent) => self.diff_patch(parent, oid),
            None => self.diff_patch(self.empty_tree_oid()?, oid),
        }
    }

    /// The blob ID and bytes of `path` as it was in the commit `commit`, if it exists there.
    pub fn blob_at_commit(
        &self,
        commit: Oid,
        path: &str,
    ) -> Result<Option<(Oid, Vec<u8>)>, git2::Error> {
        let tree = self.repo.find_commit(commit)?.tree()?;
        match tree.get_path(std::path::Path::new(path)) {
            Ok(entry) => {
                let blob = self.repo.find_blob(entry.id())?;
                Ok(Some((entry.id(), blob.content().to_vec())))
            }
            Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn tree_diff(&self, from: Oid, to: Oid) -> Result<git2::Diff<'_>, git2::Error> {
        let from_tree = self.repo.find_commit(from)?.tree()?;
        let to_tree = self.repo.find_commit(to)?.tree()?;
        // Fixed prefixes: clients parse `a/` and `b/`, whatever diff.mnemonicPrefix says.
        let mut options = DiffOptions::new();
        options.old_prefix("a/").new_prefix("b/");
        self.repo
            .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut options))
    }

    fn empty_tree_oid(&self) -> Result<Oid, git2::Error> {
        self.repo.treebuilder(None)?.write()
    }

    /// Points `refs/pull/<number>/head` at a commit. Clients fetch this ref to get
    /// a request's head, so it must exist for every open request.
    pub fn set_pull_ref(&self, number: i64, oid: Oid) -> Result<(), git2::Error> {
        self.repo
            .reference(&pull_head_ref(number), oid, true, "Twig pull request head")
            .map(|_| ())
    }

    /// The commit a request's head ref currently names, if the ref exists.
    pub fn pull_ref(&self, number: i64) -> Result<Option<Oid>, git2::Error> {
        match self.repo.find_reference(&pull_head_ref(number)) {
            Ok(reference) => Ok(reference.target()),
            Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
}

fn pull_head_ref(number: i64) -> String {
    format!("refs/pull/{number}/head")
}
