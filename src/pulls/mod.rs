//! Pull request rules shared by the compatibility API and the web UI. Handlers
//! authenticate and parse input; these functions decide what is allowed.

use chrono::Utc;

use crate::{
    auth::{NamespacePermission, PermissionSet},
    db::{
        Database,
        pull_requests::{
            NewPullRequest, PullRequest, PullRequestEdit, PullRequestFilter, PullRequestState,
            comments::{NewReviewComment, PullRequestComment},
            reactions::{ReactionContent, comment_key, pull_key},
            reviews::{PullRequestReview, ReviewState},
        },
    },
    git::bare::MergeStrategy,
    git::bare::{MergeBuild, RepoHandle},
};

/// The repository a request belongs to.
pub struct RepoRef<'a> {
    pub repo_id: &'a str,
}

/// The signed-in user acting on a request.
pub struct Actor<'a> {
    pub user_id: &'a str,
}

#[derive(Debug)]
pub enum PullError {
    NotFound,
    Forbidden,
    Invalid(String),
    Conflict(String),
    Internal(String),
}

pub struct CreateInput<'a> {
    pub base: &'a str,
    pub head: &'a str,
    pub title: &'a str,
    pub body: &'a str,
}

const EQUAL_BRANCHES: &str = "These branches are equal. There is no need to create a pull request.";

fn require(perms: &PermissionSet, permission: NamespacePermission) -> Result<(), PullError> {
    if perms.contains(permission) {
        Ok(())
    } else {
        Err(PullError::Forbidden)
    }
}

/// Opens a request from `head` into `base`. Requires `PushChanges`, because a
/// request asks to move a branch the caller can already write.
pub async fn create(
    db: &Database,
    handle: &RepoHandle,
    repo: &RepoRef<'_>,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    input: CreateInput<'_>,
) -> Result<PullRequest, PullError> {
    require(perms, NamespacePermission::PushChanges)?;
    let title = input.title.trim();
    if title.is_empty() {
        return Err(PullError::Invalid("Title is required".to_string()));
    }
    if input.base == input.head {
        return Err(PullError::Invalid(EQUAL_BRANCHES.to_string()));
    }

    let base_tip = handle
        .branch_tip(input.base)
        .map_err(|_| PullError::Invalid("Invalid base branch name".to_string()))?
        .ok_or_else(|| {
            PullError::Invalid(format!("Base branch '{}' does not exist", input.base))
        })?;
    let head_tip = handle
        .branch_tip(input.head)
        .map_err(|_| PullError::Invalid("Invalid head branch name".to_string()))?
        .ok_or_else(|| {
            PullError::Invalid(format!("Head branch '{}' does not exist", input.head))
        })?;
    let ahead = handle
        .commit_ids_in_range(base_tip, head_tip)
        .map_err(|error| PullError::Internal(format!("Failed to compare branches: {error}")))?;
    if ahead.is_empty() {
        return Err(PullError::Invalid(EQUAL_BRANCHES.to_string()));
    }

    if db
        .find_open_pull_request(repo.repo_id, input.head, input.base)
        .await
        .map_err(PullError::Internal)?
        .is_some()
    {
        return Err(PullError::Conflict(
            "A pull request for these branches already exists".to_string(),
        ));
    }

    let pull_request = db
        .create_pull_request(NewPullRequest {
            repo_id: repo.repo_id,
            title,
            body: input.body,
            author_user_id: actor.user_id,
            head_branch: input.head,
            base_branch: input.base,
        })
        .await
        .map_err(PullError::Internal)?;
    if let Err(error) = handle.set_pull_ref(pull_request.number, head_tip) {
        // The next fetch repairs the ref from the stored request, so this is not fatal.
        log::warn!(
            "Failed to set head ref for pull request #{}: {error}",
            pull_request.number
        );
    }
    Ok(pull_request)
}

