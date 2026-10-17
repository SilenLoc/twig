use crate::{auth, config, db::Database, http::repository::pages as repo_pages};
use actix_http::Request;
use actix_web::{App, http::StatusCode, test, web};

mod support;
use support::*;
mod admin;
#[path = "auth.rs"]
mod auth_flows;
mod namespace;
mod repository_auth;
mod repository_binaries;
mod repository_content;
mod repository_editor;
mod repository_tabs;
mod test_page;
