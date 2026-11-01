//! CI run rows. A run is one execution of a repository's `ci` task against one
//! commit. Runs are numbered per repository, starting at 1, and listed newest
//! first. The log is kept out of the row reads, because listings never need it.

use chrono::Utc;

use crate::db::{
    Database,
    sqlite::{self, Connection, Row},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Waiting,
    Running,
    Success,
    Failure,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "waiting" => Some(Self::Waiting),
            "running" => Some(Self::Running),
            "success" => Some(Self::Success),
            "failure" => Some(Self::Failure),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiRun {
    pub id: i64,
    pub repo_id: String,
    pub run_number: i64,
    pub workflow: String,
    pub event: String,
    pub head_branch: String,
    pub head_sha: String,
    pub display_title: String,
    pub status: RunStatus,
    pub triggered_by_user_id: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

pub struct NewCiRun<'a> {
    pub repo_id: &'a str,
    pub workflow: &'a str,
    pub event: &'a str,
    pub head_branch: &'a str,
    pub head_sha: &'a str,
    pub display_title: &'a str,
    pub triggered_by_user_id: Option<&'a str>,
}

const COLUMNS: &str = "id, repo_id, run_number, workflow, event, head_branch, head_sha,
    display_title, status, triggered_by_user_id, created_at, started_at, finished_at";

fn from_row(row: &Row) -> Result<CiRun, String> {
    let text = |index: usize| row.get::<String>(index).map_err(|e| e.to_string());
    let optional = |index: usize| row.get::<Option<String>>(index).map_err(|e| e.to_string());
    let status_text = text(8)?;
    let status = RunStatus::parse(&status_text)
        .ok_or_else(|| format!("Unknown CI run status in database: {status_text}"))?;
    Ok(CiRun {
        id: row.get::<i64>(0).map_err(|e| e.to_string())?,
        repo_id: text(1)?,
        run_number: row.get::<i64>(2).map_err(|e| e.to_string())?,
        workflow: text(3)?,
        event: text(4)?,
        head_branch: text(5)?,
        head_sha: text(6)?,
        display_title: text(7)?,
        status,
        triggered_by_user_id: optional(9)?,
        created_at: text(10)?,
        started_at: optional(11)?,
        finished_at: optional(12)?,
    })
}

async fn collect(
    conn: &Connection,
    sql: &str,
    params: sqlite::Params,
) -> Result<Vec<CiRun>, String> {
    let mut rows = conn.query(sql, params).await.map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        found.push(from_row(&row)?);
    }
    Ok(found)
}

async fn insert_in_transaction(conn: &Connection, new: &NewCiRun<'_>) -> Result<CiRun, String> {
    let mut rows = conn
        .query(
            "SELECT COALESCE(MAX(run_number), 0) + 1 FROM ci_runs WHERE repo_id = ?1",
            sqlite::params![new.repo_id],
        )
        .await
        .map_err(|e| e.to_string())?;
    let run_number: i64 = rows
        .next()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("CI run numbering returned no row")?
        .get(0)
        .map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO ci_runs (repo_id, run_number, workflow, event, head_branch, head_sha,
            display_title, status, triggered_by_user_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'waiting', ?8, ?9)",
        sqlite::params![
            new.repo_id,
            run_number,
            new.workflow,
            new.event,
            new.head_branch,
            new.head_sha,
            new.display_title,
            new.triggered_by_user_id,
            now
        ],
    )
    .await
    .map_err(|e| e.to_string())?;

    let mut rows = conn
        .query(
            "SELECT id FROM ci_runs WHERE repo_id = ?1 AND run_number = ?2",
            sqlite::params![new.repo_id, run_number],
        )
        .await
        .map_err(|e| e.to_string())?;
    let id: i64 = rows
        .next()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Inserted CI run was not found")?
        .get(0)
        .map_err(|e| e.to_string())?;

    Ok(CiRun {
        id,
        repo_id: new.repo_id.to_string(),
        run_number,
        workflow: new.workflow.to_string(),
        event: new.event.to_string(),
        head_branch: new.head_branch.to_string(),
        head_sha: new.head_sha.to_string(),
        display_title: new.display_title.to_string(),
        status: RunStatus::Waiting,
        triggered_by_user_id: new.triggered_by_user_id.map(str::to_string),
        created_at: now,
        started_at: None,
        finished_at: None,
    })
}

