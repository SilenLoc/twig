//! Pull request rows. A request belongs to a repository by its stable ID (see
//! `RepoHandle::repo_id`) and is numbered per repository, starting at 1.

use chrono::Utc;
use uuid::Uuid;

use crate::db::{
    Database,
    sqlite::{self, Connection, Row},
};

pub mod comments;
pub mod reactions;
pub mod reviewers;
pub mod reviews;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestState {
    Open,
    Closed,
}

impl PullRequestState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    pub id: String,
    pub repo_id: String,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub state: PullRequestState,
    pub author_user_id: String,
    pub head_branch: String,
    pub base_branch: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub merged_at: Option<String>,
    pub merged_by_user_id: Option<String>,
    pub merge_commit_oid: Option<String>,
}

pub struct NewPullRequest<'a> {
    pub repo_id: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub author_user_id: &'a str,
    pub head_branch: &'a str,
    pub base_branch: &'a str,
}

#[derive(Debug, Clone, Default)]
pub struct PullRequestFilter {
    pub state: Option<PullRequestState>,
    pub head_branch: Option<String>,
    /// Case-insensitive title substring.
    pub query: Option<String>,
    /// Order by last update instead of number.
    pub recently_updated: bool,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Default)]
pub struct PullRequestEdit {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<PullRequestState>,
}

const COLUMNS: &str = "id, repo_id, number, title, body, state, author_user_id, head_branch,
    base_branch, created_at, updated_at, closed_at, merged_at, merged_by_user_id, merge_commit_oid";

fn from_row(row: &Row) -> Result<PullRequest, String> {
    let text = |index: usize| row.get::<String>(index).map_err(|e| e.to_string());
    let optional = |index: usize| row.get::<Option<String>>(index).map_err(|e| e.to_string());
    let state_text = text(5)?;
    let state = PullRequestState::parse(&state_text)
        .ok_or_else(|| format!("Unknown pull request state in database: {state_text}"))?;
    Ok(PullRequest {
        id: text(0)?,
        repo_id: text(1)?,
        number: row.get::<i64>(2).map_err(|e| e.to_string())?,
        title: text(3)?,
        body: text(4)?,
        state,
        author_user_id: text(6)?,
        head_branch: text(7)?,
        base_branch: text(8)?,
        created_at: text(9)?,
        updated_at: text(10)?,
        closed_at: optional(11)?,
        merged_at: optional(12)?,
        merged_by_user_id: optional(13)?,
        merge_commit_oid: optional(14)?,
    })
}

async fn collect(
    conn: &Connection,
    sql: &str,
    params: sqlite::Params,
) -> Result<Vec<PullRequest>, String> {
    let mut rows = conn.query(sql, params).await.map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        found.push(from_row(&row)?);
    }
    Ok(found)
}

async fn insert_in_transaction(
    conn: &Connection,
    new: &NewPullRequest<'_>,
) -> Result<PullRequest, String> {
    let mut rows = conn
        .query(
            "SELECT COALESCE(MAX(number), 0) + 1 FROM pull_requests WHERE repo_id = ?1",
            sqlite::params![new.repo_id],
        )
        .await
        .map_err(|e| e.to_string())?;
    let number: i64 = rows
        .next()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Pull request numbering returned no row")?
        .get(0)
        .map_err(|e| e.to_string())?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO pull_requests (id, repo_id, number, title, body, state, author_user_id,
            head_branch, base_branch, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, ?8, ?9, ?9)",
        sqlite::params![
            id,
            new.repo_id,
            number,
            new.title,
            new.body,
            new.author_user_id,
            new.head_branch,
            new.base_branch,
            now
        ],
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(PullRequest {
        id,
        repo_id: new.repo_id.to_string(),
        number,
        title: new.title.to_string(),
        body: new.body.to_string(),
        state: PullRequestState::Open,
        author_user_id: new.author_user_id.to_string(),
        head_branch: new.head_branch.to_string(),
        base_branch: new.base_branch.to_string(),
        created_at: now.clone(),
        updated_at: now,
        closed_at: None,
        merged_at: None,
        merged_by_user_id: None,
        merge_commit_oid: None,
    })
}

