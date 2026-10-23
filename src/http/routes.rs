use actix_web::{guard, web};

use super::{
    api, assets, auth, health, info, namespace, overview, repo_edit, repository, settings,
    test_page, tree,
};
use crate::git;

/// Registers every route in the same order as the previous inline chain.
/// Static/UI routes must remain before the dynamic namespace/repository paths
/// they would otherwise be shadowed by.
pub(crate) fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.app_data(
        web::JsonConfig::default()
            .limit(2 << 20)
            .error_handler(api::v1::json_payload_error),
    );
    cfg.service(health::health)
        .service(health::up)
        .service(assets::assets)
        .service(api::version_endpoint);
    configure_v1_api_routes(cfg);

    cfg.service(info::pages::index);
    configure_auth_routes(cfg);
    configure_tree_routes(cfg);
    configure_repository_routes(cfg);
}

fn configure_auth_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(auth::pages::invite_page)
        .service(auth::pages::signup_page)
        .service(auth::pages::login_page)
        .service(auth::device::start_device_authorization)
        .service(auth::device::device_authorization_page)
        .service(auth::device::decide_device_authorization)
        .service(auth::device::poll_device_authorization)
        .service(auth::pages::namespace_page)
        .service(auth::namespace_invitations::accept_invitation_page)
        .service(auth::namespace_invitations::accept_invitation_handler)
        .service(auth::handlers::create_invite_ui_handler)
        .service(auth::handlers::signup_ui_handler)
        .service(auth::handlers::login_ui_handler)
        .service(auth::handlers::create_namespace_ui_handler)
        .service(auth::handlers::logout_ui_handler);
}

fn configure_tree_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(overview::pages::index)
        .service(overview::pages::pin_namespace)
        .service(tree::pages::tree_page)
        .service(tree::pages::namespaces_page)
        .service(tree::pages::namespace_settings_page)
        .service(tree::pages::repositories_page)
        .service(tree::user_management::user_management_page)
        .service(tree::user_management::create_namespace_invitation)
        .service(tree::user_management::resend_namespace_invitation)
        .service(tree::pages::data_page)
        .service(tree::pages::data_rows)
        // Test suite endpoints (gated by TEST_USER and admin auth)
        .service(test_page::pages::test_page_alias)
        .service(test_page::pages::test_runner)
        .service(test_page::pages::test_stopped)
        .service(test_page::pages::test_ping)
        .service(test_page::pages::test_feature_check)
        .service(test_page::pages::test_pin_create)
        .service(test_page::pages::test_pin_remove)
        .service(test_page::pages::test_pin_qr);
}

fn configure_repository_routes(cfg: &mut web::ServiceConfig) {
    // Settings and the namespace UI must precede dynamic repository routes.
    cfg.service(settings::pages::settings_page)
        .service(settings::pages::personal_access_tokens_page)
        .service(settings::pages::create_personal_access_token)
        .service(settings::pages::revoke_personal_access_token)
        .service(settings::pages::update_email)
        .service(settings::pages::move_repo)
        .service(settings::pages::rename_repo)
        .service(settings::pages::delete_repo)
        .service(settings::pages::rename_namespace)
        .service(settings::pages::delete_namespace)
        .service(namespace::pages::handler)
        .service(namespace::pages::create_repo_form_handler)
        .service(namespace::pages::create_repo_handler)
        .service(repository::binaries::upload_binary)
        .service(repository::binaries::download_binary)
        .service(repo_edit::workspace_page)
        .service(repo_edit::tree_fragment)
        .service(
            web::resource("/{namespace}/{repo}/edit/operation")
                .app_data(
                    web::FormConfig::default().limit(crate::http::body::DEFAULT_SMALL_BODY_LIMIT),
                )
                .route(web::post().to(repo_edit::apply_operation)),
        )
        .service(
            web::resource("/{namespace}/{repo}/edit/commit")
                .app_data(
                    web::FormConfig::default().limit(crate::http::body::DEFAULT_SMALL_BODY_LIMIT),
                )
                .route(web::post().to(repo_edit::commit_workspace)),
        )
        .service(repo_edit::file_fragment)
        .service(repo_edit::legacy_file_entry)
        .service(repository::pages::handler)
        .service(repository::pages::hooks_tab_handler)
        .service(repository::pages::disable_hook_handler)
        .service(repository::pages::delete_hook_handler)
        .service(repository::pages::tab_handler)
        .service(repository::pages::markdown_tab_handler)
        .service(repository::pages::content_tab_handler)
        .service(repository::pages::commits_tab_handler)
        .service(repository::pages::config_tab_handler)
        .service(repository::pages::present_tab_handler)
        .service(repository::pages::present_print_handler)
        .service(repository::pages::paper_tab_handler)
        .service(repository::pages::scripts_tab_handler)
        .service(repository::pages::scripts_group_handler)
        .service(repository::pages::raw_handler)
        .service(repository::pages::license_tab_handler)
        .service(repository::pages::slide_handler)
        .service(repository::pages::markdown_handler)
        .service(repository::pages::content_handler)
        .service(repository::pages::paper_handler)
        // Git endpoints with auth
        .service(git::repo::init)
        .route(
            "/{namespace}/{repo}/{endpoint:.*}",
            web::get().guard(is_git()).to(git::git_handler),
        )
        .route(
            "/{namespace}/{repo}/{endpoint:.*}",
            web::post().guard(is_git()).to(git::git_handler),
        );
}

