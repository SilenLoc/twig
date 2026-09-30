use actix_web::{Error, FromRequest, HttpRequest, HttpResponse, Responder, error, get, web};
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::{auth::TwigContext, config, git};

#[derive(Debug, Deserialize, Serialize)]
pub struct NamespaceTree {
    pub namespaces: Vec<NamespaceNode>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NamespaceNode {
    pub name: String,
    pub repositories: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct VersionResponse {
    pub version: &'static str,
}

/// Returns the current application version as JSON.
#[get("/api/version")]
pub async fn version_endpoint() -> impl Responder {
    web::Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Extracts and decodes a `MessagePack` request body.
///
/// Actix keeps the request head and streaming payload separate, so callers
/// must pass the request's mutable `dev::Payload` alongside its `HttpRequest`.
#[allow(dead_code)]
pub async fn msgpack_extractor<T: for<'de> Deserialize<'de>>(
    req: HttpRequest,
    payload: web::Payload,
) -> Result<T, Error> {
    let mut payload = payload.into_inner();
    let bytes = Bytes::from_request(&req, &mut payload).await?;
    rmp_serde::from_slice(&bytes)
        .map_err(|e| error::ErrorBadRequest(format!("MessagePack decode error: {e}")))
}

pub fn msgpack_responder<T: Serialize>(data: T) -> impl Responder {
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
) -> Result<impl Responder, Error> {
    let username = crate::view::session_auth::get_username_from_request(&req, &auth_state).await;
    let is_logged_in = username.is_some();

    let namespaces = match auth_state.db().get_all_namespaces_with_owners().await {
        Ok(namespaces) => namespaces,
        Err(e) => {
            log::error!("Failed to get namespaces for tree endpoint: {e}");
            return Err(error::ErrorInternalServerError(
                "Failed to load namespace tree",
            ));
        }
    };

    let tree = NamespaceTree {
        namespaces: namespaces
            .into_iter()
            .map(|(namespace, _owner)| {
                let mut repositories =
                    git::bare::get_repos_with_info(server.project_root(), &namespace.name)
                        .into_iter()
                        .filter(|repo| is_logged_in || !repo.is_private)
                        .map(|repo| repo.name)
                        .collect::<Vec<_>>();
                repositories.sort();

                NamespaceNode {
                    name: namespace.name,
                    repositories,
                }
            })
            .collect(),
    };

    Ok(msgpack_responder(tree))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{
        App, HttpRequest, Responder, http::header::CONTENT_TYPE, test as aw_test, web,
    };

    fn test_config(project_root: &str, db_path: &str) -> config::Server {
        config::Server::new(
            ("127.0.0.1".to_string(), 8080),
            "debug".to_string(),
            project_root.to_string(),
            db_path.to_string(),
            "secure".to_string(),
            false,
            1.0,
        )
    }

    #[actix_web::test]
    async fn test_version_endpoint_returns_package_version() {
        let app = aw_test::init_service(App::new().service(version_endpoint)).await;
        let response = aw_test::call_service(
            &app,
            aw_test::TestRequest::get().uri("/api/version").to_request(),
        )
        .await;

        assert_eq!(response.status(), actix_web::http::StatusCode::OK);
        let body: serde_json::Value = aw_test::read_body_json(response).await;
        assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
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

    async fn echo_tree(req: HttpRequest, payload: web::Payload) -> Result<impl Responder, Error> {
        let tree_data: NamespaceTree = msgpack_extractor(req, payload).await?;
        Ok(msgpack_responder(tree_data))
    }

    #[actix_web::test]
    async fn test_msgpack_extractor_rejects_invalid_messagepack() {
        let app = aw_test::init_service(App::new().route("/echo", web::post().to(echo_tree))).await;
        let response = aw_test::call_service(
            &app,
            aw_test::TestRequest::post()
                .uri("/echo")
                .set_payload(vec![0xc1])
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);
        let body = aw_test::read_body(response).await;
        assert_eq!(
            body,
            "MessagePack decode error: wrong msgpack marker Reserved"
        );
    }

    #[actix_web::test]
    async fn test_tree_endpoint_returns_namespaces_and_repositories() {
        let id = uuid::Uuid::new_v4();
        let project_root = std::env::temp_dir().join(format!("twig_api_root_{id}"));
        let db_path = std::env::temp_dir().join(format!("twig_api_db_{id}.db"));
        std::fs::create_dir_all(project_root.join("silen")).expect("create namespace");
        git2::Repository::init_bare(project_root.join("silen").join("twig"))
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
        git2::Repository::init_bare(project_root.join("silen").join("public"))
            .expect("create repository");

        let secret_repo = project_root.join("silen").join("secret");
        std::fs::create_dir_all(&secret_repo).expect("create secret repo dir");
        crate::git::repo::bare_init(&secret_repo, "main", "Owner", "owner@example.com")
            .expect("bare init");
        {
            let sh = xshell::Shell::new().unwrap();
            let _p = sh.push_dir(&secret_repo);
            let blob = xshell::cmd!(sh, "git hash-object -w --stdin")
                .stdin("private = true\n")
                .read()
                .unwrap();
            let tree = xshell::cmd!(sh, "git mktree")
                .stdin(format!("100644 blob {blob}\t.twig.toml\n"))
                .read()
                .unwrap();
            let commit = xshell::cmd!(sh, "git commit-tree {tree} -m 'private'")
                .read()
                .unwrap();
            xshell::cmd!(sh, "git update-ref refs/heads/main {commit}")
                .run()
                .unwrap();
        }

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

        let token = "test_token_for_tree_endpoint_12345678901234567890123456789012";
        db.create_token(token, &user.id)
            .await
            .expect("create token");

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
        assert_eq!(decoded.namespaces[0].repositories, ["public"]);

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
        assert_eq!(decoded.namespaces[0].repositories, ["public", "secret"]);

        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_dir_all(project_root);
    }
}
