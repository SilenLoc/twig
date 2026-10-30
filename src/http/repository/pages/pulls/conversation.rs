//! One pull request's Conversation page: header, timeline, the composer with
//! review and close or reopen actions, and the reviewers panel.

use actix_web::{HttpRequest, Result as AwResult, get, web};
use maud::{Markup, html};

use crate::{
    auth::{NamespacePermission, PermissionSet, TwigContext, User},
    config,
    db::pull_requests::{PullRequest, PullRequestState},
    http::repository::pages::{
        context::{RepoContext, open_repo},
        tabs::render_tab_response,
    },
    pulls::{self, RepoRef},
};

use super::signed_in;
use super::timeline::{self, Entry};

#[derive(serde::Deserialize)]
pub(super) struct PullPath {
    namespace: String,
    repo: String,
    number: i64,
}

/// `GET /{namespace}/{repo}/pulls/{number}`
#[get("/{namespace}/{repo}/pulls/{number:\\d+}")]
pub async fn pull_page_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, &path.namespace, &path.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };
    let viewer = signed_in(&req, &server, &auth_state, &path.namespace).await;
    let body = match load_and_render(&ctx, &auth_state, path.number, viewer.as_ref(), None).await {
        Ok(body) => body,
        Err(markup) => return Ok(markup),
    };
    Ok(render_tab_response(&req, &ctx.frame(), "pulls", &body))
}

/// Loads the request and everything its page shows, and renders the page body.
/// A missing request or a storage failure becomes an error notice instead.
pub(super) async fn load_and_render(
    ctx: &RepoContext,
    auth_state: &web::Data<TwigContext>,
    number: i64,
    viewer: Option<&(User, PermissionSet)>,
    notice: Option<&str>,
) -> Result<Markup, Markup> {
    let fail = |context: &str, error: String| {
        log::error!("Pull request page failed to {context}: {error}");
        crate::http::view::render_error("Pull request could not be loaded.")
    };
    let repo_id = ctx
        .handle
        .repo_id()
        .map_err(|error| fail("read repository identity", error.to_string()))?;
    let scope = RepoRef { repo_id: &repo_id };
    let pull = pulls::get(auth_state.db(), &scope, number)
        .await
        .map_err(|_| crate::http::view::render_error("Pull request was not found."))?;
    let db = auth_state.db();
    let comments = db
        .list_pull_request_comments(&pull.id)
        .await
        .map_err(|error| fail("load comments", error))?;
    let reviews = db
        .list_pull_request_reviews(&pull.id)
        .await
        .map_err(|error| fail("load reviews", error))?;
    let review_comments = db
        .list_review_comments(&pull.id)
        .await
        .map_err(|error| fail("load review comments", error))?;
    let requested = db
        .list_requested_reviewers(&pull.id)
        .await
        .map_err(|error| fail("load reviewers", error))?;
    let viewer_name = viewer.map(|(user, _)| user.username.as_str());
    let entries = timeline::load(
        auth_state,
        &pull,
        &comments,
        &reviews,
        &review_comments,
        viewer_name,
    )
    .await
    .map_err(|error| fail("load timeline", error))?;
    let candidates = reviewer_candidates(auth_state, &ctx.namespace, &pull, &requested)
        .await
        .map_err(|error| fail("load reviewer candidates", error))?;
    let commits = commit_count(ctx, &pull);
    let mergeable = crate::pulls::mergeable(&ctx.handle, &pull);
    Ok(render(&Page {
        ctx,
        pull: &pull,
        entries: &entries,
        commits,
        mergeable,
        viewer,
        requested: &requested,
        candidates: &candidates,
        notice,
    }))
}

/// Namespace members who can still be asked to review: everyone but the author
/// and those already asked.
async fn reviewer_candidates(
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
    pull: &PullRequest,
    requested: &[String],
) -> Result<Vec<String>, String> {
    let members = auth_state.db().list_namespace_members(namespace).await?;
    Ok(members
        .into_iter()
        .filter(|member: &User| member.id != pull.author_user_id)
        .map(|member| member.username)
        .filter(|name| !requested.contains(name))
        .collect())
}

