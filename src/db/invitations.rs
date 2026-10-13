use crate::auth::{
    NamespaceInvitation, NamespaceRole, NewNamespaceInvitation, User, generate_token,
};
use crate::db::Database;

const SELECT_INVITATION: &str = "SELECT i.token, i.email, i.namespace_id, n.name, i.role,
    i.created_at, i.expires_at, i.accepted_at, i.accepted_user_id
    FROM namespace_invitations i
    JOIN namespaces n ON n.id = i.namespace_id";

struct ValidatedInvitation {
    namespace_id: String,
    role: NamespaceRole,
}

async fn accept_invitation_in_transaction(
    conn: &turso::Connection,
    token: &str,
    user: &User,
) -> Result<(), String> {
    let invitation = validate_invitation_for_user(conn, token, user).await?;
    ensure_username_available(conn, &user.username).await?;
    insert_invited_user(conn, user).await?;

    let now = chrono::Utc::now().to_rfc3339();
    add_invited_user_to_namespace(conn, &invitation, user, &now).await?;
    mark_invitation_accepted(conn, token, &user.id, &now).await
}

async fn validate_invitation_for_user(
    conn: &turso::Connection,
    token: &str,
    user: &User,
) -> Result<ValidatedInvitation, String> {
    let mut rows = conn
        .query(
            "SELECT email, namespace_id, role, expires_at, accepted_at, accepted_user_id
             FROM namespace_invitations WHERE token = ?1",
            turso::params![token],
        )
        .await
        .map_err(|error| error.to_string())?;
    let Some(row) = rows.next().await.map_err(|error| error.to_string())? else {
        return Err("Invitation not found".to_string());
    };
    let email: String = row.get(0).map_err(|error| error.to_string())?;
    let namespace_id: String = row.get(1).map_err(|error| error.to_string())?;
    let role_name: String = row.get(2).map_err(|error| error.to_string())?;
    let expires_at: Option<String> = row.get(3).map_err(|error| error.to_string())?;
    let accepted_at: Option<String> = row.get(4).map_err(|error| error.to_string())?;
    let accepted_user_id: Option<String> = row.get(5).map_err(|error| error.to_string())?;

    if accepted_at.is_some() || accepted_user_id.is_some() {
        return Err("Invitation has already been accepted".to_string());
    }
    if let Some(expires_at) = expires_at {
        let expires_at = chrono::DateTime::parse_from_rfc3339(&expires_at)
            .map_err(|_| "Invitation expiry is invalid".to_string())?;
        if expires_at <= chrono::Utc::now() {
            return Err("Invitation has expired".to_string());
        }
    }
    if user.email.as_deref() != Some(email.as_str()) {
        return Err("Account email does not match invitation".to_string());
    }
    let role = NamespaceRole::parse(&role_name)
        .ok_or_else(|| format!("Unknown namespace invitation role: {role_name}"))?;
    Ok(ValidatedInvitation { namespace_id, role })
}

async fn ensure_username_available(conn: &turso::Connection, username: &str) -> Result<(), String> {
    let mut rows = conn
        .query(
            "SELECT 1 FROM users WHERE username = ?1 LIMIT 1",
            turso::params![username],
        )
        .await
        .map_err(|error| error.to_string())?;
    if rows
        .next()
        .await
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err("Username already exists".to_string());
    }
    Ok(())
}

async fn insert_invited_user(conn: &turso::Connection, user: &User) -> Result<(), String> {
    conn.execute(
        "INSERT INTO users (id, username, email, password_hash, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        turso::params![
            user.id.clone(),
            user.username.clone(),
            user.email.clone(),
            user.password_hash.clone(),
            user.created_at.clone(),
        ],
    )
    .await
    .map(|_| ())
    .map_err(|error| error.to_string())
}

async fn add_invited_user_to_namespace(
    conn: &turso::Connection,
    invitation: &ValidatedInvitation,
    user: &User,
    now: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
         VALUES (?1, ?2, ?3, ?4)",
        turso::params![
            invitation.namespace_id.clone(),
            user.id.clone(),
            invitation.role.as_str(),
            now,
        ],
    )
    .await
    .map(|_| ())
    .map_err(|error| error.to_string())
}

async fn mark_invitation_accepted(
    conn: &turso::Connection,
    token: &str,
    user_id: &str,
    now: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE namespace_invitations
         SET accepted_at = ?1, accepted_user_id = ?2
          WHERE token = ?3 AND accepted_at IS NULL",
        turso::params![now, user_id, token],
    )
    .await
    .map(|_| ())
    .map_err(|error| error.to_string())
}