fn configure_v1_api_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(api::v1::me)
        .service(api::v1::namespaces::list_namespaces)
        .service(api::v1::namespaces::create_namespace)
        .service(api::v1::namespaces::get_namespace)
        .service(api::v1::namespaces::rename_namespace)
        .service(api::v1::namespaces::delete_namespace)
        .service(api::v1::namespaces::list_repositories)
        .service(api::v1::namespaces::create_repository)
        .service(api::v1::namespaces::list_members)
        .service(api::v1::namespaces::list_invitations)
        .service(api::v1::namespaces::create_invitation)
        .service(api::v1::namespaces::resend_invitation)
        .service(api::v1::repositories::get_repository)
        .service(api::v1::repositories::update_repository)
        .service(api::v1::repositories::delete_repository)
        .service(api::v1::repositories::list_branches)
        .service(api::v1::repositories::list_commits)
        .service(api::v1::contents::get_tree)
        .service(api::v1::contents::get_content)
        .service(api::v1::contents::put_content)
        .service(api::v1::contents::delete_content)
        .service(api::v1::contents::rename_content)
        .service(api::v1::binaries::list_binaries)
        .service(api::v1::binaries::get_binary)
        .service(api::v1::binaries::put_binary);
}

fn is_git() -> impl guard::Guard {
    guard::fn_guard(|ctx| {
        ctx.head()
            .headers
            .get("User-Agent")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ua| ua.starts_with("git/"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{HttpResponse, test as aw_test};

    #[actix_web::test]
    async fn test_is_git_guard_matches_git_user_agent() {
        let app = aw_test::init_service(
            actix_web::App::new().route(
                "/{namespace}/{repo}/{endpoint:.*}",
                web::get()
                    .guard(is_git())
                    .to(|| async { HttpResponse::Ok().body("git") }),
            ),
        )
        .await;

        let req = aw_test::TestRequest::get()
            .uri("/ns/repo/info/refs")
            .insert_header(("User-Agent", "git/2.43.0"))
            .to_request();
        let resp = aw_test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_is_git_guard_rejects_non_git_user_agent() {
        let app = aw_test::init_service(
            actix_web::App::new().route(
                "/{namespace}/{repo}/{endpoint:.*}",
                web::get()
                    .guard(is_git())
                    .to(|| async { HttpResponse::Ok().body("git") }),
            ),
        )
        .await;

        let req = aw_test::TestRequest::get()
            .uri("/ns/repo/info/refs")
            .insert_header(("User-Agent", "Mozilla/5.0"))
            .to_request();
        let resp = aw_test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_is_git_guard_rejects_missing_user_agent() {
        let app = aw_test::init_service(
            actix_web::App::new().route(
                "/{namespace}/{repo}/{endpoint:.*}",
                web::get()
                    .guard(is_git())
                    .to(|| async { HttpResponse::Ok().body("git") }),
            ),
        )
        .await;

        let req = aw_test::TestRequest::get()
            .uri("/ns/repo/info/refs")
            .to_request();
        let resp = aw_test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    }
}
