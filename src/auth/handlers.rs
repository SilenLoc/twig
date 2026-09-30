use actix_identity::{Identity, IdentityExt};
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, post, web};
use log::info;
use serde::Deserialize;

use crate::{
    auth::{Invite, TwigContext, create_namespace, create_user, generate_token, verify_password},
    config,
    view::auth::{render_invite_success, render_login_success, render_signup_success},
    view::{render_error, render_success},
};

// Form data types for HTMX UI submissions
#[derive(Debug, Deserialize)]
pub struct InviteForm {
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct SignupForm {
    pub invite: String,
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateNamespaceForm {
    pub name: String,
}

#[post("/auth/logout")]
pub async fn logout_ui_handler(
    req: HttpRequest,
    auth_state: web::Data<TwigContext>,
) -> impl Responder {
    if let Some(cookie) = req.cookie("session")
        && let Err(e) = auth_state.invalidate_token(cookie.value()).await
    {
        log::error!("Failed to invalidate token: {e}");
        return HttpResponse::InternalServerError().body("Failed to logout");
    }

    if req.cookie("id").is_some()
        && let Ok(identity) = req.get_identity()
    {
        identity.logout();
    }

    // Clear session cookie and redirect
    HttpResponse::Found()
        .insert_header(("Location", "/"))
        .cookie(
            actix_web::cookie::Cookie::build("session", "")
                .path("/")
                .http_only(true)
                .same_site(actix_web::cookie::SameSite::Strict)
                .max_age(actix_web::cookie::time::Duration::ZERO)
                .finish(),
        )
        .finish()
}

// ========== UI Form Handlers (HTML) ==========

#[post("/auth/invite")]
pub async fn create_invite_ui_handler(
    _req: HttpRequest,
    auth_state: web::Data<TwigContext>,
    form: web::Form<InviteForm>,
) -> impl Responder {
    // Validate API key
    if !auth_state.validate_api_key(&form.api_key) {
        return HttpResponse::Unauthorized().body(render_error("Invalid API key").into_string());
    }

    // Generate a new invite
    let invite_id = generate_token();
    let now = chrono::Utc::now().to_rfc3339();

    let invite = Invite {
        id: invite_id.clone(),
        user_id: None,
        used: false,
        created_at: now,
        used_at: None,
    };

    let db = auth_state.db();

    if let Err(e) = db.create_invite(&invite).await {
        log::error!("Failed to create invite: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to create invite").into_string());
    }

    info!("Generated signup invite via UI: {invite_id}");

    // Return success with HX-Retarget to replace the whole form area
    HttpResponse::Ok()
        .content_type("text/html")
        .insert_header(("HX-Retarget", "#auth-content"))
        .insert_header(("HX-Reswap", "innerHTML"))
        .body(render_invite_success(&invite_id).into_string())
}

#[post("/auth/signup")]
pub async fn signup_ui_handler(
    _req: HttpRequest,
    auth_state: web::Data<TwigContext>,
    form: web::Form<SignupForm>,
) -> impl Responder {
    let db = auth_state.db();

    // Validate invite first
    let invite = match db.get_invite(&form.invite).await {
        Ok(Some(invite)) => invite,
        Ok(None) => {
            return HttpResponse::Unauthorized().body(render_error("Invalid invite").into_string());
        }
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    };

    if invite.used {
        return HttpResponse::Unauthorized()
            .body(render_error("Invite has already been used").into_string());
    }

    // Validate input
    if form.username.len() < 3 {
        return HttpResponse::BadRequest()
            .body(render_error("Username must be at least 3 characters").into_string());
    }
    if form.email.is_empty() || !form.email.contains('@') {
        return HttpResponse::BadRequest()
            .body(render_error("Please enter a valid email address").into_string());
    }
    if form.password.len() < 8 {
        return HttpResponse::BadRequest()
            .body(render_error("Password must be at least 8 characters").into_string());
    }

    // Check if user already exists
    match db.get_user_by_username(&form.username).await {
        Ok(Some(_)) => {
            return HttpResponse::Conflict()
                .body(render_error("Username already exists").into_string());
        }
        Ok(None) => {}
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    }

    // Create user
    let user = match create_user(form.username.clone(), form.email.clone(), &form.password) {
        Ok(user) => user,
        Err(e) => {
            log::error!("Failed to create user: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to create user").into_string());
        }
    };

    // Save user to database
    if let Err(e) = db.create_user(&user).await {
        log::error!("Failed to save user: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to save user").into_string());
    }

    // Mark invite as used
    if let Err(e) = db.mark_invite_used(&invite.id).await {
        log::error!("Failed to mark invite used: {e}");
        // Don't fail here, user is already created
    }

    info!(
        "Created user via UI: {} with invite: {}",
        user.username, invite.id
    );

    // Return HTML response for HTMX
    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_signup_success(&user.username).into_string())
}