/// Makes each open request's `refs/pull/<number>/head` name the current tip of its
/// head branch. Clients fetch these refs by number, so they must follow pushes.
pub async fn sync_refs(db: &Database, handle: &RepoHandle, repo_id: &str) -> Result<(), String> {
    for (number, head) in db.open_pull_request_heads(repo_id).await? {
        let Some(tip) = handle
            .branch_tip(&head)
            .map_err(|error| error.to_string())?
        else {
            continue;
        };
        if handle.pull_ref(number).map_err(|error| error.to_string())? != Some(tip) {
            handle
                .set_pull_ref(number, tip)
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

/// Runs before a fetch advertises refs. A failure is logged and never blocks the
/// fetch, because the branches themselves do not depend on request refs.
pub async fn refresh_for_fetch(db: &Database, project_root: &str, namespace: &str, repo: &str) {
    let Ok(handle) = RepoHandle::open(project_root, namespace, repo) else {
        return;
    };
    let repo_id = match handle.repo_id() {
        Ok(id) => id,
        Err(error) => {
            log::warn!("Failed to read repository identity for {namespace}/{repo}: {error}");
            return;
        }
    };
    if let Err(error) = sync_refs(db, &handle, &repo_id).await {
        log::warn!("Failed to refresh pull request refs for {namespace}/{repo}: {error}");
    }
}

/// One page of requests, newest first, for the given filter.
pub async fn list(
    db: &Database,
    repo: &RepoRef<'_>,
    filter: &PullRequestFilter,
) -> Result<(Vec<PullRequest>, bool), PullError> {
    db.list_pull_requests(repo.repo_id, filter)
        .await
        .map_err(PullError::Internal)
}

pub async fn get(db: &Database, repo: &RepoRef<'_>, number: i64) -> Result<PullRequest, PullError> {
    db.get_pull_request(repo.repo_id, number)
        .await
        .map_err(PullError::Internal)?
        .ok_or(PullError::NotFound)
}

/// Adds a conversation comment. Anyone who can view the namespace may comment,
/// as on Forgejo.
pub async fn add_comment(
    db: &Database,
    pull_request: &PullRequest,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    body: &str,
) -> Result<PullRequestComment, PullError> {
    require(perms, NamespacePermission::ViewNamespace)?;
    let body = non_empty(body, "Comment body is required")?;
    db.add_pull_request_comment(&pull_request.id, actor.user_id, body)
        .await
        .map_err(PullError::Internal)
}

/// Replaces a comment's text. Only the author may edit a comment.
pub async fn edit_comment(
    db: &Database,
    pull_request: &PullRequest,
    comment_id: i64,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    body: &str,
) -> Result<PullRequestComment, PullError> {
    require(perms, NamespacePermission::ViewNamespace)?;
    let comment = find_comment(db, pull_request, comment_id).await?;
    if comment.author_user_id != actor.user_id {
        return Err(PullError::Forbidden);
    }
    let body = non_empty(body, "Comment body is required")?;
    db.update_pull_request_comment(comment_id, body)
        .await
        .map_err(PullError::Internal)?;
    find_comment(db, pull_request, comment_id).await
}

/// Removes a comment. The author may remove their own; so may anyone who can push.
pub async fn delete_comment(
    db: &Database,
    pull_request: &PullRequest,
    comment_id: i64,
    actor: &Actor<'_>,
    perms: &PermissionSet,
) -> Result<(), PullError> {
    let comment = find_comment(db, pull_request, comment_id).await?;
    if comment.author_user_id != actor.user_id {
        require(perms, NamespacePermission::PushChanges)?;
    }
    db.delete_pull_request_comment(comment_id)
        .await
        .map_err(PullError::Internal)
}

/// Changes a request's title, description, or state. The author and anyone who
/// can push may do so.
pub async fn edit(
    db: &Database,
    pull_request: &PullRequest,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    change: PullRequestEdit,
) -> Result<PullRequest, PullError> {
    if pull_request.author_user_id != actor.user_id {
        require(perms, NamespacePermission::PushChanges)?;
    }
    if let Some(title) = &change.title {
        non_empty(title, "Title is required")?;
    }
    if change.state == Some(PullRequestState::Open) && pull_request.merged_at.is_some() {
        return Err(PullError::Conflict(
            "A merged pull request cannot be reopened.".to_string(),
        ));
    }
    let change = PullRequestEdit {
        title: change.title.map(|title| title.trim().to_string()),
        ..change
    };
    let now = Utc::now().to_rfc3339();
    db.edit_pull_request(&pull_request.id, &change, &now)
        .await
        .map_err(PullError::Internal)?;
    db.get_pull_request(&pull_request.repo_id, pull_request.number)
        .await
        .map_err(PullError::Internal)?
        .ok_or(PullError::NotFound)
}

/// Adds or removes a reaction on the request (`comment_id` of `None`) or on one of
/// its comments. Reactions follow the same rule as viewing the namespace.
pub async fn react(
    db: &Database,
    pull_request: &PullRequest,
    comment_id: Option<i64>,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    content: ReactionContent,
    on: bool,
) -> Result<(), PullError> {
    require(perms, NamespacePermission::ViewNamespace)?;
    let key = match comment_id {
        Some(id) => {
            find_comment(db, pull_request, id).await?;
            comment_key(id)
        }
        None => pull_key(&pull_request.id),
    };
    db.set_reaction(&key, actor.user_id, content, on)
        .await
        .map_err(PullError::Internal)
}

fn non_empty<'a>(value: &'a str, message: &str) -> Result<&'a str, PullError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(PullError::Invalid(message.to_string()))
    } else {
        Ok(trimmed)
    }
}

