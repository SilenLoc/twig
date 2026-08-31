//! Server-side ingest of a pushed ticket submission.
//!
//! Runs as git's `proc-receive` hook. Clients push to `refs/for/main`, which
//! does not exist on the remote and is therefore always a ref *creation* — so
//! git's client-side fast-forward check never fires and a stale push is never
//! rejected. This hook then merges the submission into the server-owned
//! `refs/heads/main` and reports success.
//!
//! Nothing in the submission is trusted: paths, sizes, TOML validity and the
//! claimed author are all checked or overwritten against `REMOTE_USER`, which
//! Fig sets from HTTP Basic Auth.

use std::collections::BTreeSet;
use std::io::{Read, Write};

use git2::{Oid, Repository};

use crate::ticket::merge::merge3;
use crate::ticket::model::{
    Comment, REPO_CONFIG_PATH, TICKETS_DIR, Ticket, is_ticket_path, number_from_path, ticket_path,
};
use crate::ticket::ops::{now, stamp_new_ticket};
use crate::ticket::pktline;
use crate::ticket::store::{Change, Ingest, TicketStore};

/// Largest accepted ticket file. History is forever, so this is a hard cap.
const MAX_BLOB_BYTES: usize = 512 * 1024;

/// Largest accepted number of new commits in one submission.
const MAX_COMMITS: usize = 1_000;

/// Files a client may touch outside `tickets/`.
const ALLOWED_ROOT_FILES: &[&str] = &[REPO_CONFIG_PATH, "README.md"];

/// Entry point for `fig ticket-ingest`.
pub fn run() -> Result<(), String> {
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    handle(&mut stdin, &mut stdout)
}

