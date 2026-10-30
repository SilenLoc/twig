//! Emoji reactions on a request or a comment. Forgejo's eight reaction names are
//! the only values accepted.

use chrono::Utc;

use crate::db::{Database, sqlite};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionContent {
    PlusOne,
    MinusOne,
    Laugh,
    Hooray,
    Confused,
    Heart,
    Rocket,
    Eyes,
}

impl ReactionContent {
    /// Every reaction, in the order the UI shows them.
    pub const ALL: [Self; 8] = [
        Self::PlusOne,
        Self::MinusOne,
        Self::Laugh,
        Self::Hooray,
        Self::Confused,
        Self::Heart,
        Self::Rocket,
        Self::Eyes,
    ];

    /// The name Forgejo clients send, such as `+1` or `heart`.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reaction| reaction.api_value() == value)
    }

    pub fn api_value(self) -> &'static str {
        match self {
            Self::PlusOne => "+1",
            Self::MinusOne => "-1",
            Self::Laugh => "laugh",
            Self::Hooray => "hooray",
            Self::Confused => "confused",
            Self::Heart => "heart",
            Self::Rocket => "rocket",
            Self::Eyes => "eyes",
        }
    }
}

/// `target_key` names what is reacted to: `pull:<id>` or `comment:<id>`.
pub fn pull_key(pull_request_id: &str) -> String {
    format!("pull:{pull_request_id}")
}

pub fn comment_key(comment_id: i64) -> String {
    format!("comment:{comment_id}")
}

impl Database {
    /// Adds or removes one user's reaction. Adding twice is a no-op.
    pub async fn set_reaction(
        &self,
        target_key: &str,
        user_id: &str,
        content: ReactionContent,
        on: bool,
    ) -> Result<(), String> {
        let conn = self.conn().await?;
        if on {
            conn.execute(
                "INSERT OR IGNORE INTO pull_request_reactions (target_key, user_id, content, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                sqlite::params![target_key, user_id, content.api_value(), Utc::now().to_rfc3339()],
            )
            .await
            .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "DELETE FROM pull_request_reactions
                 WHERE target_key = ?1 AND user_id = ?2 AND content = ?3",
                sqlite::params![target_key, user_id, content.api_value()],
            )
            .await
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Every reaction on a target as `(username, reaction)`, oldest first.
    pub async fn list_reactions(
        &self,
        target_key: &str,
    ) -> Result<Vec<(String, ReactionContent)>, String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT u.username, r.content FROM pull_request_reactions r
                 JOIN users u ON u.id = r.user_id
                 WHERE r.target_key = ?1
                 ORDER BY r.id ASC",
                sqlite::params![target_key],
            )
            .await
            .map_err(|e| e.to_string())?;
        let mut reactions = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            let username: String = row.get(0).map_err(|e| e.to_string())?;
            let content: String = row.get(1).map_err(|e| e.to_string())?;
            if let Some(content) = ReactionContent::parse(&content) {
                reactions.push((username, content));
            }
        }
        Ok(reactions)
    }
}
