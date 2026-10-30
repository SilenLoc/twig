use git2::{Oid, Repository, Signature};

use super::super::RepoHandle;
use super::pulls::{FileStatus, LineKind, MergeBuild, MergeStrategy};

struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let path = std::env::temp_dir().join(format!("twig-pr-git-{name}-{}", uuid::Uuid::new_v4()));
    Scratch(path)
}

/// Writes one commit that adds `file` to `parent`'s tree and moves `branch` onto it.
fn commit(repo: &Repository, parent: Option<Oid>, branch: &str, file: &str, content: &str) -> Oid {
    let blob = repo.blob(content.as_bytes()).expect("blob");
    let mut builder = match parent {
        Some(parent) => {
            let tree = repo
                .find_commit(parent)
                .expect("parent")
                .tree()
                .expect("tree");
            repo.treebuilder(Some(&tree)).expect("builder")
        }
        None => repo.treebuilder(None).expect("builder"),
    };
    builder.insert(file, blob, 0o100_644).expect("insert");
    let tree = repo
        .find_tree(builder.write().expect("write tree"))
        .expect("tree");
    let signature = Signature::now("Twig Test", "twig@example.com").expect("signature");
    let parents = parent
        .map(|parent| repo.find_commit(parent).expect("parent commit"))
        .into_iter()
        .collect::<Vec<_>>();
    let parent_refs = parents.iter().collect::<Vec<_>>();
    repo.commit(
        Some(&format!("refs/heads/{branch}")),
        &signature,
        &signature,
        &format!("{file} {content}"),
        &tree,
        &parent_refs,
    )
    .expect("commit")
}

fn handle_at(path: &std::path::Path) -> RepoHandle {
    RepoHandle::from_repository(Repository::open_bare(path).expect("open bare"))
}

#[test]
fn repo_id_is_generated_once_and_persisted() {
    let dir = scratch("id");
    let repo = Repository::init_bare(&dir.0).expect("init bare");
    let first = handle_at(&dir.0).repo_id().expect("first id");
    let second = handle_at(&dir.0).repo_id().expect("second id");
    assert_eq!(first, second, "a repository keeps its identifier");
    assert!(
        uuid::Uuid::parse_str(&first).is_ok(),
        "the identifier is a UUID"
    );
    drop(repo);
}

#[test]
fn repo_id_survives_moving_the_repository_directory() {
    let dir = scratch("move");
    Repository::init_bare(&dir.0).expect("init bare");
    let before = handle_at(&dir.0).repo_id().expect("id");

    let moved = dir.0.with_extension("moved");
    std::fs::rename(&dir.0, &moved).expect("move directory");
    let _cleanup = Scratch(moved.clone());

    assert_eq!(handle_at(&moved).repo_id().expect("id after move"), before);
}

#[test]
fn commit_ids_in_range_lists_only_head_commits_oldest_first() {
    let dir = scratch("range");
    let repo = Repository::init_bare(&dir.0).expect("init bare");
    let base_root = commit(&repo, None, "main", "README.md", "root");
    let feature_one = commit(&repo, Some(base_root), "feature", "one.txt", "1");
    let feature_two = commit(&repo, Some(feature_one), "feature", "two.txt", "2");
    // Main moves on after the branch was cut; that history is not part of the request.
    let main_next = commit(&repo, Some(base_root), "main", "later.txt", "later");

    let handle = handle_at(&dir.0);
    let ids = handle
        .commit_ids_in_range(main_next, feature_two)
        .expect("range");
    assert_eq!(ids, vec![feature_one, feature_two]);
    assert_eq!(
        handle
            .merge_base(main_next, feature_two)
            .expect("merge base"),
        Some(base_root)
    );
}

#[test]
fn commit_ids_in_range_is_empty_when_head_is_already_in_base() {
    let dir = scratch("empty");
    let repo = Repository::init_bare(&dir.0).expect("init bare");
    let root = commit(&repo, None, "main", "README.md", "root");
    let next = commit(&repo, Some(root), "main", "next.txt", "next");

    let ids = handle_at(&dir.0)
        .commit_ids_in_range(next, root)
        .expect("range");
    assert!(ids.is_empty());
}

