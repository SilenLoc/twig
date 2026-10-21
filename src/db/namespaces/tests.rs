use super::*;
use crate::auth::{Namespace, User};

async fn setup_db_with_user() -> (Database, String, String) {
    let db_path = format!("/tmp/test_twig_namespaces_{}.db", uuid::Uuid::new_v4());
    let db = Database::new(&db_path);
    db.init_tables().await.expect("init tables");

    let user_id = uuid::Uuid::new_v4().to_string();
    let user = User {
        id: user_id.clone(),
        username: "nsuser".to_string(),
        email: Some("ns@example.com".to_string()),
        password_hash: "hash".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    db.create_user(&user).await.expect("create user");

    (db, db_path, user_id)
}

fn test_namespace(owner_id: &str) -> Namespace {
    Namespace {
        id: uuid::Uuid::new_v4().to_string(),
        name: "testns".to_string(),
        owner_id: owner_id.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

#[tokio::test]
async fn test_create_and_get_namespace_by_name() {
    let (db, db_path, user_id) = setup_db_with_user().await;
    let namespace = test_namespace(&user_id);

    db.create_namespace(&namespace)
        .await
        .expect("create namespace");

    let retrieved = db
        .get_namespace_by_name(&namespace.name)
        .await
        .expect("get namespace");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, namespace.id);

    // Cleanup
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn test_get_all_namespaces_with_owners() {
    let (db, db_path, user_id) = setup_db_with_user().await;
    let namespace = test_namespace(&user_id);

    db.create_namespace(&namespace)
        .await
        .expect("create namespace");

    let result = db
        .get_all_namespaces_with_owners()
        .await
        .expect("get all namespaces");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0.name, namespace.name);
    assert_eq!(result[0].1, "nsuser");

    // Cleanup
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn test_user_has_namespace_access_owner() {
    let (db, db_path, user_id) = setup_db_with_user().await;
    let namespace = test_namespace(&user_id);

    db.create_namespace(&namespace)
        .await
        .expect("create namespace");

    let has_access = db
        .user_has_namespace_access(&user_id, &namespace.name)
        .await
        .expect("check access");
    assert!(has_access);

    // Cleanup
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn test_user_has_namespace_access_denied() {
    let (db, db_path, user_id) = setup_db_with_user().await;
    let namespace = test_namespace(&user_id);

    db.create_namespace(&namespace)
        .await
        .expect("create namespace");

    let other_user_id = uuid::Uuid::new_v4().to_string();
    let has_access = db
        .user_has_namespace_access(&other_user_id, &namespace.name)
        .await
        .expect("check access");
    assert!(!has_access);

    // Cleanup
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn namespace_roles_control_owner_scopes_and_creation_permissions() {
    let (db, db_path, owner_id) = setup_db_with_user().await;
    let namespace = test_namespace(&owner_id);
    db.create_namespace(&namespace)
        .await
        .expect("create owner namespace");

    assert_eq!(
        db.get_namespace_role(&owner_id, &namespace.name)
            .await
            .expect("get owner role"),
        Some(crate::auth::NamespaceRole::Owner)
    );
    assert!(
        db.user_owns_namespace(&owner_id, &namespace.name)
            .await
            .expect("owner permission")
    );
    assert_eq!(
        db.get_owned_namespaces_for_user(&owner_id)
            .await
            .expect("list owned namespaces")
            .len(),
        1
    );
    assert!(
        db.user_can_create_namespace(&owner_id)
            .await
            .expect("owner may create a namespace")
    );

    let contributor_id = uuid::Uuid::new_v4().to_string();
    let contributor = User {
        id: contributor_id.clone(),
        username: "namespace-contributor".to_string(),
        email: Some("contributor@example.com".to_string()),
        password_hash: "hash".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    db.create_user(&contributor)
        .await
        .expect("create contributor");
    db.conn()
        .await
        .unwrap()
        .execute(
            "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
             VALUES (?1, ?2, 'contributor', ?3)",
            crate::db::sqlite::params![
                namespace.id.clone(),
                contributor_id.clone(),
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .await
        .expect("add contributor membership");

    assert_eq!(
        db.get_namespace_role(&contributor_id, &namespace.name)
            .await
            .expect("get contributor role"),
        Some(crate::auth::NamespaceRole::Contributor)
    );
    assert!(
        !db.user_owns_namespace(&contributor_id, &namespace.name)
            .await
            .expect("contributor is not an owner")
    );
    assert!(
        db.get_owned_namespaces_for_user(&contributor_id)
            .await
            .expect("list contributor-owned namespaces")
            .is_empty()
    );
    assert!(
        !db.user_can_create_namespace(&contributor_id)
            .await
            .expect("contributor cannot create a namespace")
    );

    let new_user_id = uuid::Uuid::new_v4().to_string();
    assert!(
        db.user_can_create_namespace(&new_user_id)
            .await
            .expect("a user with no memberships may create their first namespace")
    );

    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
async fn test_search_namespaces_with_owners() {
    let (db, db_path, user_id) = setup_db_with_user().await;
    let namespace = test_namespace(&user_id);

    db.create_namespace(&namespace)
        .await
        .expect("create namespace");

    let result = db
        .search_namespaces_with_owners("test")
        .await
        .expect("search namespaces");
    assert_eq!(result.len(), 1);

    let no_result = db
        .search_namespaces_with_owners("xyz")
        .await
        .expect("search namespaces");
    assert!(no_result.is_empty());

    // Cleanup
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn test_rename_namespace() {
    let (db, db_path, user_id) = setup_db_with_user().await;
    let namespace = test_namespace(&user_id);

    db.create_namespace(&namespace)
        .await
        .expect("create namespace");
    db.rename_namespace(&namespace.id, "renamed")
        .await
        .expect("rename namespace");

    assert!(
        db.get_namespace_by_name(&namespace.name)
            .await
            .expect("get old name")
            .is_none()
    );
    let renamed = db
        .get_namespace_by_name("renamed")
        .await
        .expect("get new name")
        .expect("renamed namespace exists");
    assert_eq!(renamed.id, namespace.id);

    // Cleanup
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn test_delete_namespace() {
    let (db, db_path, user_id) = setup_db_with_user().await;
    let namespace = test_namespace(&user_id);

    db.create_namespace(&namespace)
        .await
        .expect("create namespace");
    db.delete_namespace(&namespace.id)
        .await
        .expect("delete namespace");

    let retrieved = db
        .get_namespace_by_name(&namespace.name)
        .await
        .expect("get namespace");
    assert!(retrieved.is_none());

    // Cleanup
    let _ = std::fs::remove_file(&db_path);
}
