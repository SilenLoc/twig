use super::{
    CreateInvitationForm, HttpRequest, HttpResponse, InvitationDelivery, Markup,
    NamespaceInvitation, NamespaceRole, NewNamespaceInvitation, TwigContext, UserManagementScope,
    config, html, invitation_url, post, render_error, send_invitation_if_configured,
    user_management_scope, web,
};

pub(super) fn render_invitation_list(invitations: &[NamespaceInvitation], oob: bool) -> Markup {
    let now = chrono::Utc::now();
    html! {
        section
            id="invitation-list"
            hx-swap-oob=[oob.then_some("outerHTML")]
            class="twig-panel"
            aria-labelledby="invitation-list-heading"
        {
            header class="twig-panel-head" {
                h2 class="twig-eyebrow" id="invitation-list-heading" { "Invitations" }
            }
            div class="twig-panel-body" {
                @if invitations.is_empty() {
                    p class="twig-empty-body" { "No invitations yet." }
                } @else {
                    div class="twig-list" {
                        @for invitation in invitations {
                            @let status = if invitation.accepted_at.is_some()
                                || invitation.accepted_user_id.is_some() {
                                "Accepted"
                            } else if is_expired(invitation, now) {
                                "Expired"
                            } else {
                                "Pending"
                            };
                            div class="twig-row" {
                                span class="twig-row-id" { (&invitation.email) }
                                span class="twig-row-meta" {
                                    (&invitation.namespace_name) " · " (invitation.role.display_name()) " · " (status)
                                    @if let Some(expires_at) = &invitation.expires_at {
                                        " · expires " (format_expiry(expires_at))
                                    } @else {
                                        " · never expires"
                                    }
                                }
                                @if status == "Pending" {
                                    form
                                        hx-post=(format!("/tree/invites/{}/resend", invitation.token))
                                        hx-target="#invite-result"
                                        hx-swap="innerHTML"
                                        "hx-status:4xx"="swap:innerHTML target:#invite-result"
                                        "hx-status:5xx"="swap:innerHTML target:#invite-result"
                                    {
                                        button
                                            class="twig-btn twig-btn--ghost"
                                            type="submit"
                                            aria-label=(format!("Resend invitation to {}", invitation.email))
                                        { "Resend email" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn is_expired(
    invitation: &NamespaceInvitation,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    invitation.expires_at.as_deref().is_some_and(|expires_at| {
        chrono::DateTime::parse_from_rfc3339(expires_at).is_ok_and(|expires_at| expires_at <= now)
    })
}

pub(super) fn format_expiry(expires_at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(expires_at)
        .map(|date| date.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|_| "unknown".to_string())
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

pub(super) fn render_invitation_created(
    invitation: &NamespaceInvitation,
    invite_url: &str,
    invitations: &[NamespaceInvitation],
    delivery: &InvitationDelivery,
) -> Markup {
    let (eyebrow, message, class, role) = match delivery {
        InvitationDelivery::Sent => (
            "EMAIL SENT",
            format!("Invitation email sent to {}.", invitation.email),
            "twig-notice--success",
            "status",
        ),
        InvitationDelivery::SkippedNoApiKey => (
            "INVITATION CREATED",
            format!(
                "No email was sent because RESEND_API_KEY is not set. Copy and share the link with {}.",
                invitation.email
            ),
            "twig-notice--warning",
            "status",
        ),
        InvitationDelivery::Failed(reason) => (
            "EMAIL NOT SENT",
            if reason.contains("PUBLIC_BASE_URL") {
                "Email was not sent: configure PUBLIC_BASE_URL before using Resend.".to_string()
            } else {
                "Email was not sent. Retry delivery or share this link directly.".to_string()
            },
            "twig-notice--warning",
            "alert",
        ),
    };
    html! {
        div class=(format!("twig-notice {class}")) role=(role) {
            p class="twig-eyebrow" { (eyebrow) }
            p class="twig-notice-body" { (message) }
            p class="twig-code" { (invite_url) }
            a class="twig-btn twig-btn--primary" href=(format!("/auth/accept-invite/{}", invitation.token)) {
                "Open account setup"
            }
        }
        (render_invitation_list(invitations, true))
    }
}

pub(super) fn display_invite_url(
    req: &HttpRequest,
    server: &config::Server,
    token: &str,
) -> String {
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

pub(super) async fn deliver_invitation(
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
    let expires_at = match parse_expiry(&form.expires, chrono::Utc::now()) {
        Ok(expires_at) => expires_at,
        Err(()) => {
            return HttpResponse::BadRequest()
                .body(render_error("Choose a supported invitation expiry").into_string());
        }
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