/// A comment that exists and belongs to `pull_request`. Other requests' comments
/// are reported as not found, so IDs cannot be used across requests.
async fn find_comment(
    db: &Database,
    pull_request: &PullRequest,
    comment_id: i64,
) -> Result<PullRequestComment, PullError> {
    db.get_pull_request_comment(comment_id)
        .await
        .map_err(PullError::Internal)?
        .filter(|comment| comment.pull_request_id == pull_request.id)
        .ok_or(PullError::NotFound)
}

/// One inline comment on a review, before it is stored.
pub struct InlineInput<'a> {
    pub path: &'a str,
    pub body: &'a str,
    pub old_position: i64,
    pub new_position: i64,
}

/// Records a review. Approving and requesting changes need push rights and are
/// not allowed on one's own request; a comment review needs something to say.
pub async fn submit_review(
    db: &Database,
    pull_request: &PullRequest,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    state: ReviewState,
    body: &str,
    commit_oid: &str,
    inline: &[InlineInput<'_>],
) -> Result<PullRequestReview, PullError> {
    require(perms, NamespacePermission::PushChanges)?;
    let own = pull_request.author_user_id == actor.user_id;
    if own && state != ReviewState::Comment {
        return Err(PullError::Invalid(
            "You cannot approve or request changes on your own pull request.".to_string(),
        ));
    }
    let body = body.trim();
    if state == ReviewState::Comment && body.is_empty() && inline.is_empty() {
        return Err(PullError::Invalid(
            "A comment review needs a body or inline comments.".to_string(),
        ));
    }
    for comment in inline {
        if comment.path.trim().is_empty() || comment.body.trim().is_empty() {
            return Err(PullError::Invalid(
                "Inline comments need a path and a body.".to_string(),
            ));
        }
        if comment.old_position <= 0 && comment.new_position <= 0 {
            return Err(PullError::Invalid(
                "Inline comments need a line position.".to_string(),
            ));
        }
    }
    let rows: Vec<NewReviewComment<'_>> = inline
        .iter()
        .map(|comment| NewReviewComment {
            path: comment.path,
            body: comment.body.trim(),
            old_position: comment.old_position,
            new_position: comment.new_position,
            commit_oid,
        })
        .collect();
    db.submit_pull_request_review(
        &pull_request.id,
        actor.user_id,
        state,
        body,
        commit_oid,
        &rows,
    )
    .await
    .map_err(PullError::Internal)
}

/// Asks a namespace member to review, or withdraws the request. The author may
/// ask for reviews on their own request; anyone else needs push rights.
pub async fn set_requested_reviewer(
    db: &Database,
    namespace: &str,
    pull_request: &PullRequest,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    reviewer_username: &str,
    requested: bool,
) -> Result<(), PullError> {
    if pull_request.author_user_id != actor.user_id {
        require(perms, NamespacePermission::PushChanges)?;
    }
    let members = db
        .list_namespace_members(namespace)
        .await
        .map_err(PullError::Internal)?;
    let reviewer = members
        .iter()
        .find(|member| member.username == reviewer_username)
        .ok_or_else(|| {
            PullError::Invalid(format!(
                "'{reviewer_username}' is not a member of this namespace"
            ))
        })?;
    db.set_requested_reviewer(&pull_request.id, &reviewer.id, requested)
        .await
        .map_err(PullError::Internal)
}

/// Who signs the merge commit: the signed-in user as they are known to Twig.
pub struct Merger<'a> {
    pub name: &'a str,
    pub email: &'a str,
}