/// Commits the head adds beyond the base, for the byline. Zero when a branch is gone.
fn commit_count(ctx: &RepoContext, pull: &PullRequest) -> usize {
    let tip = |name: &str| ctx.handle.branch_tip(name).ok().flatten();
    match (tip(&pull.base_branch), tip(&pull.head_branch)) {
        (Some(base), Some(head)) => ctx
            .handle
            .commit_ids_in_range(base, head)
            .map_or(0, |ids| ids.len()),
        _ => 0,
    }
}

struct Page<'a> {
    ctx: &'a RepoContext,
    pull: &'a PullRequest,
    entries: &'a [Entry],
    commits: usize,
    mergeable: Option<bool>,
    viewer: Option<&'a (User, PermissionSet)>,
    requested: &'a [String],
    candidates: &'a [String],
    notice: Option<&'a str>,
}

fn render(page: &Page<'_>) -> Markup {
    let ctx = page.ctx;
    let pull = page.pull;
    let namespace = &ctx.namespace;
    let repo = &ctx.repo;
    let number = pull.number;
    let action = format!("/{namespace}/{repo}/pulls/{number}");
    let (state_label, state_key) = match (pull.state, pull.merged_at.is_some()) {
        (_, true) => ("Merged", "merged"),
        (PullRequestState::Open, false) => ("Open", "open"),
        (PullRequestState::Closed, false) => ("Closed", "closed"),
    };
    let viewer_user = page.viewer.map(|(user, _)| user);
    let can_change =
        timeline::may_change_request(pull, page.viewer.map(|(user, perms)| (user, perms)));
    let can_push = page
        .viewer
        .is_some_and(|(_, perms)| perms.contains(NamespacePermission::PushChanges));
    let author = page
        .entries
        .first()
        .map_or("unknown", |entry| entry.author.as_str());
    let is_author = viewer_user.is_some_and(|user| user.id == pull.author_user_id);
    let may_review = can_push && !is_author;
    let signed_in = page.viewer.is_some();

    html! {
        div class="twig-stack twig-pr" {
            header class="twig-pr-head" {
                div class="twig-pr-title-row" {
                    h2 class="twig-pr-title" {
                        (pull.title) " "
                        span class="twig-pr-number" { "#" (number) }
                    }
                    @if can_change {
                        a class="twig-btn" href=(format!("{action}/edit")) { "Edit" }
                    }
                }
                div class="twig-pr-byline" {
                    span class="twig-pr-state" data-state=(state_key) { (state_label) }
                    " "
                    (author) " wants to merge " (page.commits) " "
                    (if page.commits == 1 { "commit" } else { "commits" })
                    " into "
                    span class="twig-mono" { (pull.base_branch) }
                    " from "
                    span class="twig-mono" { (pull.head_branch) }
                }
            }
            @if let Some(message) = page.notice {
                div class="twig-notice twig-notice--info" role="status" { (message) }
            }
            (super::request_nav(namespace, repo, number, "conversation"))
            div class="twig-pr-layout" {
                div class="twig-pr-main" {
                    div class="twig-pr-timeline" {
                        @for entry in page.entries {
                            @let mine = timeline::is_mine(entry, viewer_user);
                            (timeline::render_entry(
                                namespace,
                                repo,
                                number,
                                entry,
                                mine,
                                mine || can_push,
                                signed_in,
                            ))
                        }
                    }
                    @if signed_in {
                        section class="twig-pr-composer" aria-label="Add a comment or review" {
                            form method="POST" action=(format!("{action}/comments")) class="twig-form" {
                                label class="twig-label" for="pr-comment" { "Comment" }
                                textarea id="pr-comment" class="twig-input twig-input--mono" name="body" rows="6" {}
                                div class="twig-form-actions" {
                                    @if can_change {
                                        @if pull.state == PullRequestState::Open {
                                            button type="submit" class="twig-btn" formaction=(format!("{action}/state")) name="action" value="close" {
                                                "Close pull request"
                                            }
                                        } @else {
                                            button type="submit" class="twig-btn" formaction=(format!("{action}/state")) name="action" value="reopen" {
                                                "Reopen pull request"
                                            }
                                        }
                                    }
                                    @if may_review {
                                        button type="submit" class="twig-btn" formaction=(format!("{action}/reviews")) name="event" value="REQUEST_CHANGES" {
                                            "Request changes"
                                        }
                                        button type="submit" class="twig-btn" formaction=(format!("{action}/reviews")) name="event" value="APPROVED" {
                                            "Approve"
                                        }
                                    }
                                    button type="submit" class="twig-btn twig-btn--primary" { "Comment" }
                                }
                            }
                        }
                    } @else {
                        p class="twig-hint" { "Sign in to comment on this pull request." }
                    }
                }
                aside class="twig-pr-side" {
                    @if can_push && pull.state == PullRequestState::Open && pull.merged_at.is_none() {
                        section class="twig-pr-merge" aria-label="Merge" {
                            h3 class="twig-section" { "Merge" }
                            @if page.mergeable == Some(true) {
                                p class="twig-hint" { "This branch has no conflicts with the base branch." }
                            } @else if page.mergeable == Some(false) {
                                p class="twig-notice twig-notice--danger" role="alert" {
                                    "This branch has conflicts with the base branch."
                                }
                            }
                            form method="POST" action=(format!("{action}/merge")) class="twig-form" {
                                label class="twig-label" for="pr-merge-method" { "Merge style" }
                                select id="pr-merge-method" name="method" class="twig-input" {
                                    option value="merge" { "Create a merge commit" }
                                    option value="squash" { "Squash and merge" }
                                    option value="rebase" { "Rebase and merge" }
                                }
                                button type="submit" class="twig-btn twig-btn--primary" { "Merge pull request" }
                            }
                        }
                    }
                    section aria-label="Reviewers" {
                    h3 class="twig-section" { "Reviewers" }
                    @if page.requested.is_empty() {
                        p class="twig-hint" { "No reviews requested." }
                    } @else {
                        ul class="twig-pr-reviewers" {
                            @for name in page.requested {
                                li {
                                    span class="twig-mono" { (name) }
                                    @if can_change {
                                        form method="POST" action=(format!("{action}/reviewers")) {
                                            input type="hidden" name="action" value="remove";
                                            input type="hidden" name="reviewer" value=(name);
                                            button type="submit" class="twig-btn twig-btn--quiet" { "Remove" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    @if can_change && !page.candidates.is_empty() {
                        form method="POST" action=(format!("{action}/reviewers")) class="twig-form" {
                            input type="hidden" name="action" value="add";
                            label class="twig-label" for="pr-reviewer" { "Request a review" }
                            select id="pr-reviewer" name="reviewer" class="twig-input twig-input--mono" {
                                @for name in page.candidates {
                                    option value=(name) { (name) }
                                }
                            }
                            button type="submit" class="twig-btn" { "Request" }
                        }
                    }
                    }
                }
            }
        }
    }
}

/// The title and description form. Only people who may change the request reach it.
pub(super) fn edit_form(ctx: &RepoContext, pull: &PullRequest) -> Markup {
    let namespace = &ctx.namespace;
    let repo = &ctx.repo;
    let number = pull.number;
    html! {
        div class="twig-stack twig-pr" {
            h2 class="twig-section" { "Edit pull request #" (number) }
            form method="POST" action=(format!("/{namespace}/{repo}/pulls/{number}/edit")) class="twig-form" {
                label class="twig-label" for="pr-edit-title" { "Title" }
                input id="pr-edit-title" class="twig-input" type="text" name="title" required value=(pull.title);
                label class="twig-label" for="pr-edit-body" { "Description" }
                textarea id="pr-edit-body" class="twig-input twig-input--mono" name="body" rows="12" {
                    (pull.body)
                }
                div class="twig-form-actions" {
                    a class="twig-btn twig-btn--quiet" href=(format!("/{namespace}/{repo}/pulls/{number}")) { "Cancel" }
                    button type="submit" class="twig-btn twig-btn--primary" { "Save changes" }
                }
            }
        }
    }
}
