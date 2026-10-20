#[cfg(test)]
mod tests {
    use crate::{auth, config, db::Database, http::repository::pages as repo_pages};
    use actix_http::Request;
    use actix_web::{App, http::StatusCode, test, web};

    #[path = "../admin_tree_settings.rs"]
    mod admin_tree_settings;
    #[path = "../auth_and_invitations.rs"]
    mod auth_and_invitations;
    #[path = "../device_authorization.rs"]
    mod device_authorization;
    #[path = "../namespace_permissions.rs"]
    mod namespace_permissions;
    #[path = "../personal_access_tokens.rs"]
    mod personal_access_tokens;
    #[path = "../repo_access_and_routes.rs"]
    mod repo_access_and_routes;
    #[path = "../repo_binaries.rs"]
    mod repo_binaries;
    #[path = "../repo_content.rs"]
    mod repo_content;
    #[path = "../repo_git_http.rs"]
    mod repo_git_http;
    #[path = "../repo_management.rs"]
    mod repo_management;
    #[path = "../repo_traversal.rs"]
    mod repo_traversal;
    #[path = "../repo_workspace_commit.rs"]
    mod repo_workspace_commit;
    #[path = "../repo_workspace_ui.rs"]
    mod repo_workspace_ui;
    #[path = "../support.rs"]
    mod support;
    #[path = "../test_page.rs"]
    mod test_page;
}