/// Merges a request into its base. Requires push rights. The base branch moves only
/// if it still names the commit the merge was built on, so a concurrent push is never
/// overwritten.
pub async fn merge(
    db: &Database,
    handle: &RepoHandle,
    pull_request: &PullRequest,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    strategy: MergeStrategy,
    title: Option<&str>,
    message: Option<&str>,
    merger: &Merger<'_>,
) -> Result<String, PullError> {
    require(perms, NamespacePermission::PushChanges)?;
    if pull_request.merged_at.is_some() {
        return Err(PullError::Conflict(
            "This pull request is already merged.".to_string(),
        ));
    }
    if pull_request.state != PullRequestState::Open {
        return Err(PullError::Conflict(
            "This pull request is closed. Reopen it before merging.".to_string(),
        ));
    }
    let internal = |error: git2::Error| PullError::Internal(error.to_string());
    let base = handle
        .branch_tip(&pull_request.base_branch)
        .map_err(internal)?
        .ok_or_else(|| {
            PullError::Invalid(format!(
                "Base branch '{}' does not exist",
                pull_request.base_branch
            ))
        })?;
    let head = handle
        .branch_tip(&pull_request.head_branch)
        .map_err(internal)?
        .ok_or_else(|| {
            PullError::Invalid(format!(
                "Head branch '{}' does not exist",
                pull_request.head_branch
            ))
        })?;
    if handle.merge_base(base, head).map_err(internal)? == Some(head) {
        return Err(PullError::Conflict(
            "Nothing to merge: the head is already in the base branch.".to_string(),
        ));
    }

    // A title the client chose is used as it is. Otherwise the request's own title is
    // used, with its number, as Forgejo does.
    let chosen = title.map(str::trim).filter(|title| !title.is_empty());
    let body = message.unwrap_or(&pull_request.body).trim();
    let headline = match (chosen, strategy) {
        (Some(chosen), _) => chosen.to_string(),
        (None, MergeStrategy::Squash) => {
            format!("{} (#{})", pull_request.title, pull_request.number)
        }
        (None, _) => format!(
            "Merge pull request '{}' (#{}) from {} into {}",
            pull_request.title,
            pull_request.number,
            pull_request.head_branch,
            pull_request.base_branch
        ),
    };
    let full_message = if body.is_empty() {
        headline
    } else {
        format!("{headline}\n\n{body}")
    };
    let signature = git2::Signature::now(merger.name, merger.email)
        .map_err(|error| PullError::Internal(format!("Invalid merger identity: {error}")))?;
    let tip = match handle
        .build_merge(strategy, base, head, &signature, &full_message)
        .map_err(internal)?
    {
        MergeBuild::Conflict { paths } => {
            return Err(PullError::Conflict(format!(
                "Merge conflicts in: {}",
                paths.join(", ")
            )));
        }
        MergeBuild::Ready { tip } => tip,
    };
    if !handle
        .move_branch_if_unchanged(
            &pull_request.base_branch,
            base,
            tip,
            "Merge pull request in Twig",
        )
        .map_err(internal)?
    {
        return Err(PullError::Conflict(
            "The base branch moved while merging. Try again.".to_string(),
        ));
    }
    let now = Utc::now().to_rfc3339();
    db.mark_pull_request_merged(&pull_request.id, actor.user_id, &tip.to_string(), &now)
        .await
        .map_err(PullError::Internal)?;
    Ok(tip.to_string())
}

