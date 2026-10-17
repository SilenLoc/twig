use serde::Deserialize;

#[derive(Deserialize)]
struct Params {
    namespace: String,
    repo: String,
}

#[derive(Deserialize)]
struct TabParams {
    namespace: String,
    repo: String,
    tab: String,
}

#[derive(Deserialize)]
struct MarkdownParams {
    namespace: String,
    repo: String,
    file_path: String,
}

#[derive(Deserialize)]
struct SlideParams {
    namespace: String,
    repo: String,
    index: usize,
}

#[derive(Deserialize)]
struct ContentParams {
    namespace: String,
    repo: String,
    path: String,
}

#[derive(Deserialize)]
struct ContentQuery {
    committed: Option<String>,
    conflict_copy: Option<bool>,
}

#[derive(Deserialize)]
struct PaperParams {
    namespace: String,
    repo: String,
    page: String,
}

#[derive(Deserialize)]
struct ScriptGroupParams {
    namespace: String,
    repo: String,
    /// Slash-joined group key, e.g. `linux/maintenance`.
    group: String,
}

#[derive(Deserialize)]
struct RawParams {
    namespace: String,
    repo: String,
    path: String,
}

mod content;
mod context;
mod markdown;
mod paper;
mod presentation;
mod scripts;
mod tab_handlers;
mod tabs;

#[cfg(test)]
use content::*;
pub use content::{content_handler, raw_handler};
#[cfg(test)]
use context::*;
pub use markdown::markdown_handler;
#[cfg(test)]
use markdown::*;
pub use paper::paper_handler;
#[cfg(test)]
use paper::*;
#[cfg(test)]
use presentation::*;
pub use presentation::{present_print_handler, slide_handler};
#[cfg(test)]
use scripts::*;
pub use tab_handlers::{
    commits_tab_handler, config_tab_handler, content_tab_handler, license_tab_handler,
    markdown_tab_handler, paper_tab_handler, present_tab_handler, scripts_group_handler,
    scripts_tab_handler, tab_handler,
};
pub use tabs::handler;
#[cfg(test)]
use tabs::*;

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
