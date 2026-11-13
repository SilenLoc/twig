//! Issue rules shared by the compatibility API and the web UI. Handlers authenticate
//! and parse input; these functions decide what is allowed.

use chrono::Utc;

use crate::{
    auth::{NamespacePermission, PermissionSet},
    db::{
        Database,
        issues::{Issue, IssueEdit, IssueFilter, IssueState, NewIssue, comments::IssueComment},
    },
};

/// The repository an issue belongs to.
pub struct RepoRef<'a> {
    pub repo_id: &'a str,
    pub namespace: &'a str,
}

/// The signed-in user acting on an issue.
pub struct Actor<'a> {
    pub user_id: &'a str,
}

#[derive(Debug)]
pub enum IssueError {
    NotFound,
    Forbidden,
    Invalid(String),
    Internal(String),
}

pub struct CreateInput<'a> {
    pub title: &'a str,
    pub body: &'a str,
    /// Usernames of namespace members to assign. Only people who can push may assign.
    pub assignees: &'a [String],
}

/// One page of issues and the total that matches the filter.
pub struct IssuePage {
    pub items: Vec<Issue>,
    pub has_more: bool,
    pub total: u64,
}

fn require(perms: &PermissionSet, permission: NamespacePermission) -> Result<(), IssueError> {
    if perms.contains(permission) {
        Ok(())
    } else {
        Err(IssueError::Forbidden)
    }
}

fn non_empty<'a>(value: &'a str, message: &str) -> Result<&'a str, IssueError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(IssueError::Invalid(message.to_string()))
    } else {
        Ok(trimmed)
    }
}

/// The IDs of the named namespace members, without repeats. A name that is not a member
/// is refused, so an assignment can only name people who can see the repository.
pub async fn resolve_assignees(
    db: &Database,
    scope: &RepoRef<'_>,
    usernames: &[String],
) -> Result<Vec<String>, IssueError> {
    if usernames.is_empty() {
        return Ok(Vec::new());
    }
    let members = db
        .list_namespace_members(scope.namespace)
        .await
        .map_err(IssueError::Internal)?;
    let mut ids: Vec<String> = Vec::new();
    for name in usernames {
        let name = name.trim();
        let member = members
            .iter()
            .find(|user| user.username == name)
            .ok_or_else(|| {
                IssueError::Invalid(format!("{name} is not a member of {}", scope.namespace))
            })?;
        if !ids.contains(&member.id) {
            ids.push(member.id.clone());
        }
    }
    Ok(ids)
}

/// Opens an issue. Anyone who can view the namespace may open one, as on Forgejo.
pub async fn create(
    db: &Database,
    scope: &RepoRef<'_>,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    input: CreateInput<'_>,
) -> Result<Issue, IssueError> {
    require(perms, NamespacePermission::ViewNamespace)?;
    let title = non_empty(input.title, "Title is required")?;
    if !input.assignees.is_empty() {
        require(perms, NamespacePermission::PushChanges)?;
    }
    let assignee_ids = resolve_assignees(db, scope, input.assignees).await?;
    db.create_issue(NewIssue {
        repo_id: scope.repo_id,
        title,
        body: input.body,
        author_user_id: actor.user_id,
        assignee_user_ids: &assignee_ids,
    })
    .await
    .map_err(IssueError::Internal)
}

/// The changes to an issue. Each present field changes; an absent one stays as it is.
#[derive(Debug, Default)]
pub struct IssueChange {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<IssueState>,
    /// Usernames of namespace members. The list replaces the current assignees.
    pub assignees: Option<Vec<String>>,
}

/// Changes an issue. The author and anyone who can push may change its title, body, or
/// state. Only people who can push may change assignees.
pub async fn change(
    db: &Database,
    scope: &RepoRef<'_>,
    issue: &Issue,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    change: IssueChange,
) -> Result<Issue, IssueError> {
    let is_author = issue.author_user_id == actor.user_id;
    let may_push = perms.contains(NamespacePermission::PushChanges);
    let edits_text_or_state =
        change.title.is_some() || change.body.is_some() || change.state.is_some();
    if edits_text_or_state && !is_author && !may_push {
        return Err(IssueError::Forbidden);
    }
    if change.assignees.is_some() && !may_push {
        return Err(IssueError::Forbidden);
    }
    let title = match &change.title {
        Some(title) => Some(non_empty(title, "Title is required")?.to_string()),
        None => None,
    };
    let assignee_ids = match &change.assignees {
        Some(names) => Some(resolve_assignees(db, scope, names).await?),
        None => None,
    };
    let now = Utc::now().to_rfc3339();
    db.edit_issue(
        &issue.id,
        &IssueEdit {
            title,
            body: change.body,
            state: change.state,
        },
        &now,
    )
    .await
    .map_err(IssueError::Internal)?;
    if let Some(ids) = assignee_ids {
        db.replace_issue_assignees(&issue.id, &ids, &now)
            .await
            .map_err(IssueError::Internal)?;
    }
    db.get_issue_by_id(&issue.id)
        .await
        .map_err(IssueError::Internal)?
        .ok_or(IssueError::NotFound)
}

