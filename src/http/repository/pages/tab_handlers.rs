use actix_web::Result as AwResult;
use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse, get, web};
use maud::Markup;

use crate::auth::TwigContext;
use crate::config;

use super::tabs::{TAB_IDS, respond_tab};
use super::{Params, ScriptGroupParams, TabParams};

#[get("/{namespace}/{repo}/markdown")]
pub async fn markdown_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "markdown",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/content")]
pub async fn content_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "content",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/commits")]
pub async fn commits_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "commits",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/config")]
pub async fn config_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "config",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/present")]
pub async fn present_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "present",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/paper")]
pub async fn paper_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "paper",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/license")]
pub async fn license_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "license",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/scripts")]
pub async fn scripts_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "scripts",
        None,
    )
    .await
}

#[get("/{namespace}/{repo}/scripts/{group:.*}")]
pub async fn scripts_group_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<ScriptGroupParams>,
) -> AwResult<Markup> {
    respond_tab(
        req,
        server,
        auth_state,
        &params.namespace,
        &params.repo,
        "scripts",
        Some(params.group.trim_matches('/')),
    )
    .await
}

/// Serves a repository file as raw bytes so the Scripts tab's `curl` command
/// can fetch it. Paths ignored by `.twig.toml` stay unreachable here, and
/// private repositories need a session like every other read.
#[get("/{namespace}/{repo}/tab/{tab}")]
pub async fn tab_handler(params: web::Path<TabParams>) -> HttpResponse {
    let target = if TAB_IDS.contains(&params.tab.as_str()) {
        format!("/{}/{}/{}", params.namespace, params.repo, params.tab)
    } else {
        format!("/{}/{}", params.namespace, params.repo)
    };
    HttpResponse::PermanentRedirect()
        .insert_header((header::LOCATION, target))
        .finish()
}
