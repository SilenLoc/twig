#![allow(clippy::unused_async)]
// Resolvers are `async` because async-graphql requires it, even when a field reads only
// in-memory data.

//! GitHub-compatible GraphQL at `POST /api/graphql`, the endpoint `gh` uses for most
//! `gh pr` and `gh issue` commands. The schema covers the viewer, repositories, issues,
//! pull requests, and the mutations those commands send. Rules live in `crate::issues`
//! and `crate::pulls`; this module shapes requests and responses.
//!
//! Node IDs carry their location: a repository is `R_<hex of owner/name>`, and an issue
//! or pull request is `I_`/`PR_` plus hex of `owner/name#number`. A node ID therefore
//! decodes to the repository and number without a database lookup.
//!
//! Git access is synchronous and short: each helper opens its own `RepoHandle`, uses
//! it, and returns plain data. A handle is never held across an `await`, because
//! GraphQL resolver futures must be `Send` and libgit2 handles are not `Sync`.

use actix_web::{HttpRequest, HttpResponse, post, web};
use async_graphql::{
    Context, EmptySubscription, Enum, ErrorExtensions, InputObject, InputValueError,
    InputValueResult, Object, Result, Scalar, ScalarType, Schema, SimpleObject, Value,
};

use super::{
    shared::{numeric_id, origin},
    v1,
};
use crate::{
    auth::{
        AuthenticatedPrincipal, NamespacePermission, PermissionSet, TwigContext, User, permissions,
    },
    config,
    db::{
        Database,
        issues::{Issue, IssueFilter, IssueState},
        pull_requests::{PullRequest, PullRequestEdit, PullRequestFilter, PullRequestState},
    },
    git::bare::{MergeStrategy, RepoHandle},
    issues::{self, CreateInput, IssueChange, RepoRef as IssueScope},
    pulls::{self, Merger, RepoRef as PullScope},
};

pub type TwigSchema = Schema<Query, Mutation, EmptySubscription>;

/// A git object name, as GitHub's `GitObjectID` scalar: a hex string.
#[derive(Clone, Debug)]
pub struct GitObjectId(pub String);

#[Scalar(name = "GitObjectID")]
impl ScalarType for GitObjectId {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(text) => Ok(Self(text)),
            other => Err(InputValueError::expected_type(other)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.clone())
    }
}

/// What every resolver needs: the caller, the database, and where the repositories live.
pub struct GqlContext {
    pub db: Database,
    pub principal: AuthenticatedPrincipal,
    pub project_root: String,
    pub origin: String,
    pub is_admin: bool,
}

fn gql_context<'a>(ctx: &Context<'a>) -> Result<&'a GqlContext> {
    ctx.data::<GqlContext>()
}

/// A GraphQL error with a `type` extension, as GitHub reports them.
fn gql_error(message: impl Into<String>, kind: &'static str) -> async_graphql::Error {
    async_graphql::Error::new(message.into()).extend_with(|_, extensions| {
        extensions.set("type", kind);
    })
}

fn not_found(owner: &str, name: &str) -> async_graphql::Error {
    gql_error(
        format!("Could not resolve to a Repository with the name '{owner}/{name}'."),
        "NOT_FOUND",
    )
}

fn internal(detail: impl std::fmt::Display) -> async_graphql::Error {
    log::error!("GraphQL request failed: {detail}");
    gql_error("The request could not be completed", "INTERNAL")
}

/// Opens a repository on disk. Synchronous: callers use the handle and drop it before
/// any `await`.
fn handle_at(root: &str, owner: &str, name: &str) -> Result<RepoHandle> {
    RepoHandle::open(root, owner, name).map_err(|_| not_found(owner, name))
}

/// Resolves the caller's permission in `owner`, or fails as if the repository were absent.
async fn permission(
    g: &GqlContext,
    owner: &str,
    need: NamespacePermission,
) -> Result<PermissionSet> {
    match permissions::for_principal(&g.db, &g.principal, owner, g.is_admin).await {
        Ok(Some(current)) if current.contains(need) => Ok(current),
        Ok(_) => Err(gql_error(
            "The required permission is not granted",
            "FORBIDDEN",
        )),
        Err(detail) => Err(internal(detail)),
    }
}

/// A repository the caller may view. Holds no git handle, so it is safe across awaits.
pub struct OpenRepo {
    pub owner: String,
    pub name: String,
    pub repo_id: String,
    pub permissions: PermissionSet,
}

async fn open_repo(g: &GqlContext, owner: &str, name: &str) -> Result<OpenRepo> {
    v1::validate_namespace_name(owner).map_err(|_| not_found(owner, name))?;
    v1::validate_repository_name(name).map_err(|_| not_found(owner, name))?;
    let permissions = permission(g, owner, NamespacePermission::ViewNamespace)
        .await
        .map_err(|_| not_found(owner, name))?;
    let repo_id = handle_at(&g.project_root, owner, name)?
        .repo_id()
        .map_err(internal)?;
    Ok(OpenRepo {
        owner: owner.to_string(),
        name: name.to_string(),
        repo_id,
        permissions,
    })
}

fn hex_text(text: &str) -> String {
    hex::encode(text)
}

fn unhex(text: &str) -> Result<String> {
    let bytes =
        hex::decode(text).map_err(|_| gql_error("Could not resolve to a node", "NOT_FOUND"))?;
    String::from_utf8(bytes).map_err(|_| gql_error("Could not resolve to a node", "NOT_FOUND"))
}

fn repo_node_id(owner: &str, name: &str) -> String {
    format!("R_{}", hex_text(&format!("{owner}/{name}")))
}

fn numbered_node_id(prefix: &str, owner: &str, name: &str, number: i64) -> String {
    format!("{prefix}_{}", hex_text(&format!("{owner}/{name}#{number}")))
}

/// Splits a node ID into its owner, name, and number, checking the prefix.
fn decode_numbered(prefix: &str, id: &str) -> Result<(String, String, i64)> {
    let not_a_node = || gql_error("Could not resolve to a node", "NOT_FOUND");
    let body = id
        .strip_prefix(&format!("{prefix}_"))
        .ok_or_else(not_a_node)?;
    let text = unhex(body)?;
    let (path, number) = text.rsplit_once('#').ok_or_else(not_a_node)?;
    let (owner, name) = path.split_once('/').ok_or_else(not_a_node)?;
    let number: i64 = number.parse().map_err(|_| not_a_node())?;
    Ok((owner.to_string(), name.to_string(), number))
}

fn repo_from_node(id: &str) -> Result<(String, String)> {
    let not_a_repo = || gql_error("Could not resolve to a Repository", "NOT_FOUND");
    let body = id.strip_prefix("R_").ok_or_else(not_a_repo)?;
    let text = unhex(body)?;
    let (owner, name) = text.split_once('/').ok_or_else(not_a_repo)?;
    Ok((owner.to_string(), name.to_string()))
}

/// Pagination over an in-memory list. Cursors are the offset of the next row.
pub struct Page<T> {
    nodes: Vec<T>,
    total: i64,
    has_next: bool,
    end_cursor: Option<String>,
}

fn paginate<T>(rows: Vec<T>, first: Option<i32>, after: Option<&str>) -> Page<T> {
    let total = rows.len() as i64;
    let offset = after
        .and_then(|cursor| cursor.parse::<usize>().ok())
        .unwrap_or(0);
    let limit = first.unwrap_or(30).clamp(1, 100) as usize;
    let nodes: Vec<T> = rows.into_iter().skip(offset).take(limit).collect();
    let end = offset + nodes.len();
    Page {
        has_next: (end as i64) < total,
        end_cursor: (!nodes.is_empty()).then(|| end.to_string()),
        total,
        nodes,
    }
}

#[derive(SimpleObject, Clone)]
pub struct PageInfo {
    pub has_next_page: bool,
    pub has_previous_page: bool,
    pub end_cursor: Option<String>,
}

/// The last `n` rows, for `last:` arguments.
fn last_n<T>(mut rows: Vec<T>, last: Option<i32>) -> Vec<T> {
    if let Some(n) = last {
        let keep = n.max(0) as usize;
        if rows.len() > keep {
            rows.drain(..rows.len() - keep);
        }
    }
    rows
}

/// Declares a connection type with `nodes`, `pageInfo`, and `totalCount`. With a third
/// argument it also declares the edge type and an `edges` field.
macro_rules! connection {
    ($name:ident, $node:ty) => {
        connection!(@def $name, $node, );
    };
    ($name:ident, $node:ty, $edge:ident) => {
        connection!(@def $name, $node,
            async fn edges(&self) -> Vec<$edge> {
                self.nodes
                    .iter()
                    .cloned()
                    .map(|node| $edge { node })
                    .collect()
            }
        );

        #[derive(Clone)]
        pub struct $edge {
            node: $node,
        }

        #[Object]
        impl $edge {
            async fn node(&self) -> &$node {
                &self.node
            }

            async fn cursor(&self) -> String {
                String::new()
            }
        }
    };
    (@def $name:ident, $node:ty, $($edges:tt)*) => {
        pub struct $name {
            nodes: Vec<$node>,
            total: i64,
            has_next: bool,
            end_cursor: Option<String>,
        }

        impl From<Page<$node>> for $name {
            fn from(page: Page<$node>) -> Self {
                Self {
                    nodes: page.nodes,
                    total: page.total,
                    has_next: page.has_next,
                    end_cursor: page.end_cursor,
                }
            }
        }

        #[Object]
        impl $name {
            async fn nodes(&self) -> &Vec<$node> {
                &self.nodes
            }

            async fn total_count(&self) -> i64 {
                self.total
            }

            async fn issue_count(&self) -> i64 {
                self.total
            }

            async fn page_info(&self) -> PageInfo {
                PageInfo {
                    has_next_page: self.has_next,
                    has_previous_page: false,
                    end_cursor: self.end_cursor.clone(),
                }
            }

            $($edges)*
        }
    };
}

mod mutations;
mod review;

use mutations::{
    AddReactionInput, AddReactionPayload, AutoMergePayload, ConvertToDraftInput,
    ConvertToDraftPayload, DisableAutoMergeInput, EnableAutoMergeInput, FileViewedPayload,
    MarkFileViewedInput, ReactionNode, ReadyForReviewInput, ReadyForReviewPayload,
    RemoveReactionInput, RemoveReactionPayload, ResolveThreadInput, ResolveThreadPayload,
    RevertPullRequestInput, RevertPullRequestPayload, ThreadReplyInput, ThreadReplyPayload,
    UnmarkFileViewedInput, UnresolveThreadInput, UpdateBranchInput, UpdateBranchPayload,
    UpdateIssueCommentInput, UpdateIssueCommentPayload, UpdatePullRequestInput,
    UpdatePullRequestPayload, UpdateRefsInput, UpdateRefsPayload, UpdateReviewCommentInput,
    UpdateReviewCommentPayload, unsupported,
};
use review::empty_groups;
use review::{Node, Reactable, lookup_node};
pub use review::{PullRequestReviewThreadConnection, ReactionGroupNode, reaction_groups};

#[derive(Clone)]
pub struct UserNode {
    login: String,
    user_id: String,
    origin: String,
}

#[Object(name = "User")]
impl UserNode {
    async fn id(&self) -> String {
        format!("U_{}", hex_text(&self.user_id))
    }

    async fn database_id(&self) -> i64 {
        numeric_id(&self.user_id)
    }

    async fn login(&self) -> &str {
        &self.login
    }

    async fn name(&self) -> Option<String> {
        None
    }

    async fn url(&self) -> String {
        format!("{}/{}", self.origin, self.login)
    }

    async fn avatar_url(&self) -> String {
        String::new()
    }
}

/// A namespace as the owner of a repository, shaped as a `User`.
fn owner_node(origin: &str, owner: &str) -> UserNode {
    UserNode {
        login: owner.to_string(),
        user_id: format!("namespace:{owner}"),
        origin: origin.to_string(),
    }
}

fn user_node(origin: &str, user: &User) -> UserNode {
    UserNode {
        login: user.username.clone(),
        user_id: user.id.clone(),
        origin: origin.to_string(),
    }
}

async fn user_by_id(g: &GqlContext, user_id: &str) -> Result<Option<UserNode>> {
    Ok(g.db
        .get_user_by_id(user_id)
        .await
        .map_err(internal)?
        .map(|user| user_node(&g.origin, &user)))
}

connection!(UserConnection, UserNode);

#[derive(Clone)]
pub struct LabelNode;

#[Object(name = "Label")]
impl LabelNode {
    async fn id(&self) -> String {
        String::new()
    }

    async fn name(&self) -> &str {
        ""
    }

    async fn color(&self) -> &str {
        ""
    }

    async fn description(&self) -> Option<String> {
        None
    }
}

connection!(LabelConnection, LabelNode);

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
pub enum RepositoryPermission {
    Admin,
    Write,
    Read,
}

/// Milestones are not stored on Twig, so an issue never has one.
#[derive(Clone)]
pub struct MilestoneNode;

#[Object(name = "Milestone")]
impl MilestoneNode {
    async fn title(&self) -> &str {
        ""
    }

    async fn description(&self) -> Option<String> {
        None
    }

    async fn due_on(&self) -> Option<String> {
        None
    }

    async fn number(&self) -> i64 {
        0
    }
}

