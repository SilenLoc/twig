use actix_web::{HttpRequest, HttpResponse, get, web};

use crate::auth::{
    TwigContext,
    permissions::{self, NamespacePermission},
};
use crate::config;
use crate::git;

use super::NamespacePageData;
use super::hub::render_tree_hub;

pub(super) async fn load_namespace_page_data(
    db: &crate::db::Database,
    server: &config::Server,
    user_id: &str,
    is_site_admin: bool,
) -> Result<NamespacePageData, String> {
    let manageable_namespaces = permissions::namespaces_for_user(
        db,
        user_id,
        is_site_admin,
        NamespacePermission::ManageNamespace,
    )
    .await
    .map_err(|error| format!("Failed to load manageable namespaces: {error}"))?
    .into_iter()
    .map(|namespace| namespace.name)
    .collect::<Vec<_>>();
    let repository_namespace_names = permissions::namespaces_for_user(
        db,
        user_id,
        is_site_admin,
        NamespacePermission::ManageRepositories,
    )
    .await
    .map_err(|error| format!("Failed to load repository-manageable namespaces: {error}"))?
    .into_iter()
    .map(|namespace| namespace.name)
    .collect::<Vec<_>>();
    let user_management_enabled = match permissions::namespaces_for_user(
        db,
        user_id,
        is_site_admin,
        NamespacePermission::ManageMembers,
    )
    .await
    {
        Ok(namespaces) => is_site_admin || !namespaces.is_empty(),
        Err(error) => {
            log::error!("Failed to load member-manageable namespaces: {error}");
            false
        }
    };
    let deletable_namespaces = manageable_namespaces
        .iter()
        .filter(|namespace| {
            !git::bare::namespace::has_any_repository(server.project_root(), namespace)
        })
        .cloned()
        .collect();
    let repos_by_namespace = repository_namespace_names
        .iter()
        .filter_map(|namespace| {
            let repos: Vec<_> = git::bare::get_repos_with_info(server.project_root(), namespace)
                .into_iter()
                .filter(|repo| git::reserved::validate_repo_name(&repo.name).is_ok())
                .collect();
            (!repos.is_empty()).then(|| (namespace.clone(), repos))
        })
        .collect();

    Ok(NamespacePageData {
        user_management_enabled,
        repository_namespace_names,
        manageable_namespaces,
        deletable_namespaces,
        repos_by_namespace,
    })
}

#[get("/manage/repositories")]
pub async fn repositories_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> HttpResponse {
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Found()
            .insert_header(("Location", "/auth/login"))
            .finish();
    };
    let db = auth_state.db();
    let Some(user) = (match db.get_user_by_id(&user_id).await {
        Ok(user) => user,
        Err(error) => {
            log::error!("Failed to load user for repository tree: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    }) else {
        return HttpResponse::Unauthorized()
            .insert_header(("Location", "/auth/login"))
            .finish();
    };
    let is_site_admin = server.is_configured_admin(&user.username);
    let namespaces = match permissions::namespaces_for_user(
        db,
        &user_id,
        is_site_admin,
        NamespacePermission::ManageRepositories,
    )
    .await
    {
        Ok(namespaces) => namespaces,
        Err(error) => {
            log::error!("Failed to load repository-manageable namespaces: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let user_management_enabled = match permissions::namespaces_for_user(
        db,
        &user_id,
        is_site_admin,
        NamespacePermission::ManageMembers,
    )
    .await
    {
        Ok(namespaces) => is_site_admin || !namespaces.is_empty(),
        Err(error) => {
            log::error!("Failed to load member-manageable namespaces: {error}");
            false
        }
    };
    let mut renameable_repos_by_namespace: Vec<(String, Vec<git::bare::RepoInfo>)> = Vec::new();
    let mut deletable_repos_by_namespace: Vec<(String, Vec<git::bare::RepoInfo>)> = Vec::new();
    for namespace in namespaces {
        let renameable_repos: Vec<_> =
            git::bare::get_repos_with_info(server.project_root(), &namespace.name)
                .into_iter()
                .filter(|repo| git::reserved::validate_repo_name(&repo.name).is_ok())
                .collect();
        if !renameable_repos.is_empty() {
            renameable_repos_by_namespace.push((namespace.name.clone(), renameable_repos));
        }

        let deletable_repos: Vec<_> =
            git::bare::get_repos_with_info(server.project_root(), &namespace.name)
                .into_iter()
                .filter(|repo| {
                    git::bare::TwigConfig::load(server.project_root(), &namespace.name, &repo.name)
                        .deleteable
                })
                .collect();
        if !deletable_repos.is_empty() {
            deletable_repos_by_namespace.push((namespace.name, deletable_repos));
        }
    }

    let content = maud::html! {
        (render_tree_hub(
            server.is_test_user_enabled(),
            is_site_admin,
            user_management_enabled,
            Some("repositories"),
        ))
        div class="twig-bento" {
            (crate::http::settings::pages::render_repo_rename_panel(&renameable_repos_by_namespace))
            (crate::http::settings::pages::render_repo_deletion_panel(&deletable_repos_by_namespace))
        }
    };
    let content = if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::http::view::render_layout(&content, Some(&user.username), Some("Repository"))
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}
