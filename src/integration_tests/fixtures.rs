//! Fixtures shared by the GitHub-compatible API, Git, and UI integration tests: a
//! seeded owner with a bare repository, branches, contributors, and access tokens.

use super::*;
use crate::auth::{NamespacePermission, PermissionSet};

pub(super) fn token_header(token: &str) -> (&'static str, String) {
    ("Authorization", format!("token {token}"))
}

/// Seeds a user, a namespace they own, and a bare repository on disk whose `main`
/// holds the `.twig.toml` commit. Returns the user, namespace, and repository names.
pub(super) async fn seed_owner_repo(
    db: &crate::db::Database,
    root: &str,
) -> (crate::auth::User, String, String) {
    let user = auth::create_user(
        "pr-owner".to_string(),
        "pr-owner@example.com".to_string(),
        "password123",
    )
    .expect("create owner");
    db.create_user(&user).await.expect("persist owner");
    let namespace = auth::create_namespace("pr-team".to_string(), user.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("persist namespace");
    let repo_path = std::path::Path::new(root).join("pr-team").join("tooling");
    std::fs::create_dir_all(&repo_path).expect("repository dir");
    twig_git::repo::bare_init(&repo_path, "main", "Test", "test@example.com")
        .expect("init bare repo");
    (user, "pr-team".to_string(), "tooling".to_string())
}

/// Creates a branch one commit ahead of `main`, adding `file` with `content`.
pub(super) fn push_branch(root: &str, branch: &str, file: &str, content: &str) {
    let repo_path = std::path::Path::new(root).join("pr-team").join("tooling");
    let repo = git2::Repository::open_bare(&repo_path).expect("open bare repo");
    let main = repo
        .find_reference("refs/heads/main")
        .expect("main")
        .peel_to_commit()
        .expect("main commit");
    let mut builder = repo
        .treebuilder(Some(&main.tree().expect("main tree")))
        .expect("tree builder");
    let blob = repo.blob(content.as_bytes()).expect("blob");
    builder.insert(file, blob, 0o100_644).expect("insert file");
    let tree = repo
        .find_tree(builder.write().expect("write tree"))
        .expect("tree");
    let signature = git2::Signature::now("Twig Test", "twig@example.com").expect("signature");
    repo.commit(
        Some(&format!("refs/heads/{branch}")),
        &signature,
        &signature,
        &format!("Add {file}"),
        &tree,
        &[&main],
    )
    .expect("commit branch");
}

pub(super) async fn token_for(
    db: &crate::db::Database,
    user: &auth::User,
    namespace: &str,
    permissions: &[NamespacePermission],
) -> String {
    let auth_state = auth::TwigContext::new(db.clone(), "secure".to_string());
    let ns = db
        .get_namespace_by_name(namespace)
        .await
        .expect("load namespace")
        .expect("namespace exists");
    let (_, token) = auth_state
        .issue_personal_access_token(
            &user.id,
            "test",
            std::slice::from_ref(&ns.id),
            &PermissionSet::new(permissions.iter().copied()),
            None,
        )
        .await
        .expect("issue token");
    token
}

/// Creates a second user who is a contributor of `namespace`.
pub(super) async fn seed_contributor(
    db: &crate::db::Database,
    namespace: &str,
    username: &str,
) -> crate::auth::User {
    let user = auth::create_user(
        username.to_string(),
        format!("{username}@example.com"),
        "password123",
    )
    .expect("create contributor");
    db.create_user(&user).await.expect("persist contributor");
    let ns = db
        .get_namespace_by_name(namespace)
        .await
        .expect("load namespace")
        .expect("namespace exists");
    db.conn()
        .await
        .expect("connection")
        .execute(
            "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
             VALUES (?1, ?2, 'contributor', '2026-10-30T00:00:00Z')",
            crate::db::sqlite::params![ns.id, user.id],
        )
        .await
        .expect("add contributor membership");
    user
}

/// Adds `user_id` to `namespace` as a contributor, as an accepted invitation does.
pub(super) async fn add_contributor(db: &crate::db::Database, namespace: &str, user_id: &str) {
    let ns = db
        .get_namespace_by_name(namespace)
        .await
        .expect("load namespace")
        .expect("namespace exists");
    db.conn()
        .await
        .expect("connection")
        .execute(
            "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
             VALUES (?1, ?2, 'contributor', ?3)",
            crate::db::sqlite::params![
                ns.id.clone(),
                user_id.to_string(),
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .await
        .expect("add contributor membership");
}
