use actix_web::{HttpRequest, HttpResponse, post, web};

use crate::auth::{NamespaceInvitation, NamespaceRole, NewNamespaceInvitation, TwigContext};
use crate::config;
use crate::email::{InvitationDelivery, invitation_url, send_invitation_if_configured};
use crate::http::view::render_error;

use super::access::{UserManagementScope, user_management_scope};
use super::views::render_invitation_created;
use super::*;

pub(super) fn is_expired(
    invitation: &NamespaceInvitation,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    invitation.expires_at.as_deref().is_some_and(|expires_at| {
        chrono::DateTime::parse_from_rfc3339(expires_at).is_ok_and(|expires_at| expires_at <= now)
    })
}

pub(super) fn format_expiry(expires_at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(expires_at).map_or_else(
        |_| "unknown".to_string(),
        |date| date.format("%Y-%m-%d %H:%M UTC").to_string(),
    )
}

pub(super) fn invitations_for_scope(scope: &UserManagementScope) -> Option<Vec<String>> {
    (!scope.is_site_admin).then(|| {
        scope
            .namespaces
            .iter()
            .map(|namespace| namespace.id.clone())
            .collect()
    })
}

pub(super) fn parse_expiry(
    value: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Option<String>, ()> {
    let days = match value {
        "1d" => 1,
        "7d" => 7,
        "30d" => 30,
        "never" => return Ok(None),
        _ => return Err(()),
    };
    Ok(Some((now + chrono::Duration::days(days)).to_rfc3339()))
}

fn display_invite_url(req: &HttpRequest, server: &config::Server, token: &str) -> String {
    if let Some(public_base_url) = server.public_base_url()
        && let Ok(url) = invitation_url(public_base_url, token)
    {
        return url;
    }
    let connection_info = req.connection_info();
    format!(
        "{}://{}/auth/accept-invite/{token}",
        connection_info.scheme(),
        connection_info.host(),
    )
}

async fn deliver_invitation(
    server: &config::Server,
    invitation: &NamespaceInvitation,
) -> InvitationDelivery {
    send_invitation_if_configured(
        server.resend_api_key(),
        server.resend_from(),
        server.public_base_url(),
        &invitation.email,
        &invitation.namespace_name,
        invitation.role,
        &invitation.token,
    )
    .await
}

#[post("/tree/invites")]
pub async fn create_namespace_invitation(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<CreateInvitationForm>,
) -> HttpResponse {
    let scope = match user_management_scope(&req, &server, &auth_state).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let email = form.email.trim();
    if email.is_empty() || !email.contains('@') {
        return HttpResponse::BadRequest()
            .body(render_error("Enter a valid email address").into_string());
    }
    if !scope
        .namespaces
        .iter()
        .any(|namespace| namespace.name == form.namespace)
    {
        return HttpResponse::Forbidden()
            .body(render_error("You cannot invite people to that namespace").into_string());
    }
    let Some(role) = NamespaceRole::parse(&form.role) else {
        return HttpResponse::BadRequest()
            .body(render_error("Choose a supported role").into_string());
    };
    let Ok(expires_at) = parse_expiry(&form.expires, chrono::Utc::now()) else {
        return HttpResponse::BadRequest()
            .body(render_error("Choose a supported invitation expiry").into_string());
    };
    let namespace = scope
        .namespaces
        .iter()
        .find(|namespace| namespace.name == form.namespace)
        .expect("namespace was checked against the authorized scope");
    let invitation = match auth_state
        .db()
        .create_namespace_invitation(&NewNamespaceInvitation {
            email: email.to_string(),
            namespace_id: namespace.id.clone(),
            role,
            expires_at,
        })
        .await
    {
        Ok(invitation) => invitation,
        Err(error) => {
            log::error!("Failed to create namespace invitation: {error}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to create invitation").into_string());
        }
    };
    let invite_url = display_invite_url(&req, &server, &invitation.token);
    let delivery = deliver_invitation(&server, &invitation).await;
    if let InvitationDelivery::Failed(error) = &delivery {
        log::warn!(
            "Resend delivery failed for namespace {}: {error}",
            invitation.namespace_name
        );
    }
    let namespace_ids = invitations_for_scope(&scope);
    let invitations = match auth_state
        .db()
        .list_namespace_invitations(namespace_ids.as_deref())
        .await
    {
        Ok(invitations) => invitations,
        Err(error) => {
            log::error!("Failed to refresh namespace invitations: {error}");
            return HttpResponse::InternalServerError().body(
                render_error("Invitation created, but the list could not be refreshed")
                    .into_string(),
            );
        }
    };

    HttpResponse::Ok().content_type("text/html").body(
        render_invitation_created(&invitation, &invite_url, &invitations, &delivery).into_string(),
    )
}

#[post("/tree/invites/{id}/resend")]
pub async fn resend_namespace_invitation(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<String>,
) -> HttpResponse {
    let scope = match user_management_scope(&req, &server, &auth_state).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let invitation = match auth_state.db().get_namespace_invitation(&path).await {
        Ok(Some(invitation)) => invitation,
        Ok(None) => {
            return HttpResponse::NotFound()
                .body(render_error("Invitation not found").into_string());
        }
        Err(error) => {
            log::error!("Failed to load invitation for resend: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    if !scope
        .namespaces
        .iter()
        .any(|namespace| namespace.id == invitation.namespace_id)
    {
        return HttpResponse::Forbidden()
            .body(render_error("You cannot resend this invitation").into_string());
    }
    if invitation.accepted_at.is_some() || invitation.accepted_user_id.is_some() {
        return HttpResponse::Gone()
            .body(render_error("This invitation has already been accepted").into_string());
    }
    if is_expired(&invitation, chrono::Utc::now()) {
        return HttpResponse::Gone()
            .body(render_error("This invitation has expired").into_string());
    }

    let delivery = deliver_invitation(&server, &invitation).await;
    if let InvitationDelivery::Failed(error) = &delivery {
        log::warn!(
            "Resend delivery retry failed for namespace {}: {error}",
            invitation.namespace_name
        );
    }
    let invite_url = display_invite_url(&req, &server, &invitation.token);
    let namespace_ids = invitations_for_scope(&scope);
    let invitations = match auth_state
        .db()
        .list_namespace_invitations(namespace_ids.as_deref())
        .await
    {
        Ok(invitations) => invitations,
        Err(error) => {
            log::error!("Failed to refresh invitations after resend: {error}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().content_type("text/html").body(
        render_invitation_created(&invitation, &invite_url, &invitations, &delivery).into_string(),
    )
}
