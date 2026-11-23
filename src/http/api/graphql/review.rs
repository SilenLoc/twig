//! Review threads, inline review comments, reactions, and node lookup: the parts of
//! GitHub's GraphQL schema a pull request page reads beyond the request's own fields.
//!
//! Twig does not thread replies to inline comments, so every inline comment is a thread
//! of one. Nothing is ever resolved or outdated. Reactions are stored per request and per
//! conversation comment, and inline comments share the comment table, so they react too.

use async_graphql::{Context, Enum, Interface, Object, Result, Union};

use super::{
    BotNode, CommentNode, CommitNode, CommitRef, GqlContext, IssueNode, MannequinNode, OpenRepo,
    OrganizationNode, Page, PageInfo, PullRequestNode, ReactionUsersNode, RepositoryNode,
    ReviewNode, UserNode, gql_context, gql_error, hex_text, internal, issue_for, open_repo,
    paginate, pull_for, repo_from_node, unhex, user_by_id, user_node,
};
use crate::{
    db::pull_requests::{
        comments::PullRequestComment,
        reactions::{ReactionContent, comment_key},
    },
    pulls::{self, RepoRef as PullScope},
};

/// A reaction, as GitHub names it.
#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "ReactionContent")]
pub enum ReactionContentEnum {
    #[graphql(name = "THUMBS_UP")]
    ThumbsUp,
    #[graphql(name = "THUMBS_DOWN")]
    ThumbsDown,
    #[graphql(name = "LAUGH")]
    Laugh,
    #[graphql(name = "HOORAY")]
    Hooray,
    #[graphql(name = "CONFUSED")]
    Confused,
    #[graphql(name = "HEART")]
    Heart,
    #[graphql(name = "ROCKET")]
    Rocket,
    #[graphql(name = "EYES")]
    Eyes,
}

impl ReactionContentEnum {
    /// Every reaction, in the order GitHub lists them.
    const ALL: [Self; 8] = [
        Self::ThumbsUp,
        Self::ThumbsDown,
        Self::Laugh,
        Self::Hooray,
        Self::Confused,
        Self::Heart,
        Self::Rocket,
        Self::Eyes,
    ];

    pub fn to_store(self) -> ReactionContent {
        match self {
            Self::ThumbsUp => ReactionContent::PlusOne,
            Self::ThumbsDown => ReactionContent::MinusOne,
            Self::Laugh => ReactionContent::Laugh,
            Self::Hooray => ReactionContent::Hooray,
            Self::Confused => ReactionContent::Confused,
            Self::Heart => ReactionContent::Heart,
            Self::Rocket => ReactionContent::Rocket,
            Self::Eyes => ReactionContent::Eyes,
        }
    }
}

/// One reaction on a target and the people who gave it.
#[derive(Clone)]
pub struct ReactionGroupNode {
    content: ReactionContentEnum,
    viewer_has_reacted: bool,
    reactors: Vec<UserNode>,
}

#[Object(name = "ReactionGroup")]
impl ReactionGroupNode {
    async fn content(&self) -> ReactionContentEnum {
        self.content
    }

    async fn viewer_has_reacted(&self) -> bool {
        self.viewer_has_reacted
    }

    async fn users(&self) -> ReactionUsersNode {
        ReactionUsersNode {
            total: i64::try_from(self.reactors.len()).unwrap_or_default(),
        }
    }

    async fn reactors(&self, first: Option<i32>, after: Option<String>) -> ReactorConnection {
        let nodes: Vec<ReactorNode> = self
            .reactors
            .iter()
            .cloned()
            .map(ReactorNode::User)
            .collect();
        paginate(nodes, first, after.as_deref()).into()
    }
}

/// Someone who reacted. Twig's accounts are users; the other kinds never appear.
#[derive(Union)]
pub enum ReactorNode {
    User(UserNode),
    Bot(BotNode),
    Organization(OrganizationNode),
    Mannequin(MannequinNode),
}

connection!(ReactorConnection, ReactorNode);

/// The reaction groups of a target that cannot take reactions: all eight, all empty.
pub fn empty_groups() -> Vec<ReactionGroupNode> {
    ReactionContentEnum::ALL
        .into_iter()
        .map(|content| ReactionGroupNode {
            content,
            viewer_has_reacted: false,
            reactors: Vec::new(),
        })
        .collect()
}

