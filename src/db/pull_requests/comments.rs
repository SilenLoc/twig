//! Conversation comments on a request. Review comments join this table in a later slice.

use chrono::Utc;

use crate::db::{
    Database,
    sqlite::{self, Connection},
};

/// The row ID of the last insert on `conn`.
pub(super) async fn last_insert_id(conn: &Connection) -> Result<i64, String> {
    let mut rows = conn
        .query("SELECT last_insert_rowid()", ())
        .await
        .map_err(|e| e.to_string())?;
    rows.next()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Inserted row id was not returned")?
        .get(0)
        .map_err(|e| e.to_string())
}

/// A comment on a request. Conversation comments have no `review_id`; inline
/// review comments belong to the review named by it and carry a diff position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestComment {
    pub id: i64,
    pub pull_request_id: String,
    pub review_id: Option<i64>,
    pub author_user_id: String,
    pub body: String,
    pub path: Option<String>,
    pub old_position: Option<i64>,
    pub new_position: Option<i64>,
    pub commit_oid: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

/// One inline comment to store with a review. Positions follow Forgejo: the new
/// side's line number, or the old side's line number for removed lines.
pub struct NewReviewComment<'a> {
    pub path: &'a str,
    pub body: &'a str,
    pub old_position: i64,
    pub new_position: i64,
    pub commit_oid: &'a str,
}

fn from_row(row: &crate::db::sqlite::Row) -> Result<PullRequestComment, String> {
    Ok(PullRequestComment {
        id: row.get(0).map_err(|e| e.to_string())?,
        pull_request_id: row.get(1).map_err(|e| e.to_string())?,
        review_id: row.get(2).map_err(|e| e.to_string())?,
        author_user_id: row.get(3).map_err(|e| e.to_string())?,
        body: row.get(4).map_err(|e| e.to_string())?,
        path: row.get(5).map_err(|e| e.to_string())?,
        old_position: row.get(6).map_err(|e| e.to_string())?,
        new_position: row.get(7).map_err(|e| e.to_string())?,
        commit_oid: row.get(8).map_err(|e| e.to_string())?,
        created_at: row.get(9).map_err(|e| e.to_string())?,
        updated_at: row.get(10).map_err(|e| e.to_string())?,
    })
}

pub(super) const COLUMNS: &str = "id, pull_request_id, review_id, author_user_id, body, path,
    old_position, new_position, commit_oid, created_at, updated_at";

impl Database {
    pub async fn add_pull_request_comment(
        &self,
        pull_request_id: &str,
        author_user_id: &str,
        body: &str,
    ) -> Result<PullRequestComment, String> {
        let conn = self.conn().await?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO pull_request_comments (pull_request_id, author_user_id, body, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            sqlite::params![pull_request_id, author_user_id, body, now],
        )
        .await
        .map_err(|e| e.to_string())?;
        let id = last_insert_id(&conn).await?;
        Ok(PullRequestComment {
            id,
            pull_request_id: pull_request_id.to_string(),
            review_id: None,
            author_user_id: author_user_id.to_string(),
            body: body.to_string(),
            path: None,
            old_position: None,
            new_position: None,
            commit_oid: None,
            created_at: now,
            updated_at: None,
        })
    }

    /// Every conversation comment of a request, oldest first. Review comments are
    /// returned by `list_review_comments`.
    pub async fn list_pull_request_comments(
        &self,
        pull_request_id: &str,
    ) -> Result<Vec<PullRequestComment>, String> {
        self.query_comments(
            &format!(
                "SELECT {COLUMNS} FROM pull_request_comments
                 WHERE pull_request_id = ?1 AND review_id IS NULL
                 ORDER BY created_at ASC, id ASC"
            ),
            sqlite::params![pull_request_id],
        )
        .await
    }

    /// Inline comments of every review on a request, oldest first.
    pub async fn list_review_comments(
        &self,
        pull_request_id: &str,
    ) -> Result<Vec<PullRequestComment>, String> {
        self.query_comments(
            &format!(
                "SELECT {COLUMNS} FROM pull_request_comments
                 WHERE pull_request_id = ?1 AND review_id IS NOT NULL
                 ORDER BY created_at ASC, id ASC"
            ),
            sqlite::params![pull_request_id],
        )
        .await
    }

    async fn query_comments(
        &self,
        sql: &str,
        params: sqlite::Params,
    ) -> Result<Vec<PullRequestComment>, String> {
        let conn = self.conn().await?;
        let mut rows = conn.query(sql, params).await.map_err(|e| e.to_string())?;
        let mut comments = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            comments.push(from_row(&row)?);
        }
        Ok(comments)
    }

    pub async fn get_pull_request_comment(
        &self,
        id: i64,
    ) -> Result<Option<PullRequestComment>, String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                &format!("SELECT {COLUMNS} FROM pull_request_comments WHERE id = ?1"),
                sqlite::params![id],
            )
            .await
            .map_err(|e| e.to_string())?;
        match rows.next().await.map_err(|e| e.to_string())? {
            Some(row) => Ok(Some(from_row(&row)?)),
            None => Ok(None),
        }
    }

    pub async fn update_pull_request_comment(&self, id: i64, body: &str) -> Result<(), String> {
        let conn = self.conn().await?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE pull_request_comments SET body = ?2, updated_at = ?3 WHERE id = ?1",
            sqlite::params![id, body, now],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn delete_pull_request_comment(&self, id: i64) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute(
            "DELETE FROM pull_request_comments WHERE id = ?1",
            sqlite::params![id],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}
