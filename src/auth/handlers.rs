use actix_web::{HttpRequest, HttpResponse, Responder, post, web};
use log::info;
use serde::Deserialize;

use crate::auth::{
    AuthState, CreateNamespaceRequest, CreateNamespaceResponse, SignupRequest, SignupResponse,
    Ticket, create_namespace, create_user, extract_api_key, extract_basic_auth,
    extract_bearer_token, generate_token, verify_password,
};

// Form data types for HTMX UI submissions
#[derive(Debug, Deserialize)]
pub struct TicketForm {
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct SignupForm {
    pub ticket: String,
    pub username: String,
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

// ========== API Endpoints (JSON) ==========

#[post("/api/auth/ticket")]
pub async fn create_ticket_api(
    req: HttpRequest,
    auth_state: web::Data<AuthState>,
) -> impl Responder {
    // Validate API key from header
    let api_key = match extract_api_key(&req) {
        Some(key) => key,
        None => {
            return HttpResponse::Unauthorized().body("Missing API key");
        }
    };

    if !auth_state.validate_api_key(&api_key) {
        return HttpResponse::Unauthorized().body("Invalid API key");
    }

    // Generate a new ticket (not associated with any user yet)
    let ticket_id = generate_token();
    let now = chrono::Utc::now().to_rfc3339();

    // Create ticket placeholder (user_id will be set when ticket is used for signup)
    let ticket = Ticket {
        id: ticket_id.clone(),
        user_id: None, // Will be set during signup
        used: false,
        created_at: now,
        used_at: None,
    };

    if let Err(e) = auth_state.db.create_ticket(&ticket).await {
        log::error!("Failed to create ticket: {}", e);
        return HttpResponse::InternalServerError().body("Failed to create ticket");
    }

    info!("Generated signup ticket: {}", ticket_id);

    HttpResponse::Ok().json(crate::auth::SignupResponse {
        user_id: String::new(),
        username: String::new(),
        ticket: ticket_id,
    })
}

#[post("/api/auth/signup")]
pub async fn signup(
    auth_state: web::Data<AuthState>,
    signup_req: web::Json<SignupRequest>,
) -> impl Responder {
    // Validate ticket first
    let ticket = match auth_state.db.get_ticket(&signup_req.ticket).await {
        Ok(Some(ticket)) => ticket,
        Ok(None) => {
            return HttpResponse::Unauthorized().body("Invalid ticket");
        }
        Err(e) => {
            log::error!("Database error: {}", e);
            return HttpResponse::InternalServerError().body("Database error");
        }
    };

    if ticket.used {
        return HttpResponse::Unauthorized().body("Ticket has already been used");
    }

    // Validate input
    if signup_req.username.len() < 3 {
        return HttpResponse::BadRequest().body("Username must be at least 3 characters");
    }
    if signup_req.password.len() < 8 {
        return HttpResponse::BadRequest().body("Password must be at least 8 characters");
    }

    // Check if user already exists
    match auth_state
        .db
        .get_user_by_username(&signup_req.username)
        .await
    {
        Ok(Some(_)) => {
            return HttpResponse::Conflict().body("Username already exists");
        }
        Ok(None) => {}
        Err(e) => {
            log::error!("Database error: {}", e);
            return HttpResponse::InternalServerError().body("Database error");
        }
    }

    // Create user
    let (user, _) = match create_user(signup_req.username.clone(), signup_req.password.clone()) {
        Ok((user, _)) => (user, ""),
        Err(e) => {
            log::error!("Failed to create user: {}", e);
            return HttpResponse::InternalServerError().body("Failed to create user");
        }
    };

    // Save user to database
    if let Err(e) = auth_state.db.create_user(&user).await {
        log::error!("Failed to save user: {}", e);
        return HttpResponse::InternalServerError().body("Failed to save user");
    }

    // Mark ticket as used
    if let Err(e) = auth_state.db.mark_ticket_used(&ticket.id).await {
        log::error!("Failed to mark ticket used: {}", e);
        // Don't fail here, user is already created
    }

    info!("Created user: {} with ticket: {}", user.username, ticket.id);

    let response = SignupResponse {
        user_id: user.id,
        username: user.username,
        ticket: String::new(), // No longer returning ticket
    };

    HttpResponse::Ok().json(response)
}

#[post("/api/auth/login")]
pub async fn login(req: HttpRequest, auth_state: web::Data<AuthState>) -> impl Responder {
    // Extract basic auth credentials
    let (username, password) = match extract_basic_auth(&req) {
        Some(creds) => creds,
        None => {
            return HttpResponse::Unauthorized()
                .insert_header(("WWW-Authenticate", "Basic realm=\"fig\""))
                .body("Missing credentials");
        }
    };

    // Get user from database
    let user = match auth_state.db.get_user_by_username(&username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return HttpResponse::Unauthorized().body("Invalid credentials");
        }
        Err(e) => {
            log::error!("Database error: {}", e);
            return HttpResponse::InternalServerError().body("Database error");
        }
    };

    // Verify password
    match verify_password(&password, &user.password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Unauthorized().body("Invalid credentials");
        }
        Err(e) => {
            log::error!("Password verification error: {}", e);
            return HttpResponse::InternalServerError().body("Authentication error");
        }
    }

