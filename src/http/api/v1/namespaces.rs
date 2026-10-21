use std::path::Path;

use actix_web::{HttpRequest, HttpResponse, delete, get, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{
        AuthenticatedPrincipal, Namespace, NamespaceRole, NewNamespaceInvitation, TwigContext,
        permissions::{self, NamespacePermission},
    },
    config,
    db::Database,
    email::{InvitationDelivery, invitation_url, send_invitation_if_configured},
    git,
};

use super::{
    ApiError, InvitationPath, NamespacePath, authenticate, json_error, require_namespace_permission,
};

#[derive(Debug, Serialize)]
struct NamespaceResponse {
    name: String,
    owner: String,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct RepositorySummary {
    namespace: String,
    name: String,
    private: bool,
    last_commit_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct MutationResponse {
    resource: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct CreateNamespaceRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RenameNamespaceRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CreateRepositoryRequest {
    name: String,
    #[serde(default = "default_branch")]
    default_branch: String,
}

#[derive(Debug, Deserialize, Default)]
struct RepositoryListQuery {
    q: Option<String>,
}

fn default_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Deserialize)]
struct CreateInvitationRequest {
    email: String,
    #[serde(default = "default_role")]
    role: String,
    #[serde(default = "default_expiry")]
    expires: String,
}

fn default_role() -> String {
    "contributor".to_string()
}

fn default_expiry() -> String {
    "7d".to_string()
}

#[derive(Debug, Serialize)]
struct MemberResponse {
    username: String,
    email: Option<String>,
    role: String,
}

#[derive(Debug, Serialize)]
struct InvitationSummary {
    email: String,
    namespace: String,
    role: String,
    created_at: String,
    expires_at: Option<String>,
    accepted_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct InvitationCreatedResponse {
    invitation: InvitationSummary,
    invite_url: String,
    email_delivery: &'static str,
}

async fn principal_or_response(
    req: &HttpRequest,
    auth_state: &TwigContext,
) -> Result<AuthenticatedPrincipal, HttpResponse> {
    authenticate(req, auth_state).await.map_err(json_error)
}

async fn namespace_for_action(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    name: &str,
    permission: NamespacePermission,
) -> Result<(AuthenticatedPrincipal, Namespace), HttpResponse> {
    let principal = principal_or_response(req, auth_state).await?;
    if let Err(error) = super::validate_namespace_name(name) {
        return Err(json_error(error));
    }
    let namespace = match auth_state.db().get_namespace_by_name(name).await {
        Ok(Some(namespace)) => namespace,
        Ok(None) => return Err(json_error(ApiError::NotFound)),
        Err(error) => {
            log::error!("Failed to load API namespace '{name}': {error}");
            return Err(json_error(ApiError::Internal));
        }
    };
    if let Err(error) =
        require_namespace_permission(&principal, server, auth_state, name, permission).await
    {
        return Err(json_error(error));
    }
    Ok((principal, namespace))
}

async fn namespace_response(
    db: &Database,
    namespace: &Namespace,
) -> Result<NamespaceResponse, ApiError> {
    let owner = db
        .get_user_by_id(&namespace.owner_id)
        .await
        .map_err(|error| {
            log::error!("Failed to load namespace owner: {error}");
            ApiError::Internal
        })?
        .ok_or(ApiError::NotFound)?;
    Ok(NamespaceResponse {
        name: namespace.name.clone(),
        owner: owner.username,
        created_at: namespace.created_at.clone(),
    })
}

#[get("/api/v1/namespaces")]
pub async fn list_namespaces(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> HttpResponse {
    let principal = match principal_or_response(&req, &auth_state).await {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let db = auth_state.db();
    let is_site_admin = server.is_configured_admin(&principal.user.username);
    let namespaces = if is_site_admin {
        match db.get_all_namespaces_with_owners().await {
            Ok(namespaces) => namespaces
                .into_iter()
                .map(|(namespace, _)| namespace)
                .collect(),
            Err(error) => {
                log::error!("Failed to list API namespaces: {error}");
                return json_error(ApiError::Internal);
            }
        }
    } else {
        match db.get_namespaces_for_user(&principal.user.id).await {
            Ok(namespaces) => namespaces,
            Err(error) => {
                log::error!("Failed to list API namespaces: {error}");
                return json_error(ApiError::Internal);
            }
        }
    };

    let mut visible = Vec::new();
    for namespace in namespaces {
        match permissions::for_principal(db, &principal, &namespace.name, is_site_admin).await {
            Ok(Some(grants)) if grants.contains(NamespacePermission::ViewNamespace) => {
                match namespace_response(db, &namespace).await {
                    Ok(response) => visible.push(response),
                    Err(error) => return json_error(error),
                }
            }
            Ok(_) => {}
            Err(error) => {
                log::error!(
                    "Failed to filter API namespace '{}': {error}",
                    namespace.name
                );
                return json_error(ApiError::Internal);
            }
        }
    }
    HttpResponse::Ok().json(visible)
}

#[post("/api/v1/namespaces")]
pub async fn create_namespace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    body: web::Json<CreateNamespaceRequest>,
) -> HttpResponse {
    let principal = match principal_or_response(&req, &auth_state).await {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let name = body.name.trim();
    if name != body.name {
        return json_error(ApiError::BadRequest("Invalid namespace name".to_string()));
    }
    if let Err(message) = git::reserved::validate_namespace_name(name) {
        return json_error(ApiError::BadRequest(message));
    }
    match auth_state
        .db()
        .user_can_create_namespace(&principal.user.id)
        .await
    {
        Ok(true) => {}
        Ok(false) => return json_error(ApiError::Forbidden),
        Err(error) => {
            log::error!("Failed to check API namespace creation permission: {error}");
            return json_error(ApiError::Internal);
        }
    }
    if principal.token_permissions.is_some() {
        let is_site_admin = server.is_configured_admin(&principal.user.username);
        let namespaces = match if is_site_admin {
            auth_state
                .db()
                .get_all_namespaces_with_owners()
                .await
                .map(|all| all.into_iter().map(|(namespace, _)| namespace).collect())
        } else {
            auth_state
                .db()
                .get_namespaces_for_user(&principal.user.id)
                .await
        } {
            Ok(namespaces) => namespaces,
            Err(error) => {
                log::error!("Failed to load token namespace creation scope: {error}");
                return json_error(ApiError::Internal);
            }
        };
        let mut can_create = false;
        for namespace in namespaces {
            match permissions::for_principal(
                auth_state.db(),
                &principal,
                &namespace.name,
                is_site_admin,
            )
            .await
            {
                Ok(Some(grants)) if grants.contains(NamespacePermission::ManageNamespace) => {
                    can_create = true;
                    break;
                }
                Ok(_) => {}
                Err(error) => {
                    log::error!("Failed to check token namespace creation grant: {error}");
                    return json_error(ApiError::Internal);
                }
            }
        }
        if !can_create {
            return json_error(ApiError::Forbidden);
        }
    }
    match auth_state.db().get_namespace_by_name(name).await {
        Ok(Some(_)) => {
            return json_error(ApiError::Conflict("Namespace already exists".to_string()));
        }
        Ok(None) => {}
        Err(error) => {
            log::error!("Failed to check API namespace name: {error}");
            return json_error(ApiError::Internal);
        }
    }
    if let Err(error) =
        git::repo::ensure_namespace_directory(Path::new(server.project_root()), name)
    {
        log::error!("Failed to create API namespace directory: {error}");
        return json_error(ApiError::Internal);
    }
    let namespace = crate::auth::create_namespace(name.to_string(), principal.user.id);
    if let Err(error) = auth_state.db().create_namespace(&namespace).await {
        log::error!("Failed to create API namespace: {error}");
        return json_error(ApiError::Internal);
    }
    HttpResponse::Created().json(MutationResponse {
        resource: format!("/api/v1/namespaces/{name}"),
        message: "Namespace created".to_string(),
    })
}

#[get("/api/v1/namespaces/{namespace}")]
pub async fn get_namespace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<NamespacePath>,
) -> HttpResponse {
    let (_, namespace) = match namespace_for_action(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    match namespace_response(auth_state.db(), &namespace).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(error) => json_error(error),
    }
}

#[actix_web::patch("/api/v1/namespaces/{namespace}")]
pub async fn rename_namespace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<NamespacePath>,
    body: web::Json<RenameNamespaceRequest>,
) -> HttpResponse {
    let (_, namespace) = match namespace_for_action(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        NamespacePermission::ManageNamespace,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    let new_name = body.name.trim();
    if new_name != body.name {
        return json_error(ApiError::BadRequest("Invalid namespace name".to_string()));
    }
    if let Err(message) = git::reserved::validate_namespace_name(new_name) {
        return json_error(ApiError::BadRequest(message));
    }
    if new_name == namespace.name {
        return json_error(ApiError::BadRequest(
            "Choose a different namespace name".to_string(),
        ));
    }
    match auth_state.db().get_namespace_by_name(new_name).await {
        Ok(Some(_)) => {
            return json_error(ApiError::Conflict("Namespace already exists".to_string()));
        }
        Ok(None) => {}
        Err(error) => {
            log::error!("Failed to check API namespace rename target: {error}");
            return json_error(ApiError::Internal);
        }
    }
    let root = Path::new(server.project_root());
    let source = match git::repo::checked_namespace_path(root, &namespace.name) {
        Ok(path) => path,
        Err(error) => {
            log::error!("Invalid source namespace path during API rename: {error}");
            return json_error(ApiError::BadRequest("Invalid namespace path".to_string()));
        }
    };
    let destination = match git::repo::checked_namespace_path(root, new_name) {
        Ok(path) => path,
        Err(error) => {
            log::error!("Invalid destination namespace path during API rename: {error}");
            return json_error(ApiError::BadRequest("Invalid namespace path".to_string()));
        }
    };
    if destination.exists() {
        return json_error(ApiError::Conflict(
            "Namespace directory already exists".to_string(),
        ));
    }
    let moved = source.exists();
    if moved && let Err(error) = std::fs::rename(&source, &destination) {
        log::error!("Failed to rename API namespace directory: {error}");
        return json_error(ApiError::Internal);
    }
    if let Err(error) = auth_state
        .db()
        .rename_namespace(&namespace.id, new_name)
        .await
    {
        log::error!("Failed to rename API namespace: {error}");
        if moved {
            let _ = std::fs::rename(&destination, &source);
        }
        return json_error(ApiError::Internal);
    }
    HttpResponse::Ok().json(MutationResponse {
        resource: format!("/api/v1/namespaces/{new_name}"),
        message: "Namespace renamed".to_string(),
    })
}

#[delete("/api/v1/namespaces/{namespace}")]
pub async fn delete_namespace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<NamespacePath>,
) -> HttpResponse {
    let (_, namespace) = match namespace_for_action(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        NamespacePermission::ManageNamespace,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    if git::bare::namespace::has_any_repository(server.project_root(), &namespace.name) {
        return json_error(ApiError::Conflict(
            "Delete all repositories before deleting the namespace".to_string(),
        ));
    }
    if let Err(error) = auth_state.db().delete_namespace(&namespace.id).await {
        log::error!("Failed to delete API namespace: {error}");
        return json_error(ApiError::Internal);
    }
    if let Ok(directory) =
        git::repo::checked_namespace_path(Path::new(server.project_root()), &namespace.name)
        && directory.is_dir()
    {
        let _ = std::fs::remove_dir(directory);
    }
    HttpResponse::Ok().json(MutationResponse {
        resource: format!("/api/v1/namespaces/{}", namespace.name),
        message: "Namespace deleted".to_string(),
    })
}

#[get("/api/v1/namespaces/{namespace}/repositories")]
pub async fn list_repositories(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<NamespacePath>,
    query: web::Query<RepositoryListQuery>,
) -> HttpResponse {
    if let Err(response) = namespace_for_action(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        return response;
    }
    let mut repositories = query
        .q
        .as_deref()
        .map_or_else(
            || git::bare::get_repos_with_info(server.project_root(), &path.namespace),
            |q| git::bare::search_repos_with_info(server.project_root(), &path.namespace, q),
        )
        .into_iter()
        .filter_map(|repo| {
            git::reserved::validate_repo_name(&repo.name)
                .ok()
                .map(|()| RepositorySummary {
                    namespace: path.namespace.clone(),
                    name: repo.name,
                    private: repo.is_private,
                    last_commit_at: repo.last_commit_date.map(|date| date.to_rfc3339()),
                })
        })
        .collect::<Vec<_>>();
    repositories.sort_by(|left, right| left.name.cmp(&right.name));
    HttpResponse::Ok().json(repositories)
}

#[post("/api/v1/namespaces/{namespace}/repositories")]
pub async fn create_repository(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<NamespacePath>,
    body: web::Json<CreateRepositoryRequest>,
) -> HttpResponse {
    let (principal, _) = match namespace_for_action(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        NamespacePermission::CreateRepositories,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    let name = body.name.trim();
    if name != body.name {
        return json_error(ApiError::BadRequest("Invalid repository name".to_string()));
    }
    if let Err(message) = git::reserved::validate_repo_name(name) {
        return json_error(ApiError::BadRequest(message));
    }
    if !git2::Reference::is_valid_name(&format!("refs/heads/{}", body.default_branch)) {
        return json_error(ApiError::BadRequest(
            "Invalid default branch name".to_string(),
        ));
    }
    let user = match auth_state.db().get_user_by_id(&principal.user.id).await {
        Ok(Some(user)) => user,
        Ok(None) => return json_error(ApiError::Unauthorized),
        Err(error) => {
            log::error!("Failed to load API repository creator: {error}");
            return json_error(ApiError::Internal);
        }
    };
    let Some(email) = user.email.as_deref() else {
        return json_error(ApiError::BadRequest(
            "Set an email address before creating repositories".to_string(),
        ));
    };
    let namespace_dir = match git::repo::ensure_namespace_directory(
        Path::new(server.project_root()),
        &path.namespace,
    ) {
        Ok(directory) => directory,
        Err(error) => {
            log::error!("Failed to prepare API repository namespace: {error}");
            return json_error(ApiError::Internal);
        }
    };
    let repo_path = namespace_dir.join(name);
    if repo_path.exists() {
        return json_error(ApiError::Conflict("Repository already exists".to_string()));
    }
    if let Err(error) = std::fs::create_dir(&repo_path) {
        log::error!("Failed to create API repository directory: {error}");
        return json_error(ApiError::Internal);
    }
    if let Err(error) =
        git::repo::bare_init(&repo_path, &body.default_branch, &user.username, email)
    {
        log::error!("Failed to initialize API repository: {error}");
        let _ = std::fs::remove_dir_all(&repo_path);
        return json_error(ApiError::Internal);
    }
    HttpResponse::Created().json(MutationResponse {
        resource: format!("/api/v1/namespaces/{}/repositories/{name}", path.namespace),
        message: "Repository created".to_string(),
    })
}

#[get("/api/v1/namespaces/{namespace}/members")]
pub async fn list_members(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<NamespacePath>,
) -> HttpResponse {
    let (_, namespace) = match namespace_for_action(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        NamespacePermission::ManageMembers,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    let ids = [namespace.id];
    match auth_state.db().list_users_for_management(Some(&ids)).await {
        Ok(users) => {
            let members = users
                .into_iter()
                .filter_map(|user| {
                    user.memberships
                        .into_iter()
                        .find(|(name, _)| name == &path.namespace)
                        .map(|(_, role)| MemberResponse {
                            username: user.username,
                            email: user.email,
                            role: role.as_str().to_string(),
                        })
                })
                .collect::<Vec<_>>();
            HttpResponse::Ok().json(members)
        }
        Err(error) => {
            log::error!("Failed to list API namespace members: {error}");
            json_error(ApiError::Internal)
        }
    }
}

fn invitation_summary(invitation: &crate::auth::NamespaceInvitation) -> InvitationSummary {
    InvitationSummary {
        email: invitation.email.clone(),
        namespace: invitation.namespace_name.clone(),
        role: invitation.role.as_str().to_string(),
        created_at: invitation.created_at.clone(),
        expires_at: invitation.expires_at.clone(),
        accepted_at: invitation.accepted_at.clone(),
    }
}

#[get("/api/v1/namespaces/{namespace}/invitations")]
pub async fn list_invitations(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<NamespacePath>,
) -> HttpResponse {
    let (_, namespace) = match namespace_for_action(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        NamespacePermission::ManageMembers,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    match auth_state
        .db()
        .list_namespace_invitations(Some(&[namespace.id]))
        .await
    {
        Ok(invitations) => HttpResponse::Ok().json(
            invitations
                .iter()
                .map(invitation_summary)
                .collect::<Vec<_>>(),
        ),
        Err(error) => {
            log::error!("Failed to list API invitations: {error}");
            json_error(ApiError::Internal)
        }
    }
}

fn parse_invitation_expiry(value: &str) -> Result<Option<String>, ApiError> {
    let days = match value {
        "1d" => Some(1),
        "7d" => Some(7),
        "30d" => Some(30),
        "never" => None,
        _ => {
            return Err(ApiError::BadRequest(
                "Expiry must be 1d, 7d, 30d, or never".to_string(),
            ));
        }
    };
    Ok(days.map(|days| (chrono::Utc::now() + chrono::Duration::days(days)).to_rfc3339()))
}

fn invitation_url_for_request(req: &HttpRequest, server: &config::Server, token: &str) -> String {
    if let Some(base_url) = server.public_base_url()
        && let Ok(url) = invitation_url(base_url, token)
    {
        return url;
    }
    let connection = req.connection_info();
    format!(
        "{}://{}/auth/accept-invite/{token}",
        connection.scheme(),
        connection.host()
    )
}

async fn send_invitation(
    server: &config::Server,
    invitation: &crate::auth::NamespaceInvitation,
) -> &'static str {
    match send_invitation_if_configured(
        server.resend_api_key(),
        server.resend_from(),
        server.public_base_url(),
        &invitation.email,
        &invitation.namespace_name,
        invitation.role,
        &invitation.token,
    )
    .await
    {
        InvitationDelivery::Sent => "sent",
        InvitationDelivery::SkippedNoApiKey => "not_configured",
        InvitationDelivery::Failed(error) => {
            log::warn!("Failed to send API namespace invitation: {error}");
            "failed"
        }
    }
}

#[post("/api/v1/namespaces/{namespace}/invitations")]
pub async fn create_invitation(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<NamespacePath>,
    body: web::Json<CreateInvitationRequest>,
) -> HttpResponse {
    let (_, namespace) = match namespace_for_action(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        NamespacePermission::ManageMembers,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    let email = body.email.trim();
    if email.is_empty()
        || !email.contains('@')
        || email
            .chars()
            .any(|character| matches!(character, '\r' | '\n'))
    {
        return json_error(ApiError::BadRequest(
            "Enter a valid email address".to_string(),
        ));
    }
    let Some(role) = NamespaceRole::parse(&body.role) else {
        return json_error(ApiError::BadRequest(
            "Unsupported namespace role".to_string(),
        ));
    };
    let expires_at = match parse_invitation_expiry(&body.expires) {
        Ok(expires_at) => expires_at,
        Err(error) => return json_error(error),
    };
    let invitation = match auth_state
        .db()
        .create_namespace_invitation(&NewNamespaceInvitation {
            email: email.to_string(),
            namespace_id: namespace.id,
            role,
            expires_at,
        })
        .await
    {
        Ok(invitation) => invitation,
        Err(error) => {
            log::error!("Failed to create API invitation: {error}");
            return json_error(ApiError::Internal);
        }
    };
    let invite_url = invitation_url_for_request(&req, &server, &invitation.token);
    let email_delivery = send_invitation(&server, &invitation).await;
    HttpResponse::Created().json(InvitationCreatedResponse {
        invitation: invitation_summary(&invitation),
        invite_url,
        email_delivery,
    })
}

fn invitation_is_expired(invitation: &crate::auth::NamespaceInvitation) -> bool {
    invitation.expires_at.as_deref().is_some_and(|expires_at| {
        chrono::DateTime::parse_from_rfc3339(expires_at)
            .is_ok_and(|expires_at| expires_at <= chrono::Utc::now())
    })
}

#[post("/api/v1/namespaces/{namespace}/invitations/{id}/resend")]
pub async fn resend_invitation(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<InvitationPath>,
) -> HttpResponse {
    let (_, namespace) = match namespace_for_action(
        &req,
        &server,
        &auth_state,
        &path.namespace,
        NamespacePermission::ManageMembers,
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    let invitation = match auth_state.db().get_namespace_invitation(&path.id).await {
        Ok(Some(invitation)) => invitation,
        Ok(None) => return json_error(ApiError::NotFound),
        Err(error) => {
            log::error!("Failed to load API invitation: {error}");
            return json_error(ApiError::Internal);
        }
    };
    if invitation.namespace_id != namespace.id {
        return json_error(ApiError::NotFound);
    }
    if invitation.accepted_at.is_some() || invitation.accepted_user_id.is_some() {
        return json_error(ApiError::Gone(
            "Invitation has already been accepted".to_string(),
        ));
    }
    if invitation_is_expired(&invitation) {
        return json_error(ApiError::Gone("Invitation has expired".to_string()));
    }
    let invite_url = invitation_url_for_request(&req, &server, &invitation.token);
    let email_delivery = send_invitation(&server, &invitation).await;
    HttpResponse::Ok().json(InvitationCreatedResponse {
        invitation: invitation_summary(&invitation),
        invite_url,
        email_delivery,
    })
}