/// Speaks the `proc-receive` protocol over the given streams.
pub fn handle(input: &mut impl Read, output: &mut impl Write) -> Result<(), String> {
    // Version negotiation. We advertise no capabilities, so receive-pack will
    // not send push options.
    pktline::read_section(input).map_err(|e| format!("Failed to read version: {e}"))?;
    pktline::write(output, "version=1\n").map_err(|e| e.to_string())?;
    pktline::flush(output).map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;

    let commands =
        pktline::read_section(input).map_err(|e| format!("Failed to read commands: {e}"))?;

    let repo_path = std::env::var("GIT_DIR").map_or_else(
        |_| std::env::current_dir().unwrap_or_default(),
        std::path::PathBuf::from,
    );
    let actor = std::env::var("REMOTE_USER").unwrap_or_default();

    let mut results = Vec::new();
    for command in &commands {
        let Some((_old, new, refname)) = parse_command(command) else {
            continue;
        };
        let outcome = ingest_one(&repo_path, &actor, new);
        results.push((refname.to_string(), new.to_string(), outcome));
    }

    for (refname, submitted, outcome) in results {
        match outcome {
            Ok(()) => {
                pktline::write(output, &format!("ok {refname}\n")).map_err(|e| e.to_string())?;
                pktline::write(output, "option refname refs/heads/main\n")
                    .map_err(|e| e.to_string())?;
                // Report the OID the client already has. Reporting the canonical
                // commit instead makes git fail to update the remote-tracking
                // ref, because that object was created here and never sent.
                pktline::write(output, &format!("option new-oid {submitted}\n"))
                    .map_err(|e| e.to_string())?;
            }
            Err(reason) => {
                eprintln!("fig: {reason}");
                let reason = reason.replace('\n', " ");
                pktline::write(output, &format!("ng {refname} {reason}\n"))
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    pktline::flush(output).map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_command(line: &str) -> Option<(&str, &str, &str)> {
    let mut parts = line.split(' ');
    Some((parts.next()?, parts.next()?, parts.next()?))
}

fn ingest_one(repo_path: &std::path::Path, actor: &str, submitted: &str) -> Result<(), String> {
    let submitted = Oid::from_str(submitted).map_err(|e| format!("invalid object id: {e}"))?;
    ingest_submission(repo_path, actor, submitted).map(|_| ())
}

/// Validates, merges and commits one submission. Returns the new canonical
/// commit.
pub fn ingest_submission(
    repo_path: &std::path::Path,
    actor: &str,
    submitted: Oid,
) -> Result<Oid, String> {
    // Every ticket and comment created here is attributed to `actor`, which
    // comes from REMOTE_USER. Refusing an empty one means a push that somehow
    // reached the hook without authentication cannot write anything, rather
    // than writing content attributed to nobody.
    if actor.trim().is_empty() {
        return Err("unauthenticated push".to_string());
    }

    let store = TicketStore::from_repo_path(repo_path)?;
    let ingest = store.lock()?;

    let canonical = ingest.head()?;
    let base = validate(ingest.repo(), submitted, canonical)?;

    let base_tickets = ingest.tickets_at(base)?;
    let ours = ingest.canonical_tickets()?;
    let mut theirs = ingest.tickets_at(Some(submitted))?;

    // Identity for anything newly authored comes from the authenticated user,
    // never from the file. Only established tickets are normalised here: draft
    // files go through `stamp_new_ticket` instead, and leaving them untouched
    // keeps them directly comparable with the merge base for replay detection.
    for (path, ticket) in &mut theirs {
        if number_from_path(path).is_some() {
            normalize_submitted(ticket, base_tickets.get(path), actor);
        }
    }

    let (changes, next_number) = plan(&ingest, actor, &base_tickets, &ours, &theirs)?;

    ingest.commit(
        &changes,
        next_number,
        Some(submitted),
        (actor, &format!("{actor}@fig.local")),
        &format!("Ingest push from {actor}"),
    )
}

/// Overwrites the author and timestamp of every comment that did not already
/// exist at the merge base, and assigns ids to comments written by hand.
fn normalize_submitted(ticket: &mut Ticket, base: Option<&Ticket>, actor: &str) {
    let known: BTreeSet<&str> = base
        .map(|b| b.comments.iter().filter_map(|c| c.id.as_deref()).collect())
        .unwrap_or_default();

    for comment in &mut ticket.comments {
        let is_new = comment.id.as_deref().is_none_or(|id| !known.contains(id));
        if is_new {
            *comment = Comment {
                id: Some(
                    comment
                        .id
                        .clone()
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                ),
                author: Some(actor.to_string()),
                created_at: Some(now()),
                markdown: comment.markdown.clone(),
            };
        }
    }
}

/// Computes the changes that reconcile the submission with canonical state.
fn plan(
    ingest: &Ingest,
    actor: &str,
    base: &std::collections::BTreeMap<String, Ticket>,
    ours: &std::collections::BTreeMap<String, Ticket>,
    theirs: &std::collections::BTreeMap<String, Ticket>,
) -> Result<(Vec<Change>, u64), String> {
    let mut changes = Vec::new();
    let mut next_number = ingest.next_number()?;

    let existing_uuids: BTreeSet<String> = ours.values().filter_map(|t| t.uuid.clone()).collect();

    let paths: BTreeSet<&String> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .collect();

    for path in paths {
        if number_from_path(path).is_some() {
            // An established ticket: merge it field by field.
            match merge3(base.get(path), ours.get(path), theirs.get(path)) {
                Some(merged) => changes.push(Change::Upsert(path.clone(), Box::new(merged))),
                // Only a path that is actually in the canonical tree can be
                // removed from it; the tree builder rejects anything else.
                None => {
                    if ours.contains_key(path) {
                        changes.push(Change::Delete(path.clone()));
                    }
                }
            }
            continue;
        }

        // A draft file such as `tickets/new-bug.toml`: a request to create a
        // ticket. Drafts normally exist only in the submission, so a delete is
        // emitted only when one somehow reached canonical state.
        if ours.contains_key(path) {
            changes.push(Change::Delete(path.clone()));
        }

        let Some(draft) = theirs.get(path) else {
            continue;
        };

        // Replay detection. Canonical state always consumes draft files, so a
        // draft still present at the merge base is one an earlier push already
        // turned into a ticket — this is the same submission arriving twice.
        if base.get(path) == Some(draft) {
            continue;
        }

        // A draft carrying a uuid that already exists is likewise a replay.
        if draft
            .uuid
            .as_ref()
            .is_some_and(|uuid| existing_uuids.contains(uuid))
        {
            continue;
        }

        if draft.title.trim().is_empty() {
            return Err(format!("{path}: a new ticket needs a title"));
        }

        let mut created = draft.clone();
        stamp_new_ticket(&mut created, actor, ingest.namespace(), next_number);
        changes.push(Change::Upsert(ticket_path(next_number), Box::new(created)));
        next_number += 1;
    }

    Ok((changes, next_number))
}

/// Rejects anything that is not a well-formed ticket submission and returns the
/// merge base to reconcile against.
fn validate(
    repo: &Repository,
    submitted: Oid,
    canonical: Option<Oid>,
) -> Result<Option<Oid>, String> {
    let base = match canonical {
        None => None,
        Some(canonical) => Some(repo.merge_base(canonical, submitted).map_err(|_| {
            "the pushed history is unrelated to this repository; clone it fresh".to_string()
        })?),
    };

    let mut walk = repo
        .revwalk()
        .map_err(|e| format!("cannot inspect submitted history: {e}"))?;
    walk.push(submitted)
        .map_err(|e| format!("cannot inspect submitted history: {e}"))?;
    if let Some(base) = base {
        walk.hide(base)
            .map_err(|e| format!("cannot inspect submitted history: {e}"))?;
    }
    if walk.take(MAX_COMMITS + 1).count() > MAX_COMMITS {
        return Err(format!("submission exceeds {MAX_COMMITS} commits"));
    }

    let tree = repo
        .find_commit(submitted)
        .and_then(|c| c.tree())
        .map_err(|e| format!("cannot read submitted tree: {e}"))?;

    let mut rejection = None;
    tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
        let name = entry.name().unwrap_or("");
        let path = format!("{root}{name}");

        if let Err(reason) = check_entry(repo, root, name, &path, entry) {
            rejection = Some(reason);
            return git2::TreeWalkResult::Abort;
        }
        git2::TreeWalkResult::Ok
    })
    .ok();

    if let Some(reason) = rejection {
        return Err(reason);
    }

    Ok(base)
}

fn check_entry(
    repo: &Repository,
    root: &str,
    name: &str,
    path: &str,
    entry: &git2::TreeEntry,
) -> Result<(), String> {
    let kind = entry.kind();

    // Symlinks and submodules are never legitimate here and are a classic way
    // to make a checkout write outside the working tree.
    if entry.filemode() == i32::from(git2::FileMode::Link)
        || entry.filemode() == i32::from(git2::FileMode::Commit)
    {
        return Err(format!("{path}: symlinks and submodules are not allowed"));
    }

    match root {
        "" => {
            if kind == Some(git2::ObjectType::Tree) {
                if name != TICKETS_DIR {
                    return Err(format!("{path}/: only '{TICKETS_DIR}/' is allowed"));
                }
                return Ok(());
            }
            if !ALLOWED_ROOT_FILES.contains(&name) {
                return Err(format!(
                    "{path}: tickets live in '{TICKETS_DIR}/'; this path is not allowed"
                ));
            }
            Ok(())
        }
        "tickets/" => {
            if kind == Some(git2::ObjectType::Tree) {
                return Err(format!("{path}/: '{TICKETS_DIR}/' cannot contain folders"));
            }
            if !is_ticket_path(path) {
                return Err(format!("{path}: ticket files must end in '.toml'"));
            }
            check_blob(repo, path, entry)
        }
        _ => Err(format!("{path}: this path is not allowed")),
    }
}

fn check_blob(repo: &Repository, path: &str, entry: &git2::TreeEntry) -> Result<(), String> {
    let blob = entry
        .to_object(repo)
        .and_then(|o| o.peel_to_blob())
        .map_err(|e| format!("{path}: cannot read file: {e}"))?;

    if blob.size() > MAX_BLOB_BYTES {
        return Err(format!(
            "{path}: {} bytes exceeds the {MAX_BLOB_BYTES} byte limit",
            blob.size()
        ));
    }

    let text =
        std::str::from_utf8(blob.content()).map_err(|_| format!("{path}: must be UTF-8 text"))?;

    // Parsing here means a malformed file is refused with a clear message
    // instead of corrupting canonical state.
    Ticket::parse(text).map_err(|e| format!("{path}: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ticket::repo::{CANONICAL_REF, ensure_ticket_repo, ticket_repo_path};

    struct Fixture {
        root: String,
        repo_path: std::path::PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = format!("/tmp/test_fig_ingest_{}", uuid::Uuid::new_v4());
            std::fs::create_dir_all(&root).unwrap();
            ensure_ticket_repo(&root, "acme", "Fig", "fig@localhost").unwrap();
            let repo_path = ticket_repo_path(&root, "acme");
            Self { root, repo_path }
        }

        fn repo(&self) -> Repository {
            Repository::open(&self.repo_path).unwrap()
        }

        fn head(&self) -> Oid {
            self.repo().refname_to_id(CANONICAL_REF).unwrap()
        }

        /// Builds a commit on top of `parent` containing the given
        /// `tickets/<name>` files, mimicking what a client pushes.
        fn submit(&self, parent: Oid, files: &[(&str, &str)]) -> Oid {
            let repo = self.repo();
            let parent_commit = repo.find_commit(parent).unwrap();
            let base_tree = parent_commit.tree().unwrap();

            let mut builder = git2::build::TreeUpdateBuilder::new();
            for (name, content) in files {
                let blob = repo.blob(content.as_bytes()).unwrap();
                builder.upsert(
                    format!("tickets/{name}").as_str(),
                    blob,
                    git2::FileMode::Blob,
                );
            }
            let tree_oid = builder.create_updated(&repo, &base_tree).unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            let sig = git2::Signature::now("client", "client@example.com").unwrap();
            repo.commit(None, &sig, &sig, "client work", &tree, &[&parent_commit])
                .unwrap()
        }

        /// Builds a commit that deletes the given paths, as `git rm` would.
        fn remove(&self, parent: Oid, paths: &[&str]) -> Oid {
            let repo = self.repo();
            let parent_commit = repo.find_commit(parent).unwrap();
            let base_tree = parent_commit.tree().unwrap();

            let mut builder = git2::build::TreeUpdateBuilder::new();
            for path in paths {
                builder.remove(*path);
            }
            let tree_oid = builder.create_updated(&repo, &base_tree).unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            let sig = git2::Signature::now("client", "client@example.com").unwrap();
            repo.commit(None, &sig, &sig, "client delete", &tree, &[&parent_commit])
                .unwrap()
        }

        fn canonical_paths(&self) -> Vec<String> {
            let repo = self.repo();
            let tree = repo.find_commit(self.head()).unwrap().tree().unwrap();
            let mut paths = Vec::new();
            tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
                if entry.kind() == Some(git2::ObjectType::Blob) {
                    paths.push(format!("{root}{}", entry.name().unwrap_or("")));
                }
                git2::TreeWalkResult::Ok
            })
            .unwrap();
            paths
        }

        fn read(&self, path: &str) -> Option<String> {
            let repo = self.repo();
            let tree = repo.find_commit(self.head()).unwrap().tree().unwrap();
            let entry = tree.get_path(std::path::Path::new(path)).ok()?;
            let blob = entry.to_object(&repo).ok()?.peel_to_blob().ok()?;
            Some(String::from_utf8(blob.content().to_vec()).unwrap())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn test_draft_becomes_a_numbered_ticket() {
        let fx = Fixture::new();
        let submitted = fx.submit(fx.head(), &[("new-bug.toml", "title = \"Broken\"\n")]);

        ingest_submission(&fx.repo_path, "alice", submitted).expect("ingest");

        let paths = fx.canonical_paths();
        assert!(
            paths.contains(&"tickets/1.toml".to_string()),
            "draft should become tickets/1.toml, got {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.contains("new-bug")),
            "draft file should be consumed, got {paths:?}"
        );

        let text = fx.read("tickets/1.toml").expect("ticket file");
        assert!(text.contains(r#"title = "Broken""#));
        assert!(text.contains(r#"author = "alice""#));
        assert!(text.contains("number = 1"));
    }

    #[test]
    fn test_submitted_tip_becomes_a_parent_so_pulls_fast_forward() {
        let fx = Fixture::new();
        let submitted = fx.submit(fx.head(), &[("new-a.toml", "title = \"A\"\n")]);

        let canonical = ingest_submission(&fx.repo_path, "alice", submitted).expect("ingest");

        let repo = fx.repo();
        let parents: Vec<Oid> = repo.find_commit(canonical).unwrap().parent_ids().collect();
        assert!(
            parents.contains(&submitted),
            "the client tip must be a parent, otherwise pulls would not fast-forward"
        );
    }

    #[test]
    fn test_two_stale_submissions_both_survive() {
        let fx = Fixture::new();
        let start = fx.head();

        // Both clients branch from the same point; the second is stale.
        let first = fx.submit(start, &[("new-a.toml", "title = \"A\"\n")]);
        let second = fx.submit(start, &[("new-b.toml", "title = \"B\"\n")]);

        ingest_submission(&fx.repo_path, "alice", first).expect("first");
        ingest_submission(&fx.repo_path, "bob", second).expect("second (stale)");

        let paths = fx.canonical_paths();
        assert!(paths.contains(&"tickets/1.toml".to_string()), "{paths:?}");
        assert!(paths.contains(&"tickets/2.toml".to_string()), "{paths:?}");

        let titles: Vec<String> = ["tickets/1.toml", "tickets/2.toml"]
            .iter()
            .filter_map(|p| fx.read(p))
            .collect();
        let joined = titles.join("\n");
        assert!(joined.contains(r#"title = "A""#), "lost A: {joined}");
        assert!(joined.contains(r#"title = "B""#), "lost B: {joined}");
    }

    #[test]
    fn test_repushing_the_same_draft_is_idempotent() {
        let fx = Fixture::new();
        let start = fx.head();
        let submitted = fx.submit(start, &[("new-a.toml", "title = \"A\"\n")]);

        ingest_submission(&fx.repo_path, "alice", submitted).expect("first");
        ingest_submission(&fx.repo_path, "alice", submitted).expect("second");

        let tickets: Vec<_> = fx
            .canonical_paths()
            .into_iter()
            .filter(|p| p.starts_with("tickets/"))
            .collect();
        assert_eq!(
            tickets,
            vec!["tickets/1.toml"],
            "re-push must not duplicate"
        );
    }

    #[test]
    fn test_forged_author_is_overwritten() {
        let fx = Fixture::new();
        let submitted = fx.submit(
            fx.head(),
            &[("new-a.toml", "title = \"A\"\nauthor = \"mallory\"\n")],
        );

        ingest_submission(&fx.repo_path, "alice", submitted).expect("ingest");

        let text = fx.read("tickets/1.toml").unwrap();
        assert!(text.contains(r#"author = "alice""#), "{text}");
        assert!(!text.contains("mallory"), "{text}");
    }

    #[test]
    fn test_malformed_toml_is_refused_and_canonical_is_untouched() {
        let fx = Fixture::new();
        let before = fx.head();
        let submitted = fx.submit(before, &[("new-a.toml", "title = \n")]);

        let result = ingest_submission(&fx.repo_path, "alice", submitted);

        assert!(result.is_err(), "malformed TOML must be refused");
        assert_eq!(fx.head(), before, "canonical ref must not move");
    }

    #[test]
    fn test_deleting_a_ticket_does_not_free_its_number() {
        let fx = Fixture::new();

        let first = fx.submit(fx.head(), &[("new-a.toml", "title = \"A\"\n")]);
        ingest_submission(&fx.repo_path, "alice", first).expect("create 1");
        let second = fx.submit(fx.head(), &[("new-b.toml", "title = \"B\"\n")]);
        ingest_submission(&fx.repo_path, "alice", second).expect("create 2");

        // The CLI way to delete a ticket: `git rm tickets/2.toml` and push.
        let removed = fx.remove(fx.head(), &["tickets/2.toml"]);
        ingest_submission(&fx.repo_path, "alice", removed).expect("delete 2");
        assert!(
            !fx.canonical_paths().contains(&"tickets/2.toml".to_string()),
            "uncontested delete should remove the file"
        );

        let third = fx.submit(fx.head(), &[("new-c.toml", "title = \"C\"\n")]);
        ingest_submission(&fx.repo_path, "alice", third).expect("create 3");

        let paths = fx.canonical_paths();
        assert!(
            paths.contains(&"tickets/3.toml".to_string()),
            "numbers are monotonic; #2 must not be handed out again: {paths:?}"
        );
    }

    #[test]
    fn test_unauthenticated_push_writes_nothing() {
        let fx = Fixture::new();
        let before = fx.head();
        let submitted = fx.submit(before, &[("new-a.toml", "title = \"A\"\n")]);

        // What the hook sees when REMOTE_USER is absent or empty.
        for actor in ["", "   "] {
            let result = ingest_submission(&fx.repo_path, actor, submitted);
            assert!(
                result.is_err(),
                "an unauthenticated push must not be ingested"
            );
            assert_eq!(fx.head(), before, "canonical ref must not move");
        }

        assert!(
            !fx.canonical_paths().contains(&"tickets/1.toml".to_string()),
            "no ticket may be created without an authenticated pusher"
        );
    }

    #[test]
    fn test_oversized_blob_is_refused() {
        let fx = Fixture::new();
        let before = fx.head();
        let huge = format!("title = \"{}\"\n", "a".repeat(MAX_BLOB_BYTES));
        let submitted = fx.submit(before, &[("new-a.toml", huge.as_str())]);

        assert!(ingest_submission(&fx.repo_path, "alice", submitted).is_err());
        assert_eq!(fx.head(), before);
    }
}
