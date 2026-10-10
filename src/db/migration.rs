use crate::db::Database;

struct Migration {
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        name: "create_users_table",
        sql: r"CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            email TEXT,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL
        );",
    },
    Migration {
        name: "create_namespaces_table",
        sql: r"CREATE TABLE IF NOT EXISTS namespaces (
            id TEXT PRIMARY KEY,
            name TEXT UNIQUE NOT NULL,
            owner_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (owner_id) REFERENCES users(id)
        );",
    },
    Migration {
        name: "create_namespace_members_table",
        sql: r"CREATE TABLE IF NOT EXISTS namespace_members (
            namespace_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'member',
            added_at TEXT NOT NULL,
            PRIMARY KEY (namespace_id, user_id),
            FOREIGN KEY (namespace_id) REFERENCES namespaces(id),
            FOREIGN KEY (user_id) REFERENCES users(id)
        );",
    },
    Migration {
        name: "create_tickets_table",
        sql: r"CREATE TABLE IF NOT EXISTS tickets (
            id TEXT PRIMARY KEY,
            user_id TEXT,
            used BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TEXT NOT NULL,
            used_at TEXT
        );",
    },
    Migration {
        name: "create_tokens_table",
        sql: r"CREATE TABLE IF NOT EXISTS tokens (
            token TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id)
        );",
    },
    Migration {
        name: "create_actix_sessions_table",
        sql: r"CREATE TABLE IF NOT EXISTS actix_sessions (
            session_key TEXT PRIMARY KEY,
            state TEXT NOT NULL,
            expires_at INTEGER NOT NULL
        );",
    },
    Migration {
        name: "create_actix_sessions_expiry_index",
        sql: r"CREATE INDEX IF NOT EXISTS idx_actix_sessions_expires_at ON actix_sessions(expires_at);",
    },
    Migration {
        name: "create_namespaces_owner_index",
        sql: r"CREATE INDEX IF NOT EXISTS idx_namespaces_owner ON namespaces(owner_id);",
    },
    Migration {
        name: "create_namespace_members_user_index",
        sql: r"CREATE INDEX IF NOT EXISTS idx_namespace_members_user ON namespace_members(user_id);",
    },
    Migration {
        name: "create_tickets_user_index",
        sql: r"CREATE INDEX IF NOT EXISTS idx_tickets_user ON tickets(user_id);",
    },
    Migration {
        name: "create_tokens_created_at_index",
        sql: r"CREATE INDEX IF NOT EXISTS idx_tokens_created_at ON tokens(created_at);",
    },
    // The `tickets` table holds single-use SIGNUP INVITES. The name was claimed
    // by the former ticket feature, so the signup concept is renamed to `invites`.
    // Historical migrations above are never edited; the rename is applied as its
    // own step.
    Migration {
        name: "rename_tickets_table_to_invites",
        sql: r"ALTER TABLE tickets RENAME TO invites;",
    },
    Migration {
        name: "drop_old_tickets_user_index",
        sql: r"DROP INDEX IF EXISTS idx_tickets_user;",
    },
    Migration {
        name: "create_invites_user_index",
        sql: r"CREATE INDEX IF NOT EXISTS idx_invites_user ON invites(user_id);",
    },
    Migration {
        name: "create_test_pins_table",
        sql: r"CREATE TABLE IF NOT EXISTS test_pins (
            pin TEXT PRIMARY KEY,
            created_at TEXT NOT NULL
        );",
    },
    Migration {
        name: "create_repository_binaries_table",
        sql: r"CREATE TABLE IF NOT EXISTS repository_binaries (
            namespace TEXT NOT NULL,
            repo TEXT NOT NULL,
            version TEXT NOT NULL,
            filename TEXT NOT NULL,
            content BLOB NOT NULL,
            size_bytes INTEGER NOT NULL,
            uploaded_at TEXT NOT NULL,
            PRIMARY KEY (namespace, repo, version, filename)
        );",
    },
    Migration {
        name: "create_repository_binaries_lookup_index",
        sql: r"CREATE INDEX IF NOT EXISTS idx_repository_binaries_lookup
            ON repository_binaries(namespace, repo, version, filename);",
    },
    Migration {
        name: "create_namespace_invitations_table",
        sql: r"CREATE TABLE IF NOT EXISTS namespace_invitations (
            token TEXT PRIMARY KEY,
            email TEXT NOT NULL,
            namespace_id TEXT NOT NULL,
            role TEXT NOT NULL CHECK (role IN ('owner', 'contributor')),
            created_at TEXT NOT NULL,
            expires_at TEXT,
            accepted_at TEXT,
            accepted_user_id TEXT,
            FOREIGN KEY (namespace_id) REFERENCES namespaces(id),
            FOREIGN KEY (accepted_user_id) REFERENCES users(id)
        );",
    },
    Migration {
        name: "create_namespace_invitations_namespace_created_index",
        sql: r"CREATE INDEX IF NOT EXISTS idx_namespace_invitations_namespace_created
            ON namespace_invitations(namespace_id, created_at DESC);",
    },
    Migration {
        name: "normalize_legacy_namespace_member_role",
        sql: r"UPDATE namespace_members SET role = 'contributor' WHERE role = 'member';",
    },
];

