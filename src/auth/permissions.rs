use std::collections::BTreeSet;

use crate::{auth::Namespace, db::Database};

use super::NamespaceRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NamespacePermission {
    ViewNamespace,
    PushChanges,
    CreateRepositories,
    ManageRepositories,
    ManageNamespace,
    ManageMembers,
    RewriteProtectedBranches,
}

impl NamespacePermission {
    pub const ALL: [Self; 7] = [
        Self::ViewNamespace,
        Self::PushChanges,
        Self::CreateRepositories,
        Self::ManageRepositories,
        Self::ManageNamespace,
        Self::ManageMembers,
        Self::RewriteProtectedBranches,
    ];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::ViewNamespace => "View namespace",
            Self::PushChanges => "Push changes",
            Self::CreateRepositories => "Create repositories",
            Self::ManageRepositories => "Manage repositories",
            Self::ManageNamespace => "Manage namespace",
            Self::ManageMembers => "Manage members",
            Self::RewriteProtectedBranches => "Rewrite protected branches",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PermissionSet(BTreeSet<NamespacePermission>);

impl PermissionSet {
    pub fn new(permissions: impl IntoIterator<Item = NamespacePermission>) -> Self {
        Self(permissions.into_iter().collect())
    }

    pub fn all() -> Self {
        Self::new(NamespacePermission::ALL)
    }

    pub fn from_role(role: NamespaceRole) -> Self {
        let permissions = match role {
            NamespaceRole::Owner => NamespacePermission::ALL.as_slice(),
            NamespaceRole::Contributor => &[
                NamespacePermission::ViewNamespace,
                NamespacePermission::PushChanges,
            ],
        };
        Self::new(permissions.iter().copied())
    }

    pub fn contains(&self, permission: NamespacePermission) -> bool {
        self.0.contains(&permission)
    }

    pub fn iter(&self) -> impl Iterator<Item = NamespacePermission> + '_ {
        self.0.iter().copied()
    }
}

/// Resolve effective namespace permissions for a user. Site-admin status is a
/// configuration override, but only for namespaces that exist.
pub async fn for_user(
    db: &Database,
    user_id: &str,
    namespace: &str,
    is_site_admin: bool,
) -> Result<Option<PermissionSet>, String> {
    if is_site_admin && db.get_namespace_by_name(namespace).await?.is_some() {
        return Ok(Some(PermissionSet::all()));
    }

    Ok(db
        .get_namespace_role(user_id, namespace)
        .await?
        .map(PermissionSet::from_role))
}

pub async fn has(
    db: &Database,
    user_id: &str,
    namespace: &str,
    is_site_admin: bool,
    permission: NamespacePermission,
) -> Result<bool, String> {
    Ok(for_user(db, user_id, namespace, is_site_admin)
        .await?
        .is_some_and(|permissions| permissions.contains(permission)))
}

pub async fn namespaces_for_user(
    db: &Database,
    user_id: &str,
    is_site_admin: bool,
    permission: NamespacePermission,
) -> Result<Vec<Namespace>, String> {
    if is_site_admin {
        return Ok(db
            .get_all_namespaces_with_owners()
            .await?
            .into_iter()
            .map(|(namespace, _)| namespace)
            .collect());
    }

    let mut allowed = Vec::new();
    for namespace in db.get_namespaces_for_user(user_id).await? {
        if has(db, user_id, &namespace.name, false, permission).await? {
            allowed.push(namespace);
        }
    }
    Ok(allowed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{User, create_namespace};

    #[test]
    fn owner_has_every_namespace_permission() {
        let permissions = PermissionSet::from_role(NamespaceRole::Owner);

        for permission in NamespacePermission::ALL {
            assert!(permissions.contains(permission), "missing {permission:?}");
        }
    }

    #[test]
    fn contributor_has_only_namespace_view_and_push() {
        let permissions = PermissionSet::from_role(NamespaceRole::Contributor);

        assert!(permissions.contains(NamespacePermission::ViewNamespace));
        assert!(permissions.contains(NamespacePermission::PushChanges));
        for permission in [
            NamespacePermission::CreateRepositories,
            NamespacePermission::ManageRepositories,
            NamespacePermission::ManageNamespace,
            NamespacePermission::ManageMembers,
            NamespacePermission::RewriteProtectedBranches,
        ] {
            assert!(
                !permissions.contains(permission),
                "unexpected {permission:?}"
            );
        }
    }

    #[tokio::test]
    async fn configured_admin_gets_all_permissions_without_membership() {
        let db_path = format!(
            "/tmp/test_twig_permission_admin_{}.db",
            uuid::Uuid::new_v4()
        );
        let db = Database::new(&db_path);
        db.init_tables()
            .await
            .expect("initialize permission database");

        let owner = User {
            id: uuid::Uuid::new_v4().to_string(),
            username: "namespace-owner".to_string(),
            email: None,
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let admin = User {
            id: uuid::Uuid::new_v4().to_string(),
            username: "configured-admin".to_string(),
            email: None,
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        db.create_user(&owner)
            .await
            .expect("create namespace owner");
        db.create_user(&admin)
            .await
            .expect("create configured admin");
        let namespace = create_namespace("admin-permissions".to_string(), owner.id);
        db.create_namespace(&namespace)
            .await
            .expect("create namespace");

        assert_eq!(
            for_user(&db, &admin.id, &namespace.name, true)
                .await
                .expect("resolve configured admin permissions"),
            Some(PermissionSet::all())
        );
        assert_eq!(
            for_user(&db, &admin.id, &namespace.name, false)
                .await
                .expect("resolve non-admin permissions"),
            None,
            "a non-member must not inherit permissions"
        );
        assert_eq!(
            for_user(&db, &admin.id, "not-created", true)
                .await
                .expect("resolve permissions for a missing namespace"),
            None,
            "site-admin override applies only to existing namespaces"
        );

        let _ = std::fs::remove_file(db_path);
    }
}