/// Who reacted in a group.
#[derive(Clone)]
pub struct ReactionUsersNode {
    total: i64,
}

#[Object(name = "ReactionUsers")]
impl ReactionUsersNode {
    async fn total_count(&self) -> i64 {
        self.total
    }
}

/// An issue type. Twig has no issue types, so none is ever returned.
#[derive(Clone)]
pub struct IssueTypeNode;

#[Object(name = "IssueType")]
impl IssueTypeNode {
    async fn id(&self) -> String {
        String::new()
    }

    async fn name(&self) -> &str {
        ""
    }

    async fn description(&self) -> Option<String> {
        None
    }

    async fn color(&self) -> Option<String> {
        None
    }
}

#[derive(Clone)]
pub struct SubIssuesSummaryNode;

#[Object(name = "SubIssuesSummary")]
impl SubIssuesSummaryNode {
    async fn total(&self) -> i64 {
        0
    }

    async fn completed(&self) -> i64 {
        0
    }

    async fn percent_completed(&self) -> i64 {
        0
    }
}

/// Projects are not stored on Twig, so an item never has one.
#[derive(Clone)]
pub struct ProjectV2Node;

#[Object(name = "ProjectV2")]
impl ProjectV2Node {
    async fn id(&self) -> String {
        String::new()
    }

    async fn title(&self) -> &str {
        ""
    }
}

#[derive(Clone)]
pub struct ProjectV2SingleSelectValueNode;

#[Object(name = "ProjectV2ItemFieldSingleSelectValue")]
impl ProjectV2SingleSelectValueNode {
    async fn option_id(&self) -> Option<String> {
        None
    }

    async fn name(&self) -> Option<String> {
        None
    }
}

#[derive(async_graphql::Union)]
pub enum ProjectV2ItemFieldValue {
    SingleSelect(ProjectV2SingleSelectValueNode),
}

#[derive(Clone)]
pub struct ProjectV2ItemNode;

#[Object(name = "ProjectV2Item")]
impl ProjectV2ItemNode {
    async fn id(&self) -> String {
        String::new()
    }

    async fn project(&self) -> ProjectV2Node {
        ProjectV2Node
    }

    async fn field_value_by_name(&self, name: String) -> Option<ProjectV2ItemFieldValue> {
        let _ = name;
        None
    }
}

connection!(ProjectItemConnection, ProjectV2ItemNode);

/// A GitHub App account. Twig has none, so none is ever returned.
#[derive(Clone)]
pub struct BotNode;

#[Object(name = "Bot")]
impl BotNode {
    async fn login(&self) -> &str {
        ""
    }

    async fn id(&self) -> String {
        String::new()
    }

    async fn avatar_url(&self, size: Option<i32>) -> &str {
        let _ = size;
        ""
    }
}

/// A mannequin is a placeholder for an account that no longer exists. Twig has none.
#[derive(Clone)]
pub struct MannequinNode;

#[Object(name = "Mannequin")]
impl MannequinNode {
    async fn login(&self) -> &str {
        ""
    }

    async fn avatar_url(&self, size: Option<i32>) -> &str {
        let _ = size;
        ""
    }
}

/// A team. Twig has no teams, so none is ever returned.
#[derive(Clone)]
pub struct TeamNode;

#[Object(name = "Team")]
impl TeamNode {
    async fn id(&self) -> String {
        String::new()
    }

    async fn name(&self) -> &str {
        ""
    }

    async fn slug(&self) -> &str {
        ""
    }

    async fn combined_slug(&self) -> &str {
        ""
    }

    async fn avatar_url(&self, size: Option<i32>) -> &str {
        let _ = size;
        ""
    }

    async fn organization(&self) -> Option<OrganizationNode> {
        None
    }
}

#[derive(async_graphql::Union)]
pub enum RequestedReviewer {
    User(UserNode),
    Team(TeamNode),
    Bot(BotNode),
}

/// Review states as GitHub names them.
#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "PullRequestReviewState")]
pub enum PullRequestReviewStateEnum {
    #[graphql(name = "APPROVED")]
    Approved,
    #[graphql(name = "CHANGES_REQUESTED")]
    ChangesRequested,
    #[graphql(name = "COMMENTED")]
    Commented,
}

#[derive(Clone)]
pub struct ReviewNode {
    review: crate::db::pull_requests::reviews::PullRequestReview,
    owner: String,
    name: String,
    number: i64,
    origin: String,
}

impl ReviewNode {
    fn review_state(&self) -> PullRequestReviewStateEnum {
        match self.review.state {
            crate::db::pull_requests::reviews::ReviewState::Approved => {
                PullRequestReviewStateEnum::Approved
            }
            crate::db::pull_requests::reviews::ReviewState::RequestChanges => {
                PullRequestReviewStateEnum::ChangesRequested
            }
            crate::db::pull_requests::reviews::ReviewState::Comment => {
                PullRequestReviewStateEnum::Commented
            }
        }
    }
}

#[Object(name = "PullRequestReview")]
impl ReviewNode {
    async fn id(&self) -> String {
        format!("PRR_{}", self.review.id)
    }

    async fn database_id(&self) -> i64 {
        self.review.id
    }

    async fn author(&self, ctx: &Context<'_>) -> Result<Option<UserNode>> {
        user_by_id(gql_context(ctx)?, &self.review.author_user_id).await
    }

    async fn body(&self) -> &str {
        &self.review.body
    }

    async fn body_text(&self) -> &str {
        &self.review.body
    }

    async fn state(&self) -> PullRequestReviewStateEnum {
        match self.review.state {
            crate::db::pull_requests::reviews::ReviewState::Approved => {
                PullRequestReviewStateEnum::Approved
            }
            crate::db::pull_requests::reviews::ReviewState::RequestChanges => {
                PullRequestReviewStateEnum::ChangesRequested
            }
            crate::db::pull_requests::reviews::ReviewState::Comment => {
                PullRequestReviewStateEnum::Commented
            }
        }
    }

    async fn submitted_at(&self) -> &str {
        &self.review.submitted_at
    }

    async fn created_at(&self) -> &str {
        &self.review.submitted_at
    }

    async fn commit(&self) -> Option<CommitRef> {
        Some(CommitRef {
            oid: self.review.commit_oid.clone(),
        })
    }

    async fn url(&self) -> String {
        format!(
            "{}/{}/{}/pull/{}#pullrequestreview-{}",
            self.origin, self.owner, self.name, self.number, self.review.id
        )
    }

    async fn author_association(&self) -> &str {
        "NONE"
    }

    async fn includes_created_edit(&self) -> bool {
        false
    }

    async fn viewer_did_author(&self, ctx: &Context<'_>) -> Result<bool> {
        Ok(gql_context(ctx)?.principal.user.id == self.review.author_user_id)
    }

    /// Twig does not record when a review was edited, and reviews are never minimised.
    async fn last_edited_at(&self) -> Option<&str> {
        None
    }

    async fn is_minimized(&self) -> bool {
        false
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

    /// GitHub lets people react to reviews; Twig does not, so every review has none.
    async fn reaction_groups(&self, _ctx: &Context<'_>) -> Result<Vec<ReactionGroupNode>> {
        Ok(review::empty_groups())
    }
}

connection!(ReviewConnection, ReviewNode);

/// Check suites, runs, and statuses are not produced by Twig; these shapes exist so
/// `gh pr` can read a rollup that is empty.
#[derive(Clone)]
pub struct WorkflowNode;

#[Object(name = "Workflow")]
impl WorkflowNode {
    async fn name(&self) -> &str {
        ""
    }
}

#[derive(Clone)]
pub struct WorkflowRunNode;

#[Object(name = "WorkflowRun")]
impl WorkflowRunNode {
    async fn workflow(&self) -> WorkflowNode {
        WorkflowNode
    }
}

#[derive(Clone)]
pub struct CheckSuiteNode;

#[Object(name = "CheckSuite")]
impl CheckSuiteNode {
    async fn workflow_run(&self) -> Option<WorkflowRunNode> {
        None
    }
}

#[derive(Clone)]
pub struct CheckRunNode;

#[Object(name = "CheckRun")]
impl CheckRunNode {
    async fn name(&self) -> &str {
        ""
    }

    async fn check_suite(&self) -> Option<CheckSuiteNode> {
        None
    }

    async fn status(&self) -> Option<String> {
        None
    }

    async fn conclusion(&self) -> Option<String> {
        None
    }

    async fn started_at(&self) -> Option<String> {
        None
    }

    async fn completed_at(&self) -> Option<String> {
        None
    }

    async fn details_url(&self) -> Option<String> {
        None
    }

    /// Branch protection is not configurable on Twig, so no check is required.
    async fn is_required(
        &self,
        pull_request_id: Option<String>,
        pull_request_number: Option<i32>,
    ) -> bool {
        let _ = (pull_request_id, pull_request_number);
        false
    }
}

#[derive(Clone)]
pub struct StatusContextNode;

#[Object(name = "StatusContext")]
impl StatusContextNode {
    async fn context(&self) -> &str {
        ""
    }

    async fn state(&self) -> Option<String> {
        None
    }

    async fn target_url(&self) -> Option<String> {
        None
    }

    async fn created_at(&self) -> Option<String> {
        None
    }

    async fn description(&self) -> Option<String> {
        None
    }

    /// Branch protection is not configurable on Twig, so no status is required.
    async fn is_required(
        &self,
        pull_request_id: Option<String>,
        pull_request_number: Option<i32>,
    ) -> bool {
        let _ = (pull_request_id, pull_request_number);
        false
    }
}

#[derive(async_graphql::Union)]
pub enum StatusCheckRollupContext {
    CheckRun(CheckRunNode),
    StatusContext(StatusContextNode),
}

connection!(@def StatusCheckRollupContextConnection, StatusCheckRollupContext,
    async fn check_run_counts_by_state(&self) -> Vec<StatusCountNode> {
        Vec::new()
    }

    async fn status_context_counts_by_state(&self) -> Vec<StatusCountNode> {
        Vec::new()
    }
);

/// The state of a status context, as GitHub names it.
#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "StatusState")]
pub enum StatusStateEnum {
    #[graphql(name = "EXPECTED")]
    Expected,
    #[graphql(name = "ERROR")]
    Error,
    #[graphql(name = "FAILURE")]
    Failure,
    #[graphql(name = "PENDING")]
    Pending,
    #[graphql(name = "SUCCESS")]
    Success,
}

#[derive(SimpleObject, Clone)]
pub struct StatusCountNode {
    pub state: String,
    pub count: i64,
}

#[derive(Clone)]
pub struct StatusCheckRollupNode;

#[Object(name = "StatusCheckRollup")]
impl StatusCheckRollupNode {
    async fn contexts(
        &self,
        first: Option<i32>,
        after: Option<String>,
    ) -> StatusCheckRollupContextConnection {
        paginate(
            Vec::<StatusCheckRollupContext>::new(),
            first,
            after.as_deref(),
        )
        .into()
    }

    /// Twig records no checks or statuses, so the rollup has no state.
    async fn state(&self) -> Option<StatusStateEnum> {
        None
    }

    async fn check_run_count(&self) -> i64 {
        0
    }

    async fn check_run_counts_by_state(&self) -> Vec<StatusCountNode> {
        Vec::new()
    }

    async fn status_context_count(&self) -> i64 {
        0
    }

    async fn status_context_counts_by_state(&self) -> Vec<StatusCountNode> {
        Vec::new()
    }
}

/// An organization. Twig has none, so none is ever returned.
#[derive(Clone)]
pub struct OrganizationNode;

#[Object(name = "Organization")]
impl OrganizationNode {
    async fn login(&self) -> &str {
        ""
    }

    async fn name(&self) -> Option<String> {
        None
    }
}

/// Auto-merge is not supported; a request never has an auto-merge request.
#[derive(Clone)]
pub struct AutoMergeRequestNode;

#[Object(name = "AutoMergeRequest")]
impl AutoMergeRequestNode {
    async fn merge_method(&self) -> Option<PullRequestMergeMethodEnum> {
        None
    }

    async fn enabled_by(&self) -> Option<UserNode> {
        None
    }

    async fn author_email(&self) -> Option<String> {
        None
    }

    async fn commit_body(&self) -> Option<String> {
        None
    }

    async fn commit_headline(&self) -> Option<String> {
        None
    }

    async fn enabled_at(&self) -> Option<String> {
        None
    }
}

/// A branch of a repository, as GitHub's `Ref` names it.
pub struct RefNode {
    name: String,
    repo_id: String,
    origin: String,
    owner: String,
    repo: String,
    root: String,
}

impl RefNode {
    fn for_repo(
        origin: &str,
        owner: &str,
        repo: &str,
        root: &str,
        repo_id: &str,
        name: &str,
    ) -> Self {
        Self {
            name: name.to_string(),
            repo_id: repo_id.to_string(),
            origin: origin.to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
            root: root.to_string(),
        }
    }
}

/// How two refs relate, as GitHub's `ComparisonStatus` names it.
#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "ComparisonStatus")]
pub enum ComparisonStatusEnum {
    #[graphql(name = "AHEAD")]
    Ahead,
    #[graphql(name = "BEHIND")]
    Behind,
    #[graphql(name = "DIVERGED")]
    Diverged,
    #[graphql(name = "IDENTICAL")]
    Identical,
}

/// The commits that separate a base ref from a head ref.
pub struct ComparisonNode {
    ahead_by: i64,
    behind_by: i64,
}

#[Object(name = "Comparison")]
impl ComparisonNode {
    async fn ahead_by(&self) -> i64 {
        self.ahead_by
    }

