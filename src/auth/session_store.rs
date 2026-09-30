use std::collections::HashMap;

use actix_session::config::PersistentSession;
use actix_session::storage::{LoadError, SaveError, SessionKey, SessionStore, UpdateError};
use anyhow::anyhow;

use crate::{auth::generate_token, db::Database};

#[derive(Clone)]
pub struct SqlSessionStore {
    db: Database,
}

impl SqlSessionStore {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    fn ttl_seconds(ttl: &actix_web::cookie::time::Duration) -> i64 {
        ttl.whole_seconds().max(0)
    }

    fn new_session_key() -> Result<SessionKey, anyhow::Error> {
        generate_token()
            .try_into()
            .map_err(|e| anyhow!("Failed to generate session key: {e}"))
    }
}

pub fn middleware(
    db: Database,
    key: actix_web::cookie::Key,
) -> actix_session::SessionMiddleware<SqlSessionStore> {
    actix_session::SessionMiddleware::builder(SqlSessionStore::new(db), key)
        .session_lifecycle(
            PersistentSession::default().session_ttl(actix_web::cookie::time::Duration::days(30)),
        )
        .build()
}

impl SessionStore for SqlSessionStore {
    async fn load(
        &self,
        session_key: &SessionKey,
    ) -> Result<Option<HashMap<String, String>>, LoadError> {
        let Some(state) = self
            .db
            .load_actix_session(session_key.as_ref())
            .await
            .map_err(|e| LoadError::Other(anyhow!(e)))?
        else {
            return Ok(None);
        };

        serde_json::from_str(&state)
            .map(Some)
            .map_err(|e| LoadError::Deserialization(anyhow!(e)))
    }

    async fn save(
        &self,
        session_state: HashMap<String, String>,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<SessionKey, SaveError> {
        let state = serde_json::to_string(&session_state)
            .map_err(|e| SaveError::Serialization(anyhow!(e)))?;
        let key = Self::new_session_key().map_err(SaveError::Other)?;

        self.db
            .save_actix_session(key.as_ref(), &state, Self::ttl_seconds(ttl))
            .await
            .map_err(|e| SaveError::Other(anyhow!(e)))?;

        Ok(key)
    }

    async fn update(
        &self,
        session_key: SessionKey,
        session_state: HashMap<String, String>,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<SessionKey, UpdateError> {
        let state = serde_json::to_string(&session_state)
            .map_err(|e| UpdateError::Serialization(anyhow!(e)))?;
        let updated = self
            .db
            .update_actix_session(session_key.as_ref(), &state, Self::ttl_seconds(ttl))
            .await
            .map_err(|e| UpdateError::Other(anyhow!(e)))?;

        if updated {
            Ok(session_key)
        } else {
            Err(UpdateError::Other(anyhow!(
                "Session does not exist or has expired"
            )))
        }
    }

    async fn update_ttl(
        &self,
        session_key: &SessionKey,
        ttl: &actix_web::cookie::time::Duration,
    ) -> Result<(), anyhow::Error> {
        let updated = self
            .db
            .update_actix_session_ttl(session_key.as_ref(), Self::ttl_seconds(ttl))
            .await
            .map_err(|e| anyhow!(e))?;
        if updated {
            Ok(())
        } else {
            Err(anyhow!("Session does not exist or has expired"))
        }
    }

    async fn delete(&self, session_key: &SessionKey) -> Result<(), anyhow::Error> {
        self.db
            .delete_actix_session(session_key.as_ref())
            .await
            .map_err(|e| anyhow!(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sql_session_store_saves_loads_updates_and_deletes_sessions() {
        let db_path = format!("/tmp/test_twig_actix_session_{}.db", uuid::Uuid::new_v4());
        let db = Database::new(&db_path);
        db.init_tables().await.expect("initialize tables");
        let store = SqlSessionStore::new(db);
        let ttl = actix_web::cookie::time::Duration::minutes(30);
        let mut state = HashMap::new();
        state.insert("user_id".to_string(), "user-123".to_string());

        let key = store.save(state.clone(), &ttl).await.expect("save session");
        assert_eq!(store.load(&key).await.expect("load session"), Some(state));

        let updated_state = HashMap::from([("user_id".to_string(), "user-456".to_string())]);
        let key = store
            .update(key, updated_state.clone(), &ttl)
            .await
            .expect("update session");
        assert_eq!(
            store.load(&key).await.expect("load updated session"),
            Some(updated_state)
        );

        store.delete(&key).await.expect("delete session");
        assert_eq!(store.load(&key).await.expect("load deleted session"), None);

        let _ = std::fs::remove_file(db_path);
    }
}
