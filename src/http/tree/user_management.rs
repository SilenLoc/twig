use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UserManagementQuery {
    tab: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvitationForm {
    email: String,
    namespace: String,
    #[serde(default = "default_invitation_role")]
    role: String,
    expires: String,
}

fn default_invitation_role() -> String {
    "contributor".to_string()
}

mod access;
mod invitations;
mod views;

#[cfg(test)]
use access::UserManagementScope;
pub use access::user_management_page;
#[cfg(test)]
use invitations::parse_expiry;
pub use invitations::{create_namespace_invitation, resend_namespace_invitation};
#[cfg(test)]
use views::{
    render_invitation_created, render_invitation_list, render_user_management,
    render_user_management_content, render_user_management_htmx,
};

#[cfg(test)]
#[path = "user_management/tests.rs"]
mod tests;
