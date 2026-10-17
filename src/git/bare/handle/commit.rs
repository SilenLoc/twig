use std::path::Path;

#[cfg(test)]
use super::CommitFileOutcome;
use super::{CommitChangesOutcome, RepoEditChange, RepoHandle};

use super::super::path::is_safe_repo_path;

impl RepoHandle {
    /// Create a commit replacing one regular file and move the current branch
    /// only if it still points at `expected_head`. `reference_matching` is a
    /// compare-and-swap, so a concurrent push/edit cannot be silently lost.
    /// Test-only compatibility helper for callers predating repository drafts.
    #[cfg(test)]
    pub fn commit_file(
        &self,
        file_path: &str,
        content: &[u8],
        expected_head: git2::Oid,
        author_name: &str,
        author_email: &str,
        message: &str,
    ) -> Result<CommitFileOutcome, git2::Error> {
        use git2::{FileMode, ObjectType, Signature, build::TreeUpdateBuilder};

        if !is_safe_repo_path(file_path) || file_path.is_empty() {
            return Err(git2::Error::from_str("Invalid repository file path"));
        }

        let head = self.repo.head()?;
        let reference_name = head
            .name()
            .ok()
            .filter(|name| name.starts_with("refs/heads/"))
            .ok_or_else(|| git2::Error::from_str("Repository HEAD is not a branch"))?;
        let parent = self.repo.find_commit(expected_head)?;
        let parent_tree = parent.tree()?;
        let entry = parent_tree.get_path(Path::new(file_path))?;
        if entry.kind() != Some(ObjectType::Blob)
            || !matches!(entry.filemode(), 0o100_644 | 0o100_755)
        {
            return Err(git2::Error::from_str(
                "Only regular repository files can be edited",
            ));
        }

        let blob_oid = self.repo.blob(content)?;
        let mut update = TreeUpdateBuilder::new();
        let mode = if entry.filemode() == 0o100_755 {
            FileMode::BlobExecutable
        } else {
            FileMode::Blob
        };
        update.upsert(file_path, blob_oid, mode);
        let tree_oid = update.create_updated(&self.repo, &parent_tree)?;
        let tree = self.repo.find_tree(tree_oid)?;
        let author = Signature::now(author_name, author_email)?;
        let commit_oid = self
            .repo
            .commit(None, &author, &author, message, &tree, &[&parent])?;

        match self.repo.reference_matching(
            reference_name,
            commit_oid,
            true,
            expected_head,
            "Edit file in Twig",
        ) {
            Ok(_) => Ok(CommitFileOutcome::Committed(commit_oid)),
            Err(error) if error.code() == git2::ErrorCode::Modified => {
                Ok(CommitFileOutcome::Conflict)
            }
            Err(error) => Err(error),
        }
    }

    /// Create a new regular file and advance the current branch only if it
    /// still points at `expected_head`. Existing paths are never replaced.
    /// Test-only compatibility helper for callers predating repository drafts.
    #[cfg(test)]
    pub fn commit_new_file(
        &self,
        file_path: &str,
        content: &[u8],
        expected_head: git2::Oid,
        author_name: &str,
        author_email: &str,
        message: &str,
    ) -> Result<CommitFileOutcome, git2::Error> {
        use git2::{FileMode, Signature, build::TreeUpdateBuilder};

        if !is_safe_repo_path(file_path) || file_path.is_empty() {
            return Err(git2::Error::from_str("Invalid repository file path"));
        }

        let head = self.repo.head()?;
        let reference_name = head
            .name()
            .ok()
            .filter(|name| name.starts_with("refs/heads/"))
            .ok_or_else(|| git2::Error::from_str("Repository HEAD is not a branch"))?;
        let parent = self.repo.find_commit(expected_head)?;
        let parent_tree = parent.tree()?;
        match parent_tree.get_path(Path::new(file_path)) {
            Ok(_) => return Ok(CommitFileOutcome::Conflict),
            Err(error) if error.code() == git2::ErrorCode::NotFound => {}
            Err(error) => return Err(error),
        }

        let blob_oid = self.repo.blob(content)?;
        let mut update = TreeUpdateBuilder::new();
        update.upsert(file_path, blob_oid, FileMode::Blob);
        let tree_oid = update.create_updated(&self.repo, &parent_tree)?;
        let tree = self.repo.find_tree(tree_oid)?;
        let author = Signature::now(author_name, author_email)?;
        let commit_oid = self
            .repo
            .commit(None, &author, &author, message, &tree, &[&parent])?;

        match self.repo.reference_matching(
            reference_name,
            commit_oid,
            true,
            expected_head,
            "Save conflict copy in Twig",
        ) {
            Ok(_) => Ok(CommitFileOutcome::Committed(commit_oid)),
            Err(error) if error.code() == git2::ErrorCode::Modified => {
                Ok(CommitFileOutcome::Conflict)
            }
            Err(error) => Err(error),
        }
    }