impl Database {
    /// Queues a run numbered one above the repository's highest number. The
    /// number is read and written in one `BEGIN IMMEDIATE` transaction, so
    /// concurrent queues in one repository never share a number.
    pub async fn create_ci_run(&self, new: NewCiRun<'_>) -> Result<CiRun, String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|e| e.to_string())?;
        match insert_in_transaction(&conn, &new).await {
            Ok(run) => {
                conn.execute("COMMIT", ())
                    .await
                    .map_err(|e| format!("Failed to commit CI run: {e}"))?;
                Ok(run)
            }
            Err(error) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|rollback| format!("{error} (rollback also failed: {rollback})"))?;
                Err(error)
            }
        }
    }

    /// One page of a repository's runs, newest first, with the total count.
    pub async fn list_ci_runs(
        &self,
        repo_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<CiRun>, i64), String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM ci_runs WHERE repo_id = ?1",
                sqlite::params![repo_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        let total: i64 = rows
            .next()
            .await
            .map_err(|e| e.to_string())?
            .ok_or("CI run count returned no row")?
            .get(0)
            .map_err(|e| e.to_string())?;
        let runs = collect(
            &conn,
            &format!(
                "SELECT {COLUMNS} FROM ci_runs WHERE repo_id = ?1
                 ORDER BY id DESC LIMIT ?2 OFFSET ?3"
            ),
            sqlite::params![repo_id, i64::from(limit), i64::from(offset)],
        )
        .await?;
        Ok((runs, total))
    }

    /// Every run against one commit of a repository, newest first.
    pub async fn list_ci_runs_for_sha(
        &self,
        repo_id: &str,
        sha: &str,
    ) -> Result<Vec<CiRun>, String> {
        let conn = self.conn().await?;
        collect(
            &conn,
            &format!(
                "SELECT {COLUMNS} FROM ci_runs WHERE repo_id = ?1 AND head_sha = ?2
                 ORDER BY id DESC"
            ),
            sqlite::params![repo_id, sha],
        )
        .await
    }

    /// The captured log of one run, only when it belongs to `repo_id`.
    pub async fn find_ci_run_log(&self, repo_id: &str, id: i64) -> Result<Option<String>, String> {
        let conn = self.conn().await?;
        let mut rows = conn
            .query(
                "SELECT log FROM ci_runs WHERE repo_id = ?1 AND id = ?2",
                sqlite::params![repo_id, id],
            )
            .await
            .map_err(|e| e.to_string())?;
        match rows.next().await.map_err(|e| e.to_string())? {
            Some(row) => Ok(Some(row.get::<String>(0).map_err(|e| e.to_string())?)),
            None => Ok(None),
        }
    }

    pub async fn mark_ci_run_started(&self, id: i64) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute(
            "UPDATE ci_runs SET status = 'running', started_at = ?1 WHERE id = ?2",
            sqlite::params![Utc::now().to_rfc3339(), id],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Records a run's final status and log.
    pub async fn finish_ci_run(&self, id: i64, status: RunStatus, log: &str) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute(
            "UPDATE ci_runs SET status = ?1, finished_at = ?2, log = ?3 WHERE id = ?4",
            sqlite::params![status.as_str(), Utc::now().to_rfc3339(), log, id],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Marks runs left queued or running by a previous process as failed. Their
    /// work died with that process, so they must not stay pending forever.
    pub async fn fail_unfinished_ci_runs(&self, reason: &str) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute(
            "UPDATE ci_runs SET status = 'failure', finished_at = ?1,
                log = log || ?2
             WHERE status IN ('waiting', 'running')",
            sqlite::params![Utc::now().to_rfc3339(), format!("\n{reason}\n")],
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
