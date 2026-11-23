//! The mutations a pull request page sends beyond the ones GitHub's `gh` uses: reactions,
//! draft state, and comment edits. Each delegates to the same domain rules the web UI and
//! REST API use. Operations Twig cannot perform fail with an `UNSUPPORTED` error.

use async_graphql::{Enum, InputObject, Result, SimpleObject};

use super::review::{ReviewCommentNode, ReviewThreadNode};
use super::{
    CommentNode, GqlContext, OpenRepo, PullRequestNode, gql_error, issue_actor, mutation_error,
    open_repo, pull_actor, pull_for, unhex,
};
use crate::{
    db::pull_requests::{PullRequest, PullRequestEdit},
    http::api::shared::is_work_in_progress,
    issues,
    pulls::{self, RepoRef as PullScope},
};

pub use super::review::ReactionContentEnum;

/// The reaction a payload reports.
#[derive(SimpleObject, Clone)]
#[graphql(name = "Reaction")]
pub struct ReactionNode {
    pub content: ReactionContentEnum,
}

#[derive(InputObject)]
#[graphql(name = "AddReactionInput")]
pub struct AddReactionInput {
    pub subject_id: String,
    pub content: ReactionContentEnum,
    pub client_mutation_id: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "RemoveReactionInput")]
pub struct RemoveReactionInput {
    pub subject_id: String,
    pub content: ReactionContentEnum,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "AddReactionPayload")]
pub struct AddReactionPayload {
    pub client_mutation_id: Option<String>,
    pub reaction: ReactionNode,
}

#[derive(SimpleObject)]
#[graphql(name = "RemoveReactionPayload")]
pub struct RemoveReactionPayload {
    pub client_mutation_id: Option<String>,
    pub reaction: ReactionNode,
}

#[derive(InputObject)]
#[graphql(name = "MarkPullRequestReadyForReviewInput")]
pub struct ReadyForReviewInput {
    pub pull_request_id: String,
    pub client_mutation_id: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "ConvertPullRequestToDraftInput")]
pub struct ConvertToDraftInput {
    pub pull_request_id: String,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "MarkPullRequestReadyForReviewPayload")]
pub struct ReadyForReviewPayload {
    pub client_mutation_id: Option<String>,
    pub pull_request: PullRequestNode,
}

#[derive(SimpleObject)]
#[graphql(name = "ConvertPullRequestToDraftPayload")]
pub struct ConvertToDraftPayload {
    pub client_mutation_id: Option<String>,
    pub pull_request: PullRequestNode,
}

#[derive(InputObject)]
#[graphql(name = "UpdatePullRequestInput")]
pub struct UpdatePullRequestInput {
    pub pull_request_id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub maintainer_can_modify: Option<bool>,
    pub base_ref_name: Option<String>,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "UpdatePullRequestPayload")]
pub struct UpdatePullRequestPayload {
    pub client_mutation_id: Option<String>,
    pub pull_request: PullRequestNode,
}

#[derive(InputObject)]
#[graphql(name = "UpdatePullRequestReviewCommentInput")]
pub struct UpdateReviewCommentInput {
    pub pull_request_review_comment_id: String,
    pub body: String,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "UpdatePullRequestReviewCommentPayload")]
pub struct UpdateReviewCommentPayload {
    pub client_mutation_id: Option<String>,
    pub pull_request_review_comment: ReviewCommentNode,
}

#[derive(InputObject)]
#[graphql(name = "UpdateIssueCommentInput")]
pub struct UpdateIssueCommentInput {
    pub id: String,
    pub body: String,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "UpdateIssueCommentPayload")]
pub struct UpdateIssueCommentPayload {
    pub client_mutation_id: Option<String>,
    pub issue_comment: CommentNode,
}

/// Whether a branch update merges the base in or rebases the head onto it.
#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "PullRequestBranchUpdateMethod")]
pub enum BranchUpdateMethodEnum {
    #[graphql(name = "MERGE")]
    Merge,
    #[graphql(name = "REBASE")]
    Rebase,
}

/// The error an unsupported operation returns. Clients show the message as it is.
pub fn unsupported(operation: &str) -> async_graphql::Error {
    gql_error(
        format!("{operation} is not supported on Twig."),
        "UNSUPPORTED",
    )
}

fn not_found() -> async_graphql::Error {
    gql_error("Could not resolve to a node", "NOT_FOUND")
}

/// The title a request has once it is a draft: marked `WIP:`, as Twig reads drafts.
pub fn draft_title(title: &str) -> String {
    if is_work_in_progress(title) {
        title.to_string()
    } else {
        format!("WIP: {title}")
    }
}

/// The title a request has once it is ready: the draft marker removed.
pub fn ready_title(title: &str) -> String {
    let trimmed = title.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("wip:") {
        trimmed[4..].trim().to_string()
    } else if lower.starts_with("[wip]") {
        trimmed[5..].trim().to_string()
    } else {
        title.to_string()
    }
}

