use actix_web::{Error, FromRequest, HttpRequest, HttpResponse, Responder, error, get, web};
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::{auth::FigContext, config, git};

#[derive(Debug, Deserialize, Serialize)]
pub struct NamespaceTree {
    pub namespaces: Vec<NamespaceNode>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NamespaceNode {
    pub name: String,
    pub repositories: Vec<String>,
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
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
) -> Result<impl Responder, Error> {
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

    #[test]
    fn test_tree_round_trips_as_named_messagepack() {
        let tree_data = NamespaceTree {
            namespaces: vec![NamespaceNode {
                name: "silen".to_string(),
                repositories: vec!["fig".to_string(), "site".to_string()],
            }],
        };

        let bytes = rmp_serde::to_vec_named(&tree_data).expect("serialize tree");
        let decoded: NamespaceTree = rmp_serde::from_slice(&bytes).expect("deserialize tree");
        assert_eq!(decoded.namespaces[0].name, "silen");
        assert_eq!(decoded.namespaces[0].repositories, ["fig", "site"]);
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
        let project_root = std::env::temp_dir().join(format!("fig_api_root_{id}"));
        let db_path = std::env::temp_dir().join(format!("fig_api_db_{id}.db"));
        std::fs::create_dir_all(project_root.join("silen")).expect("create namespace");
        git2::Repository::init_bare(project_root.join("silen").join("fig"))
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
                .app_data(web::Data::new(FigContext::new(db, "secure".to_string())))
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
        assert_eq!(decoded.namespaces[0].repositories, ["fig"]);

        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_dir_all(project_root);
    }
}