/// Whether a request is open and its head merges cleanly into its base right now.
/// `None` for requests that are closed or merged, where the question no longer applies.
pub fn mergeable(handle: &RepoHandle, pull_request: &PullRequest) -> Option<bool> {
    if pull_request.state != PullRequestState::Open {
        return None;
    }
    let base = handle
        .branch_tip(&pull_request.base_branch)
        .ok()
        .flatten()?;
    let head = handle
        .branch_tip(&pull_request.head_branch)
        .ok()
        .flatten()?;
    handle
        .merge_conflicts(base, head)
        .ok()
        .map(|paths| paths.is_empty())
}

/// Merges the base branch into a request's head branch, so the request is up to date.
/// Pushers only, as on Forgejo. The head branch moves by compare-and-swap.
pub fn update_branch(
    handle: &RepoHandle,
    pull_request: &PullRequest,
    perms: &PermissionSet,
    merger: &Merger<'_>,
) -> Result<String, PullError> {
    require(perms, NamespacePermission::PushChanges)?;
    if pull_request.state != PullRequestState::Open {
        return Err(PullError::Conflict(
            "Only open pull requests can be updated.".to_string(),
        ));
    }
    let internal = |error: git2::Error| PullError::Internal(error.to_string());
    let base = handle
        .branch_tip(&pull_request.base_branch)
        .map_err(internal)?
        .ok_or_else(|| {
            PullError::Invalid(format!(
                "Base branch '{}' does not exist",
                pull_request.base_branch
            ))
        })?;
    let head = handle
        .branch_tip(&pull_request.head_branch)
        .map_err(internal)?
        .ok_or_else(|| {
            PullError::Invalid(format!(
                "Head branch '{}' does not exist",
                pull_request.head_branch
            ))
        })?;
    if handle.merge_base(base, head).map_err(internal)? == Some(base) {
        return Err(PullError::Conflict(
            "This branch is already up to date with the base branch.".to_string(),
        ));
    }
    let signature = git2::Signature::now(merger.name, merger.email)
        .map_err(|error| PullError::Internal(format!("Invalid merger identity: {error}")))?;
    let message = format!(
        "Merge branch '{}' into {}",
        pull_request.base_branch, pull_request.head_branch
    );
    let tip = match handle
        .build_merge(MergeStrategy::Merge, head, base, &signature, &message)
        .map_err(internal)?
    {
        MergeBuild::Conflict { paths } => {
            return Err(PullError::Conflict(format!(
                "Merge conflicts in: {}",
                paths.join(", ")
            )));
        }
        MergeBuild::Ready { tip } => tip,
    };
    if !handle
        .move_branch_if_unchanged(
            &pull_request.head_branch,
            head,
            tip,
            "Update pull request branch in Twig",
        )
        .map_err(internal)?
    {
        return Err(PullError::Conflict(
            "The head branch moved while updating. Try again.".to_string(),
        ));
    }
    Ok(tip.to_string())
}