/// Adds a conversation comment. Anyone who can view the namespace may comment.
pub async fn add_comment(
    db: &Database,
    issue: &Issue,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    body: &str,
) -> Result<IssueComment, IssueError> {
    require(perms, NamespacePermission::ViewNamespace)?;
    let body = non_empty(body, "Comment body is required")?;
    db.add_issue_comment(&issue.id, actor.user_id, body)
        .await
        .map_err(IssueError::Internal)
}

/// Replaces a comment's text. Only the comment's author may edit it.
pub async fn edit_comment(
    db: &Database,
    issue: &Issue,
    row_id: i64,
    actor: &Actor<'_>,
    perms: &PermissionSet,
    body: &str,
) -> Result<IssueComment, IssueError> {
    require(perms, NamespacePermission::ViewNamespace)?;
    let comment = find_comment(db, issue, row_id).await?;
    if comment.author_user_id != actor.user_id {
        return Err(IssueError::Forbidden);
    }
    let body = non_empty(body, "Comment body is required")?;
    let now = Utc::now().to_rfc3339();
    db.update_issue_comment(row_id, body, &now)
        .await
        .map_err(IssueError::Internal)?;
    find_comment(db, issue, row_id).await
}

/// Removes a comment. Only its author may remove it.
pub async fn delete_comment(
    db: &Database,
    issue: &Issue,
    row_id: i64,
    actor: &Actor<'_>,
    perms: &PermissionSet,
) -> Result<(), IssueError> {
    require(perms, NamespacePermission::ViewNamespace)?;
    let comment = find_comment(db, issue, row_id).await?;
    if comment.author_user_id != actor.user_id {
        return Err(IssueError::Forbidden);
    }
    db.delete_issue_comment(row_id)
        .await
        .map_err(IssueError::Internal)
}

/// A comment that exists and belongs to `issue`. A comment on another issue is
/// reported as not found, so IDs cannot be used across issues.
async fn find_comment(
    db: &Database,
    issue: &Issue,
    row_id: i64,
) -> Result<IssueComment, IssueError> {
    db.get_issue_comment(row_id)
        .await
        .map_err(IssueError::Internal)?
        .filter(|comment| comment.issue_id == issue.id)
        .ok_or(IssueError::NotFound)
}

pub async fn get(db: &Database, scope: &RepoRef<'_>, number: i64) -> Result<Issue, IssueError> {
    db.get_issue(scope.repo_id, number)
        .await
        .map_err(IssueError::Internal)?
        .ok_or(IssueError::NotFound)
}

pub async fn list(
    db: &Database,
    scope: &RepoRef<'_>,
    filter: &IssueFilter,
) -> Result<IssuePage, IssueError> {
    let (items, has_more) = db
        .list_issues(scope.repo_id, filter)
        .await
        .map_err(IssueError::Internal)?;
    let total = db
        .count_issues(scope.repo_id, filter)
        .await
        .map_err(IssueError::Internal)?;
    Ok(IssuePage {
        items,
        has_more,
        total,
    })
}

/// What a `created_by` or `assigned_by` filter names: anyone, one account, or a name that
/// matches no account, so nothing can match.
pub enum AccountFilter {
    Anyone,
    Only(String),
    NoOne,
}

/// Resolves a username filter. An empty or missing name filters nothing.
pub async fn account_filter(
    db: &Database,
    username: Option<&str>,
) -> Result<AccountFilter, IssueError> {
    let Some(name) = username.map(str::trim).filter(|name| !name.is_empty()) else {
        return Ok(AccountFilter::Anyone);
    };
    match db
        .get_user_by_username(name)
        .await
        .map_err(IssueError::Internal)?
    {
        Some(user) => Ok(AccountFilter::Only(user.id)),
        None => Ok(AccountFilter::NoOne),
    }
}
