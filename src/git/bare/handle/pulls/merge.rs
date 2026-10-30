//! Building a request's merge in memory: a merge commit, a squash commit, or the
//! request's commits replayed onto the base. Nothing is written to a branch here;
//! `move_branch_if_unchanged` moves the branch only if it still names the expected commit.

use git2::{Commit, ErrorCode, Index, Oid, Signature};

use super::super::RepoHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    Merge,
    Squash,
    Rebase,
}

/// The result of building a merge. `Ready` names the commit the base should move to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeBuild {
    Ready { tip: Oid },
    Conflict { paths: Vec<String> },
}

/// Paths the index holds conflicts for, in path order.
fn conflict_paths(index: &Index) -> Result<Vec<String>, git2::Error> {
    let mut paths = Vec::new();
    for conflict in index.conflicts()? {
        let conflict = conflict?;
        let entry = conflict
            .our
            .or(conflict.their)
            .or(conflict.ancestor)
            .ok_or_else(|| git2::Error::from_str("Conflict without a path"))?;
        paths.push(String::from_utf8_lossy(&entry.path).into_owned());
    }
    paths.sort();
    Ok(paths)
}

impl RepoHandle {
    /// Paths that would conflict if `head` were merged into `base`. Empty means the
    /// merge is clean. Writes nothing.
    pub fn merge_conflicts(&self, base: Oid, head: Oid) -> Result<Vec<String>, git2::Error> {
        let base_commit = self.repo.find_commit(base)?;
        let head_commit = self.repo.find_commit(head)?;
        let index = self.repo.merge_commits(&base_commit, &head_commit, None)?;
        conflict_paths(&index)
    }

    /// Builds the commit a merge or squash would create, or the replayed commits for
    /// a rebase, without moving any branch.
    pub fn build_merge(
        &self,
        strategy: MergeStrategy,
        base: Oid,
        head: Oid,
        committer: &Signature<'_>,
        message: &str,
    ) -> Result<MergeBuild, git2::Error> {
        match strategy {
            MergeStrategy::Merge | MergeStrategy::Squash => {
                let base_commit = self.repo.find_commit(base)?;
                let head_commit = self.repo.find_commit(head)?;
                let mut index = self.repo.merge_commits(&base_commit, &head_commit, None)?;
                if index.has_conflicts() {
                    return Ok(MergeBuild::Conflict {
                        paths: conflict_paths(&index)?,
                    });
                }
                let tree = self.repo.find_tree(index.write_tree_to(&self.repo)?)?;
                let parents: Vec<&Commit<'_>> = match strategy {
                    MergeStrategy::Squash => vec![&base_commit],
                    _ => vec![&base_commit, &head_commit],
                };
                let tip = self
                    .repo
                    .commit(None, committer, committer, message, &tree, &parents)?;
                Ok(MergeBuild::Ready { tip })
            }
            MergeStrategy::Rebase => {
                let mut current = base;
                for id in self.commit_ids_in_range(base, head)? {
                    let picked = self.repo.find_commit(id)?;
                    let onto = self.repo.find_commit(current)?;
                    let mut index: Index = self.repo.cherrypick_commit(&picked, &onto, 0, None)?;
                    if index.has_conflicts() {
                        return Ok(MergeBuild::Conflict {
                            paths: conflict_paths(&index)?,
                        });
                    }
                    let tree = self.repo.find_tree(index.write_tree_to(&self.repo)?)?;
                    // The replayed commit keeps its author; the merger becomes its committer.
                    current = self.repo.commit(
                        None,
                        &picked.author(),
                        committer,
                        picked.message().unwrap_or_default(),
                        &tree,
                        &[&onto],
                    )?;
                }
                Ok(MergeBuild::Ready { tip: current })
            }
        }
    }

    /// Moves `branch` to `new` only if it still points at `expected_old`. Returns
    /// `false` when the branch moved in the meantime, so nothing was overwritten.
    pub fn move_branch_if_unchanged(
        &self,
        branch: &str,
        expected_old: Oid,
        new: Oid,
        reason: &str,
    ) -> Result<bool, git2::Error> {
        let name = format!("refs/heads/{branch}");
        match self
            .repo
            .reference_matching(&name, new, true, expected_old, reason)
        {
            Ok(_) => Ok(true),
            Err(error) if error.code() == ErrorCode::Modified => Ok(false),
            Err(error) => Err(error),
        }
    }
}
