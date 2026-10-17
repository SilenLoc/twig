use super::*;

pub(super) async fn create_test_service() -> impl actix_web::dev::Service<
    Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    create_test_service_in("/tmp/test_git").await
}

// Helper function to create full test app service rooted at a given
// PROJECT_ROOT, for tests that assert on repositories written to disk.
pub(super) async fn create_test_service_in(
    project_root: &str,
) -> impl actix_web::dev::Service<
    Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    create_test_service_with_db_in(project_root).await.0
}
pub(super) async fn create_test_service_with_db_in(
    project_root: &str,
) -> (
    impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    Database,
) {
    // A distinct database per call: nextest runs each test in its own
    // process, and turso takes an exclusive file lock, so a shared path
    // would make concurrent tests fail to open the database.
    let config = config::Server::new(
        ("127.0.0.1".to_string(), 8080),
        "debug".to_string(),
        project_root.to_string(),
        format!("/tmp/test_twig_service_{}.db", uuid::Uuid::new_v4()),
        "secure".to_string(),
        true,
    );

    let db = Database::new(config.db_path());
    let auth_state = web::Data::new(auth::TwigContext::new(db, "secure".to_string()));
    // Initialize database synchronously (we are already inside an async test runtime)
    auth_state.db().init_tables().await.expect("init tables");

    let config_data = web::Data::new(config);
    let test_db = auth_state.db().clone();
    let session_db = test_db.clone();

    let app = test::init_service(
        App::new()
            .app_data(config_data)
            .app_data(auth_state)
            .app_data(web::PayloadConfig::new(1 << 29))
            .wrap(actix_identity::IdentityMiddleware::default())
            .wrap(crate::auth::session_store::middleware(
                session_db,
                actix_web::cookie::Key::generate(),
            ))
            .configure(crate::http::routes::configure_routes),
    )
    .await;
    (app, test_db)
}
pub(super) async fn create_admin_test_service() -> (
    impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    String,
    String,
    String,
    Database,
    String,
) {
    let project_root = format!("/tmp/test_twig_admin_root_{}", uuid::Uuid::new_v4());
    let config = config::Server::new(
        ("127.0.0.1".to_string(), 8080),
        "debug".to_string(),
        project_root.clone(),
        format!("/tmp/test_twig_admin_{}.db", uuid::Uuid::new_v4()),
        "secure".to_string(),
        false,
    )
    .with_admin_user(Some("admin".to_string()));
    let db = Database::new(config.db_path());
    db.init_tables().await.expect("init tables");
    let user = auth::create_user(
        "admin".to_string(),
        "admin@example.com".to_string(),
        "password123",
    )
    .expect("create admin");
    db.create_user(&user).await.expect("persist admin");
    let namespace = auth::create_namespace("acme".to_string(), user.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create namespace");
    let contributor = auth::create_user(
        "contributor".to_string(),
        "contributor@example.com".to_string(),
        "password123",
    )
    .expect("create contributor");
    db.create_user(&contributor)
        .await
        .expect("persist contributor");
    db.conn()
        .await
        .expect("connect to membership DB")
        .execute(
            "INSERT INTO namespace_members (namespace_id, user_id, role, added_at)
                 VALUES (?1, ?2, 'contributor', ?3)",
            turso::params![
                namespace.id.clone(),
                contributor.id.clone(),
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .await
        .expect("add contributor membership");
    let owner = auth::create_user(
        "owner".to_string(),
        "owner@example.com".to_string(),
        "password123",
    )
    .expect("create namespace owner");
    db.create_user(&owner)
        .await
        .expect("persist namespace owner");
    let owned_namespace = auth::create_namespace("docs-team".to_string(), owner.id.clone());
    db.create_namespace(&owned_namespace)
        .await
        .expect("create owned namespace");

    let auth_state = web::Data::new(auth::TwigContext::new(db, "secure".to_string()));
    let session = auth_state
        .create_session(user.id)
        .await
        .expect("create admin session");
    let owner_session = auth_state
        .create_session(owner.id)
        .await
        .expect("create owner session");
    let contributor_session = auth_state
        .create_session(contributor.id)
        .await
        .expect("create contributor session");
    let session_db = auth_state.db().clone();
    let test_db = auth_state.db().clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(config))
            .app_data(auth_state)
            .app_data(web::PayloadConfig::new(1 << 29))
            .wrap(actix_identity::IdentityMiddleware::default())
            .wrap(crate::auth::session_store::middleware(
                session_db,
                actix_web::cookie::Key::generate(),
            ))
            .configure(crate::http::routes::configure_routes),
    )
    .await;
    (
        app,
        session,
        owner_session,
        contributor_session,
        test_db,
        project_root,
    )
}
pub(super) async fn body_of(
    app: &impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    uri: &str,
) -> (StatusCode, String) {
    let req = test::TestRequest::get().uri(uri).to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    let body = test::read_body(resp).await;
    (status, String::from_utf8(body.to_vec()).unwrap())
}