/// A comment ID, decoded. Its body is hex of `owner/name#<number>#comment:<row>` for a
/// comment on a request, or `owner/name#comment:<row>` for an issue comment.
pub enum CommentSubject {
    Pull {
        owner: String,
        name: String,
        number: i64,
        row: i64,
    },
    Issue {
        owner: String,
        name: String,
        row: i64,
    },
}

/// Decodes a comment ID with the given prefix. `None` when the ID is not of that shape.
pub fn parse_comment_id(prefix: &str, id: &str) -> Option<CommentSubject> {
    let body = id.strip_prefix(&format!("{prefix}_"))?;
    let text = unhex(body).ok()?;
    let (path, rest) = text.split_once('#')?;
    let (owner, name) = path.split_once('/')?;
    let row_of = |part: &str| part.strip_prefix("comment:")?.parse::<i64>().ok();
    let parts: Vec<&str> = rest.split('#').collect();
    match parts.as_slice() {
        [number, comment] => Some(CommentSubject::Pull {
            owner: owner.to_string(),
            name: name.to_string(),
            number: number.parse().ok()?,
            row: row_of(comment)?,
        }),
        [comment] => Some(CommentSubject::Issue {
            owner: owner.to_string(),
            name: name.to_string(),
            row: row_of(comment)?,
        }),
        _ => None,
    }
}

/// Loads a request in a repository, by its number.
async fn pull_by_number(
    g: &GqlContext,
    owner: &str,
    name: &str,
    number: i64,
) -> Result<(OpenRepo, PullRequest)> {
    let open = open_repo(g, owner, name).await?;
    let scope = PullScope {
        repo_id: &open.repo_id,
    };
    let pull = pulls::get(&g.db, &scope, number)
        .await
        .map_err(|_| not_found())?;
    Ok((open, pull))
}

/// Adds or removes a reaction on a request or on one of its comments.
pub async fn react(
    g: &GqlContext,
    subject_id: &str,
    content: ReactionContentEnum,
    on: bool,
) -> Result<()> {
    let store = content.to_store();
    if subject_id.starts_with("PR_") {
        let (open, pull) = pull_for(g, subject_id).await?;
        return pulls::react(
            &g.db,
            &pull,
            None,
            &pull_actor(g),
            &open.permissions,
            store,
            on,
        )
        .await
        .map_err(mutation_error);
    }
    let prefix = if subject_id.starts_with("PRRC_") {
        "PRRC"
    } else {
        "IC"
    };
    match parse_comment_id(prefix, subject_id) {
        Some(CommentSubject::Pull {
            owner,
            name,
            number,
            row,
        }) => {
            let (open, pull) = pull_by_number(g, &owner, &name, number).await?;
            pulls::react(
                &g.db,
                &pull,
                Some(row),
                &pull_actor(g),
                &open.permissions,
                store,
                on,
            )
            .await
            .map_err(mutation_error)
        }
        Some(CommentSubject::Issue { .. }) => Err(unsupported("Reactions on issue comments")),
        None => Err(not_found()),
    }
}

/// Changes a request's title or body. Its author and anyone who may push may do so.
pub async fn update_pull_request(
    g: &GqlContext,
    input: &UpdatePullRequestInput,
) -> Result<PullRequestNode> {
    if input.base_ref_name.is_some() {
        return Err(unsupported("Changing a pull request's base branch"));
    }
    let (open, pull) = pull_for(g, &input.pull_request_id).await?;
    let updated = pulls::edit(
        &g.db,
        &pull,
        &pull_actor(g),
        &open.permissions,
        PullRequestEdit {
            title: input.title.clone(),
            body: input.body.clone(),
            state: None,
        },
    )
    .await
    .map_err(mutation_error)?;
    Ok(PullRequestNode::new(g, &open, updated))
}

/// Marks a request as a draft, or as ready for review, by its `WIP:` title marker.
pub async fn set_draft(g: &GqlContext, id: &str, draft: bool) -> Result<PullRequestNode> {
    let (open, pull) = pull_for(g, id).await?;
    let title = if draft {
        draft_title(&pull.title)
    } else {
        ready_title(&pull.title)
    };
    if title == pull.title {
        return Ok(PullRequestNode::new(g, &open, pull));
    }
    let updated = pulls::edit(
        &g.db,
        &pull,
        &pull_actor(g),
        &open.permissions,
        PullRequestEdit {
            title: Some(title),
            body: None,
            state: None,
        },
    )
    .await
    .map_err(mutation_error)?;
    Ok(PullRequestNode::new(g, &open, updated))
}

/// Replaces the text of an inline review comment.
pub async fn update_review_comment(
    g: &GqlContext,
    id: &str,
    body: &str,
) -> Result<ReviewCommentNode> {
    let Some(CommentSubject::Pull {
        owner,
        name,
        number,
        row,
    }) = parse_comment_id("PRRC", id)
    else {
        return Err(not_found());
    };
    let (open, pull) = pull_by_number(g, &owner, &name, number).await?;
    let comment = pulls::edit_comment(&g.db, &pull, row, &pull_actor(g), &open.permissions, body)
        .await
        .map_err(mutation_error)?;
    Ok(ReviewCommentNode::new(
        &owner, &name, number, &g.origin, comment,
    ))
}