/// The reaction groups on one target, one per reaction, in GitHub's order.
pub async fn reaction_groups(g: &GqlContext, target_key: &str) -> Result<Vec<ReactionGroupNode>> {
    let rows = g.db.list_reactions(target_key).await.map_err(internal)?;
    let mut groups = Vec::with_capacity(ReactionContentEnum::ALL.len());
    for content in ReactionContentEnum::ALL {
        let store = content.to_store();
        let mut reactors = Vec::new();
        let mut viewer_has_reacted = false;
        for (username, reacted) in &rows {
            if *reacted != store {
                continue;
            }
            if username == &g.principal.user.username {
                viewer_has_reacted = true;
            }
            if let Some(user) =
                g.db.get_user_by_username(username)
                    .await
                    .map_err(internal)?
            {
                reactors.push(user_node(&g.origin, &user));
            }
        }
        groups.push(ReactionGroupNode {
            content,
            viewer_has_reacted,
            reactors,
        });
    }
    Ok(groups)
}

/// Something that takes reactions: a request, a conversation comment, a review, or an
/// inline comment.
#[derive(Interface)]
#[graphql(field(name = "reaction_groups", ty = "Vec<ReactionGroupNode>"))]
pub enum Reactable {
    PullRequest(PullRequestNode),
    IssueComment(CommentNode),
    PullRequestReview(ReviewNode),
    PullRequestReviewComment(ReviewCommentNode),
}

/// An inline comment on a request's diff, with the line it is anchored to.
#[derive(Clone)]
pub struct ReviewCommentNode {
    row: PullRequestComment,
    owner: String,
    name: String,
    number: i64,
    origin: String,
}

impl ReviewCommentNode {
    pub fn new(
        owner: &str,
        name: &str,
        number: i64,
        origin: &str,
        row: PullRequestComment,
    ) -> Self {
        Self {
            row,
            owner: owner.to_string(),
            name: name.to_string(),
            number,
            origin: origin.to_string(),
        }
    }

    fn key(&self) -> String {
        format!(
            "{}/{}#{}#comment:{}",
            self.owner, self.name, self.number, self.row.id
        )
    }

    /// The line the comment is anchored to, on the side it names.
    fn anchor(&self) -> (ReviewSideEnum, Option<i64>) {
        if self.row.new_position.unwrap_or_default() > 0 {
            (ReviewSideEnum::Right, self.row.new_position)
        } else {
            (ReviewSideEnum::Left, self.row.old_position)
        }
    }
}

/// The side of a diff a comment is anchored to.
#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "DiffSide")]
pub enum ReviewSideEnum {
    #[graphql(name = "LEFT")]
    Left,
    #[graphql(name = "RIGHT")]
    Right,
}

#[Object(name = "PullRequestReviewComment")]
impl ReviewCommentNode {
    async fn id(&self) -> String {
        format!("PRRC_{}", hex_text(&self.key()))
    }

    async fn database_id(&self) -> i64 {
        self.row.id
    }

    async fn author(&self, ctx: &Context<'_>) -> Result<Option<UserNode>> {
        user_by_id(gql_context(ctx)?, &self.row.author_user_id).await
    }

    async fn body(&self) -> &str {
        &self.row.body
    }

    async fn body_text(&self) -> &str {
        &self.row.body
    }

    async fn created_at(&self) -> &str {
        &self.row.created_at
    }

    async fn last_edited_at(&self) -> Option<&str> {
        self.row.updated_at.as_deref()
    }

    async fn url(&self) -> String {
        format!(
            "{}/{}/{}/pull/{}#discussion_r{}",
            self.origin, self.owner, self.name, self.number, self.row.id
        )
    }

    async fn is_minimized(&self) -> bool {
        false
    }

    async fn minimized_reason(&self) -> Option<String> {
        None
    }

    async fn author_association(&self) -> &str {
        "NONE"
    }

    async fn path(&self) -> Option<&str> {
        self.row.path.as_deref()
    }

    async fn line(&self) -> Option<i64> {
        self.anchor().1
    }

    async fn diff_side(&self) -> ReviewSideEnum {
        self.anchor().0
    }