    /// Apply a complete repository-edit draft as one commit, compare-and-swap
    /// advancing the branch only if it still points at `expected_head`.
    pub fn commit_changes(
        &self,
        changes: &[RepoEditChange],
        expected_head: git2::Oid,
        author_name: &str,
        author_email: &str,
        message: &str,
    ) -> Result<CommitChangesOutcome, git2::Error> {
        if changes.is_empty() {
            return Err(git2::Error::from_str("There are no changes to commit"));
        }

        let reference_name = self.head_branch_reference_name()?;
        let current_head = self.head_oid()?;
        if current_head.unwrap_or(git2::Oid::ZERO_SHA1) != expected_head {
            return Ok(CommitChangesOutcome::Conflict);
        }

        let parent = current_head
            .map(|head| self.repo.find_commit(head))
            .transpose()?;
        let parent_tree = self.parent_tree(parent.as_ref())?;
        let tree_oid = self.create_edit_tree(changes, &parent_tree)?;
        let tree = self.repo.find_tree(tree_oid)?;
        let commit_oid =
            self.create_edit_commit(parent.as_ref(), &tree, author_name, author_email, message)?;
        self.advance_branch(&reference_name, commit_oid, expected_head)
    }

    fn head_branch_reference_name(&self) -> Result<String, git2::Error> {
        self.repo
            .find_reference("HEAD")?
            .symbolic_target()?
            .filter(|name| name.starts_with("refs/heads/"))
            .map(str::to_string)
            .ok_or_else(|| git2::Error::from_str("Repository HEAD is not a branch"))
    }

