//! Forgejo-shaped response structs. Field names and types follow Forgejo's API
//! so clients that decode them strictly (fj, tea, T3 Code) accept the output.

use serde::Serialize;

use crate::auth::User;

/// Derives the integer identifier Forgejo clients expect from a stable Twig key.
/// The first 8 bytes of SHA-256 are masked to 53 bits so JavaScript clients can
/// represent the value exactly. It is deterministic, so it never changes for a key.
pub fn numeric_id(key: &str) -> i64 {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(key.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    i64::try_from(u64::from_be_bytes(bytes) & ((1u64 << 53) - 1)).unwrap_or_default()
}

#[derive(Debug, Clone, Serialize)]
pub struct UserJson {
    pub id: i64,
    pub login: String,
    pub login_name: String,
    pub username: String,
    pub full_name: String,
    pub email: String,
    pub avatar_url: String,
    pub active: bool,
    pub is_admin: bool,
    pub prohibit_login: bool,
    pub restricted: bool,
}

impl UserJson {
    /// A user as seen by someone else: no email address is disclosed.
    pub fn public(user: &User) -> Self {
        Self {
            id: numeric_id(&user.id),
            login: user.username.clone(),
            login_name: String::new(),
            username: user.username.clone(),
            full_name: String::new(),
            email: String::new(),
            avatar_url: String::new(),
            active: true,
            is_admin: false,
            prohibit_login: false,
            restricted: false,
        }
    }

    /// The caller's own account, including the email address on file.
    pub fn own(user: &User) -> Self {
        Self {
            email: user.email.clone().unwrap_or_default(),
            ..Self::public(user)
        }
    }
}

/// One side of a pull request: the branch and the commit it names.
#[derive(Debug, Clone, Serialize)]
pub struct BranchRefJson {
    pub label: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub sha: String,
    pub repo_id: i64,
    pub repo: Option<RepositoryJson>,
}

/// What a repository grants the caller, in Forgejo's shape.
#[derive(Debug, Clone, Serialize)]
pub struct PermissionsJson {
    pub admin: bool,
    pub push: bool,
    pub pull: bool,
}

/// A repository in Forgejo's shape. Twig has no SSH transport, so `ssh_url` is empty.
#[derive(Debug, Clone, Serialize)]
pub struct RepositoryJson {
    pub id: i64,
    pub owner: UserJson,
    pub name: String,
    pub full_name: String,
    pub description: String,
    pub private: bool,
    pub fork: bool,
    pub empty: bool,
    pub archived: bool,
    pub html_url: String,
    pub ssh_url: String,
    pub clone_url: String,
    pub default_branch: Option<String>,
    pub permissions: PermissionsJson,
    pub allow_merge_commits: bool,
    pub allow_squash_merge: bool,
    pub allow_rebase: bool,
    pub allow_rebase_update: bool,
    pub allow_fast_forward_only_merge: bool,
    pub has_pull_requests: bool,
    pub has_issues: bool,
}

/// A namespace as the owner of a repository. Forgejo calls it a user or organization.
pub fn namespace_owner(namespace: &str) -> UserJson {
    UserJson {
        id: numeric_id(&format!("namespace:{namespace}")),
        login: namespace.to_string(),
        login_name: String::new(),
        username: namespace.to_string(),
        full_name: String::new(),
        email: String::new(),
        avatar_url: String::new(),
        active: true,
        is_admin: false,
        prohibit_login: false,
        restricted: false,
    }
}

/// Where a repository lives and what it is called, for building its URLs.
pub struct RepositoryLocation<'a> {
    pub origin: &'a str,
    pub namespace: &'a str,
    pub name: &'a str,
    pub repo_id: &'a str,
}

impl RepositoryJson {
    /// The repository as its clients see it. `permissions` is the caller's view.
    pub fn new(
        location: &RepositoryLocation<'_>,
        private: bool,
        default_branch: Option<String>,
        empty: bool,
        permissions: PermissionsJson,
    ) -> Self {
        let full_name = format!("{}/{}", location.namespace, location.name);
        Self {
            id: numeric_id(location.repo_id),
            owner: namespace_owner(location.namespace),
            name: location.name.to_string(),
            html_url: format!("{}/{full_name}", location.origin),
            clone_url: format!("{}/{full_name}.git", location.origin),
            full_name,
            description: String::new(),
            private,
            fork: false,
            empty,
            archived: false,
            ssh_url: String::new(),
            default_branch,
            permissions,
            allow_merge_commits: true,
            allow_squash_merge: true,
            allow_rebase: true,
            allow_rebase_update: false,
            allow_fast_forward_only_merge: false,
            has_pull_requests: true,
            has_issues: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PullRequestJson {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub html_url: String,
    pub url: String,
    pub diff_url: String,
    pub patch_url: String,
    pub user: Option<UserJson>,
    pub state: &'static str,
    pub is_locked: bool,
    pub draft: bool,
    pub merged: bool,
    pub mergeable: Option<bool>,
    pub head: BranchRefJson,
    pub base: BranchRefJson,
    pub merge_base: Option<String>,
    pub merge_commit_sha: Option<String>,
    pub merged_by: Option<UserJson>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub merged_at: Option<String>,
    pub additions: Option<i64>,
    pub deletions: Option<i64>,
    pub changed_files: Option<i64>,
    pub comments: i64,
    pub labels: Vec<serde_json::Value>,
    pub requested_reviewers: Vec<UserJson>,
    pub assignees: Vec<UserJson>,
    pub allow_maintainer_edit: bool,
}

/// Forgejo titles a request as work in progress with a `WIP:` or `[WIP]` prefix
/// when it has no draft flag; clients read `draft` from this.
pub fn is_work_in_progress(title: &str) -> bool {
    let lower = title.to_ascii_lowercase();
    lower.starts_with("wip:") || lower.starts_with("[wip]")
}