impl Database {
    pub async fn create_namespace_invitation(
        &self,
        invitation: &NewNamespaceInvitation,
    ) -> Result<NamespaceInvitation, String> {
        let token = generate_token();
        let created_at = chrono::Utc::now().to_rfc3339();
        self.conn()
            .await?
            .execute(
                "INSERT INTO namespace_invitations
                    (token, email, namespace_id, role, created_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                turso::params![
                    token.clone(),
                    invitation.email.clone(),
                    invitation.namespace_id.clone(),
                    invitation.role.as_str(),
                    created_at,
                    invitation.expires_at.clone(),
                ],
            )
            .await
            .map_err(|error| error.to_string())?;

        self.get_namespace_invitation(&token)
            .await?
            .ok_or_else(|| "Created invitation could not be loaded".to_string())
    }

    pub async fn get_namespace_invitation(
        &self,
        token: &str,
    ) -> Result<Option<NamespaceInvitation>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                &format!("{SELECT_INVITATION} WHERE i.token = ?1"),
                turso::params![token],
            )
            .await
            .map_err(|error| error.to_string())?;

        let Some(row) = rows.next().await.map_err(|error| error.to_string())? else {
            return Ok(None);
        };
        let role_name: String = row.get(4).map_err(|error| error.to_string())?;
        let role = NamespaceRole::parse(&role_name)
            .ok_or_else(|| format!("Unknown namespace invitation role: {role_name}"))?;

        Ok(Some(NamespaceInvitation {
            token: row.get(0).map_err(|error| error.to_string())?,
            email: row.get(1).map_err(|error| error.to_string())?,
            namespace_id: row.get(2).map_err(|error| error.to_string())?,
            namespace_name: row.get(3).map_err(|error| error.to_string())?,
            role,
            created_at: row.get(5).map_err(|error| error.to_string())?,
            expires_at: row.get(6).map_err(|error| error.to_string())?,
            accepted_at: row.get(7).map_err(|error| error.to_string())?,
            accepted_user_id: row.get(8).map_err(|error| error.to_string())?,
        }))
    }

    pub async fn list_namespace_invitations(
        &self,
        namespace_ids: Option<&[String]>,
    ) -> Result<Vec<NamespaceInvitation>, String> {
        match namespace_ids {
            None => self.list_namespace_invitations_for(None).await,
            Some(namespace_ids) => {
                let mut invitations = Vec::new();
                for namespace_id in namespace_ids {
                    invitations.extend(
                        self.list_namespace_invitations_for(Some(namespace_id))
                            .await?,
                    );
                }
                invitations.sort_by(|left, right| right.created_at.cmp(&left.created_at));
                Ok(invitations)
            }
        }
    }

    pub async fn accept_namespace_invitation(
        &self,
        token: &str,
        user: &User,
    ) -> Result<(), String> {
        let conn = self.conn().await?;
        conn.execute("BEGIN IMMEDIATE", ())
            .await
            .map_err(|error| error.to_string())?;

        let result = accept_invitation_in_transaction(&conn, token, user).await;

        match result {
            Ok(()) => conn
                .execute("COMMIT", ())
                .await
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Err(error) => {
                conn.execute("ROLLBACK", ())
                    .await
                    .map_err(|rollback_error| {
                        format!("{error} (rollback also failed: {rollback_error})")
                    })?;
                Err(error)
            }
        }
    }

    async fn list_namespace_invitations_for(
        &self,
        namespace_id: Option<&String>,
    ) -> Result<Vec<NamespaceInvitation>, String> {
        let mut rows = self
            .conn()
            .await?
            .query(
                &format!(
                    "{SELECT_INVITATION}
                     WHERE (?1 IS NULL OR i.namespace_id = ?1)
                     ORDER BY i.created_at DESC"
                ),
                turso::params![namespace_id.cloned()],
            )
            .await
            .map_err(|error| error.to_string())?;

        let mut invitations = Vec::new();
        while let Some(row) = rows.next().await.map_err(|error| error.to_string())? {
            let role_name: String = row.get(4).map_err(|error| error.to_string())?;
            let role = NamespaceRole::parse(&role_name)
                .ok_or_else(|| format!("Unknown namespace invitation role: {role_name}"))?;
            invitations.push(NamespaceInvitation {
                token: row.get(0).map_err(|error| error.to_string())?,
                email: row.get(1).map_err(|error| error.to_string())?,
                namespace_id: row.get(2).map_err(|error| error.to_string())?,
                namespace_name: row.get(3).map_err(|error| error.to_string())?,
                role,
                created_at: row.get(5).map_err(|error| error.to_string())?,
                expires_at: row.get(6).map_err(|error| error.to_string())?,
                accepted_at: row.get(7).map_err(|error| error.to_string())?,
                accepted_user_id: row.get(8).map_err(|error| error.to_string())?,
            });
        }
        Ok(invitations)
    }
}

#[cfg(test)]
#[path = "invitations/tests.rs"]
mod tests;