    fn parent_tree<'repo>(
        &'repo self,
        parent: Option<&git2::Commit<'repo>>,
    ) -> Result<git2::Tree<'repo>, git2::Error> {
        if let Some(parent) = parent {
            parent.tree()
        } else {
            let empty_tree_oid = self.repo.treebuilder(None)?.write()?;
            self.repo.find_tree(empty_tree_oid)
        }
    }

    fn create_edit_commit(
        &self,
        parent: Option<&git2::Commit<'_>>,
        tree: &git2::Tree<'_>,
        author_name: &str,
        author_email: &str,
        message: &str,
    ) -> Result<git2::Oid, git2::Error> {
        let signature = git2::Signature::now(author_name, author_email)?;
        match parent {
            Some(parent) => {
                self.repo
                    .commit(None, &signature, &signature, message, tree, &[parent])
            }
            None => self
                .repo
                .commit(None, &signature, &signature, message, tree, &[]),
        }
    }

    fn advance_branch(
        &self,
        reference_name: &str,
        commit_oid: git2::Oid,
        expected_head: git2::Oid,
    ) -> Result<CommitChangesOutcome, git2::Error> {
        match self.repo.reference_matching(
            reference_name,
            commit_oid,
            true,
            expected_head,
            "Edit repository in Twig",
        ) {
            Ok(_) => Ok(CommitChangesOutcome::Committed(commit_oid)),
            Err(error)
                if matches!(
                    error.code(),
                    git2::ErrorCode::Modified | git2::ErrorCode::NotFound
                ) =>
            {
                Ok(CommitChangesOutcome::Conflict)
            }
            Err(error) => Err(error),
        }
    }

    fn create_edit_tree(
        &self,
        changes: &[RepoEditChange],
        parent_tree: &git2::Tree<'_>,
    ) -> Result<git2::Oid, git2::Error> {
        use std::collections::HashSet;

        use git2::build::TreeUpdateBuilder;

        let mut seen = HashSet::with_capacity(changes.len());
        let mut update = TreeUpdateBuilder::new();
        for change in changes {
            let path = match change {
                RepoEditChange::Upsert { path, .. } | RepoEditChange::Delete { path } => path,
            };
            if path.is_empty() || !is_safe_repo_path(path) || !seen.insert(path) {
                return Err(git2::Error::from_str(
                    "Draft contains an invalid or duplicate repository path",
                ));
            }
            self.apply_repo_edit(&mut update, parent_tree, change)?;
        }

        let tree_oid = update.create_updated(&self.repo, parent_tree)?;
        if tree_oid == parent_tree.id() {
            return Err(git2::Error::from_str("There are no changes to commit"));
        }
        Ok(tree_oid)
    }

    fn apply_repo_edit(
        &self,
        update: &mut git2::build::TreeUpdateBuilder,
        parent_tree: &git2::Tree<'_>,
        change: &RepoEditChange,
    ) -> Result<(), git2::Error> {
        match change {
            RepoEditChange::Upsert { path, content } => {
                self.apply_upsert(update, parent_tree, path, content)?;
            }
            RepoEditChange::Delete { path } => self.apply_delete(update, parent_tree, path)?,
        }
        Ok(())
    }

    fn apply_upsert(
        &self,
        update: &mut git2::build::TreeUpdateBuilder,
        parent_tree: &git2::Tree<'_>,
        path: &str,
        content: &[u8],
    ) -> Result<(), git2::Error> {
        use git2::FileMode;

        if !is_utf8_text(content) {
            return Err(git2::Error::from_str(
                "Workspace edits must contain UTF-8 text without NUL bytes",
            ));
        }
        let mode = match parent_tree.get_path(Path::new(path)) {
            Ok(entry) => self.regular_text_file_mode(
                &entry,
                "Only regular text files can be edited",
                "Only UTF-8 text files can be edited",
            )?,
            Err(error) if error.code() == git2::ErrorCode::NotFound => FileMode::Blob,
            Err(error) => return Err(error),
        };
        let blob_oid = self.repo.blob(content)?;
        update.upsert(path, blob_oid, mode);
        Ok(())
    }

    fn apply_delete(
        &self,
        update: &mut git2::build::TreeUpdateBuilder,
        parent_tree: &git2::Tree<'_>,
        path: &str,
    ) -> Result<(), git2::Error> {
        let entry = parent_tree.get_path(Path::new(path))?;
        self.regular_text_file_mode(
            &entry,
            "Only regular text files can be deleted",
            "Only UTF-8 text files can be deleted",
        )?;
        update.remove(path);
        Ok(())
    }

    fn regular_text_file_mode(
        &self,
        entry: &git2::TreeEntry<'_>,
        invalid_file_message: &'static str,
        invalid_text_message: &'static str,
    ) -> Result<git2::FileMode, git2::Error> {
        use git2::{FileMode, ObjectType};

        if entry.kind() != Some(ObjectType::Blob) {
            return Err(git2::Error::from_str(invalid_file_message));
        }
        let mode = match entry.filemode() {
            0o100_644 => FileMode::Blob,
            0o100_755 => FileMode::BlobExecutable,
            _ => return Err(git2::Error::from_str(invalid_file_message)),
        };
        let object = entry.to_object(&self.repo)?;
        let blob = object
            .as_blob()
            .ok_or_else(|| git2::Error::from_str(invalid_file_message))?;
        if !is_utf8_text(blob.content()) {
            return Err(git2::Error::from_str(invalid_text_message));
        }
        Ok(mode)
    }
}

fn is_utf8_text(content: &[u8]) -> bool {
    !content.contains(&0) && std::str::from_utf8(content).is_ok()
}
