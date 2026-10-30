//! The conversation timeline: the description and comments in order, each with
//! its reactions and the controls its viewer may use.

use actix_web::web;
use maud::{Markup, PreEscaped, html};

use crate::{
    auth::{NamespacePermission, PermissionSet, TwigContext, User},
    db::pull_requests::{
        PullRequest,
        comments::PullRequestComment,
        reactions::{ReactionContent, comment_key, pull_key},
        reviews::{PullRequestReview, ReviewState},
    },
    md,
};

/// One inline comment of a review, shown under the review that wrote it.
pub(super) struct Inline {
    pub(super) path: String,
    pub(super) line: i64,
    pub(super) body_html: String,
}

/// The verdict part of a review entry, with its inline comments.
pub(super) struct ReviewPart {
    pub(super) state: ReviewState,
    pub(super) inline: Vec<Inline>,
}

/// One entry of the timeline. `comment_id` is `None` for the description and for reviews.
pub(super) struct Entry {
    pub(super) comment_id: Option<i64>,
    pub(super) author: String,
    pub(super) author_id: String,
    pub(super) body_html: String,
    pub(super) created_at: String,
    pub(super) edited: bool,
    pub(super) reactions: Vec<(ReactionContent, usize, bool)>,
    pub(super) body_source: String,
    pub(super) review: Option<ReviewPart>,
}

fn reaction_summary(
    reactions: &[(String, ReactionContent)],
    viewer: Option<&str>,
) -> Vec<(ReactionContent, usize, bool)> {
    ReactionContent::ALL
        .into_iter()
        .map(|content| {
            let matching: Vec<&String> = reactions
                .iter()
                .filter(|(_, reacted)| *reacted == content)
                .map(|(username, _)| username)
                .collect();
            let mine = viewer.is_some_and(|name| matching.iter().any(|user| user.as_str() == name));
            (content, matching.len(), mine)
        })
        .collect()
}

/// Loads the description, comments, and reviews as timeline entries, oldest first.
pub(super) async fn load(
    auth_state: &web::Data<TwigContext>,
    pull: &PullRequest,
    comments: &[PullRequestComment],
    reviews: &[PullRequestReview],
    review_comments: &[PullRequestComment],
    viewer: Option<&str>,
) -> Result<Vec<Entry>, String> {
    let db = auth_state.db();
    let author_of = |user_id: &str| {
        let user_id = user_id.to_string();
        async move {
            db.get_user_by_id(&user_id)
                .await
                .ok()
                .flatten()
                .map_or_else(
                    || ("unknown".to_string(), user_id.clone()),
                    |user: User| (user.username, user.id),
                )
        }
    };

    let mut entries = Vec::with_capacity(comments.len() + reviews.len() + 1);
    let (author, author_id) = author_of(&pull.author_user_id).await;
    let reactions = db.list_reactions(&pull_key(&pull.id)).await?;
    entries.push(Entry {
        comment_id: None,
        author,
        author_id,
        body_html: md::render_untrusted_markdown(&pull.body),
        created_at: pull.created_at.clone(),
        edited: false,
        reactions: reaction_summary(&reactions, viewer),
        body_source: pull.body.clone(),
        review: None,
    });
    for comment in comments {
        let (author, author_id) = author_of(&comment.author_user_id).await;
        let reactions = db.list_reactions(&comment_key(comment.id)).await?;
        entries.push(Entry {
            comment_id: Some(comment.id),
            author,
            author_id,
            body_html: md::render_untrusted_markdown(&comment.body),
            created_at: comment.created_at.clone(),
            edited: comment.updated_at.is_some(),
            reactions: reaction_summary(&reactions, viewer),
            body_source: comment.body.clone(),
            review: None,
        });
    }
    for review in reviews {
        let (author, author_id) = author_of(&review.author_user_id).await;
        let inline = review_comments
            .iter()
            .filter(|comment| comment.review_id == Some(review.id))
            .map(|comment| Inline {
                path: comment.path.clone().unwrap_or_default(),
                line: comment
                    .new_position
                    .filter(|line| *line > 0)
                    .or(comment.old_position)
                    .unwrap_or_default(),
                body_html: md::render_untrusted_markdown(&comment.body),
            })
            .collect();
        entries.push(Entry {
            comment_id: None,
            author,
            author_id,
            body_html: md::render_untrusted_markdown(&review.body),
            created_at: review.submitted_at.clone(),
            edited: false,
            reactions: Vec::new(),
            body_source: review.body.clone(),
            review: Some(ReviewPart {
                state: review.state,
                inline,
            }),
        });
    }
    entries.sort_by(|left, right| left.created_at.cmp(&right.created_at));
    Ok(entries)
}

/// The sentence that heads a review entry, in Forgejo's wording.
fn review_verb(state: ReviewState) -> &'static str {
    match state {
        ReviewState::Approved => "approved these changes",
        ReviewState::RequestChanges => "requested changes",
        ReviewState::Comment => "reviewed",
    }
}

