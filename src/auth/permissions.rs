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

    pub const READ: [Self; 1] = [Self::ViewNamespace];

    pub const WRITE: [Self; 6] = [
        Self::PushChanges,
        Self::CreateRepositories,
        Self::ManageRepositories,
        Self::ManageNamespace,
        Self::ManageMembers,
        Self::RewriteProtectedBranches,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ViewNamespace => "view_namespace",
            Self::PushChanges => "push_changes",
            Self::CreateRepositories => "create_repositories",
            Self::ManageRepositories => "manage_repositories",
            Self::ManageNamespace => "manage_namespace",
            Self::ManageMembers => "manage_members",
            Self::RewriteProtectedBranches => "rewrite_protected_branches",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "view_namespace" => Some(Self::ViewNamespace),
            "push_changes" => Some(Self::PushChanges),
            "create_repositories" => Some(Self::CreateRepositories),
            "manage_repositories" => Some(Self::ManageRepositories),
            "manage_namespace" => Some(Self::ManageNamespace),
            "manage_members" => Some(Self::ManageMembers),
            "rewrite_protected_branches" => Some(Self::RewriteProtectedBranches),
            _ => None,
        }
    }

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

    pub fn insert(&mut self, permission: NamespacePermission) {
        self.0.insert(permission);
    }

    pub fn intersection(&self, other: &Self) -> Self {
        Self(self.0.intersection(&other.0).copied().collect())
    }

    pub fn is_subset_of(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
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

pub async fn for_principal(
    db: &Database,
    principal: &super::AuthenticatedPrincipal,
    namespace: &str,
    is_site_admin: bool,
) -> Result<Option<PermissionSet>, String> {
    let Some(current) = for_user(db, &principal.user.id, namespace, is_site_admin).await? else {
        return Ok(None);
    };

    let Some(token_permissions) = &principal.token_permissions else {
        return Ok(Some(current));
    };
    let Some(granted) = token_permissions.get(namespace) else {
        return Ok(None);
    };

    Ok(Some(current.intersection(granted)))
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

    #[test]
    fn permission_intersection_never_adds_grants() {
        let user_permissions = PermissionSet::all();
        let token_permissions = PermissionSet::new([NamespacePermission::PushChanges]);
        let effective = user_permissions.intersection(&token_permissions);

        assert!(effective.contains(NamespacePermission::PushChanges));
        assert_eq!(effective.iter().count(), 1);
        assert!(effective.is_subset_of(&user_permissions));
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

    #[tokio::test]
    async fn configured_admin_token_is_still_limited_to_its_saved_scope() {
        let db_path = format!(
            "/tmp/test_twig_permission_token_admin_{}.db",
            uuid::Uuid::new_v4()
        );
        let db = Database::new(&db_path);
        db.init_tables()
            .await
            .expect("initialize permission database");
        let owner = User {
            id: uuid::Uuid::new_v4().to_string(),
            username: "permission-owner".to_string(),
            email: None,
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let admin = User {
            id: uuid::Uuid::new_v4().to_string(),
            username: "configured-token-admin".to_string(),
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
        let namespace = create_namespace("token-admin-scope".to_string(), owner.id);
        db.create_namespace(&namespace)
            .await
            .expect("create namespace");
        let principal = super::super::AuthenticatedPrincipal {
            user: admin,
            token_permissions: Some(
                [(
                    namespace.name.clone(),
                    PermissionSet::new([NamespacePermission::PushChanges]),
                )]
                .into_iter()
                .collect(),
            ),
        };

        let effective = for_principal(&db, &principal, &namespace.name, true)
            .await
            .expect("resolve token admin permissions")
            .expect("token scope selects namespace");
        assert!(effective.contains(NamespacePermission::PushChanges));
        assert_eq!(effective.iter().count(), 1);

        let _ = std::fs::remove_file(db_path);
    }

    #[tokio::test]
    async fn token_grants_are_intersected_with_the_users_current_role() {
        let db_path = format!(
            "/tmp/test_twig_permission_token_role_{}.db",
            uuid::Uuid::new_v4()
        );
        let db = Database::new(&db_path);
        db.init_tables()
            .await
            .expect("initialize permission database");
        let owner = User {
            id: uuid::Uuid::new_v4().to_string(),
            username: "permission-role-owner".to_string(),
            email: None,
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let contributor = User {
            id: uuid::Uuid::new_v4().to_string(),
            username: "permission-role-contributor".to_string(),
            email: None,
            password_hash: "hash".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        db.create_user(&owner)
            .await
            .expect("create namespace owner");
        db.create_user(&contributor)
            .await
            .expect("create contributor");
        let namespace = create_namespace("token-role-scope".to_string(), owner.id);
        db.create_namespace(&namespace)
            .await
            .expect("create namespace");
        db.conn()
            .await
            .expect("connect to membership database")
            .execute(
                "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
                 VALUES (?1, ?2, 'contributor', ?3)",
                turso::params![
                    namespace.id,
                    contributor.id.clone(),
                    chrono::Utc::now().to_rfc3339()
                ],
            )
            .await
            .expect("add contributor membership");
        let principal = super::super::AuthenticatedPrincipal {
            user: contributor,
            token_permissions: Some(
                [(namespace.name.clone(), PermissionSet::all())]
                    .into_iter()
                    .collect(),
            ),
        };

        let effective = for_principal(&db, &principal, &namespace.name, false)
            .await
            .expect("resolve token permissions")
            .expect("user retains namespace access");
        assert!(effective.contains(NamespacePermission::ViewNamespace));
        assert!(effective.contains(NamespacePermission::PushChanges));
        assert_eq!(effective.iter().count(), 2);

        let _ = std::fs::remove_file(db_path);
    }
}
