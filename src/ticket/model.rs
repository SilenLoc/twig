//! The on-disk ticket format.
//!
//! One readable TOML file per ticket under `tickets/<number>.toml`. The file is
//! self-contained: namespace, linked repositories and branches, body and
//! comments all live in it, so a clone of the ticket repository is a complete,
//! greppable copy of the tracker.
//!
//! Fields the server owns are modelled as `Option` because a hand-written file
//! may legitimately omit them; the ingest pipeline fills them in and resets any
//! value a client tried to claim.

use serde::{Deserialize, Serialize};

/// Workflow state of a ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Open,
    InProgress,
    Blocked,
    Closed,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::InProgress => "in_progress",
            Status::Blocked => "blocked",
            Status::Closed => "closed",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Open => "Open",
            Status::InProgress => "In progress",
            Status::Blocked => "Blocked",
            Status::Closed => "Closed",
        }
    }

    pub fn all() -> [Status; 4] {
        [
            Status::Open,
            Status::InProgress,
            Status::Blocked,
            Status::Closed,
        ]
    }

    pub fn parse(value: &str) -> Option<Status> {
        Status::all().into_iter().find(|s| s.as_str() == value)
    }
}

/// A connection from a ticket to a repository, optionally to a branch in it.
/// Tickets do not belong to a repository; they are linked to any number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub repo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

impl Link {
    /// Identity of a link for set-merge purposes.
    pub fn key(&self) -> (String, Option<String>) {
        (self.repo.clone(), self.branch.clone())
    }
}

/// The markdown body of a ticket, in its own table so the multi-line string
/// stays readable in the file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Body {
    #[serde(default)]
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default)]
    pub markdown: String,
}

/// A ticket.
///
/// Field order matters: TOML requires scalars before tables, so everything the
/// `toml` serialiser emits as a bare key must be declared before [`Body`],
/// `links` and `comments`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Ticket {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assignees: Vec<String>,
    #[serde(default)]
    pub body: Body,
    #[serde(default, rename = "link", skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
    #[serde(default, rename = "comment", skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
}

impl Ticket {
    pub fn parse(text: &str) -> Result<Ticket, String> {
        toml::from_str(text).map_err(|e| format!("Invalid ticket TOML: {e}"))
    }

    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| format!("Failed to serialise ticket: {e}"))
    }

    /// Display number. Unassigned tickets (not yet ingested) report 0.
    pub fn number(&self) -> u64 {
        self.number.unwrap_or(0)
    }

    pub fn author(&self) -> &str {
        self.author.as_deref().unwrap_or("unknown")
    }
}

/// Directory holding ticket files inside the ticket repository.
pub const TICKETS_DIR: &str = "tickets";

/// Repository-level configuration file, server-owned.
pub const REPO_CONFIG_PATH: &str = ".fig-tickets.toml";

pub fn ticket_path(number: u64) -> String {
    format!("{TICKETS_DIR}/{number}.toml")
}

/// Whether `path` is a ticket file the server manages.
pub fn is_ticket_path(path: &str) -> bool {
    path.starts_with(&format!("{TICKETS_DIR}/"))
        && std::path::Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
}

/// Parses the number out of `tickets/<number>.toml`. Returns `None` for files
/// that do not carry an assigned number yet, such as `tickets/new-bug.toml`.
pub fn number_from_path(path: &str) -> Option<u64> {
    path.strip_prefix(&format!("{TICKETS_DIR}/"))?
        .strip_suffix(".toml")?
        .parse()
        .ok()
}

/// Repository-level configuration, written and owned by the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoConfig {
    pub schema_version: u32,
    pub namespace: String,
    /// Monotonic counter. Numbers are never reused, so deleting a ticket does
    /// not free its number.
    pub next_number: u64,
}

impl RepoConfig {
    pub fn parse(text: &str) -> Result<RepoConfig, String> {
        toml::from_str(text).map_err(|e| format!("Invalid {REPO_CONFIG_PATH}: {e}"))
    }
}

