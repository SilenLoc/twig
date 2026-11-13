//! Issue rows. An issue belongs to a repository by its stable ID (see
//! `RepoHandle::repo_id`) and is numbered in the same sequence as the repository's
//! pull requests, so one number always names one item.

use chrono::Utc;
use uuid::Uuid;

use crate::db::{
    Database,
    sqlite::{self, Connection, Row},
};

pub mod comments;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueState {
    Open,
    Closed,
}

impl IssueState {
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
pub struct Issue {
    pub id: String,
    pub repo_id: String,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub state: IssueState,
    pub author_user_id: String,
    pub assignee_user_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
}

pub struct NewIssue<'a> {
    pub repo_id: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub author_user_id: &'a str,
    pub assignee_user_ids: &'a [String],
}

#[derive(Debug, Clone, Default)]
pub struct IssueFilter {
    pub state: Option<IssueState>,
    /// Case-insensitive substring of the title or the body.
    pub query: Option<String>,
    pub author_user_id: Option<String>,
    pub assignee_user_id: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Default)]
pub struct IssueEdit {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<IssueState>,
}

const COLUMNS: &str = "id, repo_id, number, title, body, state, author_user_id, created_at,
    updated_at, closed_at";

/// The next number in a repository's shared sequence. Pull requests and issues both
/// draw from it. Callers must hold a `BEGIN IMMEDIATE` transaction.
pub(crate) async fn next_repo_number(conn: &Connection, repo_id: &str) -> Result<i64, String> {
    let mut rows = conn
        .query(
            "SELECT COALESCE(MAX(number), 0) + 1 FROM (
                SELECT number FROM pull_requests WHERE repo_id = ?1
                UNION ALL
                SELECT number FROM issues WHERE repo_id = ?1
            )",
            sqlite::params![repo_id],
        )
        .await
        .map_err(|e| e.to_string())?;
    rows.next()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Repository numbering returned no row")?
        .get(0)
        .map_err(|e| e.to_string())
}

fn from_row(row: &Row) -> Result<Issue, String> {
    let text = |index: usize| row.get::<String>(index).map_err(|e| e.to_string());
    let optional = |index: usize| row.get::<Option<String>>(index).map_err(|e| e.to_string());
    let state_text = text(5)?;
    let state = IssueState::parse(&state_text)
        .ok_or_else(|| format!("Unknown issue state in database: {state_text}"))?;
    Ok(Issue {
        id: text(0)?,
        repo_id: text(1)?,
        number: row.get::<i64>(2).map_err(|e| e.to_string())?,
        title: text(3)?,
        body: text(4)?,
        state,
        author_user_id: text(6)?,
        assignee_user_ids: Vec::new(),
        created_at: text(7)?,
        updated_at: text(8)?,
        closed_at: optional(9)?,
    })
}

/// Loads the assignees of each issue into `issue.assignee_user_ids`, oldest first.
async fn load_assignees(conn: &Connection, issue: &mut Issue) -> Result<(), String> {
    let mut rows = conn
        .query(
            "SELECT user_id FROM issue_assignees WHERE issue_id = ?1 ORDER BY assigned_at, user_id",
            sqlite::params![issue.id.as_str()],
        )
        .await
        .map_err(|e| e.to_string())?;
    issue.assignee_user_ids.clear();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        issue
            .assignee_user_ids
            .push(row.get::<String>(0).map_err(|e| e.to_string())?);
    }
    Ok(())
}

async fn collect(
    conn: &Connection,
    sql: &str,
    params: sqlite::Params,
) -> Result<Vec<Issue>, String> {
    let mut rows = conn.query(sql, params).await.map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        found.push(from_row(&row)?);
    }
    for issue in &mut found {
        load_assignees(conn, issue).await?;
    }
    Ok(found)
}

