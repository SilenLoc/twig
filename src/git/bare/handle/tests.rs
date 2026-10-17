use super::*;
use std::path::PathBuf;

struct TempRepo(PathBuf);

impl TempRepo {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("twig_editor_repo_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(path.join("ns/repo")).unwrap();
        Self(path)
    }

    fn handle(&self) -> RepoHandle {
        let path = self.0.join("ns/repo");
        let repo = git2::Repository::init_bare(path).unwrap();
        let blob = repo.blob(b"before\n").unwrap();
        {
            let mut root = repo.treebuilder(None).unwrap();
            root.insert("README.md", blob, 0o100_644).unwrap();
            let mut nested = repo.treebuilder(None).unwrap();
            let nested_blob = repo.blob(b"nested before\n").unwrap();
            nested.insert("file.txt", nested_blob, 0o100_755).unwrap();
            let nested_tree = nested.write().unwrap();
            root.insert("nested", nested_tree, 0o040_000).unwrap();
            let tree_oid = root.write().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            let signature = git2::Signature::now("Seed", "seed@example.com").unwrap();
            repo.commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "seed",
                &tree,
                &[],
            )
            .unwrap();
            repo.set_head("refs/heads/main").unwrap();
        }
        RepoHandle::open(self.0.to_str().unwrap(), "ns", "repo").unwrap()
    }

    fn empty_handle(&self) -> RepoHandle {
        let path = self.0.join("ns/repo");
        let repo = git2::Repository::init_bare(path).unwrap();
        repo.set_head("refs/heads/main").unwrap();
        RepoHandle::open(self.0.to_str().unwrap(), "ns", "repo").unwrap()
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn commit_file_writes_nested_content_and_preserves_other_entries_and_mode() {
    let temp = TempRepo::new();
    let handle = temp.handle();
    let expected = handle.head_oid().unwrap().unwrap();

    let outcome = handle
        .commit_file(
            "nested/file.txt",
            b"after\n",
            expected,
            "Editor",
            "editor@example.com",
            "Update nested file",
        )
        .unwrap();
    let CommitFileOutcome::Committed(oid) = outcome else {
        panic!("the unchanged branch should accept the edit");
    };

    assert_eq!(
        handle
            .read_blob_bytes("nested/file.txt")
            .unwrap()
            .as_deref(),
        Some(&b"after\n"[..])
    );
    assert_eq!(
        handle.read_blob_bytes("README.md").unwrap().as_deref(),
        Some(&b"before\n"[..])
    );
    let repo = git2::Repository::open(temp.0.join("ns/repo")).unwrap();
    let commit = repo.find_commit(oid).unwrap();
    assert_eq!(commit.author().name().unwrap(), "Editor");
    assert_eq!(commit.author().email().unwrap(), "editor@example.com");
    let entry = commit
        .tree()
        .unwrap()
        .get_path(Path::new("nested/file.txt"))
        .unwrap();
    assert_eq!(entry.filemode(), 0o100_755);
}

#[test]
fn commit_file_refuses_to_overwrite_a_newer_branch_tip() {
    let temp = TempRepo::new();
    let handle = temp.handle();
    let original = handle.head_oid().unwrap().unwrap();
    let first = handle
        .commit_file(
            "README.md",
            b"first edit\n",
            original,
            "Editor",
            "editor@example.com",
            "First edit",
        )
        .unwrap();
    let CommitFileOutcome::Committed(first_oid) = first else {
        panic!("the first edit should commit");
    };

    let second = handle
        .commit_file(
            "README.md",
            b"stale edit\n",
            original,
            "Editor",
            "editor@example.com",
            "Stale edit",
        )
        .unwrap();
    assert!(matches!(second, CommitFileOutcome::Conflict));
    assert_eq!(handle.head_oid().unwrap(), Some(first_oid));
    assert_eq!(
        handle.read_blob_bytes("README.md").unwrap().as_deref(),
        Some(&b"first edit\n"[..])
    );
}

#[test]
fn commit_new_file_adds_a_nested_path_and_refuses_overwrites_and_stale_heads() {
    let temp = TempRepo::new();
    let handle = temp.handle();
    let original = handle.head_oid().unwrap().unwrap();

    let created = handle
        .commit_new_file(
            "docs/alice-20261010-123456-README.md",
            b"draft copy\n",
            original,
            "Editor",
            "editor@example.com",
            "Save conflict copy",
        )
        .unwrap();
    let CommitFileOutcome::Committed(copy_oid) = created else {
        panic!("the copy should commit against an unchanged branch");
    };
    assert_eq!(
        handle
            .read_blob_bytes("docs/alice-20261010-123456-README.md")
            .unwrap()
            .as_deref(),
        Some(&b"draft copy\n"[..])
    );
    assert_eq!(
        handle.read_blob_bytes("README.md").unwrap().as_deref(),
        Some(&b"before\n"[..])
    );
    assert!(matches!(
        handle
            .commit_new_file(
                "docs/alice-20261010-123456-README.md",
                b"must not replace\n",
                copy_oid,
                "Editor",
                "editor@example.com",
                "Duplicate copy",
            )
            .unwrap(),
        CommitFileOutcome::Conflict
    ));

    let moved = handle
        .commit_file(
            "README.md",
            b"concurrent update\n",
            copy_oid,
            "Other",
            "other@example.com",
            "Concurrent change",
        )
        .unwrap();
    let CommitFileOutcome::Committed(moved_oid) = moved else {
        panic!("the concurrent update should commit");
    };
    assert!(matches!(
        handle
            .commit_new_file(
                "docs/stale-copy.md",
                b"stale draft\n",
                copy_oid,
                "Editor",
                "editor@example.com",
                "Stale copy",
            )
            .unwrap(),
        CommitFileOutcome::Conflict
    ));
    assert_eq!(handle.head_oid().unwrap(), Some(moved_oid));
    assert_eq!(handle.read_blob_bytes("docs/stale-copy.md").unwrap(), None);
    assert_eq!(
        handle
            .read_blob_bytes("docs/alice-20261010-123456-README.md")
            .unwrap()
            .as_deref(),
        Some(&b"draft copy\n"[..])
    );
}

#[test]
fn commit_changes_applies_multiple_file_operations_in_one_commit() {
    let temp = TempRepo::new();
    let handle = temp.handle();
    let base = handle.head_oid().unwrap().unwrap();
    let changes = [
        RepoEditChange::Delete {
            path: "README.md".to_string(),
        },
        RepoEditChange::Upsert {
            path: "docs/intro.md".to_string(),
            content: b"# Renamed README\n".to_vec(),
        },
        RepoEditChange::Upsert {
            path: "nested/file.txt".to_string(),
            content: b"nested after\n".to_vec(),
        },
        RepoEditChange::Upsert {
            path: "notes/new.md".to_string(),
            content: b"new note\n".to_vec(),
        },
    ];

    let outcome = handle
        .commit_changes(
            &changes,
            base,
            "Editor",
            "editor@example.com",
            "Update repository notes",
        )
        .unwrap();
    let CommitChangesOutcome::Committed(oid) = outcome else {
        panic!("the unchanged branch should accept the complete edit");
    };

    assert_eq!(handle.head_oid().unwrap(), Some(oid));
    assert_eq!(handle.read_blob_bytes("README.md").unwrap(), None);
    assert_eq!(
        handle.read_blob_bytes("docs/intro.md").unwrap().as_deref(),
        Some(&b"# Renamed README\n"[..])
    );
    assert_eq!(
        handle
            .read_blob_bytes("nested/file.txt")
            .unwrap()
            .as_deref(),
        Some(&b"nested after\n"[..])
    );
    assert_eq!(
        handle.read_blob_bytes("notes/new.md").unwrap().as_deref(),
        Some(&b"new note\n"[..])
    );

    let repo = git2::Repository::open(temp.0.join("ns/repo")).unwrap();
    let commit = repo.find_commit(oid).unwrap();
    assert_eq!(commit.parent_id(0).unwrap(), base);
    assert_eq!(commit.message().unwrap(), "Update repository notes");
    assert_eq!(commit.author().name().unwrap(), "Editor");
    assert_eq!(
        commit
            .tree()
            .unwrap()
            .get_path(Path::new("nested/file.txt"))
            .unwrap()
            .filemode(),
        0o100_755,
        "editing text retains its executable bit"
    );
    assert_eq!(handle.get_commits(3).unwrap().len(), 2);
}

#[test]
fn commit_changes_rejects_stale_head_and_invalid_or_no_op_batches() {
    let temp = TempRepo::new();
    let handle = temp.handle();
    let base = handle.head_oid().unwrap().unwrap();

    assert!(
        handle
            .commit_changes(
                &[RepoEditChange::Upsert {
                    path: "README.md".to_string(),
                    content: b"before\n".to_vec(),
                }],
                base,
                "Editor",
                "editor@example.com",
                "No-op edit",
            )
            .is_err()
    );
    assert!(
        handle
            .commit_changes(
                &[
                    RepoEditChange::Upsert {
                        path: "notes.md".to_string(),
                        content: b"one\n".to_vec(),
                    },
                    RepoEditChange::Delete {
                        path: "notes.md".to_string(),
                    },
                ],
                base,
                "Editor",
                "editor@example.com",
                "Duplicate path",
            )
            .is_err()
    );
    assert!(
        handle
            .commit_changes(
                &[RepoEditChange::Upsert {
                    path: "../escape.txt".to_string(),
                    content: b"unsafe\n".to_vec(),
                }],
                base,
                "Editor",
                "editor@example.com",
                "Unsafe path",
            )
            .is_err()
    );
    assert_eq!(handle.head_oid().unwrap(), Some(base));

    let concurrent = handle
        .commit_file(
            "README.md",
            b"concurrent update\n",
            base,
            "Other editor",
            "other@example.com",
            "Concurrent update",
        )
        .unwrap();
    let CommitFileOutcome::Committed(concurrent_oid) = concurrent else {
        panic!("the concurrent update should commit");
    };
    let stale = handle
        .commit_changes(
            &[RepoEditChange::Upsert {
                path: "README.md".to_string(),
                content: b"stale update\n".to_vec(),
            }],
            base,
            "Editor",
            "editor@example.com",
            "Stale update",
        )
        .unwrap();
    assert!(matches!(stale, CommitChangesOutcome::Conflict));
    assert_eq!(handle.head_oid().unwrap(), Some(concurrent_oid));
    assert_eq!(
        handle.read_blob_bytes("README.md").unwrap().as_deref(),
        Some(&b"concurrent update\n"[..])
    );
}

#[test]
fn commit_changes_can_create_the_first_commit_in_an_empty_repository() {
    let temp = TempRepo::new();
    let handle = temp.empty_handle();
    assert_eq!(handle.head_oid().unwrap(), None);

    let outcome = handle
        .commit_changes(
            &[RepoEditChange::Upsert {
                path: "README.md".to_string(),
                content: b"First file\n".to_vec(),
            }],
            git2::Oid::ZERO_SHA1,
            "Editor",
            "editor@example.com",
            "Initial web edit",
        )
        .unwrap();
    let CommitChangesOutcome::Committed(oid) = outcome else {
        panic!("the unborn branch should accept its first draft");
    };

    assert_eq!(handle.head_oid().unwrap(), Some(oid));
    assert_eq!(
        handle.read_blob_bytes("README.md").unwrap().as_deref(),
        Some(&b"First file\n"[..])
    );
    let repo = git2::Repository::open(temp.0.join("ns/repo")).unwrap();
    let commit = repo.find_commit(oid).unwrap();
    assert_eq!(commit.parent_count(), 0);
}