pub fn render_repo_config(namespace: &str, schema_version: u32, next_number: u64) -> String {
    format!(
        "# Fig ticket repository — managed by the server.\n\
         # Editing this file has no effect; the server rewrites it on every ingest.\n\
         schema_version = {schema_version}\n\
         namespace = \"{namespace}\"\n\
         next_number = {next_number}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn sample() -> Ticket {
        Ticket {
            uuid: Some("0195e0c9-1111-7777-8888-999999999999".to_string()),
            number: Some(42),
            namespace: Some("acme".to_string()),
            title: "Login fails on Safari".to_string(),
            status: Status::Open,
            author: Some("alice".to_string()),
            created_at: Some("2026-08-31T10:00:00Z".to_string()),
            updated_at: Some("2026-08-31T12:30:00Z".to_string()),
            labels: vec!["bug".to_string(), "auth".to_string()],
            assignees: vec!["bob".to_string()],
            body: Body {
                markdown: "Clicking login does nothing.\n\nSecond paragraph.".to_string(),
            },
            links: vec![Link {
                repo: "fig".to_string(),
                branch: Some("main".to_string()),
            }],
            comments: vec![Comment {
                id: Some("0195e0d1-2222-7777-8888-999999999999".to_string()),
                author: Some("bob".to_string()),
                created_at: Some("2026-08-31T11:00:00Z".to_string()),
                markdown: "Reproduced. cc @alice".to_string(),
            }],
        }
    }

    #[test]
    fn test_toml_roundtrip_is_lossless() {
        let original = sample();
        let text = original.to_toml().expect("serialise");
        let reparsed = Ticket::parse(&text).expect("parse");
        assert_eq!(original, reparsed);
    }

    #[test]
    fn test_serialised_form_is_readable() {
        let text = sample().to_toml().unwrap();
        assert!(text.contains(r#"title = "Login fails on Safari""#));
        assert!(text.contains(r#"namespace = "acme""#));
        assert!(text.contains("[body]"));
        assert!(text.contains("[[link]]"));
        assert!(text.contains("[[comment]]"));
    }

    #[test]
    fn test_minimal_handwritten_file_parses() {
        // What a CLI user realistically writes to create a ticket.
        let ticket = Ticket::parse("title = \"Something broke\"\n").expect("parse minimal");
        assert_eq!(ticket.title, "Something broke");
        assert_eq!(ticket.status, Status::Open);
        assert!(ticket.uuid.is_none());
        assert!(ticket.number.is_none());
        assert!(ticket.comments.is_empty());
    }

    #[test]
    fn test_malformed_toml_is_rejected() {
        assert!(Ticket::parse("title = ").is_err());
        assert!(Ticket::parse("this is not toml at all {{{").is_err());
    }

    #[test]
    fn test_unknown_status_is_rejected() {
        assert!(Ticket::parse("title = \"x\"\nstatus = \"banana\"\n").is_err());
    }

    #[test]
    fn test_status_roundtrip() {
        for status in Status::all() {
            assert_eq!(Status::parse(status.as_str()), Some(status));
        }
        assert_eq!(Status::parse("nope"), None);
    }

    #[test]
    fn test_ticket_path_helpers() {
        assert_eq!(ticket_path(42), "tickets/42.toml");
        assert_eq!(number_from_path("tickets/42.toml"), Some(42));
        assert_eq!(number_from_path("tickets/new-bug.toml"), None);
        assert_eq!(number_from_path("README.md"), None);
        assert!(is_ticket_path("tickets/42.toml"));
        assert!(is_ticket_path("tickets/new-bug.toml"));
        assert!(!is_ticket_path("README.md"));
        assert!(!is_ticket_path("tickets/image.png"));
    }

    #[test]
    fn test_repo_config_roundtrip() {
        let text = render_repo_config("acme", 1, 7);
        let config = RepoConfig::parse(&text).expect("parse config");
        assert_eq!(config.schema_version, 1);
        assert_eq!(config.namespace, "acme");
        assert_eq!(config.next_number, 7);
    }
}
