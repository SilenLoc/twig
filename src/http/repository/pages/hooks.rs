use actix_web::{HttpRequest, HttpResponse, get, post, web};
use maud::Markup;
use serde::Deserialize;

use super::Params;
use super::context::{RepoContext, can_manage_hooks, open_repo, render_empty};
use super::tabs::render_tab_response;
use crate::auth::TwigContext;
use crate::config;
use twig_git::hooks::{
    CustomHookInfo, CustomHookState, HookError, HookInventory, HookRun, HookRunStatus,
    ManagedHookState, delete_custom_hook, disable_custom_hook, inspect_repository_hooks,
};

#[derive(Deserialize)]
struct HookParams {
    namespace: String,
    repo: String,
    hook: String,
}

#[derive(Deserialize)]
struct DeleteHookConfirmation {
    confirm: Option<String>,
}

#[derive(Deserialize)]
struct HookNotice {
    notice: Option<String>,
}

#[get("/{namespace}/{repo}/hooks")]
pub async fn hooks_tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
    query: web::Query<HookNotice>,
) -> HttpResponse {
    let (namespace, repo) = (params.namespace.as_str(), params.repo.as_str());
    let ctx = match open_repo(&req, &server, &auth_state, namespace, repo).await {
        Ok(ctx) => ctx,
        Err(markup) => return html_response(HttpResponse::Ok(), markup),
    };
    if !ctx.can_manage_hooks {
        return HttpResponse::Forbidden().body("Manage repositories permission required");
    }

    let inventory = match inspect_repository_hooks(ctx.handle.path()) {
        Ok(inventory) => inventory,
        Err(error) => {
            log::error!("Failed to inspect hooks for {namespace}/{repo}: {error}");
            return html_response(
                HttpResponse::InternalServerError(),
                render_empty(
                    "HOOK INSPECTION FAILED",
                    "The repository hook files could not be inspected safely.",
                ),
            );
        }
    };
    let frame = ctx.frame();
    let notice = match query.notice.as_deref() {
        Some("disabled") => Some("Custom hook disabled."),
        Some("deleted") => Some("Custom hook deleted."),
        _ => None,
    };
    let body = render_hooks_view(namespace, repo, &inventory, notice);
    let page = render_tab_response(&req, &frame, "hooks", &body);
    html_response(HttpResponse::Ok(), page)
}

#[post("/{namespace}/{repo}/hooks/{hook}/disable")]
pub async fn disable_hook_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<HookParams>,
) -> HttpResponse {
    if let Err(response) = require_hook_manager(&req, &server, &auth_state, &params.namespace).await
    {
        return response;
    }
    let context = match open_managed_repo(&req, &server, &auth_state, &params).await {
        Ok(context) => context,
        Err(response) => return response,
    };
    match disable_custom_hook(context.handle.path(), &params.hook) {
        Ok(()) => hook_redirect(&params.namespace, &params.repo, "disabled"),
        Err(error) => hook_error_response(error),
    }
}

#[post("/{namespace}/{repo}/hooks/{hook}/delete")]
pub async fn delete_hook_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<HookParams>,
    confirmation: web::Form<DeleteHookConfirmation>,
) -> HttpResponse {
    if let Err(response) = require_hook_manager(&req, &server, &auth_state, &params.namespace).await
    {
        return response;
    }
    if confirmation.confirm.as_deref() != Some("yes") {
        return HttpResponse::BadRequest().body("Confirm permanent hook deletion first");
    }
    let context = match open_managed_repo(&req, &server, &auth_state, &params).await {
        Ok(context) => context,
        Err(response) => return response,
    };
    match delete_custom_hook(context.handle.path(), &params.hook) {
        Ok(()) => hook_redirect(&params.namespace, &params.repo, "deleted"),
        Err(error) => hook_error_response(error),
    }
}

async fn require_hook_manager(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
) -> Result<(), HttpResponse> {
    match can_manage_hooks(req, server, auth_state, namespace).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(HttpResponse::Forbidden().body("Manage repositories permission required")),
        Err(error) => {
            log::error!("Failed to verify hook-management permission: {error}");
            Err(HttpResponse::InternalServerError().body("Failed to verify repository permissions"))
        }
    }
}

