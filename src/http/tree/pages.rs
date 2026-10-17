use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DataQuery {
    table: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RowsQuery {
    table: String,
    offset: usize,
}

struct NamespacePageData {
    user_management_enabled: bool,
    repository_namespace_names: Vec<String>,
    manageable_namespaces: Vec<String>,
    deletable_namespaces: Vec<String>,
    repos_by_namespace: Vec<(String, Vec<crate::git::bare::RepoInfo>)>,
}

mod data;
mod hub;
mod namespaces;
mod repositories;

use data::{DATA_PAGE_SIZE, render_data_page};
pub use data::{data_page, data_rows};
pub(crate) use hub::{render_tree_hub, tree_page};
pub use namespaces::namespaces_page;
use namespaces::{render_namespace_rename_panel, render_namespaces_page};
pub use repositories::repositories_page;

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
