use actix_web::Result as AwResult;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use log::info;
use serde::Deserialize;

use crate::auth::TwigContext;
use crate::config;
use crate::db::Database;
use crate::http::view::{render_error, render_error_with_action, render_success};
use twig_git as git;

#[derive(Debug, Clone, Deserialize)]
pub struct NamespaceForm {
    pub namespace: String,
}

mod account;
mod namespaces;
mod repositories;
#[cfg(test)]
use account::*;
pub use account::{settings_page, update_email};
#[cfg(test)]
use namespaces::*;
pub use namespaces::{delete_namespace, rename_namespace};
#[cfg(test)]
use repositories::*;
pub use repositories::{delete_repo, move_repo, rename_repo};
pub(crate) use repositories::{render_repo_deletion_panel, render_repo_rename_panel};

async fn require_namespace_owner(
    db: &Database,
    user_id: &str,
    namespace: &str,
) -> Result<(), HttpResponse> {
    match db.user_owns_namespace(user_id, namespace).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(HttpResponse::Forbidden()
            .body(render_error("Namespace owner access required").into_string())),
        Err(error) => {
            log::error!("Failed to check namespace owner permission: {error}");
            Err(HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string()))
        }
    }
}

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
