use std::path::Path;

use actix_web::{HttpRequest, HttpResponse, Responder, post, web};
use log::info;

use crate::auth::TwigContext;
use crate::auth::permissions::NamespacePermission;
use crate::config;
use crate::git;
use crate::http::view::{render_error, render_success};

use super::shared::require_namespace_permission;
use super::*;

#[post("/settings/rename-namespace")]
pub async fn rename_namespace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<RenameNamespaceForm>,
) -> impl Responder {
    if !git::bare::is_safe_component(&form.namespace) {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace name").into_string());
    }
    if form.new_name.len() < 2 {
        return HttpResponse::BadRequest()
            .body(render_error("Namespace name must be at least 2 characters").into_string());
    }
    if form.new_name.starts_with('_') {
        return HttpResponse::BadRequest()
            .body(render_error("Namespace names starting with '_' are reserved").into_string());
    }
    if !git::bare::is_safe_component(&form.new_name) {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace name").into_string());
    }
    if form.new_name.trim() != form.new_name {
        return HttpResponse::BadRequest()
            .body(render_error("Invalid namespace name").into_string());
    }
    if form.new_name == form.namespace {
        return HttpResponse::BadRequest()
            .body(render_error("Choose a different namespace name").into_string());
    }

    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };

    let db = auth_state.db();
    let namespace = match db.get_namespace_by_name(&form.namespace).await {
        Ok(Some(namespace)) => namespace,
        Ok(None) => {
            return HttpResponse::NotFound()
                .body(render_error("Namespace not found").into_string());
        }
        Err(e) => {
            log::error!("Failed to load namespace: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    };
    if let Err(response) = require_namespace_permission(
        db,
        &server,
        &user_id,
        &form.namespace,
        NamespacePermission::ManageNamespace,
    )
    .await
    {
        return response;
    }

    match db.get_namespace_by_name(&form.new_name).await {
        Ok(Some(_)) => {
            return HttpResponse::Conflict()
                .body(render_error("Namespace already exists").into_string());
        }
        Ok(None) => {}
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    }

    let root = Path::new(server.project_root());
    let source = root.join(&form.namespace);
    let destination = root.join(&form.new_name);

    if destination.exists() {
        return HttpResponse::Conflict().body(
            render_error("A directory with that namespace name already exists").into_string(),
        );
    }

    let moved_directory = source.exists();
    if moved_directory && let Err(e) = std::fs::rename(&source, &destination) {
        log::error!("Failed to rename namespace directory: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to rename namespace").into_string());
    }

    if let Err(e) = db.rename_namespace(&namespace.id, &form.new_name).await {
        log::error!("Failed to rename namespace: {e}");
        if moved_directory {
            let _ = std::fs::rename(&destination, &source);
        }
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to rename namespace").into_string());
    }

    info!(
        "Renamed namespace '{}' to '{}' by user: {}",
        form.namespace, form.new_name, user_id
    );
    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_success(&format!("Namespace renamed to {}.", form.new_name)).into_string())
}

#[post("/settings/delete-namespace/")]
pub async fn delete_namespace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<NamespaceForm>,
) -> impl Responder {
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };

    let db = auth_state.db();

    let namespace = match db.get_namespace_by_name(&form.namespace).await {
        Ok(Some(ns)) => ns,
        Ok(None) => {
            return HttpResponse::NotFound()
                .body(render_error("Namespace not found").into_string());
        }
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    };

    if let Err(response) = require_namespace_permission(
        db,
        &server,
        &user_id,
        &form.namespace,
        NamespacePermission::ManageNamespace,
    )
    .await
    {
        return response;
    }

    if git::bare::namespace::has_any_repository(server.project_root(), &form.namespace) {
        return HttpResponse::BadRequest().body(
            render_error(
                "Cannot delete namespace: it still contains repositories. Delete all repositories first.",
            )
            .into_string(),
        );
    }

    if let Err(e) = db.delete_namespace(&namespace.id).await {
        log::error!("Failed to delete namespace: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to delete namespace").into_string());
    }

    let namespace_dir = Path::new(server.project_root()).join(&form.namespace);
    if namespace_dir.is_dir() {
        let _ = std::fs::remove_dir(&namespace_dir);
    }

    info!(
        "Deleted namespace '{}' by user: {}",
        form.namespace, user_id
    );

    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_success("Namespace deleted successfully.").into_string())
}
