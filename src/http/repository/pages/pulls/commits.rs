//! The Commits view of a request: the commits its head adds beyond its base.

use actix_web::{HttpRequest, Result as AwResult, get, web};
use maud::{Markup, html};

use crate::{
    auth::TwigContext,
    config,
    http::repository::pages::{
        context::{RepoContext, open_repo},
        tabs::render_tab_response,
    },
};

use super::{request_for_page, request_nav};

/// `GET /{namespace}/{repo}/pulls/{number}/commits`
#[get("/{namespace}/{repo}/pulls/{number:\\d+}/commits")]
pub async fn commits_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<CommitsPath>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, &params.namespace, &params.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };
    let pull = match request_for_page(&ctx, &auth_state, params.number).await {
        Ok(pull) => pull,
        Err(notice) => return Ok(notice),
    };
    let body = match render_commits(&ctx, &pull) {
        Ok(body) => body,
        Err(error) => {
            log::error!("Failed to list request commits: {error}");
            return Ok(crate::http::view::render_error(
                "Commits could not be loaded.",
            ));
        }
    };
    Ok(render_tab_response(&req, &ctx.frame(), "pulls", &body))
}

#[derive(serde::Deserialize)]
pub(super) struct CommitsPath {
    namespace: String,
    repo: String,
    number: i64,
}

fn render_commits(
    ctx: &RepoContext,
    pull: &crate::db::pull_requests::PullRequest,
) -> Result<Markup, git2::Error> {
    let base = ctx.handle.branch_tip(&pull.base_branch)?;
    let head = ctx.handle.branch_tip(&pull.head_branch)?;
    let commits = match (base, head) {
        (Some(base), Some(head)) => ctx
            .handle
            .commit_ids_in_range(base, head)?
            .into_iter()
            .map(|oid| ctx.handle.commit_summary(oid))
            .collect::<Result<Vec<_>, _>>()?,
        _ => Vec::new(),
    };
    let namespace = &ctx.namespace;
    let repo = &ctx.repo;
    let number = pull.number;
    Ok(html! {
        div class="twig-stack twig-pr" {
            (request_nav(namespace, repo, number, "commits"))
            p class="twig-hint" {
                (commits.len()) " " (if commits.len() == 1 { "commit" } else { "commits" })
            }
            @if commits.is_empty() {
                div class="twig-empty" {
                    p class="twig-empty-body" { "This request adds no commits." }
                }
            } @else {
                ol class="twig-commits twig-pr-commits" {
                    @for commit in &commits {
                        li class="twig-commit" {
                            span class="twig-commit-hash twig-mono" { (commit.short_id) }
                            span class="twig-commit-msg" { (commit.summary) }
                            span class="twig-commit-author" { (commit.author_name) }
                        }
                    }
                }
            }
        }
    })
}
