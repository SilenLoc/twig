use super::*;

fn scope() -> UserManagementScope {
    UserManagementScope {
        user: User {
            id: "user-1".to_string(),
            username: "admin".to_string(),
            email: Some("admin@example.com".to_string()),
            password_hash: "hash".to_string(),
            created_at: "2026-10-10T00:00:00Z".to_string(),
        },
        is_site_admin: true,
        namespaces: vec![Namespace {
            id: "namespace-1".to_string(),
            name: "acme".to_string(),
            owner_id: "user-1".to_string(),
            created_at: "2026-10-10T00:00:00Z".to_string(),
        }],
    }
}

fn invitation() -> NamespaceInvitation {
    NamespaceInvitation {
        token: "f".repeat(64),
        email: "invitee@example.com".to_string(),
        namespace_id: "namespace-1".to_string(),
        namespace_name: "acme".to_string(),
        role: NamespaceRole::Contributor,
        created_at: "2026-10-10T00:00:00Z".to_string(),
        expires_at: Some((chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339()),
        accepted_at: None,
        accepted_user_id: None,
    }
}

#[test]
fn invite_tab_renders_htmx_form_with_contributor_default_and_expiry() {
    let markup = render_user_management(&scope(), "invites", &[], &[]).into_string();
    for expected in [
        "hx-post=\"/tree/invites\"",
        "name=\"email\"",
        "name=\"namespace\"",
        "value=\"contributor\" selected",
        "value=\"7d\" selected",
        "Never",
        "hx-target=\"#invite-result\"",
        "No invitations yet.",
        "Create invitation",
    ] {
        assert!(markup.contains(expected), "missing {expected}: {markup}");
    }
}

#[test]
fn users_tab_is_reachable_with_htmx_and_renders_scoped_placeholder() {
    let markup = render_user_management(&scope(), "users", &[], &[]).into_string();
    assert!(markup.contains("hx-get=\"/tree/users?tab=users\""));
    assert!(markup.contains("No users found."));
}

#[test]
fn user_memberships_render_in_a_single_table_cell() {
    let user = UserManagementEntry {
        username: "silen".to_string(),
        email: Some("silen@example.com".to_string()),
        memberships: vec![
            ("experiments".to_string(), NamespaceRole::Owner),
            ("hco".to_string(), NamespaceRole::Owner),
        ],
    };

    let markup = render_user_management(&scope(), "users", &[], &[user]).into_string();

    assert!(markup.contains("experiments · Owner, hco · Owner"));
    assert_eq!(markup.matches("class=\"twig-row-meta\"").count(), 2);
}

#[test]
fn expiry_values_map_to_expected_expiration() {
    let now = chrono::Utc::now();
    assert!(parse_expiry("never", now).unwrap().is_none());
    for (value, days) in [("1d", 1), ("7d", 7), ("30d", 30)] {
        let expiry = parse_expiry(value, now).unwrap().unwrap();
        let expiry = chrono::DateTime::parse_from_rfc3339(&expiry).unwrap();
        assert_eq!(
            expiry.timestamp(),
            (now + chrono::Duration::days(days)).timestamp()
        );
    }
    assert!(parse_expiry("forever", now).is_err());
}

#[test]
fn pending_invitation_lists_have_an_htmx_resend_action() {
    let invitation = invitation();
    let markup = render_invitation_list(&[invitation], false).into_string();
    assert!(markup.contains("Pending"));
    assert!(markup.contains("hx-post=\"/tree/invites/"));
    assert!(markup.contains("/resend\""));
    assert!(markup.contains("aria-label=\"Resend invitation to invitee@example.com\""));
}

#[test]
fn delivery_notices_preserve_link_and_distinguish_sent_skipped_and_failed() {
    let invitation = invitation();
    let url = format!(
        "https://twig.example/auth/accept-invite/{}",
        invitation.token
    );
    for (delivery, expected) in [
        (InvitationDelivery::Sent, "EMAIL SENT"),
        (
            InvitationDelivery::SkippedNoApiKey,
            "RESEND_API_KEY is not set",
        ),
        (
            InvitationDelivery::Failed("network error".to_string()),
            "Retry delivery or share this link directly",
        ),
    ] {
        let markup = render_invitation_created(&invitation, &url, &[invitation.clone()], &delivery)
            .into_string();
        assert!(markup.contains(expected), "missing {expected}: {markup}");
        assert!(markup.contains(&url));
        assert!(markup.contains("hx-swap-oob=\"outerHTML\""));
    }
}