impl Database {
    /// Inserts a request numbered one above the repository's highest number.
    /// The number is read and written in one `BEGIN IMMEDIATE` transaction, so
    /// concurrent creates in the same repository never share a number.
    pub async fn create_pull_request(
        &self,
        new: NewPullRequest<'_>,
    ) -> Result<PullRequest, String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|e| e.to_string())?;
        match insert_in_transaction(&conn, &new).await {
            Ok(pull_request) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| format!("Failed to commit pull request: {e}"))?;
                Ok(pull_request)
            }
            Err(error) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|rollback| format!("{error} (rollback also failed: {rollback})"))?;
                Err(error)
            }
        }
    }

    pub async fn get_pull_request(
        &self,
        repo_id: &str,
        number: i64,
    ) -> Result<Option<PullRequest>, String> {
        let conn = self.conn().await?;
        let mut found = collect(
            &conn,
            &format!("SELECT {COLUMNS} FROM pull_requests WHERE repo_id = ?1 AND number = ?2"),
            sqlite::params![repo_id, number],
        )
        .await?;
        Ok(found.pop())
    }

    /// A request by its ID, for routes that reach it through a comment.
    pub async fn get_pull_request_by_id(&self, id: &str) -> Result<Option<PullRequest>, String> {
        let conn = self.conn().await?;
        let mut found = collect(
            &conn,
            &format!("SELECT {COLUMNS} FROM pull_requests WHERE id = ?1"),
            sqlite::params![id],
        )
        .await?;
        Ok(found.pop())
    }

    /// Returns one page of requests and whether a further page exists. The
    /// query asks for one extra row to answer that without a count.
    pub async fn list_pull_requests(
        &self,
        repo_id: &str,
        filter: &PullRequestFilter,
    ) -> Result<(Vec<PullRequest>, bool), String> {
        let conn = self.conn().await?;
        let state = filter.state.map(PullRequestState::as_str);
        let pattern = filter
            .query
            .as_ref()
            .map(|query| format!("%{}%", query.to_lowercase()));
        let fetch = i64::from(filter.limit) + 1;
        let mut rows = collect(
            &conn,
            &format!(
                "SELECT {COLUMNS} FROM pull_requests
                 WHERE repo_id = ?1
                   AND (?2 IS NULL OR state = ?2)
                   AND (?3 IS NULL OR head_branch = ?3)
                   AND (?4 IS NULL OR lower(title) LIKE ?4)
                 ORDER BY CASE WHEN ?5 THEN updated_at END DESC, number DESC
                 LIMIT ?6 OFFSET ?7"
            ),
            sqlite::params![
                repo_id,
                state,
                filter.head_branch.as_deref(),
                pattern.as_deref(),
                filter.recently_updated,
                fetch,
                i64::from(filter.offset)
            ],
        )
        .await?;
        let has_more = rows.len() > filter.limit as usize;
        rows.truncate(filter.limit as usize);
        Ok((rows, has_more))
    }

    /// How many requests match `filter`, ignoring its paging. Lists report it in `X-Total-Count`.
    pub async fn count_pull_requests(
        &self,
        repo_id: &str,
        filter: &PullRequestFilter,
    ) -> Result<u64, String> {
        let conn = self.conn().await?;
        let state = filter.state.map(PullRequestState::as_str);
        let pattern = filter
            .query
            .as_ref()
            .map(|query| format!("%{}%", query.to_lowercase()));
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM pull_requests
                 WHERE repo_id = ?1
                   AND (?2 IS NULL OR state = ?2)
                   AND (?3 IS NULL OR head_branch = ?3)
                   AND (?4 IS NULL OR lower(title) LIKE ?4)",
                sqlite::params![
                    repo_id,
                    state,
                    filter.head_branch.as_deref(),
                    pattern.as_deref()
                ],
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

    /// The open request for exactly this head and base, if one exists.
    pub async fn find_open_pull_request(
        &self,
        repo_id: &str,
        head: &str,
        base: &str,
    ) -> Result<Option<PullRequest>, String> {
        let conn = self.conn().await?;
        let mut found = collect(
            &conn,
            &format!(
                "SELECT {COLUMNS} FROM pull_requests
                 WHERE repo_id = ?1 AND head_branch = ?2 AND base_branch = ?3 AND state = 'open'"
            ),
            sqlite::params![repo_id, head, base],
        )
        .await?;
        Ok(found.pop())
    }

    /// Open and closed request counts, for the tab's filter labels.
    pub async fn count_pull_requests_by_state(&self, repo_id: &str) -> Result<(u64, u64), String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT state, COUNT(*) FROM pull_requests WHERE repo_id = ?1 GROUP BY state",
                sqlite::params![repo_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        let (mut open, mut closed) = (0u64, 0u64);
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            let state: String = row.get(0).map_err(|e| e.to_string())?;
            let count: i64 = row.get(1).map_err(|e| e.to_string())?;
            let count = u64::try_from(count).unwrap_or_default();
            match PullRequestState::parse(&state) {
                Some(PullRequestState::Open) => open = count,
                Some(PullRequestState::Closed) => closed = count,
                None => {}
            }
        }
        Ok((open, closed))
    }

    /// Applies the present fields of `edit`. Reopening clears `closed_at`;
    /// closing stamps it with `now`.
    pub async fn edit_pull_request(
        &self,
        pull_request_id: &str,
        edit: &PullRequestEdit,
        now: &str,
    ) -> Result<(), String> {
        let conn = self.conn().await?;
        let state = edit.state.map(PullRequestState::as_str);
        conn.execute(
            "UPDATE pull_requests SET
                title = COALESCE(?2, title),
                body = COALESCE(?3, body),
                state = COALESCE(?4, state),
                closed_at = CASE
                    WHEN ?4 = 'closed' AND state <> 'closed' THEN ?5
                    WHEN ?4 = 'open' THEN NULL
                    ELSE closed_at END,
                updated_at = ?5
             WHERE id = ?1",
            sqlite::params![
                pull_request_id,
                edit.title.as_deref(),
                edit.body.as_deref(),
                state,
                now
            ],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Number and head branch of every open request, for keeping head refs current.
    pub async fn open_pull_request_heads(
        &self,
        repo_id: &str,
    ) -> Result<Vec<(i64, String)>, String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT number, head_branch FROM pull_requests WHERE repo_id = ?1 AND state = 'open'",
                sqlite::params![repo_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        let mut heads = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            heads.push((
                row.get::<i64>(0).map_err(|e| e.to_string())?,
                row.get::<String>(1).map_err(|e| e.to_string())?,
            ));
        }
        Ok(heads)
    }

    /// Records a merge: the request closes, and the merge commit and merger are kept.
    pub async fn mark_pull_request_merged(
        &self,
        pull_request_id: &str,
        merged_by: &str,
        merge_commit: &str,
        now: &str,
    ) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute(
            "UPDATE pull_requests SET state = 'closed', closed_at = ?2, merged_at = ?2,
                merged_by_user_id = ?3, merge_commit_oid = ?4, updated_at = ?2
             WHERE id = ?1",
            sqlite::params![pull_request_id, now, merged_by, merge_commit],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Removes every request of a repository. Used when the repository is deleted.
    pub async fn delete_pull_requests_for_repo(&self, repo_id: &str) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute(
            "DELETE FROM pull_requests WHERE repo_id = ?1",
            sqlite::params![repo_id],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}