async fn open_managed_repo(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    params: &HookParams,
) -> Result<RepoContext, HttpResponse> {
    open_repo(req, server, auth_state, &params.namespace, &params.repo)
        .await
        .map_err(|markup| html_response(HttpResponse::NotFound(), markup))
}

fn hook_redirect(namespace: &str, repo: &str, notice: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((
            actix_web::http::header::LOCATION,
            format!("/{namespace}/{repo}/hooks?notice={notice}"),
        ))
        .finish()
}

fn hook_error_response(error: HookError) -> HttpResponse {
    match error {
        HookError::InvalidName => HttpResponse::BadRequest().body("Invalid custom hook name"),
        HookError::NotFound => HttpResponse::NotFound().body("Custom hook not found"),
        HookError::UnsafePath => {
            HttpResponse::Conflict().body("Custom hook path is not a safe regular file")
        }
        HookError::Unsupported => {
            HttpResponse::NotImplemented().body("Hook operation is not supported on this platform")
        }
        HookError::Io(error) => {
            log::error!("Failed to update custom hook: {error}");
            HttpResponse::InternalServerError().body("Failed to update custom hook")
        }
    }
}

pub(super) fn render_hooks_view(
    namespace: &str,
    repo: &str,
    inventory: &HookInventory,
    notice: Option<&str>,
) -> Markup {
    maud::html! {
        div class="twig-stack" {
            div class="twig-stack" {
                h2 class="twig-section" { "Git server hooks" }
                p class="twig-hint" {
                    "Repository: " code class="twig-data" { (format!("{namespace}/{repo}")) }
                }
            }

            @if let Some(notice) = notice {
                div class="twig-notice twig-notice--success" role="status" {
                    p class="twig-notice-body" { (notice) }
                }
            }

            section class="twig-stack" {
                h3 class="twig-section" { "Managed receive policy" }
                @match inventory.managed_state {
                    ManagedHookState::Healthy => {
                        div class="twig-notice twig-notice--success" role="status" {
                            p class="twig-eyebrow" { "HEALTHY" }
                            p class="twig-notice-body" {
                                "Twig's managed pre-receive policy hook is installed."
                            }
                        }
                    }
                    ManagedHookState::Missing => {
                        div class="twig-notice twig-notice--warning" role="alert" {
                            p class="twig-eyebrow" { "MISSING" }
                            p class="twig-notice-body" {
                                "Twig's managed pre-receive policy hook is not installed."
                            }
                        }
                    }
                    ManagedHookState::Invalid => {
                        div class="twig-notice twig-notice--danger" role="alert" {
                            p class="twig-eyebrow" { "INVALID" }
                            p class="twig-notice-body" {
                                "The managed pre-receive policy hook is not a valid executable file."
                            }
                        }
                    }
                }
            }

            section class="twig-stack" {
                h3 class="twig-section" { "Custom pre-receive hooks" }
                p class="twig-hint" {
                    "These existing hooks run after Twig's receive policy. Hook scripts execute with server privileges."
                }
                @if inventory.custom_hooks.is_empty() {
                    (render_empty(
                        "NO CUSTOM HOOKS",
                        "No repository-specific pre-receive hooks were found."
                    ))
                } @else {
                    div class="twig-table-scroll" {
                        table class="twig-table" {
                            thead {
                                tr {
                                    th { "Hook" }
                                    th { "State" }
                                    th { "Source" }
                                    th { "Actions" }
                                }
                            }
                            tbody {
                                @for hook in &inventory.custom_hooks {
                                    (render_custom_hook(namespace, repo, hook))
                                }
                            }
                        }
                    }
                }
            }

            @for warning in &inventory.warnings {
                div class="twig-notice twig-notice--warning" role="alert" {
                    p class="twig-eyebrow" { "HOOK WARNING" }
                    p class="twig-notice-body" { (warning) }
                }
            }

            section class="twig-stack" {
                h3 class="twig-section" { "Recent custom hook runs" }
                @if inventory.recent_runs.is_empty() {
                    (render_empty(
                        "NO RECORDED RUNS",
                        "Enabled custom hooks will appear here after their next push."
                    ))
                } @else {
                    div class="twig-stack" {
                        @for run in &inventory.recent_runs {
                            (render_hook_run(run))
                        }
                    }
                }
            }
        }
    }
}

