use actix_web::{HttpRequest, HttpResponse};

use crate::auth::permissions::{self, NamespacePermission};
use crate::config;
use crate::db::Database;
use crate::http::view::{render_error, render_error_with_action};

pub(super) async fn render_settings_navigation(
    server: &config::Server,
    db: &Database,
    user: &crate::auth::User,
    active: &str,
) -> maud::Markup {
    let is_site_admin = server.is_configured_admin(&user.username);
    let user_management_enabled = match permissions::namespaces_for_user(
        db,
        &user.id,
        is_site_admin,
        NamespacePermission::ManageMembers,
    )
    .await
    {
        Ok(namespaces) => is_site_admin || !namespaces.is_empty(),
        Err(error) => {
            log::error!("Failed to check User Management access: {error}");
            false
        }
    };

    crate::http::tree::pages::render_tree_hub(
        server.is_test_user(&user.username),
        is_site_admin,
        user_management_enabled,
        Some(active),
    )
}

pub(super) async fn require_namespace_permission(
    db: &Database,
    server: &config::Server,
    user_id: &str,
    namespace: &str,
    permission: NamespacePermission,
) -> Result<(), HttpResponse> {
    let user = match db.get_user_by_id(user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err(HttpResponse::Unauthorized()
                .body(render_error("Session expired. Please log in again.").into_string()));
        }
        Err(error) => {
            log::error!("Failed to load user for namespace permission check: {error}");
            return Err(HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string()));
        }
    };
    match permissions::has(
        db,
        user_id,
        namespace,
        server.is_configured_admin(&user.username),
        permission,
    )
    .await
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(HttpResponse::Forbidden()
            .body(render_error("Required namespace permission is missing").into_string())),
        Err(error) => {
            log::error!("Failed to check namespace permission: {error}");
            Err(HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string()))
        }
    }
}

pub(super) fn render_settings_auth_error(req: &HttpRequest, message: &str) -> maud::Markup {
    let content = render_error_with_action(message, "/auth/login", "Log in");
    if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::http::view::render_layout(&content, None, Some("Account"))
    }
}