    // Create session token
    let token = match auth_state.create_session(user.id.clone()).await {
        Ok(token) => token,
        Err(e) => {
            log::error!("Failed to create session: {}", e);
            return HttpResponse::InternalServerError().body("Failed to create session");
        }
    };

    info!("User logged in: {}", username);

    HttpResponse::Ok().json(crate::auth::AuthToken {
        token,
        user_id: user.id,
        username: user.username,
    })
}

#[post("/api/auth/namespace")]
pub async fn create_namespace_endpoint(
    req: HttpRequest,
    auth_state: web::Data<AuthState>,
    create_req: web::Json<CreateNamespaceRequest>,
) -> impl Responder {
    // Validate namespace name
    if create_req.name.len() < 2 {
        return HttpResponse::BadRequest().body("Namespace name must be at least 2 characters");
    }

    // Check if namespace already exists
    match auth_state.db.get_namespace_by_name(&create_req.name).await {
        Ok(Some(_)) => {
            return HttpResponse::Conflict().body("Namespace already exists");
        }
        Ok(None) => {}
        Err(e) => {
            log::error!("Database error: {}", e);
            return HttpResponse::InternalServerError().body("Database error");
        }
    }

    // Authenticate user via Bearer token
    let token = match extract_bearer_token(&req) {
        Some(token) => token,
        None => {
            return HttpResponse::Unauthorized().body("Missing authentication token");
        }
    };

    let user_id = match auth_state.validate_token(&token).await {
        Some(user_id) => user_id,
        None => {
            return HttpResponse::Unauthorized().body("Invalid or expired token");
        }
    };

    // Create namespace
    let namespace = create_namespace(create_req.name.clone(), user_id);

    if let Err(e) = auth_state.db.create_namespace(&namespace).await {
        log::error!("Failed to create namespace: {}", e);
        return HttpResponse::InternalServerError().body("Failed to create namespace");
    }

    info!(
        "Created namespace: {} for user: {}",
        namespace.name, namespace.owner_id
    );

    // Create the namespace directory
    let namespace_path = std::path::Path::new(
        &std::env::var("PROJECT_ROOT").unwrap_or_else(|_| "/srv/git".to_string()),
    )
    .join(&namespace.name);

    if let Err(e) = std::fs::create_dir_all(&namespace_path) {
        log::error!("Failed to create namespace directory: {}", e);
        // Don't fail here, directory can be created later
    }

    let response = CreateNamespaceResponse {
        namespace_id: namespace.id,
        name: namespace.name,
    };

    HttpResponse::Ok().json(response)
}

#[post("/api/auth/logout")]
pub async fn logout(_req: HttpRequest, auth_state: web::Data<AuthState>) -> impl Responder {
    // Try to extract token from Bearer header first, then from session cookie
    let token = match extract_bearer_token(&_req) {
        Some(token) => token,
        None => {
            // Fallback to session cookie
            match _req.cookie("session") {
                Some(cookie) => cookie.value().to_string(),
                None => {
                    return HttpResponse::Unauthorized().body("Missing token");
                }
            }
        }
    };

    // Invalidate token
    if let Err(e) = auth_state.invalidate_token(&token).await {
        log::error!("Failed to invalidate token: {}", e);
        return HttpResponse::InternalServerError().body("Failed to logout");
    }

    HttpResponse::Ok().body("Logged out")
}

