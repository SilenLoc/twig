use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use maud::Markup;
use serde::Deserialize;

use crate::{
    config,
    git::{
        self,
        bare::{Commit, Depth},
    },
};

#[derive(Deserialize)]
struct Params {
    namespace: String,
    repo: String,
}

#[get("/{namespace}/{repo}")]
pub async fn handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    // make depth configurable
    let commits = git::bare::get_commits(
        server.project_root(),
        &params.namespace,
        &params.repo,
        Depth::default(),
    );

    let content = match commits {
        Ok(commits) => render_commits(&commits),
        Err(e) => render_git_error(e),
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(super::render_layout(&content))
    }
}

fn render_git_error(e: git2::Error) -> Markup {
    let code = e.code();
    let code = format!("{:?}", code);
    let klass = e.class();
    let klass = format!("{:?}", klass);
    let message = e.message();
    maud::html! {
        p { (message) }
        p { (code) }
        p { (klass) }
    }
}

fn render_commits(commits: &Vec<Commit>) -> Markup {
    maud::html! {
        ol {
        @for commit in commits {
            li {
                (render_commit(&commit))
            }
        }
        }
    }
}

fn render_commit(commit: &Commit) -> Markup {
    let hash = &commit.hash();
    let author = &commit.author();
    let date = &commit.date();
    let commit_message = &commit.commit_message();
    maud::html! {
            p { (hash) }
            p { (author) }
            p { (date) }
            p { (commit_message) }
    }
}
