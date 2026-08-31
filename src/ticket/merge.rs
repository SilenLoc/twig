//! Schema-aware three-way merge for tickets.
//!
//! Textual merge drivers cannot do this job: `merge=union` on a TOML file
//! happily produces duplicate keys and, on a modify/delete, silently drops a
//! ticket. Because the schema is ours, every field has a defined resolution
//! instead, which is what lets concurrent writers be reconciled without ever
//! surfacing a conflict.
//!
//! Convention throughout: `base` is the merge-base version, `ours` is the
//! canonical server state, `theirs` is the incoming submission. `None` on any
//! side means the file is absent from that tree.

use std::collections::{HashMap, HashSet};

use crate::ticket::model::{Comment, Link, Ticket};

/// Merges one ticket path. Returns `None` when the path is absent from the
/// canonical result.
pub fn merge3(
    base: Option<&Ticket>,
    ours: Option<&Ticket>,
    theirs: Option<&Ticket>,
) -> Option<Ticket> {
    match (base, ours, theirs) {
        // Untouched or never existed.
        (_, None, None) => None,

        // Created by the submission.
        (None, None, Some(theirs)) => Some(theirs.clone()),

        // Present only in canonical: created there after the merge base, so the
        // submission never saw it. Not a deletion.
        (None, Some(ours), None) => Some(ours.clone()),

        // Add/add: both sides created the path independently.
        (None, Some(ours), Some(theirs)) => Some(merge_fields(&Ticket::default(), ours, theirs)),

        // Deleted by the submission.
        (Some(base), Some(ours), None) => {
            if ours == base {
                // Nothing changed canonically: the deletion stands.
                None
            } else {
                // Retention wins: a concurrent modification proves the ticket is
                // still in use, and the deleting user can simply delete again.
                Some(ours.clone())
            }
        }

        // Deleted canonically while the submission still has it.
        (Some(base), None, Some(theirs)) => {
            if theirs == base {
                // The submitter did not touch it; respect the deletion.
                None
            } else {
                let mut revived = theirs.clone();
                restore_owned_fields(&mut revived, base);
                Some(revived)
            }
        }

        // The ordinary case.
        (Some(base), Some(ours), Some(theirs)) => Some(merge_fields(base, ours, theirs)),
    }
}

/// Field-by-field resolution for a ticket present on both sides.
fn merge_fields(base: &Ticket, ours: &Ticket, theirs: &Ticket) -> Ticket {
    Ticket {
        // Server-owned and immutable: whatever the submission claims is discarded.
        uuid: pick_owned(ours.uuid.as_ref(), base.uuid.as_ref(), theirs.uuid.as_ref()),
        number: pick_owned(
            ours.number.as_ref(),
            base.number.as_ref(),
            theirs.number.as_ref(),
        ),
        namespace: pick_owned(
            ours.namespace.as_ref(),
            base.namespace.as_ref(),
            theirs.namespace.as_ref(),
        ),
        author: pick_owned(
            ours.author.as_ref(),
            base.author.as_ref(),
            theirs.author.as_ref(),
        ),
        created_at: pick_owned(
            ours.created_at.as_ref(),
            base.created_at.as_ref(),
            theirs.created_at.as_ref(),
        ),

        // Stamped by the ingest pipeline after the merge.
        updated_at: ours.updated_at.clone(),

        // Scalars: an incoming change wins, otherwise canonical stands.
        title: scalar(&base.title, &ours.title, &theirs.title),
        status: scalar(&base.status, &ours.status, &theirs.status),
        body: scalar(&base.body, &ours.body, &theirs.body),

        // Sets: additions union, removals apply.
        labels: merge_set(&base.labels, &ours.labels, &theirs.labels, Clone::clone),
        assignees: merge_set(
            &base.assignees,
            &ours.assignees,
            &theirs.assignees,
            Clone::clone,
        ),
        links: merge_set(&base.links, &ours.links, &theirs.links, Link::key),

        comments: merge_comments(&base.comments, &ours.comments, &theirs.comments),
    }
}

/// Copies server-owned fields from `source` into `target`, used when reviving a
/// ticket so a client cannot reassign its identity by deleting and re-adding it.
fn restore_owned_fields(target: &mut Ticket, source: &Ticket) {
    target.uuid.clone_from(&source.uuid);
    target.number = source.number;
    target.namespace.clone_from(&source.namespace);
    target.author.clone_from(&source.author);
    target.created_at.clone_from(&source.created_at);
}

fn pick_owned<T: Clone>(ours: Option<&T>, base: Option<&T>, theirs: Option<&T>) -> Option<T> {
    ours.or(base).or(theirs).cloned()
}

