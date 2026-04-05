use std::path::Path;

use libsql::Builder;

use crate::auth::{Namespace, Ticket, User};

pub struct Database {
    conn: libsql::Connection,
}

impl Database {
    pub async fn new(db_path: &str) -> Result<Self, String> {
        let db_path = Path::new(db_path);
        let parent = db_path.parent().unwrap_or(Path::new("."));
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let db = Builder::new_local(db_path)
            .build()
            .await
            .map_err(|e| e.to_string())?;

        let conn = db.connect().map_err(|e| e.to_string())?;

        let database = Self { conn };
        database.init_tables().await?;

        Ok(database)
    }

    async fn init_tables(&self) -> Result<(), String> {
        // Users table
        self.conn
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
        self.conn
            .execute("ALTER TABLE users ADD COLUMN email TEXT", ())
            .await
            .ok(); // Ignore error if column already exists

        // Namespaces table
        self.conn
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
        self.conn
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
        self.conn
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
        self.conn
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
        self.conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_namespaces_owner ON namespaces(owner_id)",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        self.conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_namespace_members_user ON namespace_members(user_id)",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        self.conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_tickets_user ON tickets(user_id)",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    // User operations
    pub async fn create_user(&self, user: &User) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO users (id, username, email, password_hash, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                libsql::params![user.id.clone(), user.username.clone(), user.email.clone(), user.password_hash.clone(), user.created_at.clone()],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>, String> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, username, email, password_hash, created_at FROM users WHERE username = ?1",
                libsql::params![username],
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(User {
                id: row.get(0).map_err(|e| e.to_string())?,
                username: row.get(1).map_err(|e| e.to_string())?,
                email: row.get(2).map_err(|e| e.to_string())?,
                password_hash: row.get(3).map_err(|e| e.to_string())?,
                created_at: row.get(4).map_err(|e| e.to_string())?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_user_by_id(&self, user_id: &str) -> Result<Option<User>, String> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, username, email, password_hash, created_at FROM users WHERE id = ?1",
                libsql::params![user_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(User {
                id: row.get(0).map_err(|e| e.to_string())?,
                username: row.get(1).map_err(|e| e.to_string())?,
                email: row.get(2).map_err(|e| e.to_string())?,
                password_hash: row.get(3).map_err(|e| e.to_string())?,
                created_at: row.get(4).map_err(|e| e.to_string())?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn update_user_email(&self, user_id: &str, email: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE users SET email = ?1 WHERE id = ?2",
                libsql::params![email, user_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // Namespace operations
    pub async fn create_namespace(&self, namespace: &Namespace) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO namespaces (id, name, owner_id, created_at) VALUES (?1, ?2, ?3, ?4)",
                libsql::params![
                    namespace.id.clone(),
                    namespace.name.clone(),
                    namespace.owner_id.clone(),
                    namespace.created_at.clone()
                ],
            )
            .await
            .map_err(|e| e.to_string())?;

        // Add owner as a member
        let now = chrono::Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO namespace_members (namespace_id, user_id, role, added_at) VALUES (?1, ?2, ?3, ?4)",
                libsql::params![namespace.id.clone(), namespace.owner_id.clone(), "owner", now],
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn get_namespace_by_name(&self, name: &str) -> Result<Option<Namespace>, String> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, name, owner_id, created_at FROM namespaces WHERE name = ?1",
                libsql::params![name],
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(Namespace {
                id: row.get(0).map_err(|e| e.to_string())?,
                name: row.get(1).map_err(|e| e.to_string())?,
                owner_id: row.get(2).map_err(|e| e.to_string())?,
                created_at: row.get(3).map_err(|e| e.to_string())?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_namespaces_with_owners(&self) -> Result<Vec<(Namespace, String)>, String> {
        let mut rows = self
            .conn
            .query(
                "SELECT n.id, n.name, n.owner_id, n.created_at, u.username 
                 FROM namespaces n 
                 JOIN users u ON n.owner_id = u.id 
                 ORDER BY n.name",
                (),
            )
            .await
            .map_err(|e| e.to_string())?;

        let mut result = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            let namespace = Namespace {
                id: row.get(0).map_err(|e| e.to_string())?,
                name: row.get(1).map_err(|e| e.to_string())?,
                owner_id: row.get(2).map_err(|e| e.to_string())?,
                created_at: row.get(3).map_err(|e| e.to_string())?,
            };
            let username: String = row.get(4).map_err(|e| e.to_string())?;
            result.push((namespace, username));
        }
        Ok(result)
    }

    pub async fn search_namespaces_with_owners(
        &self,
        query: &str,
    ) -> Result<Vec<(Namespace, String)>, String> {
        let search_pattern = format!("%{}%", query);
        let mut rows = self
            .conn
            .query(
                "SELECT n.id, n.name, n.owner_id, n.created_at, u.username 
                 FROM namespaces n 
                 JOIN users u ON n.owner_id = u.id 
                 WHERE n.name LIKE ?1 
                 ORDER BY n.name",
                libsql::params![search_pattern],
            )
            .await
            .map_err(|e| e.to_string())?;

        let mut result = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            let namespace = Namespace {
                id: row.get(0).map_err(|e| e.to_string())?,
                name: row.get(1).map_err(|e| e.to_string())?,
                owner_id: row.get(2).map_err(|e| e.to_string())?,
                created_at: row.get(3).map_err(|e| e.to_string())?,
            };
            let username: String = row.get(4).map_err(|e| e.to_string())?;
            result.push((namespace, username));
        }
        Ok(result)
    }

    pub async fn user_has_namespace_access(
        &self,
        user_id: &str,
        namespace_name: &str,
    ) -> Result<bool, String> {
        // Check if user is the owner of the namespace
        let mut owner_rows = self
            .conn
            .query(
                "SELECT 1 FROM namespaces WHERE name = ?1 AND owner_id = ?2",
                libsql::params![namespace_name, user_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        if owner_rows.next().await.map_err(|e| e.to_string())?.is_some() {
            return Ok(true);
        }

        // Check if user is a member of the namespace
        let mut rows = self
            .conn
            .query(
                "SELECT 1 FROM namespace_members nm
                 JOIN namespaces n ON nm.namespace_id = n.id
                 WHERE nm.user_id = ?1 AND n.name = ?2",
                libsql::params![user_id, namespace_name],
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(rows.next().await.map_err(|e| e.to_string())?.is_some())
    }

    pub async fn user_has_any_namespaces(&self, user_id: &str) -> Result<bool, String> {
        let mut rows = self
            .conn
            .query(
                "SELECT 1 FROM namespace_members WHERE user_id = ?1 LIMIT 1",
                libsql::params![user_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(rows.next().await.map_err(|e| e.to_string())?.is_some())
    }

    // Ticket operations
    pub async fn create_ticket(&self, ticket: &Ticket) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO tickets (id, user_id, used, created_at, used_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                libsql::params![
                    ticket.id.clone(),
                    ticket.user_id.clone(),
                    ticket.used,
                    ticket.created_at.clone(),
                    ticket.used_at.clone()
                ],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_ticket(&self, ticket_id: &str) -> Result<Option<Ticket>, String> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, user_id, used, created_at, used_at FROM tickets WHERE id = ?1",
                libsql::params![ticket_id],
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(Ticket {
                id: row.get(0).map_err(|e| e.to_string())?,
                user_id: row.get(1).map_err(|e| e.to_string())?,
                used: row.get(2).map_err(|e| e.to_string())?,
                created_at: row.get(3).map_err(|e| e.to_string())?,
                used_at: row.get(4).map_err(|e| e.to_string())?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn mark_ticket_used(&self, ticket_id: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn
            .execute(
                "UPDATE tickets SET used = TRUE, used_at = ?1 WHERE id = ?2",
                libsql::params![now, ticket_id],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // Token operations
    pub async fn create_token(&self, token: &str, user_id: &str) -> Result<(), String> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO tokens (token, user_id, created_at) VALUES (?1, ?2, ?3)",
                libsql::params![token, user_id, now],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_token_user(&self, token: &str) -> Result<Option<String>, String> {
        let mut rows = self
            .conn
            .query(
                "SELECT user_id FROM tokens WHERE token = ?1 AND created_at > datetime('now', '-30 days')",
                libsql::params![token],
            )
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            Ok(Some(row.get(0).map_err(|e| e.to_string())?))
        } else {
            Ok(None)
        }
    }

    pub async fn delete_token(&self, token: &str) -> Result<(), String> {
        self.conn
            .execute(
                "DELETE FROM tokens WHERE token = ?1",
                libsql::params![token],
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