    async fn commit(&self) -> Option<CommitRef> {
        self.row.commit_oid.clone().map(|oid| CommitRef { oid })
    }

    async fn viewer_did_author(&self, ctx: &Context<'_>) -> Result<bool> {
        Ok(gql_context(ctx)?.principal.user.id == self.row.author_user_id)
    }

    /// The review this comment belongs to.
    async fn pull_request_review(&self, ctx: &Context<'_>) -> Result<Option<ReviewNode>> {
        let g = gql_context(ctx)?;
        let Some(review_id) = self.row.review_id else {
            return Ok(None);
        };
        let reviews =
            g.db.list_pull_request_reviews(&self.row.pull_request_id)
                .await
                .map_err(internal)?;
        Ok(reviews
            .into_iter()
            .find(|review| review.id == review_id)
            .map(|review| ReviewNode {
                review,
                owner: self.owner.clone(),
                name: self.name.clone(),
                number: self.number,
                origin: self.origin.clone(),
            }))
    }

    async fn pull_request(&self, ctx: &Context<'_>) -> Result<Option<PullRequestNode>> {
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.name).await?;
        let scope = PullScope {
            repo_id: &open.repo_id,
        };
        match pulls::get(&g.db, &scope, self.number).await {
            Ok(pull) => Ok(Some(PullRequestNode::new(g, &open, pull))),
            Err(_) => Ok(None),
        }
    }

    async fn reaction_groups(&self, ctx: &Context<'_>) -> Result<Vec<ReactionGroupNode>> {
        reaction_groups(gql_context(ctx)?, &comment_key(self.row.id)).await
    }
}

connection!(PullRequestReviewCommentConnection, ReviewCommentNode);

/// A thread of inline comments. Twig keeps one comment per thread.
#[derive(Clone)]
pub struct ReviewThreadNode {
    comment: ReviewCommentNode,
}

impl ReviewThreadNode {
    pub fn new(comment: ReviewCommentNode) -> Self {
        Self { comment }
    }
}

#[Object(name = "PullRequestReviewThread")]
impl ReviewThreadNode {
    async fn id(&self) -> String {
        format!("PRRT_{}", hex_text(&self.comment.key()))
    }

    /// Twig has no way to resolve a thread.
    async fn is_resolved(&self) -> bool {
        false
    }

    /// A comment is never outdated on Twig: it stays on the line it was made on.
    async fn is_outdated(&self) -> bool {
        false
    }

    async fn viewer_can_resolve(&self) -> bool {
        false
    }

    async fn viewer_can_unresolve(&self) -> bool {
        false
    }

    async fn path(&self) -> Option<&str> {
        self.comment.row.path.as_deref()
    }

    async fn line(&self) -> Option<i64> {
        self.comment.anchor().1
    }

    async fn diff_side(&self) -> ReviewSideEnum {
        self.comment.anchor().0
    }

    async fn pull_request(&self, ctx: &Context<'_>) -> Result<Option<PullRequestNode>> {
        self.comment.pull_request(ctx).await
    }

    async fn comments(
        &self,
        first: Option<i32>,
        after: Option<String>,
    ) -> PullRequestReviewCommentConnection {
        paginate(vec![self.comment.clone()], first, after.as_deref()).into()
    }
}

connection!(PullRequestReviewThreadConnection, ReviewThreadNode);

impl PullRequestNode {
    /// The inline comments on this request, one thread each, oldest first.
    pub async fn review_thread_nodes(&self, g: &GqlContext) -> Result<Vec<ReviewThreadNode>> {
        let rows =
            g.db.list_review_comments(&self.pull.id)
                .await
                .map_err(internal)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                ReviewThreadNode::new(ReviewCommentNode::new(
                    &self.owner,
                    &self.name,
                    self.pull.number,
                    &self.origin,
                    row,
                ))
            })
            .collect())
    }
}

/// Anything with a global ID that `node(id:)` can return.
#[derive(Interface)]
#[graphql(field(name = "id", ty = "String"))]
pub enum Node {
    Repository(RepositoryNode),
    PullRequest(PullRequestNode),
    Issue(IssueNode),
    IssueComment(CommentNode),
    PullRequestReview(ReviewNode),
    PullRequestReviewComment(ReviewCommentNode),
    PullRequestReviewThread(ReviewThreadNode),
    Commit(CommitNode),
    User(UserNode),
    Bot(BotNode),
}

