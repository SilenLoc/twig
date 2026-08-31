//! High-level ticket mutations.
//!
//! Every mutation — from the web UI and from an incoming push alike — runs
//! through here, under the same cross-process lock, so the two paths cannot
//! interleave and produce a lost update.
//!
//! Identity is taken from the authenticated actor and never from the file. A
//! git commit's author is self-attested, so a CLI user can write any name they
//! like into a ticket; these functions overwrite it.

use crate::ticket::model::{Comment, Status, Ticket, ticket_path};
use crate::ticket::store::{Change, TicketStore};

/// The authenticated user performing a mutation.
pub struct Actor<'a> {
    pub username: &'a str,
    pub email: &'a str,
}

impl<'a> Actor<'a> {
    pub fn new(username: &'a str, email: &'a str) -> Self {
        Self { username, email }
    }

    fn signature(&self) -> (&str, &str) {
        (self.username, self.email)
    }
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Fills in the fields the server owns for a newly created ticket, discarding
/// anything the client claimed for them.
pub fn stamp_new_ticket(ticket: &mut Ticket, actor: &str, namespace: &str, number: u64) {
    let timestamp = now();
    ticket.uuid = Some(uuid::Uuid::new_v4().to_string());
    ticket.number = Some(number);
    ticket.namespace = Some(namespace.to_string());
    ticket.author = Some(actor.to_string());
    ticket.created_at = Some(timestamp.clone());
    ticket.updated_at = Some(timestamp);
    stamp_new_comments(ticket, actor);
}

/// Assigns identity to comments that do not have one yet. Run before the merge
/// so comment ordering is stable and union-by-id is well defined.
pub fn stamp_new_comments(ticket: &mut Ticket, actor: &str) {
    for comment in &mut ticket.comments {
        if comment.id.is_none() {
            comment.id = Some(uuid::Uuid::new_v4().to_string());
            comment.author = Some(actor.to_string());
            comment.created_at = Some(now());
        }
    }
}

/// Creates a ticket from a draft, assigning it the next number.
pub fn create_ticket(
    store: &TicketStore,
    actor: &Actor,
    mut draft: Ticket,
) -> Result<Ticket, String> {
    if draft.title.trim().is_empty() {
        return Err("Ticket title cannot be empty".to_string());
    }

    let ingest = store.lock()?;
    let number = ingest.next_number()?;

    stamp_new_ticket(&mut draft, actor.username, ingest.namespace(), number);

    let path = ticket_path(number);
    ingest.commit(
        &[Change::Upsert(path, Box::new(draft.clone()))],
        number + 1,
        None,
        actor.signature(),
        &format!("Create ticket #{number}: {}", draft.title),
    )?;

    Ok(draft)
}

/// Applies `mutation` to an existing ticket under the ingest lock.
pub fn mutate_ticket<F>(
    store: &TicketStore,
    actor: &Actor,
    number: u64,
    message: &str,
    mutation: F,
) -> Result<Ticket, String>
where
    F: FnOnce(&mut Ticket) -> Result<(), String>,
{
    let ingest = store.lock()?;
    let mut tickets = ingest.canonical_tickets()?;
    let path = ticket_path(number);

    let ticket = tickets
        .get_mut(&path)
        .ok_or_else(|| format!("Ticket #{number} not found"))?;

    mutation(ticket)?;
    stamp_new_comments(ticket, actor.username);
    ticket.updated_at = Some(now());

    let updated = ticket.clone();
    let next_number = ingest.next_number()?;

    ingest.commit(
        &[Change::Upsert(path, Box::new(updated.clone()))],
        next_number,
        None,
        actor.signature(),
        message,
    )?;

    Ok(updated)
}

pub fn add_comment(
    store: &TicketStore,
    actor: &Actor,
    number: u64,
    markdown: &str,
) -> Result<Ticket, String> {
    if markdown.trim().is_empty() {
        return Err("Comment cannot be empty".to_string());
    }

    let message = format!("Comment on #{number}");
    mutate_ticket(store, actor, number, &message, |ticket| {
        ticket.comments.push(Comment {
            id: Some(uuid::Uuid::new_v4().to_string()),
            author: Some(actor.username.to_string()),
            created_at: Some(now()),
            markdown: markdown.to_string(),
        });
        Ok(())
    })
}

pub fn set_status(
    store: &TicketStore,
    actor: &Actor,
    number: u64,
    status: Status,
) -> Result<Ticket, String> {
    let message = format!("Set #{number} to {}", status.as_str());
    mutate_ticket(store, actor, number, &message, |ticket| {
        ticket.status = status;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ticket::model::Body;

    fn setup() -> (String, TicketStore) {
        let root = format!("/tmp/test_fig_ops_{}", uuid::Uuid::new_v4());
        std::fs::create_dir_all(&root).unwrap();
        let store = TicketStore::open_or_create(&root, "acme").expect("create store");
        (root, store)
    }

    fn draft(title: &str) -> Ticket {
        Ticket {
            title: title.to_string(),
            body: Body {
                markdown: "body".to_string(),
            },
            ..Ticket::default()
        }
    }

    #[test]
    fn test_create_ticket_assigns_identity() {
        let (root, store) = setup();
        let actor = Actor::new("alice", "alice@example.com");

        let ticket = create_ticket(&store, &actor, draft("First")).expect("create");

        assert_eq!(ticket.number, Some(1));
        assert_eq!(ticket.author.as_deref(), Some("alice"));
        assert_eq!(ticket.namespace.as_deref(), Some("acme"));
        assert!(ticket.uuid.is_some());
        assert!(ticket.created_at.is_some());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_create_ticket_ignores_claimed_identity() {
        let (root, store) = setup();
        let actor = Actor::new("alice", "alice@example.com");

        let mut forged = draft("Forged");
        forged.author = Some("mallory".to_string());
        forged.number = Some(999);

        let ticket = create_ticket(&store, &actor, forged).expect("create");

        assert_eq!(ticket.author.as_deref(), Some("alice"));
        assert_eq!(ticket.number, Some(1));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_numbers_are_sequential() {
        let (root, store) = setup();
        let actor = Actor::new("alice", "alice@example.com");

        assert_eq!(
            create_ticket(&store, &actor, draft("a")).unwrap().number(),
            1
        );
        assert_eq!(
            create_ticket(&store, &actor, draft("b")).unwrap().number(),
            2
        );
        assert_eq!(
            create_ticket(&store, &actor, draft("c")).unwrap().number(),
            3
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_add_comment_and_set_status_round_trip() {
        let (root, store) = setup();
        let actor = Actor::new("alice", "alice@example.com");
        create_ticket(&store, &actor, draft("a")).unwrap();

        add_comment(&store, &actor, 1, "first comment").expect("comment");
        set_status(&store, &actor, 1, Status::Closed).expect("status");

        let ticket = store.read_ticket(1).unwrap().expect("ticket exists");
        assert_eq!(ticket.status, Status::Closed);
        assert_eq!(ticket.comments.len(), 1);
        assert_eq!(ticket.comments[0].author.as_deref(), Some("alice"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_concurrent_ui_writes_both_land() {
        let (root, store) = setup();
        let actor = Actor::new("alice", "alice@example.com");
        create_ticket(&store, &actor, draft("busy ticket")).unwrap();

        // Two OS threads racing on the same ticket through the real flock.
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let root = root.clone();
                std::thread::spawn(move || {
                    let store = TicketStore::open(&root, "acme").expect("open");
                    let actor = Actor::new("alice", "alice@example.com");
                    add_comment(&store, &actor, 1, &format!("comment {i}"))
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread").expect("comment should land");
        }

        let ticket = store.read_ticket(1).unwrap().expect("ticket exists");
        assert_eq!(
            ticket.comments.len(),
            8,
            "every concurrent write must survive; got {:?}",
            ticket
                .comments
                .iter()
                .map(|c| &c.markdown)
                .collect::<Vec<_>>()
        );

        let mut bodies: Vec<_> = ticket.comments.iter().map(|c| c.markdown.clone()).collect();
        bodies.sort();
        for i in 0..8 {
            assert!(bodies.contains(&format!("comment {i}")), "lost comment {i}");
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_concurrent_creates_get_distinct_numbers() {
        let (root, store) = setup();

        let handles: Vec<_> = (0..6)
            .map(|i| {
                let root = root.clone();
                std::thread::spawn(move || {
                    let store = TicketStore::open(&root, "acme").expect("open");
                    let actor = Actor::new("alice", "alice@example.com");
                    create_ticket(&store, &actor, draft(&format!("ticket {i}")))
                })
            })
            .collect();

        let mut numbers: Vec<u64> = handles
            .into_iter()
            .map(|h| h.join().expect("thread").expect("create").number())
            .collect();
        numbers.sort_unstable();

        assert_eq!(numbers, vec![1, 2, 3, 4, 5, 6], "numbers must not collide");
        assert_eq!(store.read_tickets().unwrap().len(), 6);

        let _ = std::fs::remove_dir_all(&root);
    }
}