// ========== UI Form Handlers (HTML) ==========

use crate::view::auth::{
    render_error, render_login_success, render_signup_success, render_success,
    render_ticket_success,
};

#[post("/auth/ticket")]
pub async fn create_ticket_ui_handler(
    _req: HttpRequest,
    auth_state: web::Data<AuthState>,
    form: web::Form<TicketForm>,
) -> impl Responder {
    // Validate API key
    if !auth_state.validate_api_key(&form.api_key) {
        return HttpResponse::Unauthorized().body(render_error("Invalid API key").into_string());
    }

    // Generate a new ticket
    let ticket_id = generate_token();
    let now = chrono::Utc::now().to_rfc3339();

    let ticket = Ticket {
        id: ticket_id.clone(),
        user_id: None,
        used: false,
        created_at: now,
        used_at: None,
    };

    if let Err(e) = auth_state.db.create_ticket(&ticket).await {
        log::error!("Failed to create ticket: {}", e);
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to create ticket").into_string());
    }

    info!("Generated signup ticket via UI: {}", ticket_id);

    // Return success with HX-Retarget to replace the whole form area
    HttpResponse::Ok()
        .content_type("text/html")
        .insert_header(("HX-Retarget", "#auth-content"))
        .insert_header(("HX-Reswap", "innerHTML"))
        .body(render_ticket_success(&ticket_id).into_string())
}

#[post("/auth/signup")]
pub async fn signup_ui_handler(
    _req: HttpRequest,
    auth_state: web::Data<AuthState>,
    form: web::Form<SignupForm>,
) -> impl Responder {
    // Validate ticket first
    let ticket = match auth_state.db.get_ticket(&form.ticket).await {
        Ok(Some(ticket)) => ticket,
        Ok(None) => {
            return HttpResponse::Unauthorized().body(render_error("Invalid ticket").into_string());
        }
        Err(e) => {
            log::error!("Database error: {}", e);
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    };

    if ticket.used {
        return HttpResponse::Unauthorized()
            .body(render_error("Ticket has already been used").into_string());
    }

    // Validate input
    if form.username.len() < 3 {
        return HttpResponse::BadRequest()
            .body(render_error("Username must be at least 3 characters").into_string());
    }
    if form.password.len() < 8 {
        return HttpResponse::BadRequest()
            .body(render_error("Password must be at least 8 characters").into_string());
    }

    // Check if user already exists
    match auth_state.db.get_user_by_username(&form.username).await {
        Ok(Some(_)) => {
            return HttpResponse::Conflict()
                .body(render_error("Username already exists").into_string());
        }
        Ok(None) => {}
        Err(e) => {
            log::error!("Database error: {}", e);
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    }

    // Create user
    let (user, _) = match create_user(form.username.clone(), form.password.clone()) {
        Ok((user, _)) => (user, ""),
        Err(e) => {
            log::error!("Failed to create user: {}", e);
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to create user").into_string());
        }
    };

    // Save user to database
    if let Err(e) = auth_state.db.create_user(&user).await {
        log::error!("Failed to save user: {}", e);
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to save user").into_string());
    }

    // Mark ticket as used
    if let Err(e) = auth_state.db.mark_ticket_used(&ticket.id).await {
        log::error!("Failed to mark ticket used: {}", e);
        // Don't fail here, user is already created
    }

    info!(
        "Created user via UI: {} with ticket: {}",
        user.username, ticket.id
    );

    // Return HTML response for HTMX
    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_signup_success(&user.username).into_string())
}

#[post("/auth/login")]
pub async fn login_ui_handler(
    _req: HttpRequest,
    auth_state: web::Data<AuthState>,
    form: web::Form<LoginForm>,
) -> impl Responder {
    // Get user from database
    let user = match auth_state.db.get_user_by_username(&form.username).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return HttpResponse::Unauthorized()
                .body(render_error("Invalid credentials").into_string());
        }
        Err(e) => {
            log::error!("Database error: {}", e);
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
            log::error!("Password verification error: {}", e);
            return HttpResponse::InternalServerError()
                .body(render_error("Authentication error").into_string());
        }
    }

    // Create session token
    let token = match auth_state.create_session(user.id.clone()).await {
        Ok(token) => token,
        Err(e) => {
            log::error!("Failed to create session: {}", e);
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to create session").into_string());
        }
    };

    info!("User logged in via UI: {}", form.username);

    // Return HTML response with session cookie (no token shown to user)
    // Use HX-Retarget to replace the whole form area on success
    HttpResponse::Ok()
        .content_type("text/html")
        .insert_header(("HX-Retarget", "#auth-content"))
        .insert_header(("HX-Reswap", "innerHTML"))
        .cookie(
            actix_web::cookie::Cookie::build("session", token)
                .path("/")
                .http_only(true)
                .same_site(actix_web::cookie::SameSite::Strict)
                .finish(),
        )
        .body(render_login_success(&user.username).into_string())
}