/// Looks up a global ID. An ID that names nothing this server holds resolves to `null`.
pub async fn lookup_node(g: &GqlContext, id: &str) -> Result<Option<Node>> {
    let Some((prefix, _)) = id.split_once('_') else {
        return Ok(None);
    };
    match prefix {
        "R" => {
            let (owner, name) = repo_from_node(id)?;
            let open = open_repo(g, &owner, &name).await?;
            Ok(Some(Node::Repository(RepositoryNode::from_open(&open, g))))
        }
        "PR" => {
            let (open, pull) = pull_for(g, id).await?;
            Ok(Some(Node::PullRequest(PullRequestNode::new(
                g, &open, pull,
            ))))
        }
        "I" => {
            let (open, issue) = issue_for(g, id).await?;
            Ok(Some(Node::Issue(IssueNode::new(&open, issue, g))))
        }
        "C" => Ok(commit_for(g, id).await?.map(Node::Commit)),
        "U" => {
            let body = id.strip_prefix("U_").unwrap_or_default();
            let user_id = unhex(body)?;
            Ok(user_by_id(g, &user_id).await?.map(Node::User))
        }
        "PRRC" | "PRRT" => {
            let Some(comment) = review_comment_for(g, id).await? else {
                return Ok(None);
            };
            if prefix == "PRRT" {
                Ok(Some(Node::PullRequestReviewThread(ReviewThreadNode::new(
                    comment,
                ))))
            } else {
                Ok(Some(Node::PullRequestReviewComment(comment)))
            }
        }
        _ => Ok(None),
    }
}

/// The inline comment a `PRRC_`/`PRRT_` ID names, when it is on a request of that repository.
async fn review_comment_for(g: &GqlContext, id: &str) -> Result<Option<ReviewCommentNode>> {
    let body = id.split_once('_').map_or("", |(_, body)| body);
    let text = unhex(body)?;
    let not_found = || gql_error("Could not resolve to a node", "NOT_FOUND");
    let mut parts = text.split('#');
    let path = parts.next().ok_or_else(not_found)?;
    let number: i64 = parts
        .next()
        .and_then(|number| number.parse().ok())
        .ok_or_else(not_found)?;
    let comment_part = parts.next().ok_or_else(not_found)?;
    let row_id: i64 = comment_part
        .strip_prefix("comment:")
        .and_then(|row| row.parse().ok())
        .ok_or_else(not_found)?;
    let (owner, name) = path.split_once('/').ok_or_else(not_found)?;
    let open: OpenRepo = open_repo(g, owner, name).await?;
    let scope = PullScope {
        repo_id: &open.repo_id,
    };
    let Ok(pull) = pulls::get(&g.db, &scope, number).await else {
        return Ok(None);
    };
    let Some(row) =
        g.db.get_pull_request_comment(row_id)
            .await
            .map_err(internal)?
    else {
        return Ok(None);
    };
    if row.pull_request_id != pull.id || row.review_id.is_none() {
        return Ok(None);
    }
    Ok(Some(ReviewCommentNode::new(
        owner, name, number, &g.origin, row,
    )))
}

/// Decodes a commit's global ID, `C_` plus hex of `owner/name@oid`, and reads the commit.
async fn commit_for(g: &GqlContext, id: &str) -> Result<Option<CommitNode>> {
    let not_found = || gql_error("Could not resolve to a node", "NOT_FOUND");
    let body = id.strip_prefix("C_").ok_or_else(not_found)?;
    let text = unhex(body)?;
    let (path, oid) = text.split_once('@').ok_or_else(not_found)?;
    let (owner, name) = path.split_once('/').ok_or_else(not_found)?;
    open_repo(g, owner, name).await?;
    let Ok(handle) = super::handle_at(&g.project_root, owner, name) else {
        return Ok(None);
    };
    let Ok(oid_value) = git2::Oid::from_str(oid) else {
        return Ok(None);
    };
    let Ok(summary) = handle.commit_summary(oid_value) else {
        return Ok(None);
    };
    Ok(Some(CommitNode::from_summary(
        &g.origin,
        owner,
        name,
        &g.project_root,
        oid,
        summary,
    )))
}