const EMOJI: [(&str, &str); 8] = [
    ("+1", "👍"),
    ("-1", "👎"),
    ("laugh", "😄"),
    ("hooray", "🎉"),
    ("confused", "😕"),
    ("heart", "❤️"),
    ("rocket", "🚀"),
    ("eyes", "👀"),
];

fn emoji(content: ReactionContent) -> &'static str {
    EMOJI
        .iter()
        .find(|(name, _)| *name == content.api_value())
        .map_or("", |(_, glyph)| glyph)
}

/// One timeline entry. `may_edit` and `may_delete` decide the controls shown;
/// `viewer_signed_in` decides whether reactions are offered.
pub(super) fn render_entry(
    namespace: &str,
    repo: &str,
    number: i64,
    entry: &Entry,
    may_edit: bool,
    may_delete: bool,
    viewer_signed_in: bool,
) -> Markup {
    let target = match entry.comment_id {
        Some(id) => format!("comment:{id}"),
        None => "pull".to_string(),
    };
    let action = format!("/{namespace}/{repo}/pulls/{number}");
    if let Some(review) = &entry.review {
        return html! {
            article class="twig-pr-entry twig-pr-review" data-review-state=(review.state.api_value()) {
                header class="twig-pr-entry-head" {
                    span class="twig-pr-entry-author" { (entry.author) }
                    " " (review_verb(review.state)) " "
                    time class="twig-pr-entry-time" datetime=(entry.created_at) {
                        (entry.created_at.split('T').next().unwrap_or_default())
                    }
                }
                @if !entry.body_source.trim().is_empty() {
                    div class="twig-pr-entry-body" { (PreEscaped(entry.body_html.clone())) }
                }
                @if !review.inline.is_empty() {
                    ul class="twig-pr-inline-list" {
                        @for inline in &review.inline {
                            li class="twig-pr-inline" {
                                code class="twig-mono" { (inline.path) ":" (inline.line) }
                                div class="twig-pr-entry-body" { (PreEscaped(inline.body_html.clone())) }
                            }
                        }
                    }
                }
            }
        };
    }
    html! {
        article class="twig-pr-entry" id=(entry.comment_id.map_or("description".to_string(), |id| format!("comment-{id}"))) {
            header class="twig-pr-entry-head" {
                span class="twig-pr-entry-author" { (entry.author) }
                " "
                time class="twig-pr-entry-time" datetime=(entry.created_at) {
                    (entry.created_at.split('T').next().unwrap_or_default())
                }
                @if entry.edited {
                    span class="twig-pr-entry-edited" { " · edited" }
                }
            }
            div class="twig-pr-entry-body" {
                @if entry.body_source.trim().is_empty() {
                    p class="twig-hint" { "No description provided." }
                } @else {
                    (PreEscaped(entry.body_html.clone()))
                }
            }
            @if viewer_signed_in {
                div class="twig-pr-reactions" aria-label="Reactions" {
                    @for (content, count, mine) in &entry.reactions {
                        form method="POST" action=(format!("{action}/reactions")) class="twig-pr-reaction-form" {
                            input type="hidden" name="target" value=(target);
                            input type="hidden" name="content" value=(content.api_value());
                            input type="hidden" name="on" value=(if *mine { "false" } else { "true" });
                            button type="submit" class="twig-pr-reaction" aria-pressed=(mine) {
                                (emoji(*content)) " " @if *count > 0 { (count) }
                            }
                        }
                    }
                }
            }
            @if entry.comment_id.is_some() && (may_edit || may_delete) {
                footer class="twig-pr-entry-tools" {
                    @if may_edit {
                        details class="twig-pr-edit" {
                            summary { "Edit" }
                            form method="POST" action=(format!("{action}/comments/{}/edit", entry.comment_id.unwrap_or_default())) class="twig-form" {
                                textarea class="twig-input twig-input--mono" name="body" rows="6" aria-label="Comment text" {
                                    (entry.body_source)
                                }
                                button type="submit" class="twig-btn twig-btn--primary" { "Save" }
                            }
                        }
                    }
                    @if may_delete {
                        form method="POST" action=(format!("{action}/comments/{}/delete", entry.comment_id.unwrap_or_default())) {
                            button type="submit" class="twig-btn twig-btn--quiet twig-btn--danger" { "Delete" }
                        }
                    }
                }
            }
        }
    }
}

/// Whether `viewer` wrote this entry.
pub(super) fn is_mine(entry: &Entry, viewer: Option<&User>) -> bool {
    viewer.is_some_and(|user| user.id == entry.author_id)
}

/// Whether the viewer may change a request: its author or anyone who can push.
pub(super) fn may_change_request(
    pull: &PullRequest,
    viewer: Option<(&User, &PermissionSet)>,
) -> bool {
    viewer.is_some_and(|(user, perms)| {
        user.id == pull.author_user_id || perms.contains(NamespacePermission::PushChanges)
    })
}