/// An incoming value that differs from the merge base is an intent to change;
/// otherwise the canonical value stands. Competing intents resolve toward the
/// submission, which by ingest ordering is the later arrival.
fn scalar<T: Clone + PartialEq>(base: &T, ours: &T, theirs: &T) -> T {
    if theirs == base {
        ours.clone()
    } else {
        theirs.clone()
    }
}

/// Applies the submission's additions and removals to the canonical set.
fn merge_set<T, K, F>(base: &[T], ours: &[T], theirs: &[T], key: F) -> Vec<T>
where
    T: Clone,
    K: Eq + std::hash::Hash,
    F: Fn(&T) -> K,
{
    let base_keys: HashSet<K> = base.iter().map(&key).collect();
    let theirs_keys: HashSet<K> = theirs.iter().map(&key).collect();

    let mut merged = Vec::new();
    let mut seen: HashSet<K> = HashSet::new();

    for item in ours {
        let k = key(item);
        // Present at the base but gone from the submission: an explicit removal.
        if base_keys.contains(&k) && !theirs_keys.contains(&k) {
            continue;
        }
        if seen.insert(k) {
            merged.push(item.clone());
        }
    }

    for item in theirs {
        let k = key(item);
        // Only genuine additions; anything already at the base was handled above.
        if base_keys.contains(&k) {
            continue;
        }
        if seen.insert(k) {
            merged.push(item.clone());
        }
    }

    merged
}

/// Comments are unioned by id. An edit to an existing comment is last-write-wins;
/// a comment missing from the submission but present at the base was deleted.
fn merge_comments(base: &[Comment], ours: &[Comment], theirs: &[Comment]) -> Vec<Comment> {
    let base_by_id = by_id(base);
    let theirs_by_id = by_id(theirs);

    let mut merged: Vec<Comment> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for comment in ours {
        let Some(id) = comment.id.clone() else {
            merged.push(comment.clone());
            continue;
        };

        if base_by_id.contains_key(id.as_str()) && !theirs_by_id.contains_key(id.as_str()) {
            continue; // deleted by the submission
        }

        let mut resolved = comment.clone();
        if let (Some(based), Some(theirs)) =
            (base_by_id.get(id.as_str()), theirs_by_id.get(id.as_str()))
            && theirs.markdown != based.markdown
        {
            resolved.markdown.clone_from(&theirs.markdown);
        }

        if seen.insert(id) {
            merged.push(resolved);
        }
    }

    for comment in theirs {
        match comment.id.clone() {
            // A comment without an id was authored in this submission. Ingest
            // stamps it before the merge runs, so this is only a safety net.
            None => merged.push(comment.clone()),
            Some(id) => {
                if base_by_id.contains_key(id.as_str()) {
                    continue;
                }
                if seen.insert(id) {
                    merged.push(comment.clone());
                }
            }
        }
    }

    merged.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
    merged
}

