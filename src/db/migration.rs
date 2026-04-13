use crate::db::Database;

impl Database {
    pub async fn create_tables(&self) -> Result<(), String> {
        // Enable WAL mode for better concurrent access
        // This allows multiple readers and avoids file lock issues on startup
        // PRAGMA returns a row, so we use query() instead of execute()
        let _ = self
            .conn()
            .query("PRAGMA journal_mode = WAL;", ())
            .await
            .map_err(|e| e.to_string())?;

        // Users table
        self.conn()
            .execute(
                "CREATE TABLE IF NOT EXISTS users (
                    id TEXT PRIMARY KEY,
                    username TEXT UNIQUE NOT NULL,
                    email TEXT,
                    password_hash TEXT NOT NULL,
                    created_at TEXT NOT NULL
                )",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        // Migration: Add email column to existing users table (if it doesn't exist)
        // SQLite doesn't support IF NOT EXISTS for columns, so we use ALTER TABLE
        self.conn()
            .execute("ALTER TABLE users ADD COLUMN email TEXT", ())
            .await
            .ok(); // Ignore error if column already exists

        // Namespaces table
        self.conn()
            .execute(
                "CREATE TABLE IF NOT EXISTS namespaces (
                    id TEXT PRIMARY KEY,
                    name TEXT UNIQUE NOT NULL,
                    owner_id TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (owner_id) REFERENCES users(id)
                )",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        // Namespace memberships table (users can have access to multiple namespaces)
        self.conn()
            .execute(
                "CREATE TABLE IF NOT EXISTS namespace_members (
                    namespace_id TEXT NOT NULL,
                    user_id TEXT NOT NULL,
                    role TEXT NOT NULL DEFAULT 'member',
                    added_at TEXT NOT NULL,
                    PRIMARY KEY (namespace_id, user_id),
                    FOREIGN KEY (namespace_id) REFERENCES namespaces(id),
                    FOREIGN KEY (user_id) REFERENCES users(id)
                )",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        // Tickets table (for signup) - user_id is nullable for pre-signup tickets
        self.conn()
            .execute(
                "CREATE TABLE IF NOT EXISTS tickets (
                    id TEXT PRIMARY KEY,
                    user_id TEXT,
                    used BOOLEAN NOT NULL DEFAULT FALSE,
                    created_at TEXT NOT NULL,
                    used_at TEXT
                )",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        // Tokens table (for session management)
        self.conn()
            .execute(
                "CREATE TABLE IF NOT EXISTS tokens (
                    token TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (user_id) REFERENCES users(id)
                )",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        // Create indexes
        self.conn()
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_namespaces_owner ON namespaces(owner_id)",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        self.conn()
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_namespace_members_user ON namespace_members(user_id)",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        self.conn()
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_tickets_user ON tickets(user_id)",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