#[test]
fn diff_patch_contains_only_head_changes_from_merge_base() {
    let dir = scratch("patch");
    let repo = Repository::init_bare(&dir.0).expect("init bare");
    let root = commit(&repo, None, "main", "README.md", "root");
    let feature = commit(&repo, Some(root), "feature", "one.txt", "1");
    let base_only = commit(&repo, Some(root), "main", "later.txt", "later");

    let handle = handle_at(&dir.0);
    let patch = handle.diff_patch(root, feature).expect("patch");
    assert!(patch.contains("diff --git a/one.txt b/one.txt"), "{patch}");
    assert!(
        !patch.contains("later.txt"),
        "base-only files are excluded: {patch}"
    );

    let merge_base = handle
        .merge_base(base_only, feature)
        .expect("merge base")
        .expect("base exists");
    assert_eq!(merge_base, root);
    let to_head = handle
        .diff_patch(merge_base, feature)
        .expect("patch from merge base");
    assert!(to_head.contains("+++ b/one.txt"));
}

#[test]
fn diff_files_reports_status_counts_and_line_numbers() {
    let dir = scratch("files");
    let repo = Repository::init_bare(&dir.0).expect("init bare");
    let root = commit(&repo, None, "main", "notes.txt", "keep\nold\n");
    let edited = commit(
        &repo,
        Some(root),
        "feature",
        "notes.txt",
        "keep\nnew\nextra\n",
    );

    let files = handle_at(&dir.0).diff_files(root, edited).expect("files");
    assert_eq!(files.len(), 1);
    let file = &files[0];
    assert_eq!(file.path(), "notes.txt");
    assert_eq!(file.status, FileStatus::Modified);
    assert_eq!((file.additions, file.deletions), (2, 1));
    let lines = &file.hunks[0].lines;
    let removed = lines
        .iter()
        .find(|line| line.kind == LineKind::Removed)
        .unwrap();
    assert_eq!(
        (removed.old_line, removed.new_line, removed.text.as_str()),
        (Some(2), None, "old")
    );
    let added = lines
        .iter()
        .find(|line| line.kind == LineKind::Added && line.text == "new")
        .unwrap();
    assert_eq!((added.old_line, added.new_line), (None, Some(2)));
}

#[test]
fn commit_patch_and_blob_at_commit_read_one_commit() {
    let dir = scratch("commit");
    let repo = Repository::init_bare(&dir.0).expect("init bare");
    let root = commit(&repo, None, "main", "a.txt", "a");
    let change = commit(&repo, Some(root), "feature", "b.txt", "b");

    let handle = handle_at(&dir.0);
    let patch = handle.commit_patch(change).expect("commit patch");
    assert!(patch.contains("diff --git a/b.txt b/b.txt"));
    assert!(!patch.contains("a.txt"));
    let (_, bytes) = handle
        .blob_at_commit(change, "b.txt")
        .expect("blob")
        .expect("present");
    assert_eq!(bytes, b"b".to_vec());
    assert!(
        handle
            .blob_at_commit(change, "missing.txt")
            .expect("blob")
            .is_none()
    );
    let summary = handle.commit_summary(change).expect("summary");
    assert_eq!(summary.parents, vec![root]);
    assert_eq!(summary.author_email, "twig@example.com");
}

#[test]
fn merge_conflicts_name_only_the_files_both_sides_changed() {
    let dir = scratch("conflicts");
    let repo = Repository::init_bare(&dir.0).expect("init bare");
    let root = commit(&repo, None, "main", "shared.txt", "start");
    let main_side = commit(&repo, Some(root), "main", "shared.txt", "main side");
    let feature = commit(&repo, Some(root), "feature", "shared.txt", "feature side");
    let clean = commit(&repo, Some(root), "feature-clean", "other.txt", "other");

    let handle = handle_at(&dir.0);
    assert_eq!(
        handle
            .merge_conflicts(main_side, feature)
            .expect("conflicts"),
        vec!["shared.txt".to_string()]
    );
    assert!(
        handle
            .merge_conflicts(main_side, clean)
            .expect("clean")
            .is_empty()
    );
}

