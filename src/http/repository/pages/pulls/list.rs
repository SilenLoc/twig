//! The Pull requests list: state filters with counts, one row per request, and
//! the New pull request button for people who may push.

use actix_web::web;
use maud::{Markup, html};

use crate::{
    db::pull_requests::{PullRequest, PullRequestFilter, PullRequestState},
    http::repository::pages::context::RepoContext,
    pulls::{self, RepoRef},
};

use super::author_name;

/// Loads the requested page of the list and renders the whole tab body.
pub(super) async fn load(
    ctx: &RepoContext,
    auth_state: &web::Data<crate::auth::TwigContext>,
    state: &str,
) -> Result<Markup, String> {
    let repo_id = ctx.handle.repo_id().map_err(|error| error.to_string())?;
    let db = auth_state.db();
    let scope = RepoRef { repo_id: &repo_id };
    let selected = match state {
        "closed" => Some(PullRequestState::Closed),
        "all" => None,
        _ => Some(PullRequestState::Open),
    };
    let filter = PullRequestFilter {
        state: selected,
        limit: 50,
        ..Default::default()
    };
    let (rows, _) = pulls::list(db, &scope, &filter)
        .await
        .map_err(|error| format!("{error:?}"))?;
    let (open, closed) = db.count_pull_requests_by_state(&repo_id).await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let author = author_name(auth_state, &row).await;
        items.push((row, author));
    }
    Ok(render(ctx, state, &items, open, closed, ctx.can_edit))
}

fn render(
    ctx: &RepoContext,
    state: &str,
    rows: &[(PullRequest, String)],
    open: u64,
    closed: u64,
    can_create: bool,
) -> Markup {
    let namespace = &ctx.namespace;
    let repo = &ctx.repo;
    let tab = |value: &str, label: String| {
        html! {
            a class="twig-pr-filter" href=(format!("/{namespace}/{repo}/pulls?state={value}"))
                aria-current=[(state == value).then_some("page")] {
                (label)
            }
        }
    };
    html! {
        div class="twig-stack" {
            div class="twig-pr-toolbar" {
                nav class="twig-pr-filters" aria-label="Pull request state" {
                    (tab("open", format!("{open} Open")))
                    (tab("closed", format!("{closed} Closed")))
                    (tab("all", "All".to_string()))
                }
                @if can_create {
                    a class="twig-btn twig-btn--primary" href=(format!("/{namespace}/{repo}/pulls/new")) {
                        "New pull request"
                    }
                }
            }
            @if rows.is_empty() {
                div class="twig-empty" {
                    p class="twig-eyebrow" { "NO PULL REQUESTS" }
                    p class="twig-empty-body" {
                        "There are no pull requests in this view."
                    }
                }
            } @else {
                section class="twig-panel twig-panel--flush" {
                    div class="twig-panel-body" {
                        @for (pull, author) in rows {
                            (row(namespace, repo, pull, author))
                        }
                    }
                }
            }
        }
    }
}

fn row(namespace: &str, repo: &str, pull: &PullRequest, author: &str) -> Markup {
    let number = pull.number;
    let (state_label, state_key) = match (pull.state, pull.merged_at.is_some()) {
        (_, true) => ("Merged", "merged"),
        (PullRequestState::Open, false) => ("Open", "open"),
        (PullRequestState::Closed, false) => ("Closed", "closed"),
    };
    let opened = pull.created_at.split('T').next().unwrap_or_default();
    html! {
        div class="twig-row twig-pr-row" {
            div class="twig-pr-row-title" {
                span class="twig-pr-state" data-state=(state_key) { (state_label) }
                a href=(format!("/{namespace}/{repo}/pulls/{number}")) { (pull.title) }
            }
            div class="twig-row-meta" {
                "#" (number) " opened " (opened) " by " (author) " · "
                span class="twig-mono" { (pull.head_branch) } " into "
                span class="twig-mono" { (pull.base_branch) }
            }
        }
    }
}