    async fn behind_by(&self) -> i64 {
        self.behind_by
    }

    async fn status(&self) -> ComparisonStatusEnum {
        match (self.ahead_by > 0, self.behind_by > 0) {
            (true, true) => ComparisonStatusEnum::Diverged,
            (true, false) => ComparisonStatusEnum::Ahead,
            (false, true) => ComparisonStatusEnum::Behind,
            (false, false) => ComparisonStatusEnum::Identical,
        }
    }
}

#[Object(name = "Ref")]
impl RefNode {
    async fn id(&self) -> String {
        format!(
            "REF_{}",
            hex_text(&format!("{}:{}", self.repo_id, self.name))
        )
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn prefix(&self) -> &str {
        "refs/heads/"
    }

    async fn repository(&self, ctx: &Context<'_>) -> Result<RepositoryNode> {
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.repo).await?;
        Ok(RepositoryNode::from_open(&open, g))
    }

    /// The commit at the tip of this branch.
    async fn target(&self) -> Option<CommitNode> {
        let handle = handle_at(&self.root, &self.owner, &self.repo).ok()?;
        let oid = handle.branch_tip(&self.name).ok().flatten()?;
        let summary = handle.commit_summary(oid).ok()?;
        Some(CommitNode::from_summary(
            &self.origin,
            &self.owner,
            &self.repo,
            &self.root,
            &oid.to_string(),
            summary,
        ))
    }

    /// Commits this ref has that `head_ref` lacks, and the reverse. `None` when either
    /// branch does not exist.
    async fn compare(&self, head_ref: String) -> Option<ComparisonNode> {
        let handle = handle_at(&self.root, &self.owner, &self.repo).ok()?;
        let base = handle.branch_tip(&self.name).ok().flatten()?;
        let head = handle.branch_tip(&head_ref).ok().flatten()?;
        let ahead = handle.commit_ids_in_range(base, head).ok()?.len();
        let behind = handle.commit_ids_in_range(head, base).ok()?.len();
        Some(ComparisonNode {
            ahead_by: i64::try_from(ahead).unwrap_or_default(),
            behind_by: i64::try_from(behind).unwrap_or_default(),
        })
    }

    /// Branch protection is not configurable on Twig.
    async fn branch_protection_rule(&self) -> Option<BranchProtectionRuleNode> {
        None
    }
}

#[derive(Clone)]
pub struct BranchProtectionRuleNode;

#[Object(name = "BranchProtectionRule")]
impl BranchProtectionRuleNode {
    async fn id(&self) -> String {
        String::new()
    }

    async fn pattern(&self) -> &str {
        ""
    }

    async fn requires_strict_status_checks(&self) -> bool {
        false
    }

    async fn requires_status_checks(&self) -> bool {
        false
    }

    async fn requires_approving_reviews(&self) -> bool {
        false
    }

    async fn required_approving_review_count(&self) -> i64 {
        0
    }

    async fn required_status_check_contexts(&self) -> Vec<String> {
        Vec::new()
    }

    async fn is_admin_enforced(&self) -> bool {
        false
    }
}

/// A repository the caller can view.
pub struct RepositoryNode {
    owner: String,
    name: String,
    repo_id: String,
    origin: String,
    root: String,
    permissions: PermissionSet,
}

impl RepositoryNode {
    fn from_open(open: &OpenRepo, g: &GqlContext) -> Self {
        Self {
            owner: open.owner.clone(),
            name: open.name.clone(),
            repo_id: open.repo_id.clone(),
            origin: g.origin.clone(),
            root: g.project_root.clone(),
            permissions: open.permissions.clone(),
        }
    }

    fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    fn default_branch(&self) -> Option<String> {
        handle_at(&self.root, &self.owner, &self.name)
            .ok()
            .and_then(|handle| handle.default_branch_name().ok().flatten())
    }
}

#[Object(name = "Repository")]
impl RepositoryNode {
    async fn id(&self) -> String {
        repo_node_id(&self.owner, &self.name)
    }

    async fn database_id(&self) -> i64 {
        numeric_id(&self.repo_id)
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn name_with_owner(&self) -> String {
        self.full_name()
    }

    async fn url(&self) -> String {
        format!("{}/{}", self.origin, self.full_name())
    }

    async fn ssh_url(&self) -> String {
        String::new()
    }

    async fn description(&self) -> Option<String> {
        None
    }

    async fn is_private(&self) -> bool {
        false
    }

    async fn is_fork(&self) -> bool {
        false
    }

    async fn is_archived(&self) -> bool {
        false
    }

    async fn is_empty(&self) -> bool {
        self.default_branch().is_none()
    }

    async fn has_issues_enabled(&self) -> bool {
        true
    }

    async fn merge_commit_allowed(&self) -> bool {
        true
    }

    async fn squash_merge_allowed(&self) -> bool {
        true
    }

    async fn rebase_merge_allowed(&self) -> bool {
        true
    }

    async fn default_branch_ref(&self) -> Option<RefNode> {
        self.default_branch().map(|name| {
            RefNode::for_repo(
                &self.origin,
                &self.owner,
                &self.name,
                &self.root,
                &self.repo_id,
                &name,
            )
        })
    }

    async fn owner(&self) -> UserNode {
        owner_node(&self.origin, &self.owner)
    }

    async fn has_wiki_enabled(&self) -> bool {
        false
    }

    async fn is_in_organization(&self) -> bool {
        false
    }

    async fn viewer_has_starred(&self) -> bool {
        false
    }

    async fn delete_branch_on_merge(&self) -> bool {
        false
    }

    /// Auto-merge is not supported on Twig, so a request can never be set to merge later.
    async fn auto_merge_allowed(&self) -> bool {
        false
    }

    /// Twig has no label catalogue, so the candidate list is empty.
    async fn labels(
        &self,
        first: Option<i32>,
        after: Option<String>,
        query: Option<String>,
        order_by: Option<LabelOrder>,
    ) -> LabelConnection {
        let _ = (query, order_by);
        paginate(Vec::<LabelNode>::new(), first, after.as_deref()).into()
    }

    /// Twig assigns users without an assignable list, so the candidate list is empty.
    async fn assignable_users(
        &self,
        first: Option<i32>,
        after: Option<String>,
        query: Option<String>,
    ) -> UserConnection {
        let _ = query;
        paginate(Vec::<UserNode>::new(), first, after.as_deref()).into()
    }

    async fn mentionable_users(
        &self,
        first: Option<i32>,
        after: Option<String>,
        query: Option<String>,
    ) -> UserConnection {
        let _ = query;
        paginate(Vec::<UserNode>::new(), first, after.as_deref()).into()
    }

    /// A branch by qualified name, such as `refs/heads/main`. `None` when it does not exist.
    #[graphql(name = "ref")]
    async fn ref_by_name(&self, qualified_name: String) -> Option<RefNode> {
        let name = qualified_name
            .strip_prefix("refs/heads/")
            .unwrap_or(&qualified_name);
        let handle = handle_at(&self.root, &self.owner, &self.name).ok()?;
        handle.branch_tip(name).ok().flatten()?;
        Some(RefNode::for_repo(
            &self.origin,
            &self.owner,
            &self.name,
            &self.root,
            &self.repo_id,
            name,
        ))
    }

    async fn open_graph_image_url(&self) -> String {
        String::new()
    }

    async fn security_policy_url(&self) -> Option<String> {
        None
    }

    async fn parent(&self) -> Option<RepositoryNode> {
        None
    }

    async fn viewer_permission(&self) -> RepositoryPermission {
        if self
            .permissions
            .contains(NamespacePermission::ManageRepositories)
        {
            RepositoryPermission::Admin
        } else if self.permissions.contains(NamespacePermission::PushChanges) {
            RepositoryPermission::Write
        } else {
            RepositoryPermission::Read
        }
    }

    async fn viewer_can_administer(&self) -> bool {
        self.permissions
            .contains(NamespacePermission::ManageRepositories)
    }

    async fn issue(
        &self,
        ctx: &Context<'_>,
        number: i32,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<Option<IssueNode>> {
        let _ = (first, after);
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.name).await?;
        let scope = IssueScope {
            repo_id: &open.repo_id,
            namespace: &self.owner,
        };
        match issues::get(&g.db, &scope, i64::from(number)).await {
            Ok(issue) => Ok(Some(IssueNode::new(&open, issue, g))),
            Err(_) => Ok(None),
        }
    }

    async fn issues(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
        states: Option<Vec<IssueStateEnum>>,
        order_by: Option<IssueOrder>,
        filter_by: Option<IssueFilters>,
    ) -> Result<IssueConnection> {
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.name).await?;
        let scope = IssueScope {
            repo_id: &open.repo_id,
            namespace: &self.owner,
        };
        let states = states.or_else(|| filter_by.as_ref().and_then(|f| f.states.clone()));
        let mut filter = IssueFilter {
            state: single_state(states.as_deref()),
            limit: 10_000,
            ..Default::default()
        };
        if let Some(filters) = &filter_by {
            if filters
                .labels
                .as_ref()
                .is_some_and(|labels| !labels.is_empty())
                || filters.milestone.is_some()
            {
                return Ok(paginate(Vec::new(), first, after.as_deref()).into());
            }
            if let Some(name) = filters.assignee.as_deref() {
                match issues::account_filter(&g.db, Some(name))
                    .await
                    .map_err(mutation_error)?
                {
                    issues::AccountFilter::Only(id) => filter.assignee_user_id = Some(id),
                    _ => return Ok(paginate(Vec::new(), first, after.as_deref()).into()),
                }
            }
            if let Some(name) = filters.created_by.as_deref() {
                match issues::account_filter(&g.db, Some(name))
                    .await
                    .map_err(mutation_error)?
                {
                    issues::AccountFilter::Only(id) => filter.author_user_id = Some(id),
                    _ => return Ok(paginate(Vec::new(), first, after.as_deref()).into()),
                }
            }
        }
        let found = issues::list(&g.db, &scope, &filter)
            .await
            .map_err(|e| internal(format!("{e:?}")))?;
        let mut rows = found.items;
        if order_by
            .as_ref()
            .is_some_and(|order| order.direction == OrderDirection::Asc)
        {
            rows.reverse();
        }
        let nodes: Vec<IssueNode> = rows
            .into_iter()
            .map(|issue| IssueNode::new(&open, issue, g))
            .collect();
        Ok(paginate(nodes, first, after.as_deref()).into())
    }

    async fn issue_or_pull_request(
        &self,
        ctx: &Context<'_>,
        number: i32,
    ) -> Result<Option<IssueOrPullRequest>> {
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.name).await?;
        let scope = IssueScope {
            repo_id: &open.repo_id,
            namespace: &self.owner,
        };
        if let Ok(issue) = issues::get(&g.db, &scope, i64::from(number)).await {
            return Ok(Some(IssueOrPullRequest::Issue(IssueNode::new(
                &open, issue, g,
            ))));
        }
        let pull_scope = PullScope {
            repo_id: &open.repo_id,
        };
        match pulls::get(&g.db, &pull_scope, i64::from(number)).await {
            Ok(pull) => Ok(Some(IssueOrPullRequest::PullRequest(PullRequestNode::new(
                g, &open, pull,
            )))),
            Err(_) => Ok(None),
        }
    }

    async fn pull_request(
        &self,
        ctx: &Context<'_>,
        number: i32,
    ) -> Result<Option<PullRequestNode>> {
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.name).await?;
        let scope = PullScope {
            repo_id: &open.repo_id,
        };
        match pulls::get(&g.db, &scope, i64::from(number)).await {
            Ok(pull) => Ok(Some(PullRequestNode::new(g, &open, pull))),
            Err(_) => Ok(None),
        }
    }

    async fn pull_requests(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
        states: Option<Vec<PullRequestStateEnum>>,
        head_ref_name: Option<String>,
        base_ref_name: Option<String>,
        order_by: Option<PullRequestOrder>,
    ) -> Result<PullRequestConnection> {
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.name).await?;
        if base_ref_name.is_some() {
            return Err(gql_error(
                "Filtering pull requests by base is not supported on Twig",
                "UNSUPPORTED",
            ));
        }
        let filter = PullRequestFilter {
            state: single_pull_state(states.as_deref()),
            head_branch: head_ref_name,
            query: None,
            recently_updated: false,
            limit: 10_000,
            offset: 0,
        };
        let scope = PullScope {
            repo_id: &open.repo_id,
        };
        let (rows, _) = pulls::list(&g.db, &scope, &filter)
            .await
            .map_err(|e| internal(format!("{e:?}")))?;
        let mut rows = rows;
        if order_by
            .as_ref()
            .is_some_and(|order| order.direction == OrderDirection::Asc)
        {
            rows.reverse();
        }
        let nodes: Vec<PullRequestNode> = rows
            .into_iter()
            .map(|pull| PullRequestNode::new(g, &open, pull))
            .collect();
        Ok(paginate(nodes, first, after.as_deref()).into())
    }
}