#[test]
fn merge_build_records_both_parents_and_squash_records_one() {
    let dir = scratch("merge-build");
    let repo = Repository::init_bare(&dir.0).expect("init bare");
    let root = commit(&repo, None, "main", "README.md", "root");
    let base = commit(&repo, Some(root), "main", "base.txt", "base");
    let head = commit(&repo, Some(root), "feature", "feature.txt", "feature");
    let signature = Signature::now("Merger", "merger@example.com").expect("signature");
    let handle = handle_at(&dir.0);

    let merged = handle
        .build_merge(MergeStrategy::Merge, base, head, &signature, "Merge it")
        .expect("merge");
    let MergeBuild::Ready { tip } = merged else {
        panic!("clean merge expected")
    };
    let merge_commit = repo.find_commit(tip).expect("merge commit");
    assert_eq!(
        merge_commit.parent_ids().collect::<Vec<_>>(),
        vec![base, head]
    );
    assert!(
        merge_commit
            .tree()
            .unwrap()
            .get_path(std::path::Path::new("feature.txt"))
            .is_ok()
    );
    assert!(
        merge_commit
            .tree()
            .unwrap()
            .get_path(std::path::Path::new("base.txt"))
            .is_ok()
    );

    let squashed = handle
        .build_merge(MergeStrategy::Squash, base, head, &signature, "Squash it")
        .expect("squash");
    let MergeBuild::Ready { tip } = squashed else {
        panic!("clean squash expected")
    };
    let squash_commit = repo.find_commit(tip).expect("squash commit");
    assert_eq!(squash_commit.parent_ids().collect::<Vec<_>>(), vec![base]);
    assert_eq!(squash_commit.message().unwrap(), "Squash it");
    assert_eq!(
        squash_commit.tree_id(),
        merge_commit.tree_id(),
        "same content either way"
    );
}

#[test]
fn rebase_build_replays_head_commits_onto_the_base_in_order() {
    let dir = scratch("rebase-build");
    let repo = Repository::init_bare(&dir.0).expect("init bare");
    let root = commit(&repo, None, "main", "README.md", "root");
    let base = commit(&repo, Some(root), "main", "base.txt", "base");
    let one = commit(&repo, Some(root), "feature", "one.txt", "one");
    let two = commit(&repo, Some(one), "feature", "two.txt", "two");
    let signature = Signature::now("Merger", "merger@example.com").expect("signature");

    let MergeBuild::Ready { tip } = handle_at(&dir.0)
        .build_merge(MergeStrategy::Rebase, base, two, &signature, "unused")
        .expect("rebase")
    else {
        panic!("clean rebase expected");
    };
    let replayed_two = repo.find_commit(tip).expect("replayed tip");
    let replayed_one = replayed_two.parent(0).expect("replayed first");
    assert_eq!(
        replayed_one.parent_id(0).unwrap(),
        base,
        "replay starts from the base"
    );
    assert_eq!(replayed_one.summary().unwrap(), Some("one.txt one"));
    assert_eq!(replayed_two.summary().unwrap(), Some("two.txt two"));
    assert!(
        replayed_two
            .tree()
            .unwrap()
            .get_path(std::path::Path::new("base.txt"))
            .is_ok()
    );
}

#[test]
fn branch_move_is_compare_and_swap() {
    let dir = scratch("cas");
    let repo = Repository::init_bare(&dir.0).expect("init bare");
    let root = commit(&repo, None, "main", "README.md", "root");
    let moved = commit(&repo, Some(root), "main", "later.txt", "later");
    let other = commit(&repo, Some(root), "feature", "x.txt", "x");

    let handle = handle_at(&dir.0);
    assert!(
        !handle
            .move_branch_if_unchanged("main", root, other, "stale")
            .expect("stale"),
        "main already moved"
    );
    assert_eq!(
        repo.find_reference("refs/heads/main").unwrap().target(),
        Some(moved)
    );
    assert!(
        handle
            .move_branch_if_unchanged("main", moved, other, "fresh")
            .expect("fresh")
    );
    assert_eq!(
        repo.find_reference("refs/heads/main").unwrap().target(),
        Some(other)
    );
}