fn by_id(comments: &[Comment]) -> HashMap<&str, &Comment> {
    comments
        .iter()
        .filter_map(|c| c.id.as_deref().map(|id| (id, c)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ticket::model::{Body, Status};

    fn comment(id: &str, author: &str, at: &str, markdown: &str) -> Comment {
        Comment {
            id: Some(id.to_string()),
            author: Some(author.to_string()),
            created_at: Some(at.to_string()),
            markdown: markdown.to_string(),
        }
    }

    fn base_ticket() -> Ticket {
        Ticket {
            uuid: Some("uuid-1".to_string()),
            number: Some(7),
            namespace: Some("acme".to_string()),
            title: "Original title".to_string(),
            status: Status::Open,
            author: Some("alice".to_string()),
            created_at: Some("2026-01-01T00:00:00Z".to_string()),
            updated_at: Some("2026-01-01T00:00:00Z".to_string()),
            labels: vec!["bug".to_string()],
            assignees: vec![],
            body: Body {
                markdown: "original body".to_string(),
            },
            links: vec![],
            comments: vec![comment("c1", "alice", "2026-01-01T01:00:00Z", "first")],
        }
    }

    // ---- field-level rules ----

    #[test]
    fn test_immutable_fields_reset_to_ours() {
        let base = base_ticket();
        let ours = base.clone();
        let mut theirs = base.clone();
        // A CLI user can write anything into these; git authorship is self-attested.
        theirs.uuid = Some("forged-uuid".to_string());
        theirs.number = Some(999);
        theirs.namespace = Some("other-namespace".to_string());
        theirs.author = Some("mallory".to_string());
        theirs.created_at = Some("1999-01-01T00:00:00Z".to_string());

        let merged = merge3(Some(&base), Some(&ours), Some(&theirs)).expect("kept");

        assert_eq!(merged.uuid, base.uuid);
        assert_eq!(merged.number, base.number);
        assert_eq!(merged.namespace, base.namespace);
        assert_eq!(merged.author, base.author, "author must not be forgeable");
        assert_eq!(merged.created_at, base.created_at);
    }

    #[test]
    fn test_scalar_changed_only_in_theirs_applies() {
        let base = base_ticket();
        let ours = base.clone();
        let mut theirs = base.clone();
        theirs.title = "Submitted title".to_string();
        theirs.status = Status::Closed;
        theirs.body = Body {
            markdown: "submitted body".to_string(),
        };

        let merged = merge3(Some(&base), Some(&ours), Some(&theirs)).expect("kept");

        assert_eq!(merged.title, "Submitted title");
        assert_eq!(merged.status, Status::Closed);
        assert_eq!(merged.body.markdown, "submitted body");
    }

    #[test]
    fn test_scalar_changed_in_both_theirs_wins() {
        let base = base_ticket();
        let mut ours = base.clone();
        ours.title = "Canonical title".to_string();
        ours.status = Status::Blocked;
        let mut theirs = base.clone();
        theirs.title = "Submitted title".to_string();
        theirs.status = Status::Closed;

        let merged = merge3(Some(&base), Some(&ours), Some(&theirs)).expect("kept");

        // Server arrival order decides; the submission is the later arrival.
        assert_eq!(merged.title, "Submitted title");
        assert_eq!(merged.status, Status::Closed);
    }

    #[test]
    fn test_scalar_unchanged_keeps_ours() {
        let base = base_ticket();
        let mut ours = base.clone();
        ours.title = "Canonical title".to_string();
        ours.status = Status::InProgress;
        let theirs = base.clone(); // submitter touched something else entirely

        let merged = merge3(Some(&base), Some(&ours), Some(&theirs)).expect("kept");

        assert_eq!(merged.title, "Canonical title");
        assert_eq!(merged.status, Status::InProgress);
    }

    #[test]
    fn test_labels_add_and_remove_are_set_ops() {
        let base = base_ticket(); // labels: ["bug"]
        let mut ours = base.clone();
        ours.labels = vec!["bug".to_string(), "ui".to_string()]; // canonical added "ui"
        let mut theirs = base.clone();
        theirs.labels = vec!["auth".to_string()]; // removed "bug", added "auth"

        let merged = merge3(Some(&base), Some(&ours), Some(&theirs)).expect("kept");

        assert!(
            !merged.labels.contains(&"bug".to_string()),
            "removal applies"
        );
        assert!(
            merged.labels.contains(&"ui".to_string()),
            "concurrent addition survives"
        );
        assert!(
            merged.labels.contains(&"auth".to_string()),
            "addition applies"
        );
    }

    #[test]
    fn test_assignees_disjoint_adds_union() {
        let base = base_ticket();
        let mut ours = base.clone();
        ours.assignees = vec!["bob".to_string()];
        let mut theirs = base.clone();
        theirs.assignees = vec!["carol".to_string()];

        let merged = merge3(Some(&base), Some(&ours), Some(&theirs)).expect("kept");

        assert!(merged.assignees.contains(&"bob".to_string()));
        assert!(merged.assignees.contains(&"carol".to_string()));
        assert_eq!(merged.assignees.len(), 2);
    }

    #[test]
    fn test_links_union_by_repo_branch_pair() {
        let base = base_ticket();
        let mut ours = base.clone();
        ours.links = vec![Link {
            repo: "fig".to_string(),
            branch: Some("main".to_string()),
        }];
        let mut theirs = base.clone();
        theirs.links = vec![
            Link {
                repo: "fig".to_string(),
                branch: Some("feature".to_string()),
            },
            Link {
                repo: "docs".to_string(),
                branch: None,
            },
        ];

        let merged = merge3(Some(&base), Some(&ours), Some(&theirs)).expect("kept");

        assert_eq!(
            merged.links.len(),
            3,
            "same repo, different branch is distinct"
        );
        assert!(
            merged
                .links
                .iter()
                .any(|l| l.key() == ("fig".to_string(), Some("main".to_string())))
        );
        assert!(
            merged
                .links
                .iter()
                .any(|l| l.key() == ("fig".to_string(), Some("feature".to_string())))
        );
        assert!(
            merged
                .links
                .iter()
                .any(|l| l.key() == ("docs".to_string(), None))
        );
    }

    #[test]
    fn test_comments_union_by_id() {
        let base = base_ticket();
        let mut ours = base.clone();
        ours.comments
            .push(comment("c2", "bob", "2026-01-01T02:00:00Z", "canonical"));
        let mut theirs = base.clone();
        theirs
            .comments
            .push(comment("c3", "carol", "2026-01-01T03:00:00Z", "submitted"));

        let merged = merge3(Some(&base), Some(&ours), Some(&theirs)).expect("kept");

        let ids: Vec<_> = merged
            .comments
            .iter()
            .filter_map(|c| c.id.clone())
            .collect();
        assert_eq!(ids, vec!["c1", "c2", "c3"], "union, ordered by created_at");
    }

    #[test]
    fn test_comment_body_edit_last_write_wins() {
        let base = base_ticket();
        let mut ours = base.clone();
        ours.comments[0].markdown = "canonical edit".to_string();
        let mut theirs = base.clone();
        theirs.comments[0].markdown = "submitted edit".to_string();

        let merged = merge3(Some(&base), Some(&ours), Some(&theirs)).expect("kept");

        assert_eq!(merged.comments[0].markdown, "submitted edit");
    }

    #[test]
    fn test_comment_removed_in_theirs_is_deleted() {
        let base = base_ticket();
        let ours = base.clone();
        let mut theirs = base.clone();
        theirs.comments.clear();

        let merged = merge3(Some(&base), Some(&ours), Some(&theirs)).expect("kept");

        assert!(merged.comments.is_empty(), "deletion of a comment applies");
    }

    #[test]
    fn test_updated_at_always_server_assigned() {
        let base = base_ticket();
        let ours = base.clone();
        let mut theirs = base.clone();
        theirs.updated_at = Some("2099-12-31T23:59:59Z".to_string());
        theirs.title = "changed".to_string();

        let merged = merge3(Some(&base), Some(&ours), Some(&theirs)).expect("kept");

        assert_eq!(
            merged.updated_at, ours.updated_at,
            "client timestamps are never trusted; ingest stamps this"
        );
    }

    // ---- file-level rules ----

    #[test]
    fn test_new_file_in_theirs_is_created() {
        let theirs = base_ticket();
        let merged = merge3(None, None, Some(&theirs)).expect("created");
        assert_eq!(merged.title, theirs.title);
    }

    #[test]
    fn test_delete_unchanged_in_ours_succeeds() {
        let base = base_ticket();
        let ours = base.clone();
        assert!(
            merge3(Some(&base), Some(&ours), None).is_none(),
            "an uncontested delete removes the ticket"
        );
    }

    #[test]
    fn test_delete_vs_modify_retains_ticket_with_ours_applied() {
        let base = base_ticket();
        let mut ours = base.clone();
        ours.title = "Still being worked on".to_string();

        let merged = merge3(Some(&base), Some(&ours), None).expect("retained");

        assert_eq!(
            merged.title, "Still being worked on",
            "modification wins over deletion"
        );
    }

    #[test]
    fn test_modify_vs_canonical_delete_revives_with_owned_fields() {
        let base = base_ticket();
        let mut theirs = base.clone();
        theirs.title = "Actually still needed".to_string();
        theirs.author = Some("mallory".to_string());

        let merged = merge3(Some(&base), None, Some(&theirs)).expect("revived");

        assert_eq!(merged.title, "Actually still needed");
        assert_eq!(merged.author, base.author, "identity survives a revive");
        assert_eq!(merged.number, base.number);
    }

    #[test]
    fn test_unmodified_vs_canonical_delete_stays_deleted() {
        let base = base_ticket();
        let theirs = base.clone();
        assert!(
            merge3(Some(&base), None, Some(&theirs)).is_none(),
            "a submitter who did not touch it does not resurrect it"
        );
    }

    #[test]
    fn test_delete_on_both_sides_is_noop() {
        let base = base_ticket();
        assert!(merge3(Some(&base), None, None).is_none());
    }

    #[test]
    fn test_canonical_only_file_is_not_a_deletion() {
        let ours = base_ticket();
        let merged = merge3(None, Some(&ours), None).expect("kept");
        assert_eq!(merged.title, ours.title);
    }

    #[test]
    fn test_add_add_merges_both_sides() {
        let mut ours = base_ticket();
        ours.labels = vec!["from-ui".to_string()];
        let mut theirs = base_ticket();
        theirs.labels = vec!["from-cli".to_string()];

        let merged = merge3(None, Some(&ours), Some(&theirs)).expect("merged");

        assert!(merged.labels.contains(&"from-ui".to_string()));
        assert!(merged.labels.contains(&"from-cli".to_string()));
    }
}