/// Replaces the text of a conversation comment, on a request or on an issue.
pub async fn update_issue_comment(g: &GqlContext, id: &str, body: &str) -> Result<CommentNode> {
    match parse_comment_id("IC", id) {
        Some(CommentSubject::Pull {
            owner,
            name,
            number,
            row,
        }) => {
            let (open, pull) = pull_by_number(g, &owner, &name, number).await?;
            let comment =
                pulls::edit_comment(&g.db, &pull, row, &pull_actor(g), &open.permissions, body)
                    .await
                    .map_err(mutation_error)?;
            Ok(CommentNode::for_pull_comment(
                &g.origin, &owner, &name, number, comment,
            ))
        }
        Some(CommentSubject::Issue { owner, name, row }) => {
            let open = open_repo(g, &owner, &name).await?;
            let comment =
                g.db.get_issue_comment(row)
                    .await
                    .map_err(super::internal)?
                    .ok_or_else(not_found)?;
            let issue =
                g.db.get_issue_by_id(&comment.issue_id)
                    .await
                    .map_err(super::internal)?
                    .ok_or_else(not_found)?;
            let edited =
                issues::edit_comment(&g.db, &issue, row, &issue_actor(g), &open.permissions, body)
                    .await
                    .map_err(mutation_error)?;
            Ok(CommentNode::for_issue_comment(
                &g.origin, &owner, &name, edited,
            ))
        }
        None => Err(not_found()),
    }
}

/// Input for turning auto-merge on. Twig has no auto-merge, so the mutation fails.
#[derive(InputObject)]
#[graphql(name = "EnablePullRequestAutoMergeInput")]
pub struct EnableAutoMergeInput {
    pub pull_request_id: String,
    pub merge_method: Option<super::PullRequestMergeMethodEnum>,
    pub commit_headline: Option<String>,
    pub commit_body: Option<String>,
    pub expected_head_oid: Option<super::GitObjectId>,
    pub author_email: Option<String>,
    pub client_mutation_id: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "DisablePullRequestAutoMergeInput")]
pub struct DisableAutoMergeInput {
    pub pull_request_id: String,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "PullRequestAutoMergePayload")]
pub struct AutoMergePayload {
    pub client_mutation_id: Option<String>,
    pub pull_request: Option<PullRequestNode>,
}

#[derive(InputObject)]
#[graphql(name = "UpdatePullRequestBranchInput")]
pub struct UpdateBranchInput {
    pub pull_request_id: String,
    pub expected_head_oid: Option<super::GitObjectId>,
    pub update_method: Option<BranchUpdateMethodEnum>,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "UpdatePullRequestBranchPayload")]
pub struct UpdateBranchPayload {
    pub client_mutation_id: Option<String>,
    pub pull_request: Option<PullRequestNode>,
}

#[derive(InputObject)]
#[graphql(name = "RevertPullRequestInput")]
pub struct RevertPullRequestInput {
    pub pull_request_id: String,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "RevertPullRequestPayload")]
pub struct RevertPullRequestPayload {
    pub client_mutation_id: Option<String>,
    pub revert_pull_request: Option<PullRequestNode>,
}

#[derive(InputObject)]
#[graphql(name = "RefUpdate")]
pub struct RefUpdateInput {
    pub name: String,
    pub before_oid: Option<super::GitObjectId>,
    pub after_oid: Option<super::GitObjectId>,
    pub force: Option<bool>,
}

#[derive(InputObject)]
#[graphql(name = "UpdateRefsInput")]
pub struct UpdateRefsInput {
    pub ref_updates: Vec<RefUpdateInput>,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "UpdateRefsPayload")]
pub struct UpdateRefsPayload {
    pub client_mutation_id: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "AddPullRequestReviewThreadReplyInput")]
pub struct ThreadReplyInput {
    pub pull_request_review_thread_id: String,
    pub body: String,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "AddPullRequestReviewThreadReplyPayload")]
pub struct ThreadReplyPayload {
    pub client_mutation_id: Option<String>,
    pub comment: Option<ReviewCommentNode>,
}

#[derive(InputObject)]
#[graphql(name = "ResolveReviewThreadInput")]
pub struct ResolveThreadInput {
    pub thread_id: String,
    pub client_mutation_id: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "UnresolveReviewThreadInput")]
pub struct UnresolveThreadInput {
    pub thread_id: String,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "ResolveReviewThreadPayload")]
pub struct ResolveThreadPayload {
    pub client_mutation_id: Option<String>,
    pub thread: Option<ReviewThreadNode>,
}

#[derive(InputObject)]
#[graphql(name = "MarkFileAsViewedInput")]
pub struct MarkFileViewedInput {
    pub pull_request_id: String,
    pub path: String,
    pub client_mutation_id: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "UnmarkFileAsViewedInput")]
pub struct UnmarkFileViewedInput {
    pub pull_request_id: String,
    pub path: String,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(name = "FileViewedPayload")]
pub struct FileViewedPayload {
    pub client_mutation_id: Option<String>,
}