impl Database {
    pub async fn create_tables(&self) -> Result<(), String> {
        let conn = self.conn().await?;

        conn.query("PRAGMA journal_mode = WAL;", ())
            .await
            .map_err(|e| format!("Failed to set WAL mode: {e}"))?;

        conn.execute(
            r"CREATE TABLE IF NOT EXISTS _migrations (
                name TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
            (),
        )
        .await
        .map_err(|e| format!("Failed to create migrations table: {e}"))?;

        let mut rows = conn
            .query("SELECT name FROM _migrations", ())
            .await
            .map_err(|e| format!("Failed to list applied migrations: {e}"))?;

        let mut applied = Vec::new();
        while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
            applied.push(row.get::<String>(0).map_err(|e| e.to_string())?);
        }

        for migration in MIGRATIONS {
            if applied.contains(&migration.name.to_string()) {
                continue;
            }

            conn.execute("BEGIN IMMEDIATE", ())
                .await
                .map_err(|e| e.to_string())?;

            let result: Result<bool, String> = async {
                let mut rows = conn
                    .query(
                        "SELECT 1 FROM _migrations WHERE name = ?1",
                        turso::params![migration.name],
                    )
                    .await
                    .map_err(|e| format!("Failed to check migration {}: {e}", migration.name))?;

                if rows.next().await.map_err(|e| e.to_string())?.is_some() {
                    return Ok(false);
                }

                conn.execute(migration.sql, ())
                    .await
                    .map_err(|e| format!("Migration {} failed: {e}", migration.name))?;

                conn.execute(
                    "INSERT INTO _migrations (name) VALUES (?1)",
                    turso::params![migration.name],
                )
                .await
                .map_err(|e| format!("Failed to record migration {}: {e}", migration.name))?;

                Ok(true)
            }
            .await;

            match result {
                Ok(true) => {
                    conn.execute("COMMIT", ()).await.map_err(|e| {
                        format!("Failed to commit migration {}: {e}", migration.name)
                    })?;
                }
                Ok(false) => {
                    conn.execute("ROLLBACK", ()).await.map_err(|e| {
                        format!("Failed to rollback migration {}: {e}", migration.name)
                    })?;
                }
                Err(e) => {
                    conn.execute("ROLLBACK", ())
                        .await
                        .map_err(|re| format!("{e} (rollback also failed: {re})"))?;
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_tables_runs_all_migrations() {
        let db_path = format!("/tmp/test_twig_migrations_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);

        let result = db.create_tables().await;
        assert!(result.is_ok(), "create_tables should succeed: {result:?}");

        let mut rows = db
            .conn()
            .await
            .unwrap()
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
                (),
            )
            .await
            .unwrap();

        let mut tables = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            tables.push(row.get::<String>(0).unwrap());
        }

        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"namespaces".to_string()));
        assert!(tables.contains(&"namespace_members".to_string()));
        assert!(tables.contains(&"invites".to_string()));
        assert!(tables.contains(&"namespace_invitations".to_string()));
        assert!(tables.contains(&"tokens".to_string()));
        assert!(tables.contains(&"test_pins".to_string()));
        assert!(tables.contains(&"repository_binaries".to_string()));
        assert!(tables.contains(&"_migrations".to_string()));
        assert!(
            !tables.contains(&"tickets".to_string()),
            "`tickets` must be renamed to `invites`"
        );

        let mut rows = db
            .conn()
            .await
            .unwrap()
            .query(
                "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'",
                (),
            )
            .await
            .unwrap();

        let mut indexes = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            indexes.push(row.get::<String>(0).unwrap());
        }

        assert!(indexes.contains(&"idx_invites_user".to_string()));
        assert!(!indexes.contains(&"idx_tickets_user".to_string()));

        let mut rows = db
            .conn()
            .await
            .unwrap()
            .query("SELECT name FROM _migrations ORDER BY name", ())
            .await
            .unwrap();

        let mut recorded = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            recorded.push(row.get::<String>(0).unwrap());
        }

        assert_eq!(recorded.len(), MIGRATIONS.len());
        for migration in MIGRATIONS {
            assert!(recorded.contains(&migration.name.to_string()));
        }

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_legacy_member_roles_migrate_to_contributors() {
        use crate::auth::{User, create_namespace};

        let db_path = format!("/tmp/test_twig_role_migration_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("init tables");
        let owner = User {
            id: uuid::Uuid::new_v4().to_string(),
            username: "role-owner".to_string(),
            email: Some("owner@example.com".to_string()),
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let member = User {
            id: uuid::Uuid::new_v4().to_string(),
            username: "role-member".to_string(),
            email: Some("member@example.com".to_string()),
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        db.create_user(&owner).await.expect("create owner");
        db.create_user(&member).await.expect("create member");
        let namespace = create_namespace("role-migration".to_string(), owner.id.clone());
        db.create_namespace(&namespace)
            .await
            .expect("create namespace");
        db.conn()
            .await
            .unwrap()
            .execute(
                "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
                 VALUES (?1, ?2, 'member', ?3)",
                turso::params![
                    namespace.id.clone(),
                    member.id.clone(),
                    chrono::Utc::now().to_rfc3339()
                ],
            )
            .await
            .expect("insert legacy member role");
        db.conn()
            .await
            .unwrap()
            .execute(
                "DELETE FROM _migrations WHERE name = ?1",
                turso::params!["normalize_legacy_namespace_member_role"],
            )
            .await
            .expect("mark role migration unapplied");

        db.create_tables()
            .await
            .expect("apply role normalization migration");

        let role = db
            .get_namespace_role(&member.id, &namespace.name)
            .await
            .expect("read normalized role");
        assert_eq!(role, Some(crate::auth::NamespaceRole::Contributor));

        let _ = std::fs::remove_file(&db_path);
    }
}