/// The state filter for issues. No states, or `OPEN`, keeps open issues; `CLOSED` keeps
/// closed ones. Both states together keep all of them, which is `None`.
fn single_state(states: Option<&[IssueStateEnum]>) -> Option<IssueState> {
    match states.unwrap_or(&[]) {
        [] | [IssueStateEnum::Open] => Some(IssueState::Open),
        [IssueStateEnum::Closed] => Some(IssueState::Closed),
        _ => None,
    }
}

/// The state filter for pull requests. `MERGED` and `CLOSED` both keep closed rows.
fn single_pull_state(states: Option<&[PullRequestStateEnum]>) -> Option<PullRequestState> {
    match states.unwrap_or(&[]) {
        [] | [PullRequestStateEnum::Open] => Some(PullRequestState::Open),
        [PullRequestStateEnum::Closed] | [PullRequestStateEnum::Merged] => {
            Some(PullRequestState::Closed)
        }
        _ => None,
    }
}

#[derive(async_graphql::Union)]
pub enum IssueOrPullRequest {
    Issue(IssueNode),
    PullRequest(PullRequestNode),
}

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "IssueState")]
pub enum IssueStateEnum {
    Open,
    Closed,
}

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "PullRequestState")]
pub enum PullRequestStateEnum {
    Open,
    Closed,
    Merged,
}

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "PullRequestMergeMethod")]
pub enum PullRequestMergeMethodEnum {
    Merge,
    Squash,
    Rebase,
}

#[derive(Clone)]
pub struct IssueNode {
    owner: String,
    name: String,
    origin: String,
    issue: Issue,
}

impl IssueNode {
    fn new(open: &OpenRepo, issue: Issue, g: &GqlContext) -> Self {
        Self {
            owner: open.owner.clone(),
            name: open.name.clone(),
            origin: g.origin.clone(),
            issue,
        }
    }
}

#[Object(name = "Issue")]
impl IssueNode {
    async fn id(&self) -> String {
        numbered_node_id("I", &self.owner, &self.name, self.issue.number)
    }

    async fn database_id(&self) -> i64 {
        numeric_id(&self.issue.id)
    }

    async fn number(&self) -> i64 {
        self.issue.number
    }

    async fn title(&self) -> &str {
        &self.issue.title
    }

    async fn body(&self) -> &str {
        &self.issue.body
    }

    async fn body_text(&self) -> &str {
        &self.issue.body
    }

    async fn state(&self) -> IssueStateEnum {
        match self.issue.state {
            IssueState::Open => IssueStateEnum::Open,
            IssueState::Closed => IssueStateEnum::Closed,
        }
    }

    async fn state_reason(&self) -> Option<&'static str> {
        (self.issue.state == IssueState::Closed).then_some("COMPLETED")
    }

    async fn closed(&self) -> bool {
        self.issue.state == IssueState::Closed
    }

    async fn closed_at(&self) -> Option<String> {
        self.issue.closed_at.clone()
    }

    async fn created_at(&self) -> &str {
        &self.issue.created_at
    }

    async fn updated_at(&self) -> &str {
        &self.issue.updated_at
    }

    async fn url(&self) -> String {
        format!(
            "{}/{}/{}/issues/{}",
            self.origin, self.owner, self.name, self.issue.number
        )
    }

    async fn milestone(&self) -> Option<MilestoneNode> {
        None
    }

    async fn project_items(
        &self,
        first: Option<i32>,
        after: Option<String>,
    ) -> ProjectItemConnection {
        paginate(Vec::<ProjectV2ItemNode>::new(), first, after.as_deref()).into()
    }

    async fn reaction_groups(&self) -> Vec<ReactionGroupNode> {
        Vec::new()
    }

    async fn issue_type(&self) -> Option<IssueTypeNode> {
        None
    }

    async fn parent(&self) -> Option<IssueNode> {
        None
    }

    async fn sub_issues(&self, first: Option<i32>) -> IssueConnection {
        paginate(Vec::<IssueNode>::new(), first, None).into()
    }

    async fn sub_issues_summary(&self) -> SubIssuesSummaryNode {
        SubIssuesSummaryNode
    }

    async fn is_pinned(&self) -> bool {
        false
    }

    async fn locked(&self) -> bool {
        false
    }

    async fn author(&self, ctx: &Context<'_>) -> Result<Option<UserNode>> {
        user_by_id(gql_context(ctx)?, &self.issue.author_user_id).await
    }

    async fn assignees(&self, ctx: &Context<'_>, first: Option<i32>) -> Result<UserConnection> {
        let g = gql_context(ctx)?;
        let mut users = Vec::new();
        for user_id in &self.issue.assignee_user_ids {
            if let Some(user) = user_by_id(g, user_id).await? {
                users.push(user);
            }
        }
        Ok(paginate(users, first, None).into())
    }

    async fn labels(&self, first: Option<i32>) -> LabelConnection {
        paginate(Vec::<LabelNode>::new(), first, None).into()
    }

    async fn comments(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<CommentConnection> {
        let g = gql_context(ctx)?;
        let rows =
            g.db.list_issue_comments(&self.issue.id)
                .await
                .map_err(internal)?;
        let nodes: Vec<CommentNode> = rows
            .into_iter()
            .map(|row| CommentNode {
                row_id: row.row_id,
                body: row.body,
                author_user_id: row.author_user_id,
                created_at: row.created_at,
                updated_at: row.updated_at,
                origin: self.origin.clone(),
                owner: self.owner.clone(),
                name: self.name.clone(),
                pull_number: None,
            })
            .collect();
        Ok(paginate(last_n(nodes, last), first, None).into())
    }

    async fn repository(&self, ctx: &Context<'_>) -> Result<RepositoryNode> {
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.name).await?;
        Ok(RepositoryNode::from_open(&open, g))
    }
}

connection!(IssueConnection, IssueNode, IssueEdge);

impl CommentNode {
    /// A conversation comment on a request.
    pub fn for_pull_comment(
        origin: &str,
        owner: &str,
        name: &str,
        number: i64,
        comment: crate::db::pull_requests::comments::PullRequestComment,
    ) -> Self {
        Self {
            row_id: comment.id,
            body: comment.body,
            author_user_id: comment.author_user_id,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
            origin: origin.to_string(),
            owner: owner.to_string(),
            name: name.to_string(),
            pull_number: Some(number),
        }
    }

    /// A comment on an issue.
    pub fn for_issue_comment(
        origin: &str,
        owner: &str,
        name: &str,
        comment: crate::db::issues::comments::IssueComment,
    ) -> Self {
        Self {
            row_id: comment.row_id,
            body: comment.body,
            author_user_id: comment.author_user_id,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
            origin: origin.to_string(),
            owner: owner.to_string(),
            name: name.to_string(),
            pull_number: None,
        }
    }
}

#[derive(Clone)]
pub struct CommentNode {
    row_id: i64,
    body: String,
    author_user_id: String,
    created_at: String,
    updated_at: Option<String>,
    origin: String,
    owner: String,
    name: String,
    /// The request a conversation comment belongs to. Issue comments have none, take no
    /// reactions, and name no request.
    pull_number: Option<i64>,
}

#[Object(name = "IssueComment")]
impl CommentNode {
    async fn id(&self) -> String {
        let key = match self.pull_number {
            Some(number) => format!(
                "{}/{}#{}#comment:{}",
                self.owner, self.name, number, self.row_id
            ),
            None => format!("{}/{}#comment:{}", self.owner, self.name, self.row_id),
        };
        format!("IC_{}", hex_text(&key))
    }

    async fn database_id(&self) -> i64 {
        self.row_id
    }

    async fn body(&self) -> &str {
        &self.body
    }

    async fn body_text(&self) -> &str {
        &self.body
    }

    async fn created_at(&self) -> &str {
        &self.created_at
    }

    async fn updated_at(&self) -> Option<&str> {
        self.updated_at.as_deref()
    }

    async fn last_edited_at(&self) -> Option<&str> {
        self.updated_at.as_deref()
    }

    async fn is_minimized(&self) -> bool {
        false
    }

    /// The request this comment belongs to, when it is a request's conversation comment.
    async fn pull_request(&self, ctx: &Context<'_>) -> Result<Option<PullRequestNode>> {
        let Some(number) = self.pull_number else {
            return Ok(None);
        };
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.name).await?;
        let scope = PullScope {
            repo_id: &open.repo_id,
        };
        match pulls::get(&g.db, &scope, number).await {
            Ok(pull) => Ok(Some(PullRequestNode::new(g, &open, pull))),
            Err(_) => Ok(None),
        }
    }

    async fn author_association(&self) -> &str {
        "NONE"
    }

    async fn viewer_did_author(&self, ctx: &Context<'_>) -> Result<bool> {
        Ok(gql_context(ctx)?.principal.user.id == self.author_user_id)
    }

    async fn includes_created_edit(&self) -> bool {
        false
    }

    async fn minimized_reason(&self) -> Option<String> {
        None
    }

    async fn reaction_groups(&self, ctx: &Context<'_>) -> Result<Vec<ReactionGroupNode>> {
        if self.pull_number.is_none() {
            return Ok(empty_groups());
        }
        let key = crate::db::pull_requests::reactions::comment_key(self.row_id);
        reaction_groups(gql_context(ctx)?, &key).await
    }

    async fn url(&self) -> String {
        format!(
            "{}/{}/{}#issuecomment-{}",
            self.origin, self.owner, self.name, self.row_id
        )
    }

    async fn author(&self, ctx: &Context<'_>) -> Result<Option<UserNode>> {
        user_by_id(gql_context(ctx)?, &self.author_user_id).await
    }
}

connection!(CommentConnection, CommentNode);

/// A stack of requests that a request belongs to. Twig does not stack requests.
#[derive(SimpleObject, Clone)]
#[graphql(name = "PullRequestStack")]
pub struct PullRequestStackNode {
    pub number: i64,
    pub size: i64,
    pub base_ref_name: String,
}

/// A request's place in its stack. Twig does not stack requests.
#[derive(SimpleObject, Clone)]
#[graphql(name = "PullRequestStackEntry")]
pub struct PullRequestStackEntryNode {
    pub position: i64,
}

/// The review a dismissal refers to, by ID only.
#[derive(SimpleObject, Clone)]
#[graphql(name = "ReviewRef")]
pub struct ReviewRefNode {
    pub id: String,
}

/// A review was dismissed. Twig never dismisses a review, so none is ever listed.
#[derive(Clone)]
pub struct ReviewDismissedEventNode;

#[Object(name = "ReviewDismissedEvent")]
impl ReviewDismissedEventNode {
    async fn dismissal_message(&self) -> Option<String> {
        None
    }

    async fn review(&self) -> Option<ReviewRefNode> {
        None
    }
}

/// A pull request left a merge queue. Twig has no merge queue, so none is ever listed.
#[derive(Clone)]
pub struct RemovedFromMergeQueueEventNode;

#[Object(name = "RemovedFromMergeQueueEvent")]
impl RemovedFromMergeQueueEventNode {
    async fn id(&self) -> String {
        String::new()
    }

    async fn created_at(&self) -> Option<String> {
        None
    }
}

#[derive(async_graphql::Union)]
pub enum PullRequestTimelineItems {
    ReviewDismissedEvent(ReviewDismissedEventNode),
    RemovedFromMergeQueueEvent(RemovedFromMergeQueueEventNode),
}

connection!(PullRequestTimelineItemConnection, PullRequestTimelineItems);

/// The kinds of timeline event a listing may keep, as GitHub names them.
#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "PullRequestTimelineItemsItemType")]
pub enum PullRequestTimelineItemTypeEnum {
    #[graphql(name = "REVIEW_DISMISSED_EVENT")]
    ReviewDismissedEvent,
    #[graphql(name = "REMOVED_FROM_MERGE_QUEUE_EVENT")]
    RemovedFromMergeQueueEvent,
}

#[derive(Clone)]
pub struct PullRequestNode {
    owner: String,
    name: String,
    origin: String,
    root: String,
    pull: PullRequest,
}

impl PullRequestNode {
    fn new(g: &GqlContext, open: &OpenRepo, pull: PullRequest) -> Self {
        Self {
            owner: open.owner.clone(),
            name: open.name.clone(),
            origin: g.origin.clone(),
            root: g.project_root.clone(),
            pull,
        }
    }

    fn branch_ref(&self, branch: &str) -> RefNode {
        RefNode::for_repo(
            &self.origin,
            &self.owner,
            &self.name,
            &self.root,
            &self.pull.repo_id,
            branch,
        )
    }

    fn pr_state(&self) -> PullRequestStateEnum {
        if self.pull.merged_at.is_some() {
            PullRequestStateEnum::Merged
        } else if self.pull.state == PullRequestState::Open {
            PullRequestStateEnum::Open
        } else {
            PullRequestStateEnum::Closed
        }
    }

    /// The base and head tips as hex, when both branches exist.
    fn tips(&self) -> Option<(String, String)> {
        let handle = handle_at(&self.root, &self.owner, &self.name).ok()?;
        let base = handle.branch_tip(&self.pull.base_branch).ok().flatten()?;
        let head = handle.branch_tip(&self.pull.head_branch).ok().flatten()?;
        Some((base.to_string(), head.to_string()))
    }

