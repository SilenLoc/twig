use actix_web::{Error, HttpRequest, HttpResponse, error, get, web};
use serde::{Deserialize, Serialize};

use crate::auth::permissions::{self, NamespacePermission};
use crate::auth::{CredentialAuth, TwigContext};
use crate::config;
use twig_git as git;

#[derive(Debug, Deserialize, Serialize)]
pub struct NamespaceTree {
    pub namespaces: Vec<NamespaceNode>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NamespaceNode {
    pub name: String,
    pub repositories: Vec<String>,
}

pub fn msgpack_responder<T: Serialize>(data: T) -> HttpResponse {
    match rmp_serde::to_vec_named(&data) {
        Ok(body) => HttpResponse::Ok()
            .content_type("application/msgpack")
            .body(body),
        Err(e) => {
            log::error!("MessagePack serialization failed: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

/// Returns the public namespace/repository hierarchy as named `MessagePack`.
#[get("/api/tree")]
pub async fn tree_endpoint(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> Result<HttpResponse, Error> {
    let username = crate::http::auth::session::get_username_from_request(&req, &auth_state).await;
    let token_principal = if username.is_none() {
        match auth_state.authenticate_bearer(&req).await {
            Ok(CredentialAuth::Authenticated(principal)) => Some(principal),
            Ok(CredentialAuth::Invalid) => return Ok(HttpResponse::Unauthorized().finish()),
            Ok(CredentialAuth::Missing) => None,
            Err(error) => {
                log::error!("Failed to authenticate tree API bearer token: {error}");
                return Err(error::ErrorInternalServerError("Authentication failed"));
            }
        }
    } else {
        None
    };

    let namespaces = match auth_state.db().get_all_namespaces_with_owners().await {
        Ok(namespaces) => namespaces,
        Err(e) => {
            log::error!("Failed to get namespaces for tree endpoint: {e}");
            return Err(error::ErrorInternalServerError(
                "Failed to load namespace tree",
            ));
        }
    };

    let mut tree_namespaces = Vec::with_capacity(namespaces.len());
    for (namespace, _owner) in namespaces {
        let can_view_private = if let Some(principal) = &token_principal {
            match permissions::for_principal(
                auth_state.db(),
                principal,
                &namespace.name,
                server.is_configured_admin(&principal.user.username),
            )
            .await
            {
                Ok(Some(permissions)) => permissions.contains(NamespacePermission::ViewNamespace),
                Ok(None) => false,
                Err(error) => {
                    log::error!("Failed to resolve token tree permissions: {error}");
                    return Err(error::ErrorInternalServerError(
                        "Failed to resolve tree permissions",
                    ));
                }
            }
        } else {
            username.is_some()
        };

        let mut repositories =
            git::bare::get_repos_with_info(server.project_root(), &namespace.name)
                .into_iter()
                .filter(|repo| can_view_private || !repo.is_private)
                .map(|repo| repo.name)
                .collect::<Vec<_>>();
        repositories.sort();

        tree_namespaces.push(NamespaceNode {
            name: namespace.name,
            repositories,
        });
    }
    let tree = NamespaceTree {
        namespaces: tree_namespaces,
    };

    Ok(msgpack_responder(tree))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, http::header::CONTENT_TYPE, test as aw_test, web};

    fn test_config(project_root: &str, db_path: &str) -> config::Server {
        config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            project_root.to_string(),
            db_path.to_string(),
            "secure".to_string(),
            false,
        )
    }

    fn init_private_repository(repo_path: &std::path::Path) {
        std::fs::create_dir_all(repo_path).expect("create private repo directory");
        twig_git::repo::bare_init(repo_path, "main", "Owner", "owner@example.com")
            .expect("bare init private repository");
        let sh = xshell::Shell::new().expect("create shell");
        let _p = sh.push_dir(repo_path);
        let blob = xshell::cmd!(sh, "git hash-object -w --stdin")
            .stdin("private = true\n")
            .read()
            .expect("write private repo config");
        let tree = xshell::cmd!(sh, "git mktree")
            .stdin(format!("100644 blob {blob}\t.twig.toml\n"))
            .read()
            .expect("create private repo tree");
        let commit = xshell::cmd!(sh, "git commit-tree {tree} -m 'private'")
            .read()
            .expect("create private repo commit");
        xshell::cmd!(sh, "git update-ref refs/heads/main {commit}")
            .run()
            .expect("set private repo head");
    }

    fn repositories_in<'a>(tree: &'a NamespaceTree, namespace: &str) -> &'a [String] {
        tree.namespaces
            .iter()
            .find(|node| node.name == namespace)
            .map(|node| node.repositories.as_slice())
            .expect("namespace exists in tree")
    }

    #[test]
    fn test_tree_round_trips_as_named_messagepack() {
        let tree_data = NamespaceTree {
            namespaces: vec![NamespaceNode {
                name: "silen".to_string(),
                repositories: vec!["twig".to_string(), "site".to_string()],
            }],
        };

        let bytes = rmp_serde::to_vec_named(&tree_data).expect("serialize tree");
        let decoded: NamespaceTree = rmp_serde::from_slice(&bytes).expect("deserialize tree");
        assert_eq!(decoded.namespaces[0].name, "silen");
        assert_eq!(decoded.namespaces[0].repositories, ["twig", "site"]);
    }

    #[actix_web::test]
    async fn test_tree_endpoint_returns_namespaces_and_repositories() {
        let id = uuid::Uuid::new_v4();
        let project_root = std::env::temp_dir().join(format!("twig_api_root_{id}"));
        let db_path = std::env::temp_dir().join(format!("twig_api_db_{id}.db"));
        std::fs::create_dir_all(project_root.join("silen")).expect("create namespace");
        std::fs::create_dir_all(project_root.join("silen").join("twig"))
            .expect("create repository directory");
        twig_git::repo::bare_init(
            &project_root.join("silen").join("twig"),
            "main",
            "Test",
            "test@example.com",
        )
        .expect("create repository");

        let db = crate::db::Database::new(db_path.to_str().expect("db path"));
        db.init_tables().await.expect("init tables");
        let user = crate::auth::create_user(
            "owner".to_string(),
            "owner@example.com".to_string(),
            "password",
        )
        .expect("create user");
        db.create_user(&user).await.expect("store user");
        let namespace = crate::auth::create_namespace("silen".to_string(), user.id);
        db.create_namespace(&namespace)
            .await
            .expect("store namespace");

        let app = aw_test::init_service(
            App::new()
                .app_data(web::Data::new(test_config(
                    project_root.to_str().expect("project root"),
                    db_path.to_str().expect("db path"),
                )))
                .app_data(web::Data::new(TwigContext::new(db, "secure".to_string())))
                .service(tree_endpoint),
        )
        .await;

        let response = aw_test::call_service(
            &app,
            aw_test::TestRequest::get().uri("/api/tree").to_request(),
        )
        .await;
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "application/msgpack"
        );
        let body = aw_test::read_body(response).await;
        let decoded: NamespaceTree = rmp_serde::from_slice(&body).expect("decode response");
        assert_eq!(decoded.namespaces.len(), 1);
        assert_eq!(decoded.namespaces[0].name, "silen");
        assert_eq!(decoded.namespaces[0].repositories, ["twig"]);

        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_dir_all(project_root);
    }

    #[actix_web::test]
    async fn test_tree_endpoint_filters_private_repositories() {
        let id = uuid::Uuid::new_v4();
        let project_root = std::env::temp_dir().join(format!("twig_api_root_{id}"));
        let db_path = std::env::temp_dir().join(format!("twig_api_db_{id}.db"));
        std::fs::create_dir_all(project_root.join("silen")).expect("create namespace");
        std::fs::create_dir_all(project_root.join("silen").join("public"))
            .expect("create repository directory");
        twig_git::repo::bare_init(
            &project_root.join("silen").join("public"),
            "main",
            "Test",
            "test@example.com",
        )
        .expect("create repository");

        init_private_repository(&project_root.join("silen").join("secret"));
        init_private_repository(&project_root.join("other").join("private"));

        let db = crate::db::Database::new(db_path.to_str().expect("db path"));
        db.init_tables().await.expect("init tables");
        let user = crate::auth::create_user(
            "owner".to_string(),
            "owner@example.com".to_string(),
            "password",
        )
        .expect("create user");
        db.create_user(&user).await.expect("store user");
        let namespace = crate::auth::create_namespace("silen".to_string(), user.id.clone());
        db.create_namespace(&namespace)
            .await
            .expect("store namespace");
        let other_namespace = crate::auth::create_namespace("other".to_string(), user.id.clone());
        db.create_namespace(&other_namespace)
            .await
            .expect("store other namespace");

        let token = "test_token_for_tree_endpoint_12345678901234567890123456789012";
        db.create_token(token, &user.id)
            .await
            .expect("create token");
        let view_other_token = "twigt_tree_other_view_123456789012345678901234567890";
        db.create_personal_access_token(
            "tree-view-other",
            &user.id,
            "View other namespace",
            &crate::auth::hash_personal_access_token(view_other_token),
            "7890",
            &[other_namespace.id.clone()],
            &[NamespacePermission::ViewNamespace],
            None,
        )
        .await
        .expect("create scoped read token");
        let push_silen_token = "twigt_tree_silen_push_123456789012345678901234567890";
        db.create_personal_access_token(
            "tree-push-silen",
            &user.id,
            "Push silen only",
            &crate::auth::hash_personal_access_token(push_silen_token),
            "7890",
            &[namespace.id.clone()],
            &[NamespacePermission::PushChanges],
            None,
        )
        .await
        .expect("create scoped write token");

        let app = aw_test::init_service(
            App::new()
                .app_data(web::Data::new(test_config(
                    project_root.to_str().expect("project root"),
                    db_path.to_str().expect("db path"),
                )))
                .app_data(web::Data::new(TwigContext::new(db, "secure".to_string())))
                .service(tree_endpoint),
        )
        .await;

        // Anonymous request should omit secret repo
        let response = aw_test::call_service(
            &app,
            aw_test::TestRequest::get().uri("/api/tree").to_request(),
        )
        .await;
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
        let body = aw_test::read_body(response).await;
        let decoded: NamespaceTree = rmp_serde::from_slice(&body).expect("decode response");
        assert_eq!(repositories_in(&decoded, "silen"), ["public"]);
        assert!(repositories_in(&decoded, "other").is_empty());

        // Authenticated request should include secret repo
        let response = aw_test::call_service(
            &app,
            aw_test::TestRequest::get()
                .uri("/api/tree")
                .cookie(actix_web::cookie::Cookie::new("session", token))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
        let body = aw_test::read_body(response).await;
        let decoded: NamespaceTree = rmp_serde::from_slice(&body).expect("decode response");
        assert_eq!(repositories_in(&decoded, "silen"), ["public", "secret"]);
        assert_eq!(repositories_in(&decoded, "other"), ["private"]);

        let response = aw_test::call_service(
            &app,
            aw_test::TestRequest::get()
                .uri("/api/tree")
                .insert_header(("Authorization", format!("Bearer {view_other_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
        let body = aw_test::read_body(response).await;
        let decoded: NamespaceTree = rmp_serde::from_slice(&body).expect("decode scoped tree");
        assert_eq!(repositories_in(&decoded, "silen"), ["public"]);
        assert_eq!(repositories_in(&decoded, "other"), ["private"]);

        let response = aw_test::call_service(
            &app,
            aw_test::TestRequest::get()
                .uri("/api/tree")
                .insert_header(("Authorization", format!("Bearer {push_silen_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
        let body = aw_test::read_body(response).await;
        let decoded: NamespaceTree = rmp_serde::from_slice(&body).expect("decode write-only tree");
        assert_eq!(repositories_in(&decoded, "silen"), ["public"]);
        assert!(repositories_in(&decoded, "other").is_empty());

        let response = aw_test::call_service(
            &app,
            aw_test::TestRequest::get()
                .uri("/api/tree")
                .insert_header(("Authorization", "Bearer twigt_invalid"))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), actix_web::http::StatusCode::UNAUTHORIZED);

        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_dir_all(project_root);
    }
}
