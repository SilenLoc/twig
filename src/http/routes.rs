use actix_web::{guard, web};

use super::{
    api, assets, auth, health, info, namespace, overview, repository, settings, test_page, tree,
};
use crate::git;

/// Registers every route in the same order as the previous inline chain.
/// Static/UI routes must remain before the dynamic namespace/repository paths
/// they would otherwise be shadowed by.
pub(crate) fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(health::health)
        .service(health::up)
        .service(assets::assets)
        .service(tree::api::tree_endpoint)
        .service(api::version_endpoint)
        .service(info::pages::index)
        // Auth UI endpoints (HTML forms)
        .service(auth::pages::invite_page)
        .service(auth::pages::signup_page)
        .service(auth::pages::login_page)
        .service(auth::pages::namespace_page)
        .service(auth::handlers::create_invite_ui_handler)
        .service(auth::handlers::signup_ui_handler)
        .service(auth::handlers::login_ui_handler)
        .service(auth::handlers::create_namespace_ui_handler)
        .service(auth::handlers::logout_ui_handler)
        // Web UI endpoints (MUST come before git routes to avoid pattern conflicts)
        .service(overview::pages::index)
        .service(tree::pages::tree_page)
        .service(tree::pages::namespaces_page)
        .service(tree::pages::repositories_page)
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
        .service(test_page::pages::test_pin_qr)
        // Settings page MUST come before namespace handler (which matches /{namespace})
        .service(settings::pages::settings_page)
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
        .service(repository::pages::handler)
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
        .service(repository::editor::edit_file_page)
        .service(repository::editor::commit_file_handler)
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
