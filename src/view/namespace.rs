use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use serde::Deserialize;

use crate::{
    config,
    git::{self},
};

#[derive(Deserialize)]
struct Params {
    namespace: String,
}

#[get("/{namespace}")]
pub async fn handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    params: web::Path<Params>,
) -> AwResult<maud::Markup> {
    let namespace = &params.namespace;
    // todo handle error
    let repos = git::bare::get_repos(server.project_root(), namespace).unwrap();

    let content = maud::html! {
        @for repo in repos {
            a href=(format!("{}/{}", namespace, repo)) {
                (repo)
            }
        }
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(super::render_layout(&content))
    }
}
