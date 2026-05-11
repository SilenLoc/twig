use crate::db::Database;
use migs::{collect, migs};

migs! {
    sql = r#"CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY,
        username TEXT UNIQUE NOT NULL,
        email TEXT,
        password_hash TEXT NOT NULL,
        created_at TEXT NOT NULL
    );"#,
    scope = "init",
    order = 1
}

migs! {
    sql = r#"CREATE TABLE IF NOT EXISTS namespaces (
        id TEXT PRIMARY KEY,
        name TEXT UNIQUE NOT NULL,
        owner_id TEXT NOT NULL,
        created_at TEXT NOT NULL,
        FOREIGN KEY (owner_id) REFERENCES users(id)
    );"#,
    scope = "init",
    order = 2
}

migs! {
    sql = r#"CREATE TABLE IF NOT EXISTS namespace_members (
        namespace_id TEXT NOT NULL,
        user_id TEXT NOT NULL,
        role TEXT NOT NULL DEFAULT 'member',
        added_at TEXT NOT NULL,
        PRIMARY KEY (namespace_id, user_id),
        FOREIGN KEY (namespace_id) REFERENCES namespaces(id),
        FOREIGN KEY (user_id) REFERENCES users(id)
    );"#,
    scope = "init",
    order = 3
}

migs! {
    sql = r#"CREATE TABLE IF NOT EXISTS tickets (
        id TEXT PRIMARY KEY,
        user_id TEXT,
        used BOOLEAN NOT NULL DEFAULT FALSE,
        created_at TEXT NOT NULL,
        used_at TEXT
    );"#,
    scope = "init",
    order = 4
}

migs! {
    sql = r#"CREATE TABLE IF NOT EXISTS tokens (
        token TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        created_at TEXT NOT NULL,
        FOREIGN KEY (user_id) REFERENCES users(id)
    );"#,
    scope = "init",
    order = 5
}

migs! {
    sql = r#"CREATE INDEX IF NOT EXISTS idx_namespaces_owner ON namespaces(owner_id);"#,
    scope = "init",
    order = 6
}

migs! {
    sql = r#"CREATE INDEX IF NOT EXISTS idx_namespace_members_user ON namespace_members(user_id);"#,
    scope = "init",
    order = 7
}

migs! {
    sql = r#"CREATE INDEX IF NOT EXISTS idx_tickets_user ON tickets(user_id);"#,
    scope = "init",
    order = 8
}

migs! {
    sql = r#"CREATE INDEX IF NOT EXISTS idx_tokens_created_at ON tokens(created_at);"#,
    scope = "init",
    order = 9
}

impl Database {
    pub async fn create_tables(&self) -> Result<(), String> {
        let _ = self
            .conn()
            .await?
            .query("PRAGMA journal_mode = WAL;", ())
            .await
            .map_err(|e| e.to_string())?;

        let migrations = collect!();

        let mut migrations: Vec<_> = migrations;
        migrations.sort_by_key(|m| m.order.unwrap_or(u32::MAX));

        for migration in &migrations {
            self.conn()
                .await?
                .execute(migration.content, ())
                .await
                .map_err(|e| format!("Migration failed: {e}"))?;
        }

        Ok(())
    }
}