    /// Whether the head merges cleanly now, or `None` once the request is closed.
    fn mergeable_flag(&self) -> Option<bool> {
        let handle = handle_at(&self.root, &self.owner, &self.name).ok()?;
        crate::pulls::mergeable(&handle, &self.pull)
    }

    /// Lines added, lines removed, and files changed since the merge base.
    fn change_size(&self) -> (i64, i64, i64) {
        match handle_at(&self.root, &self.owner, &self.name) {
            Ok(handle) => crate::http::api::shared::change_size(&handle, &self.pull),
            Err(_) => (0, 0, 0),
        }
    }

    fn commit_nodes(&self) -> Result<Vec<PullRequestCommitNode>> {
        let handle = handle_at(&self.root, &self.owner, &self.name)?;
        let ids = crate::pulls::change_commits(&handle, &self.pull);
        let mut nodes = Vec::with_capacity(ids.len());
        for oid in ids.iter().rev() {
            let summary = handle.commit_summary(*oid).map_err(internal)?;
            nodes.push(PullRequestCommitNode {
                commit: CommitNode {
                    oid: oid.to_string(),
                    headline: summary.summary,
                    body: summary.body,
                    author_name: summary.author_name,
                    author_email: summary.author_email,
                    committed_at: summary.committed_at,
                    origin: self.origin.clone(),
                    owner: self.owner.clone(),
                    name: self.name.clone(),
                    root: self.root.clone(),
                    parents: summary.parents.iter().map(ToString::to_string).collect(),
                },
            });
        }
        Ok(nodes)
    }

    fn file_nodes(&self) -> Result<Vec<PullRequestFileNode>> {
        let handle = handle_at(&self.root, &self.owner, &self.name)?;
        let Some((from, to)) = crate::pulls::change_range(&handle, &self.pull) else {
            return Ok(Vec::new());
        };
        let files = handle.diff_files(from, to).map_err(internal)?;
        Ok(files
            .iter()
            .map(|file| PullRequestFileNode {
                path: file.path().to_string(),
                additions: i64::try_from(file.additions).unwrap_or_default(),
                deletions: i64::try_from(file.deletions).unwrap_or_default(),
            })
            .collect())
    }
}

#[Object(name = "PullRequest")]
impl PullRequestNode {
    async fn id(&self) -> String {
        numbered_node_id("PR", &self.owner, &self.name, self.pull.number)
    }

    async fn database_id(&self) -> i64 {
        numeric_id(&self.pull.id)
    }

    async fn number(&self) -> i64 {
        self.pull.number
    }

    async fn title(&self) -> &str {
        &self.pull.title
    }

    async fn body(&self) -> &str {
        &self.pull.body
    }

    async fn body_text(&self) -> &str {
        &self.pull.body
    }

    async fn state(&self) -> PullRequestStateEnum {
        self.pr_state()
    }

    async fn is_draft(&self) -> bool {
        crate::http::api::shared::is_work_in_progress(&self.pull.title)
    }

    async fn merged(&self) -> bool {
        self.pull.merged_at.is_some()
    }

    async fn merged_at(&self) -> Option<String> {
        self.pull.merged_at.clone()
    }

    async fn closed(&self) -> bool {
        self.pull.state == PullRequestState::Closed
    }

    async fn closed_at(&self) -> Option<String> {
        self.pull.closed_at.clone()
    }

    async fn created_at(&self) -> &str {
        &self.pull.created_at
    }

    async fn updated_at(&self) -> &str {
        &self.pull.updated_at
    }

    async fn url(&self) -> String {
        format!(
            "{}/{}/{}/pull/{}",
            self.origin, self.owner, self.name, self.pull.number
        )
    }

    async fn head_ref_name(&self) -> &str {
        &self.pull.head_branch
    }

    async fn base_ref_name(&self) -> &str {
        &self.pull.base_branch
    }

    async fn head_ref_oid(&self) -> String {
        self.tips().map(|(_, head)| head).unwrap_or_default()
    }

    async fn base_ref_oid(&self) -> String {
        self.tips().map(|(base, _)| base).unwrap_or_default()
    }

    async fn is_cross_repository(&self) -> bool {
        false
    }

    async fn maintainer_can_modify(&self) -> bool {
        false
    }

    async fn latest_reviews(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<ReviewConnection> {
        let g = gql_context(ctx)?;
        let rows =
            g.db.list_pull_request_reviews(&self.pull.id)
                .await
                .map_err(internal)?;
        let nodes: Vec<ReviewNode> = rows
            .into_iter()
            .map(|review| ReviewNode {
                review,
                owner: self.owner.clone(),
                name: self.name.clone(),
                number: self.pull.number,
                origin: self.origin.clone(),
            })
            .collect();
        Ok(paginate(nodes, first, after.as_deref()).into())
    }

    async fn base_ref(&self) -> Option<RefNode> {
        Some(self.branch_ref(&self.pull.base_branch))
    }

    async fn head_ref(&self) -> Option<RefNode> {
        Some(self.branch_ref(&self.pull.head_branch))
    }

    async fn auto_merge_request(&self) -> Option<AutoMergeRequestNode> {
        None
    }

    async fn reviews(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
        last: Option<i32>,
        states: Option<Vec<PullRequestReviewStateEnum>>,
    ) -> Result<ReviewConnection> {
        let g = gql_context(ctx)?;
        let rows =
            g.db.list_pull_request_reviews(&self.pull.id)
                .await
                .map_err(internal)?;
        let nodes: Vec<ReviewNode> = rows
            .into_iter()
            .map(|review| ReviewNode {
                review,
                owner: self.owner.clone(),
                name: self.name.clone(),
                number: self.pull.number,
                origin: self.origin.clone(),
            })
            .filter(|node| {
                states.as_ref().is_none_or(|wanted| {
                    let state = node.review_state();
                    wanted.contains(&state)
                })
            })
            .collect();
        Ok(paginate(last_n(nodes, last), first, after.as_deref()).into())
    }

    async fn milestone(&self) -> Option<MilestoneNode> {
        None
    }

    async fn project_items(
        &self,
        first: Option<i32>,
        after: Option<String>,
    ) -> ProjectItemConnection {
        paginate(Vec::<ProjectV2ItemNode>::new(), first, after.as_deref()).into()
    }

    async fn reaction_groups(&self, ctx: &Context<'_>) -> Result<Vec<ReactionGroupNode>> {
        let key = crate::db::pull_requests::reactions::pull_key(&self.pull.id);
        reaction_groups(gql_context(ctx)?, &key).await
    }

    /// The inline comments on the request's diff, one thread each.
    async fn review_threads(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<PullRequestReviewThreadConnection> {
        let threads = self.review_thread_nodes(gql_context(ctx)?).await?;
        Ok(paginate(threads, first, after.as_deref()).into())
    }

    /// Twig does not stack requests, so no request is part of a stack.
    async fn stack(&self) -> Option<PullRequestStackNode> {
        None
    }

    async fn stack_entry(&self) -> Option<PullRequestStackEntryNode> {
        None
    }

    /// The default merge body for a merge method. Twig writes its own merge commits and
    /// offers no default body.
    async fn viewer_merge_body_text(
        &self,
        merge_type: PullRequestMergeMethodEnum,
    ) -> Option<String> {
        let _ = merge_type;
        None
    }

    async fn head_repository(&self, ctx: &Context<'_>) -> Result<Option<RepositoryNode>> {
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.name).await?;
        Ok(Some(RepositoryNode::from_open(&open, g)))
    }

    async fn base_repository(&self, ctx: &Context<'_>) -> Result<Option<RepositoryNode>> {
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.name).await?;
        Ok(Some(RepositoryNode::from_open(&open, g)))
    }

    async fn head_repository_owner(&self, ctx: &Context<'_>) -> Result<Option<UserNode>> {
        let g = gql_context(ctx)?;
        Ok(Some(owner_node(&g.origin, &self.owner)))
    }