#[post("/auth/namespace")]
pub async fn create_namespace_ui_handler(
    req: HttpRequest,
    auth_state: web::Data<AuthState>,
    form: web::Form<CreateNamespaceForm>,
) -> impl Responder {
    // Validate namespace name
    if form.name.len() < 2 {
        return HttpResponse::BadRequest()
            .body(render_error("Namespace name must be at least 2 characters").into_string());
    }

    // Check if namespace already exists
    match auth_state.db.get_namespace_by_name(&form.name).await {
        Ok(Some(_)) => {
            return HttpResponse::Conflict()
                .body(render_error("Namespace already exists").into_string());
        }
        Ok(None) => {}
        Err(e) => {
            log::error!("Database error: {}", e);
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    }

    // Authenticate user via session cookie
    let token = match req.cookie("session") {
        Some(cookie) => cookie.value().to_string(),
        None => {
            return HttpResponse::Unauthorized()
                .body(render_error("Not logged in. Please log in first.").into_string());
        }
    };

    let user_id = match auth_state.validate_token(&token).await {
        Some(user_id) => user_id,
        None => {
            return HttpResponse::Unauthorized()
                .body(render_error("Session expired. Please log in again.").into_string());
        }
    };

    // Create namespace
    let namespace = create_namespace(form.name.clone(), user_id);

    if let Err(e) = auth_state.db.create_namespace(&namespace).await {
        log::error!("Failed to create namespace: {}", e);
        return HttpResponse::InternalServerError()
            .body(render_error("Failed to create namespace").into_string());
    }

    info!(
        "Created namespace via UI: {} for user: {}",
        namespace.name, namespace.owner_id
    );

    // Create the namespace directory
    let namespace_path = std::path::Path::new(
        &std::env::var("PROJECT_ROOT").unwrap_or_else(|_| "/srv/git".to_string()),
    )
    .join(&namespace.name);

    if let Err(e) = std::fs::create_dir_all(&namespace_path) {
        log::error!("Failed to create namespace directory: {}", e);
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
    use crate::auth::{Ticket, generate_token};
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
    fn test_ticket_struct_with_none_user_id() {
        let ticket = Ticket {
            id: generate_token(),
            user_id: None,
            used: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            used_at: None,
        };

        assert!(ticket.user_id.is_none());
        assert!(!ticket.used);
    }

    #[test]
    fn test_ticket_struct_with_some_user_id() {
        let ticket = Ticket {
            id: generate_token(),
            user_id: Some("user-123".to_string()),
            used: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            used_at: Some(chrono::Utc::now().to_rfc3339()),
        };

        assert_eq!(ticket.user_id, Some("user-123".to_string()));
        assert!(ticket.used);
    }

    #[tokio::test]
    async fn test_create_ticket_with_null_user_id() {
        // Create a temporary database for testing
        let db_path = format!("/tmp/test_fig_db_{}.db", generate_token());
        let db = Database::new(&db_path)
            .await
            .expect("Failed to create database");

        // Create a ticket with NULL user_id (for signup)
        let ticket = Ticket {
            id: generate_token(),
            user_id: None,
            used: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            used_at: None,
        };

        // This should succeed now with the migration
        let result = db.create_ticket(&ticket).await;
        assert!(
            result.is_ok(),
            "Failed to create ticket with NULL user_id: {:?}",
            result.err()
        );

        // Verify we can retrieve the ticket
        let retrieved = db
            .get_ticket(&ticket.id)
            .await
            .expect("Failed to get ticket");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, ticket.id);
        assert!(retrieved.user_id.is_none());
        assert!(!retrieved.used);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }
}