#[post("/auth/login")]
pub async fn login_ui_handler(
    req: HttpRequest,
    auth_state: web::Data<TwigContext>,
    form: web::Form<LoginForm>,
) -> impl Responder {
    let db = auth_state.db();

    // Get user from database
    let user = match db.get_user_by_username(&form.username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return HttpResponse::Unauthorized()
                .body(render_error("Invalid credentials").into_string());
        }
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    };

    // Verify password
    match verify_password(&form.password, &user.password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Unauthorized()
                .body(render_error("Invalid credentials").into_string());
        }
        Err(e) => {
            log::error!("Password verification error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Authentication error").into_string());
        }
    }

    if let Err(e) = Identity::login(&req.extensions(), user.id.clone()) {
        log::error!("Failed to create identity session: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to create session").into_string());
    }

    info!("User logged in via UI: {}", form.username);

    // Check if user has any namespaces
    let has_namespaces = match db.user_has_any_namespaces(&user.id).await {
        Ok(has) => has,
        Err(e) => {
            log::error!("Failed to check namespaces for user '{}': {e}", user.id);
            false
        }
    };

    if has_namespaces {
        // Redirect to home page if user already has namespaces
        HttpResponse::Ok()
            .insert_header(("HX-Redirect", "/"))
            .body("")
    } else {
        // Show create namespace page if user has no namespaces
        HttpResponse::Ok()
            .content_type("text/html")
            .insert_header(("HX-Retarget", "#auth-content"))
            .insert_header(("HX-Reswap", "innerHTML"))
            .body(render_login_success(&user.username).into_string())
    }
}

#[post("/auth/namespace")]
pub async fn create_namespace_ui_handler(
    req: HttpRequest,
    auth_state: web::Data<TwigContext>,
    form: web::Form<CreateNamespaceForm>,
) -> impl Responder {
    let db = auth_state.db();

    // Validate namespace name
    if form.name.len() < 2 {
        return HttpResponse::BadRequest()
            .body(render_error("Namespace name must be at least 2 characters").into_string());
    }
    if form.name.starts_with('_') {
        return HttpResponse::BadRequest()
            .body(render_error("Namespace names starting with '_' are reserved").into_string());
    }

    // Check if namespace already exists
    match db.get_namespace_by_name(&form.name).await {
        Ok(Some(_)) => {
            return HttpResponse::Conflict()
                .body(render_error("Namespace already exists").into_string());
        }
        Ok(None) => {}
        Err(e) => {
            log::error!("Database error: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    }

    // Authenticate user via session cookie
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        let message = if req.cookie("session").is_some() {
            "Session expired. Please log in again."
        } else {
            "Not logged in. Please log in first."
        };
        return HttpResponse::Unauthorized().body(render_error(message).into_string());
    };

    // Create namespace
    let namespace = create_namespace(form.name.clone(), user_id.clone());

    if let Err(e) = db.create_namespace(&namespace).await {
        log::error!("Failed to create namespace: {e}");
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to create namespace").into_string());
    }

    info!(
        "Created namespace via UI: {} for user: {}",
        namespace.name, namespace.owner_id
    );

    // Get project root from config
    let project_root = req.app_data::<web::Data<config::Server>>().map_or_else(
        || "/srv/git".to_string(),
        |cfg| cfg.project_root().to_string(),
    );

    // Create the namespace directory
    let namespace_path = std::path::Path::new(&project_root).join(&namespace.name);

    if let Err(e) = std::fs::create_dir_all(&namespace_path) {
        log::error!("Failed to create namespace directory: {e}");
        // Don't fail here, directory can be created later
    }

    // Return HTML response for HTMX
    let success_msg = format!(
        "Namespace '{}' created successfully! You can now create repositories in this namespace.",
        namespace.name
    );
    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_success(&success_msg).into_string())
}

#[cfg(test)]
mod tests {
    use crate::auth::{Invite, generate_token};
    use crate::db::Database;

    #[test]
    fn test_generate_token_format() {
        let token = generate_token();
        // Token should be 64 hex characters (32 bytes * 2)
        assert_eq!(token.len(), 64);
        // Token should only contain hex characters
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_invite_struct_with_none_user_id() {
        let invite = Invite {
            id: generate_token(),
            user_id: None,
            used: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            used_at: None,
        };

        assert!(invite.user_id.is_none());
        assert!(!invite.used);
    }

    #[test]
    fn test_invite_struct_with_some_user_id() {
        let invite = Invite {
            id: generate_token(),
            user_id: Some("user-123".to_string()),
            used: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            used_at: Some(chrono::Utc::now().to_rfc3339()),
        };

        assert_eq!(invite.user_id, Some("user-123".to_string()));
        assert!(invite.used);
    }

    #[tokio::test]
    async fn test_create_invite_with_null_user_id() {
        // Create a temporary database for testing
        let db_path = format!("/tmp/test_twig_db_{}.db", generate_token());
        let db = Database::new(&db_path);

        // Initialize tables for testing
        db.init_tables()
            .await
            .expect("Failed to initialize database tables");

        // Create a invite with NULL user_id (for signup)
        let invite = Invite {
            id: generate_token(),
            user_id: None,
            used: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            used_at: None,
        };

        // This should succeed now with the migration
        let result = db.create_invite(&invite).await;
        assert!(
            result.is_ok(),
            "Failed to create invite with NULL user_id: {:?}",
            result.err()
        );

        // Verify we can retrieve the invite
        let retrieved = db
            .get_invite(&invite.id)
            .await
            .expect("Failed to get invite");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, invite.id);
        assert!(retrieved.user_id.is_none());
        assert!(!retrieved.used);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }
}