    async fn mergeable(&self) -> &'static str {
        match self.mergeable_flag() {
            Some(true) => "MERGEABLE",
            Some(false) => "CONFLICTING",
            None => "UNKNOWN",
        }
    }

    async fn merge_state_status(&self) -> &'static str {
        match self.mergeable_flag() {
            Some(true) => "CLEAN",
            Some(false) => "DIRTY",
            None => "UNKNOWN",
        }
    }

    async fn review_decision(&self) -> Option<&'static str> {
        None
    }

    async fn author(&self, ctx: &Context<'_>) -> Result<Option<UserNode>> {
        user_by_id(gql_context(ctx)?, &self.pull.author_user_id).await
    }

    async fn merged_by(&self, ctx: &Context<'_>) -> Result<Option<UserNode>> {
        match &self.pull.merged_by_user_id {
            Some(user_id) => user_by_id(gql_context(ctx)?, user_id).await,
            None => Ok(None),
        }
    }

    async fn assignees(&self, first: Option<i32>) -> UserConnection {
        paginate(Vec::<UserNode>::new(), first, None).into()
    }

    async fn labels(&self, first: Option<i32>) -> LabelConnection {
        paginate(Vec::<LabelNode>::new(), first, None).into()
    }

    async fn comments(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
        last: Option<i32>,
        order_by: Option<IssueCommentOrder>,
    ) -> Result<CommentConnection> {
        let g = gql_context(ctx)?;
        let rows =
            g.db.list_pull_request_comments(&self.pull.id)
                .await
                .map_err(internal)?;
        let mut nodes: Vec<CommentNode> = rows
            .into_iter()
            .map(|row| CommentNode {
                row_id: row.id,
                pull_number: Some(self.pull.number),
                body: row.body,
                author_user_id: row.author_user_id,
                created_at: row.created_at,
                updated_at: row.updated_at,
                origin: self.origin.clone(),
                owner: self.owner.clone(),
                name: self.name.clone(),
            })
            .collect();
        if let Some(order) = &order_by {
            // Only `UPDATED_AT` exists. A comment never edited counts as updated when it
            // was created.
            nodes.sort_by_key(|node| node.updated_at.clone().unwrap_or(node.created_at.clone()));
            if order.direction == OrderDirection::Desc {
                nodes.reverse();
            }
        }
        Ok(paginate(last_n(nodes, last), first, after.as_deref()).into())
    }

    async fn repository(&self, ctx: &Context<'_>) -> Result<RepositoryNode> {
        let g = gql_context(ctx)?;
        let open = open_repo(g, &self.owner, &self.name).await?;
        Ok(RepositoryNode::from_open(&open, g))
    }

    async fn additions(&self) -> i64 {
        self.change_size().0
    }

    async fn deletions(&self) -> i64 {
        self.change_size().1
    }

    async fn changed_files(&self) -> i64 {
        self.change_size().2
    }

    async fn merge_commit(&self) -> Option<CommitRef> {
        self.pull
            .merge_commit_oid
            .clone()
            .map(|oid| CommitRef { oid })
    }

    async fn review_requests(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<ReviewRequestConnection> {
        let g = gql_context(ctx)?;
        let names =
            g.db.list_requested_reviewers(&self.pull.id)
                .await
                .map_err(internal)?;
        let mut nodes = Vec::new();
        for name in names {
            if let Some(user) = g.db.get_user_by_username(&name).await.map_err(internal)? {
                nodes.push(ReviewRequestNode {
                    reviewer: user_node(&g.origin, &user),
                });
            }
        }
        Ok(paginate(nodes, first, after.as_deref()).into())
    }

    async fn commits(
        &self,
        first: Option<i32>,
        after: Option<String>,
        last: Option<i32>,
    ) -> Result<PullRequestCommitConnection> {
        Ok(paginate(last_n(self.commit_nodes()?, last), first, after.as_deref()).into())
    }

    async fn files(
        &self,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<PullRequestFileConnection> {
        Ok(paginate(self.file_nodes()?, first, after.as_deref()).into())
    }

    /// Whether the caller wrote this request.
    async fn viewer_did_author(&self, ctx: &Context<'_>) -> Result<bool> {
        Ok(gql_context(ctx)?.principal.user.id == self.pull.author_user_id)
    }

    /// Whether the caller may change this request: its author, or anyone who may push.
    async fn viewer_can_update(&self, ctx: &Context<'_>) -> Result<bool> {
        let g = gql_context(ctx)?;
        if g.principal.user.id == self.pull.author_user_id {
            return Ok(true);
        }
        Ok(permission(g, &self.owner, NamespacePermission::PushChanges)
            .await
            .is_ok())
    }

    /// Twig has no update-branch operation, so the head is never brought up to date.
    async fn viewer_can_update_branch(&self) -> bool {
        false
    }

    /// Twig has no merge queue.
    async fn is_merge_queue_enabled(&self) -> bool {
        false
    }

    /// Twig records no timeline events that GitHub lists here, so the timeline is empty.
    async fn timeline_items(
        &self,
        first: Option<i32>,
        after: Option<String>,
        item_types: Option<Vec<PullRequestTimelineItemTypeEnum>>,
    ) -> PullRequestTimelineItemConnection {
        let _ = item_types;
        paginate(
            Vec::<PullRequestTimelineItems>::new(),
            first,
            after.as_deref(),
        )
        .into()
    }
}

#[derive(SimpleObject, Clone)]
pub struct CommitRef {
    pub oid: String,
}

pub struct CommitNode {
    oid: String,
    headline: String,
    body: String,
    author_name: String,
    author_email: String,
    committed_at: i64,
    origin: String,
    owner: String,
    name: String,
    root: String,
    parents: Vec<String>,
}

impl CommitNode {
    pub fn from_summary(
        origin: &str,
        owner: &str,
        name: &str,
        root: &str,
        oid: &str,
        summary: crate::git::bare::CommitSummary,
    ) -> Self {
        Self {
            oid: oid.to_string(),
            headline: summary.summary,
            body: summary.body,
            author_name: summary.author_name,
            author_email: summary.author_email,
            committed_at: summary.committed_at,
            origin: origin.to_string(),
            owner: owner.to_string(),
            name: name.to_string(),
            root: root.to_string(),
            parents: summary.parents.iter().map(ToString::to_string).collect(),
        }
    }

    /// Lines added and removed by this commit against its first parent. A root commit
    /// has no parent to compare with, so it reports none.
    fn line_stats(&self) -> (i64, i64) {
        let Some(parent) = self.parents.first() else {
            return (0, 0);
        };
        let (Ok(handle), Ok(from), Ok(to)) = (
            handle_at(&self.root, &self.owner, &self.name),
            git2::Oid::from_str(parent),
            git2::Oid::from_str(&self.oid),
        ) else {
            return (0, 0);
        };
        match handle.diff_files(from, to) {
            Ok(files) => files.iter().fold((0, 0), |(add, del), file| {
                (
                    add + i64::try_from(file.additions).unwrap_or_default(),
                    del + i64::try_from(file.deletions).unwrap_or_default(),
                )
            }),
            Err(_) => (0, 0),
        }
    }

    fn actor(&self) -> GitActor {
        GitActor {
            name: self.author_name.clone(),
            email: self.author_email.clone(),
        }
    }
}

#[Object(name = "Commit")]
impl CommitNode {
    async fn id(&self) -> String {
        format!(
            "C_{}",
            hex_text(&format!("{}/{}@{}", self.owner, self.name, self.oid))
        )
    }

    async fn additions(&self) -> i64 {
        self.line_stats().0
    }

    async fn deletions(&self) -> i64 {
        self.line_stats().1
    }

    /// The commits this one was made on top of.
    async fn parents(&self, first: Option<i32>, after: Option<String>) -> CommitParentConnection {
        let Ok(handle) = handle_at(&self.root, &self.owner, &self.name) else {
            return paginate(Vec::<CommitNode>::new(), first, after.as_deref()).into();
        };
        let nodes: Vec<CommitNode> = self
            .parents
            .iter()
            .filter_map(|parent| {
                let oid = git2::Oid::from_str(parent).ok()?;
                let summary = handle.commit_summary(oid).ok()?;
                Some(CommitNode {
                    oid: parent.clone(),
                    headline: summary.summary,
                    body: summary.body,
                    author_name: summary.author_name,
                    author_email: summary.author_email,
                    committed_at: summary.committed_at,
                    origin: self.origin.clone(),
                    owner: self.owner.clone(),
                    name: self.name.clone(),
                    root: self.root.clone(),
                    parents: summary.parents.iter().map(ToString::to_string).collect(),
                })
            })
            .collect();
        paginate(nodes, first, after.as_deref()).into()
    }

    /// The people who authored this commit. Git records one author per commit.
    async fn authors(&self, first: Option<i32>, after: Option<String>) -> GitActorConnection {
        paginate(vec![self.actor()], first, after.as_deref()).into()
    }

    async fn oid(&self) -> &str {
        &self.oid
    }

    async fn abbreviated_oid(&self) -> String {
        self.oid.chars().take(7).collect()
    }

    async fn message(&self) -> String {
        if self.body.is_empty() {
            self.headline.clone()
        } else {
            format!("{}\n\n{}", self.headline, self.body)
        }
    }

    async fn message_headline(&self) -> &str {
        &self.headline
    }

    async fn message_body(&self) -> &str {
        &self.body
    }

    async fn committed_date(&self) -> String {
        chrono::DateTime::from_timestamp(self.committed_at, 0)
            .map(|time| time.to_rfc3339())
            .unwrap_or_default()
    }

    async fn url(&self) -> String {
        format!(
            "{}/{}/{}/commit/{}",
            self.origin, self.owner, self.name, self.oid
        )
    }

    async fn status_check_rollup(&self) -> Option<StatusCheckRollupNode> {
        None
    }

    async fn author(&self) -> GitActor {
        GitActor {
            name: self.author_name.clone(),
            email: self.author_email.clone(),
        }
    }
}

pub struct GitActor {
    name: String,
    email: String,
}

connection!(GitActorConnection, GitActor);
connection!(CommitParentConnection, CommitNode);

#[Object(name = "GitActor")]
impl GitActor {
    async fn name(&self) -> &str {
        &self.name
    }

    async fn email(&self) -> &str {
        &self.email
    }

    /// Git authors have no avatar on Twig.
    async fn avatar_url(&self, size: Option<i32>) -> &str {
        let _ = size;
        ""
    }

    /// The account that wrote the commit, when git's author email matches one.
    /// Twig does not map git authors to accounts, so none is known.
    async fn user(&self) -> Option<UserNode> {
        None
    }
}

pub struct PullRequestCommitNode {
    commit: CommitNode,
}

#[Object(name = "PullRequestCommit")]
impl PullRequestCommitNode {
    async fn commit(&self) -> &CommitNode {
        &self.commit
    }
}

connection!(PullRequestCommitConnection, PullRequestCommitNode);

pub struct PullRequestFileNode {
    path: String,
    additions: i64,
    deletions: i64,
}

/// Whether the viewer has marked a file as viewed. Twig does not record it.
#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "FileViewedState")]
pub enum FileViewedStateEnum {
    #[graphql(name = "DISMISSED")]
    Dismissed,
    #[graphql(name = "UNVIEWED")]
    Unviewed,
    #[graphql(name = "VIEWED")]
    Viewed,
}

#[Object(name = "PullRequestChangedFile")]
impl PullRequestFileNode {
    async fn path(&self) -> &str {
        &self.path
    }

    async fn viewer_viewed_state(&self) -> FileViewedStateEnum {
        FileViewedStateEnum::Unviewed
    }

    async fn additions(&self) -> i64 {
        self.additions
    }

    async fn deletions(&self) -> i64 {
        self.deletions
    }
}

connection!(PullRequestFileConnection, PullRequestFileNode);

pub struct ReviewRequestNode {
    reviewer: UserNode,
}

#[Object(name = "ReviewRequest")]
impl ReviewRequestNode {
    async fn requested_reviewer(&self) -> RequestedReviewer {
        RequestedReviewer::User(self.reviewer.clone())
    }
}

connection!(ReviewRequestConnection, ReviewRequestNode);

connection!(PullRequestConnection, PullRequestNode, PullRequestEdge);

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
pub enum IssueOrderField {
    CreatedAt,
    UpdatedAt,
    Comments,
}

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
pub enum PullRequestOrderField {
    CreatedAt,
    UpdatedAt,
}

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
pub enum OrderDirection {
    Asc,
    Desc,
}

/// The fields a label listing may sort by.
#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "LabelOrderField")]
pub enum LabelOrderField {
    #[graphql(name = "CREATED_AT")]
    CreatedAt,
    #[graphql(name = "NAME")]
    Name,
}

#[derive(InputObject)]
#[graphql(name = "LabelOrder")]
pub struct LabelOrder {
    pub field: LabelOrderField,
    pub direction: OrderDirection,
}

/// The fields a comment listing may sort by.
#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "IssueCommentOrderField")]
pub enum IssueCommentOrderField {
    #[graphql(name = "UPDATED_AT")]
    UpdatedAt,
}

#[derive(InputObject)]
#[graphql(name = "IssueCommentOrder")]
pub struct IssueCommentOrder {
    pub field: IssueCommentOrderField,
    pub direction: OrderDirection,
}

#[derive(InputObject)]
pub struct IssueOrder {
    pub field: IssueOrderField,
    pub direction: OrderDirection,
}

#[derive(InputObject)]
pub struct PullRequestOrder {
    pub field: PullRequestOrderField,
    pub direction: OrderDirection,
}

/// Issue filters as `gh issue list` sends them. Labels and milestones are not stored on
/// Twig, so they match nothing when named.
#[derive(InputObject)]
pub struct IssueFilters {
    pub assignee: Option<String>,
    pub created_by: Option<String>,
    pub mentioned: Option<String>,
    pub labels: Option<Vec<String>>,
    pub milestone: Option<String>,
    pub states: Option<Vec<IssueStateEnum>>,
}

#[derive(async_graphql::Union)]
pub enum SearchItem {
    PullRequest(PullRequestNode),
    Issue(IssueNode),
}

impl Clone for SearchItem {
    fn clone(&self) -> Self {
        match self {
            Self::PullRequest(node) => Self::PullRequest(node.clone()),
            Self::Issue(node) => Self::Issue(node.clone()),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "SearchType")]
pub enum SearchTypeEnum {
    Issue,
    IssueAdvanced,
    Repository,
    User,
}

/// The parts of a search query Twig understands: a repository, the kind of item, its
/// state, and its people. Free text is ignored.
#[derive(Default)]
struct SearchQuery {
    repo: Option<(String, String)>,
    want_pulls: Option<bool>,
    open: Option<bool>,
    merged: Option<bool>,
    author: Option<String>,
    assignee: Option<String>,
    review_requested: Option<String>,
}

fn parse_search(query: &str) -> SearchQuery {
    let mut parsed = SearchQuery::default();
    for token in query.split_whitespace() {
        let Some((key, value)) = token.split_once(':') else {
            continue;
        };
        match key {
            "repo" => {
                parsed.repo = value
                    .split_once('/')
                    .map(|(owner, name)| (owner.to_string(), name.to_string()));
            }
            "is" | "type" => match value {
                "pr" | "pull-request" => parsed.want_pulls = Some(true),
                "issue" => parsed.want_pulls = Some(false),
                "open" => parsed.open = Some(true),
                "closed" => parsed.open = Some(false),
                "merged" => {
                    parsed.merged = Some(true);
                    parsed.open = Some(false);
                }
                _ => {}
            },
            "author" => parsed.author = Some(value.to_string()),
            "assignee" => parsed.assignee = Some(value.to_string()),
            "review-requested" => parsed.review_requested = Some(value.to_string()),
            _ => {}
        }
    }
    parsed
}

connection!(SearchConnection, SearchItem, SearchEdge);

/// The user ID for a login, if the account exists.
async fn user_id_for(g: &GqlContext, login: &str) -> Result<Option<String>> {
    Ok(g.db
        .get_user_by_username(login)
        .await
        .map_err(internal)?
        .map(|user| user.id))
}

pub struct Query;

#[Object]
impl Query {
    /// Looks up anything with a global ID: a repository, request, issue, commit, or
    /// review thread. An ID that names nothing returns `null`.
    async fn node(&self, ctx: &Context<'_>, id: String) -> Result<Option<Node>> {
        lookup_node(gql_context(ctx)?, &id).await
    }

    /// Looks up several global IDs, in the order given.
    async fn nodes(&self, ctx: &Context<'_>, ids: Vec<String>) -> Result<Vec<Option<Node>>> {
        let g = gql_context(ctx)?;
        let mut found = Vec::with_capacity(ids.len());
        for id in &ids {
            found.push(lookup_node(g, id).await?);
        }
        Ok(found)
    }

    /// Searches issues and pull requests in one repository (`repo:owner/name`). Results
    /// are filtered by `is:`, `author:`, `assignee:`, and `review-requested:`.
    async fn search(
        &self,
        ctx: &Context<'_>,
        query: String,
        #[graphql(name = "type")] search_type: SearchTypeEnum,
        first: Option<i32>,
        after: Option<String>,
    ) -> Result<SearchConnection> {
        let g = gql_context(ctx)?;
        let parsed = parse_search(&query);
        let Some((owner, name)) = parsed.repo.clone() else {
            return Ok(paginate(Vec::<SearchItem>::new(), first, after.as_deref()).into());
        };
        if !matches!(
            search_type,
            SearchTypeEnum::Issue | SearchTypeEnum::IssueAdvanced
        ) {
            return Ok(paginate(Vec::<SearchItem>::new(), first, after.as_deref()).into());
        }
        let open = open_repo(g, &owner, &name).await?;
        let author_id = match &parsed.author {
            Some(login) => user_id_for(g, login).await?,
            None => None,
        };
        let assignee_id = match &parsed.assignee {
            Some(login) => user_id_for(g, login).await?,
            None => None,
        };
        let mut items: Vec<(i64, SearchItem)> = Vec::new();
        let state = parsed.open.map(|open| {
            if open {
                PullRequestState::Open
            } else {
                PullRequestState::Closed
            }
        });
        if parsed.want_pulls != Some(false) {
            let scope = PullScope {
                repo_id: &open.repo_id,
            };
            let filter = PullRequestFilter {
                state,
                head_branch: None,
                query: None,
                recently_updated: false,
                limit: 10_000,
                offset: 0,
            };
            let (rows, _) = pulls::list(&g.db, &scope, &filter)
                .await
                .map_err(|e| internal(format!("{e:?}")))?;
            for pull in rows {
                if parsed.merged == Some(true) && pull.merged_at.is_none() {
                    continue;
                }
                if parsed.merged == Some(false) && pull.merged_at.is_some() {
                    continue;
                }
                if author_id
                    .as_ref()
                    .is_some_and(|id| *id != pull.author_user_id)
                {
                    continue;
                }
                if let Some(login) = &parsed.review_requested {
                    let requested =
                        g.db.list_requested_reviewers(&pull.id)
                            .await
                            .map_err(internal)?;
                    if !requested.iter().any(|name| name == login) {
                        continue;
                    }
                }
                let key = pull.number;
                items.push((
                    key,
                    SearchItem::PullRequest(PullRequestNode::new(g, &open, pull)),
                ));
            }
        }
        if parsed.want_pulls != Some(true) && parsed.review_requested.is_none() {
            let scope = IssueScope {
                repo_id: &open.repo_id,
                namespace: &owner,
            };
            let filter = IssueFilter {
                state: match parsed.open {
                    Some(true) => Some(IssueState::Open),
                    Some(false) => Some(IssueState::Closed),
                    None => None,
                },
                limit: 10_000,
                ..Default::default()
            };
            let found = issues::list(&g.db, &scope, &filter)
                .await
                .map_err(|e| internal(format!("{e:?}")))?;
            for issue in found.items {
                if author_id
                    .as_ref()
                    .is_some_and(|id| *id != issue.author_user_id)
                {
                    continue;
                }
                if assignee_id
                    .as_ref()
                    .is_some_and(|id| !issue.assignee_user_ids.contains(id))
                {
                    continue;
                }
                let key = issue.number;
                items.push((key, SearchItem::Issue(IssueNode::new(&open, issue, g))));
            }
        }
        items.sort_by(|a, b| b.0.cmp(&a.0));
        let rows: Vec<SearchItem> = items.into_iter().map(|(_, item)| item).collect();
        Ok(paginate(rows, first, after.as_deref()).into())
    }

    /// The signed-in account.
    async fn viewer(&self, ctx: &Context<'_>) -> Result<UserNode> {
        let g = gql_context(ctx)?;
        Ok(user_node(&g.origin, &g.principal.user))
    }

    /// A repository the caller may view, by owner and name.
    async fn repository(
        &self,
        ctx: &Context<'_>,
        owner: String,
        name: String,
    ) -> Result<Option<RepositoryNode>> {
        let g = gql_context(ctx)?;
        match open_repo(g, &owner, &name).await {
            Ok(open) => Ok(Some(RepositoryNode::from_open(&open, g))),
            Err(_) => Ok(None),
        }
    }
}

pub struct Mutation;

#[derive(InputObject)]
pub struct CreateIssueInput {
    pub repository_id: String,
    pub title: String,
    pub body: Option<String>,
}

#[derive(SimpleObject)]
pub struct CreateIssuePayload {
    pub client_mutation_id: Option<String>,
    pub issue: IssueNode,
}

#[derive(InputObject)]
pub struct UpdateIssueInput {
    pub id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub assignee_ids: Option<Vec<String>>,
    pub label_ids: Option<Vec<String>>,
    pub client_mutation_id: Option<String>,
}

#[derive(InputObject)]
pub struct CloseIssueInput {
    pub issue_id: String,
    pub state_reason: Option<String>,
    pub client_mutation_id: Option<String>,
}

#[derive(InputObject)]
pub struct ReopenIssueInput {
    pub issue_id: String,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
pub struct IssuePayload {
    pub client_mutation_id: Option<String>,
    pub issue: IssueNode,
}

#[derive(InputObject)]
pub struct AddCommentInput {
    pub subject_id: String,
    pub body: String,
}

#[derive(SimpleObject)]
pub struct CommentEdge {
    pub node: CommentNode,
}

#[derive(SimpleObject)]
pub struct AddCommentPayload {
    pub client_mutation_id: Option<String>,
    pub comment_edge: CommentEdge,
}

#[derive(InputObject)]
pub struct CreatePullRequestInput {
    pub repository_id: String,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub title: String,
    pub body: Option<String>,
    pub draft: Option<bool>,
    pub maintainer_can_modify: Option<bool>,
    pub client_mutation_id: Option<String>,
    pub head_repository_id: Option<String>,
}

#[derive(SimpleObject)]
pub struct PullRequestPayload {
    pub client_mutation_id: Option<String>,
    pub pull_request: PullRequestNode,
}

#[derive(InputObject)]
pub struct PullRequestIdInput {
    pub pull_request_id: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
#[graphql(name = "PullRequestReviewEvent")]
pub enum PullRequestReviewEventEnum {
    #[graphql(name = "APPROVE")]
    Approve,
    #[graphql(name = "REQUEST_CHANGES")]
    RequestChanges,
    #[graphql(name = "COMMENT")]
    Comment,
}

#[derive(InputObject)]
pub struct AddPullRequestReviewInput {
    pub pull_request_id: String,
    pub event: Option<PullRequestReviewEventEnum>,
    pub body: Option<String>,
    pub commit_oid: Option<String>,
    pub client_mutation_id: Option<String>,
}

#[derive(SimpleObject)]
pub struct AddPullRequestReviewPayload {
    pub client_mutation_id: Option<String>,
    pub pull_request_review: ReviewNode,
}

#[derive(InputObject)]
pub struct MergePullRequestInput {
    pub pull_request_id: String,
    pub merge_method: Option<PullRequestMergeMethodEnum>,
    pub commit_headline: Option<String>,
    pub commit_body: Option<String>,
    pub expected_head_oid: Option<String>,
}

async fn issue_for(g: &GqlContext, id: &str) -> Result<(OpenRepo, Issue)> {
    let (owner, name, number) = decode_numbered("I", id)?;
    let open = open_repo(g, &owner, &name).await?;
    let scope = IssueScope {
        repo_id: &open.repo_id,
        namespace: &owner,
    };
    let issue = issues::get(&g.db, &scope, number)
        .await
        .map_err(|_| gql_error("Could not resolve to an issue", "NOT_FOUND"))?;
    Ok((open, issue))
}

async fn pull_for(g: &GqlContext, id: &str) -> Result<(OpenRepo, PullRequest)> {
    let (owner, name, number) = decode_numbered("PR", id)?;
    let open = open_repo(g, &owner, &name).await?;
    let scope = PullScope {
        repo_id: &open.repo_id,
    };
    let pull = pulls::get(&g.db, &scope, number)
        .await
        .map_err(|_| gql_error("Could not resolve to a pull request", "NOT_FOUND"))?;
    Ok((open, pull))
}

fn issue_actor(g: &GqlContext) -> issues::Actor<'_> {
    issues::Actor {
        user_id: &g.principal.user.id,
    }
}

fn pull_actor(g: &GqlContext) -> pulls::Actor<'_> {
    pulls::Actor {
        user_id: &g.principal.user.id,
    }
}

/// Turns a domain failure into the GraphQL error a client shows.
trait DomainFailure {
    fn into_gql(self) -> async_graphql::Error;
}

impl DomainFailure for crate::pulls::PullError {
    fn into_gql(self) -> async_graphql::Error {
        use crate::pulls::PullError;
        match self {
            PullError::NotFound => gql_error("Could not resolve to a pull request", "NOT_FOUND"),
            PullError::Forbidden => {
                gql_error("The required permission is not granted", "FORBIDDEN")
            }
            PullError::Invalid(message) | PullError::Conflict(message) => {
                gql_error(message, "UNPROCESSABLE")
            }
            PullError::Internal(detail) => internal(detail),
        }
    }
}

impl DomainFailure for crate::issues::IssueError {
    fn into_gql(self) -> async_graphql::Error {
        use crate::issues::IssueError;
        match self {
            IssueError::NotFound => gql_error("Could not resolve to an issue", "NOT_FOUND"),
            IssueError::Forbidden => {
                gql_error("The required permission is not granted", "FORBIDDEN")
            }
            IssueError::Invalid(message) => gql_error(message, "UNPROCESSABLE"),
            IssueError::Internal(detail) => internal(detail),
        }
    }
}

fn mutation_error<E: DomainFailure>(failure: E) -> async_graphql::Error {
    failure.into_gql()
}

#[Object]
impl Mutation {
    async fn create_issue(
        &self,
        ctx: &Context<'_>,
        input: CreateIssueInput,
    ) -> Result<CreateIssuePayload> {
        let g = gql_context(ctx)?;
        let (owner, name) = repo_from_node(&input.repository_id)?;
        let open = open_repo(g, &owner, &name).await?;
        let scope = IssueScope {
            repo_id: &open.repo_id,
            namespace: &owner,
        };
        let issue = issues::create(
            &g.db,
            &scope,
            &issue_actor(g),
            &open.permissions,
            CreateInput {
                title: &input.title,
                body: input.body.as_deref().unwrap_or(""),
                assignees: &[],
            },
        )
        .await
        .map_err(mutation_error)?;
        Ok(CreateIssuePayload {
            client_mutation_id: None,
            issue: IssueNode::new(&open, issue, g),
        })
    }

    async fn update_issue(
        &self,
        ctx: &Context<'_>,
        input: UpdateIssueInput,
    ) -> Result<IssuePayload> {
        let g = gql_context(ctx)?;
        if input
            .label_ids
            .as_ref()
            .is_some_and(|labels| !labels.is_empty())
        {
            return Err(gql_error(
                "Labels are not supported on Twig",
                "UNPROCESSABLE",
            ));
        }
        let (open, issue) = issue_for(g, &input.id).await?;
        let mut assignees = None;
        if let Some(ids) = &input.assignee_ids {
            let mut names = Vec::with_capacity(ids.len());
            for id in ids {
                let user_id = id
                    .strip_prefix("U_")
                    .and_then(|hex| unhex(hex).ok())
                    .ok_or_else(|| gql_error("Could not resolve to a User", "NOT_FOUND"))?;
                let user =
                    g.db.get_user_by_id(&user_id)
                        .await
                        .map_err(internal)?
                        .ok_or_else(|| gql_error("Could not resolve to a User", "NOT_FOUND"))?;
                names.push(user.username);
            }
            assignees = Some(names);
        }
        let scope = IssueScope {
            repo_id: &open.repo_id,
            namespace: &open.owner,
        };
        let updated = issues::change(
            &g.db,
            &scope,
            &issue,
            &issue_actor(g),
            &open.permissions,
            IssueChange {
                title: input.title.clone(),
                body: input.body.clone(),
                state: None,
                assignees,
            },
        )
        .await
        .map_err(mutation_error)?;
        Ok(IssuePayload {
            client_mutation_id: None,
            issue: IssueNode::new(&open, updated, g),
        })
    }

    async fn close_issue(&self, ctx: &Context<'_>, input: CloseIssueInput) -> Result<IssuePayload> {
        set_issue_state(ctx, &input.issue_id, IssueState::Closed).await
    }

    async fn reopen_issue(
        &self,
        ctx: &Context<'_>,
        input: ReopenIssueInput,
    ) -> Result<IssuePayload> {
        set_issue_state(ctx, &input.issue_id, IssueState::Open).await
    }

    async fn add_comment(
        &self,
        ctx: &Context<'_>,
        input: AddCommentInput,
    ) -> Result<AddCommentPayload> {
        let g = gql_context(ctx)?;
        if input.subject_id.starts_with("PR_") {
            let (open, pull) = pull_for(g, &input.subject_id).await?;
            let comment =
                pulls::add_comment(&g.db, &pull, &pull_actor(g), &open.permissions, &input.body)
                    .await
                    .map_err(mutation_error)?;
            return Ok(AddCommentPayload {
                client_mutation_id: None,
                comment_edge: CommentEdge {
                    node: CommentNode {
                        row_id: comment.id,
                        pull_number: Some(pull.number),
                        body: comment.body,
                        author_user_id: comment.author_user_id,
                        created_at: comment.created_at,
                        updated_at: comment.updated_at,
                        origin: g.origin.clone(),
                        owner: open.owner.clone(),
                        name: open.name.clone(),
                    },
                },
            });
        }
        let (open, issue) = issue_for(g, &input.subject_id).await?;
        let comment = issues::add_comment(
            &g.db,
            &issue,
            &issue_actor(g),
            &open.permissions,
            &input.body,
        )
        .await
        .map_err(mutation_error)?;
        Ok(AddCommentPayload {
            client_mutation_id: None,
            comment_edge: CommentEdge {
                node: CommentNode {
                    row_id: comment.row_id,
                    pull_number: None,
                    body: comment.body,
                    author_user_id: comment.author_user_id,
                    created_at: comment.created_at,
                    updated_at: comment.updated_at,
                    origin: g.origin.clone(),
                    owner: open.owner.clone(),
                    name: open.name.clone(),
                },
            },
        })
    }

    async fn create_pull_request(
        &self,
        ctx: &Context<'_>,
        input: CreatePullRequestInput,
    ) -> Result<PullRequestPayload> {
        let g = gql_context(ctx)?;
        let (owner, name) = repo_from_node(&input.repository_id)?;
        let open = open_repo(g, &owner, &name).await?;
        if !open.permissions.contains(NamespacePermission::PushChanges) {
            return Err(gql_error(
                "The required permission is not granted",
                "FORBIDDEN",
            ));
        }
        let title = if input.draft == Some(true)
            && !crate::http::api::shared::is_work_in_progress(&input.title)
        {
            format!("WIP: {}", input.title)
        } else {
            input.title.clone()
        };
        let title = pulls::checked_title(&title)
            .map_err(mutation_error)?
            .to_string();
        let base = input.base_ref_name.clone();
        let head = input.head_ref_name.clone();
        // Git checks run in a short block: the handle is gone before the first await.
        let head_tip = {
            let handle = handle_at(&g.project_root, &owner, &name)?;
            pulls::check_create_branches(&handle, &base, &head).map_err(mutation_error)?
        };
        let scope = PullScope {
            repo_id: &open.repo_id,
        };
        let pull = pulls::create_record(
            &g.db,
            &scope,
            &pull_actor(g),
            &open.permissions,
            &title,
            input.body.as_deref().unwrap_or(""),
            &base,
            &head,
        )
        .await
        .map_err(mutation_error)?;
        {
            let handle = handle_at(&g.project_root, &owner, &name)?;
            pulls::attach_head_ref(&handle, &pull, head_tip);
        }
        Ok(PullRequestPayload {
            client_mutation_id: None,
            pull_request: PullRequestNode::new(g, &open, pull),
        })
    }

    async fn add_pull_request_review(
        &self,
        ctx: &Context<'_>,
        input: AddPullRequestReviewInput,
    ) -> Result<AddPullRequestReviewPayload> {
        let g = gql_context(ctx)?;
        let (open, pull) = pull_for(g, &input.pull_request_id).await?;
        let state = match input.event.unwrap_or(PullRequestReviewEventEnum::Comment) {
            PullRequestReviewEventEnum::Approve => {
                crate::db::pull_requests::reviews::ReviewState::Approved
            }
            PullRequestReviewEventEnum::RequestChanges => {
                crate::db::pull_requests::reviews::ReviewState::RequestChanges
            }
            PullRequestReviewEventEnum::Comment => {
                crate::db::pull_requests::reviews::ReviewState::Comment
            }
        };
        // The head tip is read in a short block; the handle is gone before the await.
        let head = {
            let handle = handle_at(&g.project_root, &open.owner, &open.name)?;
            handle
                .branch_tip(&pull.head_branch)
                .ok()
                .flatten()
                .map(|oid| oid.to_string())
                .ok_or_else(|| gql_error("Head branch does not exist", "UNPROCESSABLE"))?
        };
        let commit = input.commit_oid.clone().unwrap_or_else(|| head.clone());
        if commit != head {
            return Err(gql_error(
                "commitOID must name the head of the pull request",
                "UNPROCESSABLE",
            ));
        }
        let review = pulls::submit_review(
            &g.db,
            &pull,
            &pull_actor(g),
            &open.permissions,
            state,
            input.body.as_deref().unwrap_or(""),
            &commit,
            &[],
        )
        .await
        .map_err(mutation_error)?;
        Ok(AddPullRequestReviewPayload {
            client_mutation_id: None,
            pull_request_review: ReviewNode {
                review,
                owner: open.owner.clone(),
                name: open.name.clone(),
                number: pull.number,
                origin: g.origin.clone(),
            },
        })
    }

    async fn add_reaction(
        &self,
        ctx: &Context<'_>,
        input: AddReactionInput,
    ) -> Result<AddReactionPayload> {
        mutations::react(gql_context(ctx)?, &input.subject_id, input.content, true).await?;
        Ok(AddReactionPayload {
            client_mutation_id: input.client_mutation_id,
            reaction: ReactionNode {
                content: input.content,
            },
        })
    }

    async fn remove_reaction(
        &self,
        ctx: &Context<'_>,
        input: RemoveReactionInput,
    ) -> Result<RemoveReactionPayload> {
        mutations::react(gql_context(ctx)?, &input.subject_id, input.content, false).await?;
        Ok(RemoveReactionPayload {
            client_mutation_id: input.client_mutation_id,
            reaction: ReactionNode {
                content: input.content,
            },
        })
    }

    async fn update_pull_request(
        &self,
        ctx: &Context<'_>,
        input: UpdatePullRequestInput,
    ) -> Result<UpdatePullRequestPayload> {
        let pull_request = mutations::update_pull_request(gql_context(ctx)?, &input).await?;
        Ok(UpdatePullRequestPayload {
            client_mutation_id: input.client_mutation_id,
            pull_request,
        })
    }

    async fn mark_pull_request_ready_for_review(
        &self,
        ctx: &Context<'_>,
        input: ReadyForReviewInput,
    ) -> Result<ReadyForReviewPayload> {
        let pull_request =
            mutations::set_draft(gql_context(ctx)?, &input.pull_request_id, false).await?;
        Ok(ReadyForReviewPayload {
            client_mutation_id: input.client_mutation_id,
            pull_request,
        })
    }

    async fn convert_pull_request_to_draft(
        &self,
        ctx: &Context<'_>,
        input: ConvertToDraftInput,
    ) -> Result<ConvertToDraftPayload> {
        let pull_request =
            mutations::set_draft(gql_context(ctx)?, &input.pull_request_id, true).await?;
        Ok(ConvertToDraftPayload {
            client_mutation_id: input.client_mutation_id,
            pull_request,
        })
    }

    async fn update_pull_request_review_comment(
        &self,
        ctx: &Context<'_>,
        input: UpdateReviewCommentInput,
    ) -> Result<UpdateReviewCommentPayload> {
        let comment = mutations::update_review_comment(
            gql_context(ctx)?,
            &input.pull_request_review_comment_id,
            &input.body,
        )
        .await?;
        Ok(UpdateReviewCommentPayload {
            client_mutation_id: input.client_mutation_id,
            pull_request_review_comment: comment,
        })
    }

    async fn update_issue_comment(
        &self,
        ctx: &Context<'_>,
        input: UpdateIssueCommentInput,
    ) -> Result<UpdateIssueCommentPayload> {
        let comment =
            mutations::update_issue_comment(gql_context(ctx)?, &input.id, &input.body).await?;
        Ok(UpdateIssueCommentPayload {
            client_mutation_id: input.client_mutation_id,
            issue_comment: comment,
        })
    }

    async fn enable_pull_request_auto_merge(
        &self,
        input: EnableAutoMergeInput,
    ) -> Result<AutoMergePayload> {
        let _ = input;
        Err(unsupported("Auto-merge"))
    }

    async fn disable_pull_request_auto_merge(
        &self,
        input: DisableAutoMergeInput,
    ) -> Result<AutoMergePayload> {
        let _ = input;
        Err(unsupported("Auto-merge"))
    }

    async fn update_pull_request_branch(
        &self,
        input: UpdateBranchInput,
    ) -> Result<UpdateBranchPayload> {
        let _ = input;
        Err(unsupported("Updating a pull request's branch"))
    }

    async fn revert_pull_request(
        &self,
        input: RevertPullRequestInput,
    ) -> Result<RevertPullRequestPayload> {
        let _ = input;
        Err(unsupported("Reverting a pull request"))
    }

    async fn update_refs(&self, input: UpdateRefsInput) -> Result<UpdateRefsPayload> {
        let _ = input;
        Err(unsupported("Changing branches through the API"))
    }

    async fn add_pull_request_review_thread_reply(
        &self,
        input: ThreadReplyInput,
    ) -> Result<ThreadReplyPayload> {
        let _ = input;
        Err(unsupported("Replying to review threads"))
    }

    async fn resolve_review_thread(
        &self,
        input: ResolveThreadInput,
    ) -> Result<ResolveThreadPayload> {
        let _ = input;
        Err(unsupported("Resolving review threads"))
    }

    async fn unresolve_review_thread(
        &self,
        input: UnresolveThreadInput,
    ) -> Result<ResolveThreadPayload> {
        let _ = input;
        Err(unsupported("Resolving review threads"))
    }

    async fn mark_file_as_viewed(&self, input: MarkFileViewedInput) -> Result<FileViewedPayload> {
        let _ = input;
        Err(unsupported("Marking files as viewed"))
    }

    async fn unmark_file_as_viewed(
        &self,
        input: UnmarkFileViewedInput,
    ) -> Result<FileViewedPayload> {
        let _ = input;
        Err(unsupported("Marking files as viewed"))
    }

    async fn close_pull_request(
        &self,
        ctx: &Context<'_>,
        input: PullRequestIdInput,
    ) -> Result<PullRequestPayload> {
        set_pull_state(ctx, &input.pull_request_id, PullRequestState::Closed).await
    }

    async fn reopen_pull_request(
        &self,
        ctx: &Context<'_>,
        input: PullRequestIdInput,
    ) -> Result<PullRequestPayload> {
        set_pull_state(ctx, &input.pull_request_id, PullRequestState::Open).await
    }

    async fn merge_pull_request(
        &self,
        ctx: &Context<'_>,
        input: MergePullRequestInput,
    ) -> Result<PullRequestPayload> {
        let g = gql_context(ctx)?;
        let (open, pull) = pull_for(g, &input.pull_request_id).await?;
        let strategy = match input
            .merge_method
            .unwrap_or(PullRequestMergeMethodEnum::Merge)
        {
            PullRequestMergeMethodEnum::Merge => MergeStrategy::Merge,
            PullRequestMergeMethodEnum::Squash => MergeStrategy::Squash,
            PullRequestMergeMethodEnum::Rebase => MergeStrategy::Rebase,
        };
        let email = g
            .principal
            .user
            .email
            .clone()
            .unwrap_or_else(|| format!("{}@localhost", g.principal.user.username));
        let merger = Merger {
            name: &g.principal.user.username,
            email: &email,
        };
        // The merge is built and the base moved in one synchronous block; the handle
        // is dropped before the database record is written.
        let tip = {
            let handle = handle_at(&g.project_root, &open.owner, &open.name)?;
            if let Some(expected) = &input.expected_head_oid {
                let head = handle
                    .branch_tip(&pull.head_branch)
                    .ok()
                    .flatten()
                    .map(|oid| oid.to_string());
                if head.as_deref() != Some(expected.as_str()) {
                    return Err(gql_error(
                        "Head branch was modified. Review and try the merge again.",
                        "UNPROCESSABLE",
                    ));
                }
            }
            pulls::merge_git(
                &handle,
                &pull,
                &open.permissions,
                strategy,
                input.commit_headline.as_deref(),
                input.commit_body.as_deref(),
                &merger,
            )
            .map_err(mutation_error)?
        };
        pulls::record_merge(&g.db, &pull, &pull_actor(g), &tip)
            .await
            .map_err(mutation_error)?;
        let (open, pull) = pull_for(g, &input.pull_request_id).await?;
        Ok(PullRequestPayload {
            client_mutation_id: None,
            pull_request: PullRequestNode::new(g, &open, pull),
        })
    }
}

async fn set_issue_state(ctx: &Context<'_>, id: &str, state: IssueState) -> Result<IssuePayload> {
    let g = gql_context(ctx)?;
    let (open, issue) = issue_for(g, id).await?;
    let scope = IssueScope {
        repo_id: &open.repo_id,
        namespace: &open.owner,
    };
    let updated = issues::change(
        &g.db,
        &scope,
        &issue,
        &issue_actor(g),
        &open.permissions,
        IssueChange {
            state: Some(state),
            ..Default::default()
        },
    )
    .await
    .map_err(mutation_error)?;
    Ok(IssuePayload {
        client_mutation_id: None,
        issue: IssueNode::new(&open, updated, g),
    })
}

async fn set_pull_state(
    ctx: &Context<'_>,
    id: &str,
    state: PullRequestState,
) -> Result<PullRequestPayload> {
    let g = gql_context(ctx)?;
    let (open, pull) = pull_for(g, id).await?;
    let updated = pulls::edit(
        &g.db,
        &pull,
        &pull_actor(g),
        &open.permissions,
        PullRequestEdit {
            title: None,
            body: None,
            state: Some(state),
        },
    )
    .await
    .map_err(mutation_error)?;
    Ok(PullRequestPayload {
        client_mutation_id: None,
        pull_request: PullRequestNode::new(g, &open, updated),
    })
}

/// Builds the schema. Resolvers read their context from the request, so one schema serves
/// every request.
pub fn schema() -> TwigSchema {
    // Interfaces are reached only through fragments on them, so they are registered here.
    Schema::build(Query, Mutation, EmptySubscription)
        .register_output_type::<Reactable>()
        .finish()
}

/// `POST /api/graphql`. Requires a token, as the REST API does. GraphQL errors are
/// reported in the body with a `200`, as GitHub does.
#[post("/api/graphql")]
pub async fn graphql(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    schema: web::Data<TwigSchema>,
    body: web::Json<async_graphql::Request>,
) -> HttpResponse {
    let principal = match v1::authenticate(&req, &auth_state).await {
        Ok(principal) => principal,
        Err(_) => {
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "message": "Requires authentication",
                "documentation_url": "https://docs.github.com/graphql",
            }));
        }
    };
    let context = GqlContext {
        db: auth_state.db().clone(),
        is_admin: server.is_configured_admin(&principal.user.username),
        principal,
        project_root: server.project_root().to_string(),
        origin: origin(&req),
    };
    let response = schema.execute(body.into_inner().data(context)).await;
    HttpResponse::Ok().json(response)
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.app_data(web::Data::new(schema())).service(graphql);
}
