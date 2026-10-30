//! Reviews: a verdict on a request at a commit, with its inline comments written
//! in the same transaction so a review is never saved without them.

use chrono::Utc;

use crate::db::{
    Database,
    sqlite::{self, Connection},
};

use super::comments::{NewReviewComment, last_insert_id};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewState {
    Approved,
    RequestChanges,
    Comment,
}

impl ReviewState {
    /// The event name Forgejo clients send and receive.
    pub fn api_value(self) -> &'static str {
        match self {
            Self::Approved => "APPROVED",
            Self::RequestChanges => "REQUEST_CHANGES",
            Self::Comment => "COMMENT",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "APPROVED" => Some(Self::Approved),
            "REQUEST_CHANGES" => Some(Self::RequestChanges),
            "COMMENT" => Some(Self::Comment),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestReview {
    pub id: i64,
    pub pull_request_id: String,
    pub author_user_id: String,
    pub state: ReviewState,
    pub body: String,
    pub commit_oid: String,
    pub submitted_at: String,
}

fn from_row(row: &sqlite::Row) -> Result<PullRequestReview, String> {
    let state: String = row.get(3).map_err(|e| e.to_string())?;
    Ok(PullRequestReview {
        id: row.get(0).map_err(|e| e.to_string())?,
        pull_request_id: row.get(1).map_err(|e| e.to_string())?,
        author_user_id: row.get(2).map_err(|e| e.to_string())?,
        state: ReviewState::parse(&state)
            .ok_or_else(|| format!("Unknown review state in database: {state}"))?,
        body: row.get(4).map_err(|e| e.to_string())?,
        commit_oid: row.get(5).map_err(|e| e.to_string())?,
        submitted_at: row.get(6).map_err(|e| e.to_string())?,
    })
}

const COLUMNS: &str = "id, pull_request_id, author_user_id, state, body, commit_oid, submitted_at";

async fn insert_review(
    conn: &Connection,
    pull_request_id: &str,
    author_user_id: &str,
    state: ReviewState,
    body: &str,
    commit_oid: &str,
    inline: &[NewReviewComment<'_>],
) -> Result<PullRequestReview, String> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO pull_request_reviews (pull_request_id, author_user_id, state, body, commit_oid, submitted_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        sqlite::params![pull_request_id, author_user_id, state.api_value(), body, commit_oid, now],
    )
    .await
    .map_err(|e| e.to_string())?;
    let id = last_insert_id(conn).await?;
    for comment in inline {
        conn.execute(
            "INSERT INTO pull_request_comments (pull_request_id, review_id, author_user_id, body,
                path, old_position, new_position, commit_oid, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            sqlite::params![
                pull_request_id,
                id,
                author_user_id,
                comment.body,
                comment.path,
                comment.old_position,
                comment.new_position,
                comment.commit_oid,
                now
            ],
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(PullRequestReview {
        id,
        pull_request_id: pull_request_id.to_string(),
        author_user_id: author_user_id.to_string(),
        state,
        body: body.to_string(),
        commit_oid: commit_oid.to_string(),
        submitted_at: now,
    })
}

impl Database {
    /// Stores a review and its inline comments atomically. A failure in either
    /// part leaves no trace of the review.
    pub async fn submit_pull_request_review(
        &self,
        pull_request_id: &str,
        author_user_id: &str,
        state: ReviewState,
        body: &str,
        commit_oid: &str,
        inline: &[NewReviewComment<'_>],
    ) -> Result<PullRequestReview, String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|e| e.to_string())?;
        match insert_review(
            &conn,
            pull_request_id,
            author_user_id,
            state,
            body,
            commit_oid,
            inline,
        )
        .await
        {
            Ok(review) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| format!("Failed to commit review: {e}"))?;
                Ok(review)
            }
            Err(error) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|rollback| format!("{error} (rollback also failed: {rollback})"))?;
                Err(error)
            }
        }
    }

    /// Every review on a request, oldest first.
    pub async fn list_pull_request_reviews(
        &self,
        pull_request_id: &str,
    ) -> Result<Vec<PullRequestReview>, String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                &format!(
                    "SELECT {COLUMNS} FROM pull_request_reviews
                     WHERE pull_request_id = ?1 ORDER BY id ASC"
                ),
                sqlite::params![pull_request_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        let mut reviews = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            reviews.push(from_row(&row)?);
        }
        Ok(reviews)
    }

    pub async fn get_pull_request_review(
        &self,
        pull_request_id: &str,
        review_id: i64,
    ) -> Result<Option<PullRequestReview>, String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                &format!("SELECT {COLUMNS} FROM pull_request_reviews WHERE pull_request_id = ?1 AND id = ?2"),
                sqlite::params![pull_request_id, review_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        match rows.next().await.map_err(|e| e.to_string())? {
            Some(row) => Ok(Some(from_row(&row)?)),
            None => Ok(None),
        }
    }
}
