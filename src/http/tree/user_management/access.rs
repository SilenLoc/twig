use actix_web::{HttpRequest, HttpResponse, get, web};

use crate::auth::{Namespace, TwigContext, User, permissions::NamespacePermission};
use crate::config;
use crate::http::view::render_layout;

use super::UserManagementQuery;
use super::invitations::invitations_for_scope;
use super::views::{render_user_management, render_user_management_content};

pub(super) struct UserManagementScope {
    pub(super) user: User,
    pub(super) is_site_admin: bool,
    pub(super) namespaces: Vec<Namespace>,
}

pub(super) async fn user_management_scope(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
) -> Result<UserManagementScope, HttpResponse> {
    let Some(user_id) = auth_state.user_id_from_request(req).await else {
        return Err(HttpResponse::Found()
            .insert_header(("Location", "/auth/login"))
            .finish());
    };

    let db = auth_state.db();
    let user = match db.get_user_by_id(&user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return Err(HttpResponse::Unauthorized().finish()),
        Err(error) => {
            log::error!("Failed to load User Management actor: {error}");
            return Err(HttpResponse::InternalServerError().finish());
        }
    };

    let is_site_admin = server.is_configured_admin(&user.username);
    let namespaces = match crate::auth::permissions::namespaces_for_user(
        db,
        &user_id,
        is_site_admin,
        NamespacePermission::ManageMembers,
    )
    .await
    {
        Ok(namespaces) => namespaces,
        Err(error) => {
            log::error!("Failed to load manageable namespaces for User Management: {error}");
            return Err(HttpResponse::InternalServerError().finish());
        }
    };

    if namespaces.is_empty() && !is_site_admin {
        return Err(HttpResponse::Forbidden().body("Namespace member-management access required"));
    }

    Ok(UserManagementScope {
        user,
        is_site_admin,
        namespaces,
    })
}

#[get("/tree/users")]
pub async fn user_management_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    query: web::Query<UserManagementQuery>,
) -> HttpResponse {
    let scope = match user_management_scope(&req, &server, &auth_state).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let active_tab = query
        .tab
        .as_deref()
        .filter(|tab| matches!(*tab, "users" | "invites"))
        .unwrap_or("invites");
    let namespace_ids = invitations_for_scope(&scope);
    let invitations = match auth_state
        .db()
        .list_namespace_invitations(namespace_ids.as_deref())
        .await
    {
        Ok(invitations) => invitations,
        Err(error) => {
            log::error!("Failed to load namespace invitations: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let users = if active_tab == "users" {
        match auth_state
            .db()
            .list_users_for_management(namespace_ids.as_deref())
            .await
        {
            Ok(mut users) => {
                for user in &mut users {
                    user.is_site_admin = server.is_configured_admin(&user.username);
                }
                users
            }
            Err(error) => {
                log::error!("Failed to load users for User Management: {error}");
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        Vec::new()
    };
    let content = if req.headers().contains_key("HX-Request") {
        render_user_management_content(&scope, active_tab, &invitations, &users).into_string()
    } else {
        let page = maud::html! {
            (crate::http::tree::pages::render_tree_hub(
                server.is_test_user_enabled(),
                scope.is_site_admin,
                true,
                Some("users"),
            ))
            (render_user_management(&scope, active_tab, &invitations, &users))
        };
        render_layout(&page, Some(&scope.user.username), Some("User Management")).into_string()
    };
    HttpResponse::Ok().content_type("text/html").body(content)
}
