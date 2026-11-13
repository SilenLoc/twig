//! Conversation comments on an issue. Their API IDs are offset from the row IDs, so
//! they never collide with pull request comment IDs in the shared comment routes.

use chrono::Utc;

use crate::db::{
    Database,
    sqlite::{self, Connection, Row},
};

/// Issue comment IDs start at this value. Pull request comment IDs stay far below it.
pub const ISSUE_COMMENT_ID_BASE: i64 = 1 << 40;

/// The ID a client sees for an issue comment stored at `row_id`.
pub fn issue_comment_api_id(row_id: i64) -> i64 {
    ISSUE_COMMENT_ID_BASE + row_id
}

/// The row an API ID names, or `None` when the ID is in the pull request range.
pub fn issue_comment_row_id(api_id: i64) -> Option<i64> {
    (api_id >= ISSUE_COMMENT_ID_BASE).then_some(api_id - ISSUE_COMMENT_ID_BASE)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueComment {
    pub row_id: i64,
    pub issue_id: String,
    pub author_user_id: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: Option<String>,
}

const COLUMNS: &str = "id, issue_id, author_user_id, body, created_at, updated_at";

fn from_row(row: &Row) -> Result<IssueComment, String> {
    Ok(IssueComment {
        row_id: row.get(0).map_err(|e| e.to_string())?,
        issue_id: row.get(1).map_err(|e| e.to_string())?,
        author_user_id: row.get(2).map_err(|e| e.to_string())?,
        body: row.get(3).map_err(|e| e.to_string())?,
        created_at: row.get(4).map_err(|e| e.to_string())?,
        updated_at: row.get(5).map_err(|e| e.to_string())?,
    })
}

async fn collect(
    conn: &Connection,
    sql: &str,
    params: sqlite::Params,
) -> Result<Vec<IssueComment>, String> {
    let mut rows = conn.query(sql, params).await.map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        found.push(from_row(&row)?);
    }
    Ok(found)
}

/// The row ID of the last insert on `conn`.
async fn last_insert_id(conn: &Connection) -> Result<i64, String> {
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

impl Database {
    pub async fn add_issue_comment(
        &self,
        issue_id: &str,
        author_user_id: &str,
        body: &str,
    ) -> Result<IssueComment, String> {
        let conn = self.conn().await?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO issue_comments (issue_id, author_user_id, body, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            sqlite::params![issue_id, author_user_id, body, now.as_str()],
        )
        .await
        .map_err(|e| e.to_string())?;
        let row_id = last_insert_id(&conn).await?;
        Ok(IssueComment {
            row_id,
            issue_id: issue_id.to_string(),
            author_user_id: author_user_id.to_string(),
            body: body.to_string(),
            created_at: now,
            updated_at: None,
        })
    }

    /// Every comment of an issue, oldest first.
    pub async fn list_issue_comments(&self, issue_id: &str) -> Result<Vec<IssueComment>, String> {
        let conn = self.conn().await?;
        collect(
            &conn,
            &format!(
                "SELECT {COLUMNS} FROM issue_comments WHERE issue_id = ?1
                 ORDER BY created_at ASC, id ASC"
            ),
            sqlite::params![issue_id],
        )
        .await
    }

    pub async fn count_issue_comments(&self, issue_id: &str) -> Result<u64, String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM issue_comments WHERE issue_id = ?1",
                sqlite::params![issue_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        let count: i64 = rows
            .next()
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Count returned no row")?
            .get(0)
            .map_err(|e| e.to_string())?;
        Ok(u64::try_from(count).unwrap_or_default())
    }

    pub async fn get_issue_comment(&self, row_id: i64) -> Result<Option<IssueComment>, String> {
        let conn = self.conn().await?;
        let mut found = collect(
            &conn,
            &format!("SELECT {COLUMNS} FROM issue_comments WHERE id = ?1"),
            sqlite::params![row_id],
        )
        .await?;
        Ok(found.pop())
    }

    pub async fn update_issue_comment(
        &self,
        row_id: i64,
        body: &str,
        now: &str,
    ) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute(
            "UPDATE issue_comments SET body = ?2, updated_at = ?3 WHERE id = ?1",
            sqlite::params![row_id, body, now],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn delete_issue_comment(&self, row_id: i64) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute(
            "DELETE FROM issue_comments WHERE id = ?1",
            sqlite::params![row_id],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_ids_are_offset_and_round_trip() {
        assert_eq!(issue_comment_row_id(issue_comment_api_id(7)), Some(7));
        assert_eq!(
            issue_comment_row_id(42),
            None,
            "pull request comment IDs stay below the base"
        );
        assert!(issue_comment_api_id(1) > 1 << 39);
    }
}
