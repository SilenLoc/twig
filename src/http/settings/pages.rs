use serde::Deserialize;

#[derive(Deserialize)]
struct UpdateEmailForm {
    email: String,
}

#[derive(Deserialize)]
struct DeleteRepoForm {
    namespace: String,
    repo_name: String,
}

#[derive(Deserialize)]
struct MoveRepoForm {
    source_namespace: String,
    repo_name: String,
    target_namespace: String,
}

#[derive(Deserialize)]
struct RenameRepoForm {
    namespace: String,
    repo_name: String,
    new_name: String,
}

#[derive(Deserialize)]
struct RenameNamespaceForm {
    namespace: String,
    new_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamespaceForm {
    pub namespace: String,
}

mod namespaces;
mod profile;
mod repositories;
mod shared;

pub use namespaces::{delete_namespace, rename_namespace};
#[cfg(test)]
use profile::render_settings;
pub use profile::{settings_page, update_email};
pub use repositories::{delete_repo, move_repo, rename_repo};
pub(crate) use repositories::{render_repo_deletion_panel, render_repo_rename_panel};

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