/// The pattern for a case-insensitive substring match. `%`, `_`, and `\` in the
/// query match themselves, not wildcards.
fn like_pattern(query: &str) -> String {
    let escaped = query
        .to_lowercase()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

async fn replace_assignees_in_transaction(
    conn: &Connection,
    issue_id: &str,
    user_ids: &[String],
    now: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM issue_assignees WHERE issue_id = ?1",
        sqlite::params![issue_id],
    )
    .await
    .map_err(|e| e.to_string())?;
    for user_id in user_ids {
        conn.execute(
            "INSERT INTO issue_assignees (issue_id, user_id, assigned_at) VALUES (?1, ?2, ?3)",
            sqlite::params![issue_id, user_id.as_str(), now],
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn insert_in_transaction(conn: &Connection, new: &NewIssue<'_>) -> Result<Issue, String> {
    let number = next_repo_number(conn, new.repo_id).await?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO issues (id, repo_id, number, title, body, state, author_user_id,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, ?7)",
        sqlite::params![
            id,
            new.repo_id,
            number,
            new.title,
            new.body,
            new.author_user_id,
            now
        ],
    )
    .await
    .map_err(|e| e.to_string())?;
    for user_id in new.assignee_user_ids {
        conn.execute(
            "INSERT INTO issue_assignees (issue_id, user_id, assigned_at) VALUES (?1, ?2, ?3)",
            sqlite::params![id, user_id.as_str(), now],
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(Issue {
        id,
        repo_id: new.repo_id.to_string(),
        number,
        title: new.title.to_string(),
        body: new.body.to_string(),
        state: IssueState::Open,
        author_user_id: new.author_user_id.to_string(),
        assignee_user_ids: new.assignee_user_ids.to_vec(),
        created_at: now.clone(),
        updated_at: now,
        closed_at: None,
    })
}

impl Database {
    /// Inserts an issue with the repository's next shared number. The number is read
    /// and written in one `BEGIN IMMEDIATE` transaction, so concurrent creates never
    /// share a number.
    pub async fn create_issue(&self, new: NewIssue<'_>) -> Result<Issue, String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|e| e.to_string())?;
        match insert_in_transaction(&conn, &new).await {
            Ok(issue) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| format!("Failed to commit issue: {e}"))?;
                Ok(issue)
            }
            Err(error) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|rollback| format!("{error} (rollback also failed: {rollback})"))?;
                Err(error)
            }
        }
    }

    pub async fn get_issue(&self, repo_id: &str, number: i64) -> Result<Option<Issue>, String> {
        let conn = self.conn().await?;
        let mut found = collect(
            &conn,
            &format!("SELECT {COLUMNS} FROM issues WHERE repo_id = ?1 AND number = ?2"),
            sqlite::params![repo_id, number],
        )
        .await?;
        Ok(found.pop())
    }

    pub async fn get_issue_by_id(&self, id: &str) -> Result<Option<Issue>, String> {
        let conn = self.conn().await?;
        let mut found = collect(
            &conn,
            &format!("SELECT {COLUMNS} FROM issues WHERE id = ?1"),
            sqlite::params![id],
        )
        .await?;
        Ok(found.pop())
    }

    /// One page of issues, newest number first, and whether a further page exists.
    /// The query asks for one extra row to answer that without a count.
    pub async fn list_issues(
        &self,
        repo_id: &str,
        filter: &IssueFilter,
    ) -> Result<(Vec<Issue>, bool), String> {
        let conn = self.conn().await?;
        let state = filter.state.map(IssueState::as_str);
        let pattern = filter.query.as_deref().map(like_pattern);
        let fetch = i64::from(filter.limit) + 1;
        let mut rows = collect(
            &conn,
            &format!(
                "SELECT {COLUMNS} FROM issues
                 WHERE repo_id = ?1
                   AND (?2 IS NULL OR state = ?2)
                   AND (?3 IS NULL OR lower(title) LIKE ?3 ESCAPE '\\' OR lower(body) LIKE ?3 ESCAPE '\\')
                   AND (?4 IS NULL OR author_user_id = ?4)
                   AND (?5 IS NULL OR EXISTS (
                        SELECT 1 FROM issue_assignees a
                        WHERE a.issue_id = issues.id AND a.user_id = ?5))
                 ORDER BY number DESC
                 LIMIT ?6 OFFSET ?7"
            ),
            sqlite::params![
                repo_id,
                state,
                pattern.as_deref(),
                filter.author_user_id.as_deref(),
                filter.assignee_user_id.as_deref(),
                fetch,
                i64::from(filter.offset)
            ],
        )
        .await?;
        let has_more = rows.len() > filter.limit as usize;
        rows.truncate(filter.limit as usize);
        Ok((rows, has_more))
    }

    /// How many issues match `filter`, ignoring its paging.
    pub async fn count_issues(&self, repo_id: &str, filter: &IssueFilter) -> Result<u64, String> {
        let conn = self.conn().await?;
        let state = filter.state.map(IssueState::as_str);
        let pattern = filter.query.as_deref().map(like_pattern);
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM issues
                 WHERE repo_id = ?1
                   AND (?2 IS NULL OR state = ?2)
                   AND (?3 IS NULL OR lower(title) LIKE ?3 ESCAPE '\\' OR lower(body) LIKE ?3 ESCAPE '\\')
                   AND (?4 IS NULL OR author_user_id = ?4)
                   AND (?5 IS NULL OR EXISTS (
                        SELECT 1 FROM issue_assignees a
                        WHERE a.issue_id = issues.id AND a.user_id = ?5))",
                sqlite::params![
                    repo_id,
                    state,
                    pattern.as_deref(),
                    filter.author_user_id.as_deref(),
                    filter.assignee_user_id.as_deref()
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

    /// Open and closed issue counts, for the tab's filter labels.
    pub async fn count_issues_by_state(&self, repo_id: &str) -> Result<(u64, u64), String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT state, COUNT(*) FROM issues WHERE repo_id = ?1 GROUP BY state",
                sqlite::params![repo_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        let (mut open, mut closed) = (0u64, 0u64);
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            let state: String = row.get(0).map_err(|e| e.to_string())?;
            let count: i64 = row.get(1).map_err(|e| e.to_string())?;
            let count = u64::try_from(count).unwrap_or_default();
            match IssueState::parse(&state) {
                Some(IssueState::Open) => open = count,
                Some(IssueState::Closed) => closed = count,
                None => {}
            }
        }
        Ok((open, closed))
    }

    /// Applies the present fields of `edit`. Reopening clears `closed_at`; closing
    /// stamps it with `now`.
    pub async fn edit_issue(
        &self,
        issue_id: &str,
        edit: &IssueEdit,
        now: &str,
    ) -> Result<(), String> {
        let conn = self.conn().await?;
        let state = edit.state.map(IssueState::as_str);
        conn.execute(
            "UPDATE issues SET
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
                issue_id,
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

    /// Replaces an issue's assignees with `user_ids`, in one `BEGIN IMMEDIATE` transaction.
    /// The previous set is removed first, so an empty list clears the issue.
    pub async fn replace_issue_assignees(
        &self,
        issue_id: &str,
        user_ids: &[String],
        now: &str,
    ) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|e| e.to_string())?;
        match replace_assignees_in_transaction(&conn, issue_id, user_ids, now).await {
            Ok(()) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| format!("Failed to commit assignees: {e}"))?;
                Ok(())
            }
            Err(error) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|rollback| format!("{error} (rollback also failed: {rollback})"))?;
                Err(error)
            }
        }
    }

    /// Removes every issue of a repository, with its assignees and comments. Used when
    /// the repository is deleted.
    pub async fn delete_issues_for_repo(&self, repo_id: &str) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute(
            "DELETE FROM issue_comments WHERE issue_id IN (SELECT id FROM issues WHERE repo_id = ?1)",
            sqlite::params![repo_id],
        )
        .await
        .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM issue_assignees WHERE issue_id IN (SELECT id FROM issues WHERE repo_id = ?1)",
            sqlite::params![repo_id],
        )
        .await
        .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM issues WHERE repo_id = ?1",
            sqlite::params![repo_id],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}