fn render_hook_run(run: &HookRun) -> Markup {
    let (status_label, status_class) = match run.status {
        HookRunStatus::Passed => ("PASSED", "twig-notice--success"),
        HookRunStatus::Failed => ("FAILED", "twig-notice--danger"),
    };
    maud::html! {
        article class=(format!("twig-notice {status_class}")) {
            div class="twig-stack--tight" {
                p class="twig-eyebrow" { (status_label) }
                p class="twig-notice-body" {
                    code class="twig-data" { (run.hook_name) }
                    " · pre-receive · " (run.started_at)
                    @if let Some(actor) = &run.actor {
                        " · by " (actor)
                    }
                }
                @if !run.ref_updates.is_empty() {
                    p class="twig-hint" {
                        "Ref updates: "
                        @for (index, update) in run.ref_updates.iter().enumerate() {
                            @if index > 0 { "; " }
                            code class="twig-data" { (update) }
                        }
                    }
                }
                details {
                    summary class="twig-hint" { "View hook output" }
                    @if run.output_truncated {
                        p class="twig-hint" { "Output was truncated to the diagnostic size limit." }
                    }
                    @if !run.stdout.is_empty() {
                        h4 class="twig-eyebrow" { "STANDARD OUTPUT" }
                        pre class="twig-code" tabindex="0" aria-label=(format!("{} standard output", run.hook_name)) {
                            code { (run.stdout) }
                        }
                    }
                    @if !run.stderr.is_empty() {
                        h4 class="twig-eyebrow" { "STANDARD ERROR" }
                        pre class="twig-code" tabindex="0" aria-label=(format!("{} standard error", run.hook_name)) {
                            code { (run.stderr) }
                        }
                    }
                    @if run.stdout.is_empty() && run.stderr.is_empty() {
                        p class="twig-hint" { "The hook produced no output." }
                    }
                }
            }
        }
    }
}

fn render_custom_hook(namespace: &str, repo: &str, hook: &CustomHookInfo) -> Markup {
    let state = match hook.state {
        CustomHookState::Enabled => "Enabled",
        CustomHookState::Disabled => "Disabled",
        CustomHookState::Unsafe => "Unsafe — inspect on the server",
    };
    maud::html! {
        tr {
            td { code class="twig-data" { (hook.name) } }
            td { (state) }
            td {
                @if let Some(source) = &hook.source {
                    details {
                        summary class="twig-hint" { "View source" }
                        pre class="twig-code" tabindex="0" aria-label=(format!("{} source", hook.name)) {
                            code { (source) }
                        }
                    }
                } @else {
                    span { "Source not read" }
                }
            }
            td {
                @if hook.state == CustomHookState::Enabled {
                    form method="post" action=(format!("/{namespace}/{repo}/hooks/{}/disable", hook.name)) {
                        button class="twig-btn twig-btn--quiet" type="submit" { "Disable" }
                    }
                }
                @if hook.state != CustomHookState::Unsafe {
                    form
                        method="post"
                        action=(format!("/{namespace}/{repo}/hooks/{}/delete", hook.name))
                        onsubmit="return confirm('Delete this hook permanently? This cannot be undone.')"
                    {
                        input type="hidden" name="confirm" value="yes";
                        button class="twig-btn twig-btn--danger" type="submit" { "Delete hook" }
                    }
                }
            }
        }
    }
}

fn html_response(mut builder: actix_web::HttpResponseBuilder, markup: Markup) -> HttpResponse {
    builder
        .content_type("text/html; charset=utf-8")
        .body(markup.into_string())
}
