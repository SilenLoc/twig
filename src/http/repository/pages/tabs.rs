use super::{
    AwResult, Commit, Deserialize, HttpRequest, HttpResponse, MAX_COMMITS, Markup, Params,
    RepoContext, TwigContext, config, get, header, open_repo, render_empty, render_for_request,
    render_git_error, render_tab_response, web,
};
use super::{content, markdown, paper, presentation, scripts};

#[derive(Deserialize)]
pub(super) struct TabParams {
    namespace: String,
    repo: String,
    tab: String,
}

pub(super) fn markdown_files(ctx: &RepoContext) -> Vec<String> {
    ctx.handle
        .list_files(Some(&ctx.twig_result.config))
        .unwrap_or_default()
        .markdown_files
}

pub(super) fn commits_body(ctx: &RepoContext) -> Result<Markup, twig_git::GitError> {
    let commits = ctx.handle.get_commits(MAX_COMMITS)?;
    Ok(render_commits_view(&commits))
}

pub(super) fn config_body(ctx: &RepoContext) -> Markup {
    render_config_view(
        ctx.twig_result.raw.as_deref(),
        ctx.twig_result.filename.as_deref(),
    )
}

pub(super) fn license_body(ctx: &RepoContext) -> Markup {
    let content = ctx.handle.get_license_content();
    render_license_view(Some(&content))
}

/// Renders a tab by name. Only the requested tab's data is loaded, so a Commits
/// request never reads markdown, slides, or paper pages. `group` selects a
/// Scripts sub-group and is ignored by every other tab.
pub(super) fn tab_body(
    ctx: &RepoContext,
    tab: &str,
    group: Option<&str>,
    releases: &[crate::db::binaries::BinaryRelease],
    binaries_unavailable: bool,
) -> Result<Markup, twig_git::GitError> {
    match tab {
        "content" => Ok(content::content_body(ctx)),
        "config" => Ok(config_body(ctx)),
        "present" => Ok(presentation::present_body(ctx)),
        "paper" => Ok(paper::paper_body(ctx)),
        "license" => Ok(license_body(ctx)),
        "scripts" => Ok(scripts::scripts_body(
            ctx,
            group,
            releases,
            binaries_unavailable,
        )),
        "commits" => commits_body(ctx),
        // Documentation, and the fallback for an unknown tab name.
        _ => Ok(markdown::markdown_body(ctx)),
    }
}

/// Shared shape of every per-tab handler: open, build the one body, respond.
/// `group` is the Scripts sub-group key; other tabs pass `None`.
pub(super) async fn respond_tab(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    namespace: &str,
    repo: &str,
    tab: &str,
    group: Option<&str>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, namespace, repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };
    let (releases, binaries_unavailable) = if tab == "scripts" {
        match auth_state.db().list_binary_releases(namespace, repo).await {
            Ok(releases) => (releases, false),
            Err(error) => {
                log::error!("Failed to load binaries for {namespace}/{repo}: {error}");
                (Vec::new(), true)
            }
        }
    } else {
        (Vec::new(), false)
    };
    let body = match tab_body(&ctx, tab, group, &releases, binaries_unavailable) {
        Ok(body) => body,
        Err(e) => {
            let content = render_git_error(&e);
            return Ok(render_for_request(
                &req,
                content,
                ctx.username.as_deref(),
                &ctx.page_title,
            ));
        }
    };
    Ok(render_tab_response(&req, &ctx.frame(), tab, &body))
}

/// Every tab a repository can show.
pub(super) const TAB_IDS: [&str; 8] = [
    "markdown", "paper", "content", "commits", "config", "present", "scripts", "license",
];

/// The tab the repository home shows: the first configured tab, else Markdown
/// when the repository has markdown, else Commits. A configured name that is
/// not a real tab falls back the same way.
pub(super) fn resolved_default_tab<'a>(
    tabs_config: &'a [String],
    markdown_files: &[String],
) -> &'a str {
    let tab = default_tab(tabs_config, markdown_files);
    if TAB_IDS.contains(&tab) {
        tab
    } else if markdown_files.is_empty() {
        "commits"
    } else {
        "markdown"
    }
}

#[get("/{namespace}/{repo}")]
pub async fn handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    let ctx = match open_repo(&req, &server, &auth_state, &params.namespace, &params.repo).await {
        Ok(ctx) => ctx,
        Err(rendered) => return Ok(rendered),
    };

    // The default tab needs the markdown list either way; reuse it when the
    // default turns out to be Documentation.
    let files = markdown_files(&ctx);
    let tab = resolved_default_tab(&ctx.twig_result.config.tabs, &files);
    let body = if tab == "markdown" {
        markdown::markdown_body_from(&ctx, &files)
    } else {
        let (releases, binaries_unavailable) = if tab == "scripts" {
            match auth_state
                .db()
                .list_binary_releases(&params.namespace, &params.repo)
                .await
            {
                Ok(releases) => (releases, false),
                Err(error) => {
                    log::error!(
                        "Failed to load binaries for {}/{}: {error}",
                        params.namespace,
                        params.repo
                    );
                    (Vec::new(), true)
                }
            }
        } else {
            (Vec::new(), false)
        };
        match tab_body(&ctx, tab, None, &releases, binaries_unavailable) {
            Ok(body) => body,
            Err(e) => {
                let content = render_git_error(&e);
                return Ok(render_for_request(
                    &req,
                    content,
                    ctx.username.as_deref(),
                    &ctx.page_title,
                ));
            }
        }
    };
    Ok(render_tab_response(&req, &ctx.frame(), tab, &body))
}

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

/// The URL tabs used before each got its own route. A permanent redirect keeps
/// old links and refreshes landing on the tab's current address.
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

/// The tab shown when no tab is named: the first configured one, else Markdown
/// when the repository has markdown, else Commits.
pub(super) fn default_tab<'a>(tabs_config: &'a [String], markdown_files: &[String]) -> &'a str {
    if let Some(first) = tabs_config.first() {
        first.as_str()
    } else if markdown_files.is_empty() {
        "commits"
    } else {
        "markdown"
    }
}

pub(super) fn render_commits_view(commits: &[Commit]) -> Markup {
    maud::html! {
        div class="twig-stack" {
            @if commits.is_empty() {
                (render_empty("NO COMMITS", "This repository has no commits yet."))
            } @else {
                ol class="twig-commits" {
                    @for commit in commits {
                        (render_commit(commit))
                    }
                }
            }
        }
    }
}

pub(super) fn render_config_view(
    twig_content: Option<&str>,
    twig_filename: Option<&str>,
) -> Markup {
    let config_filename = twig_filename.unwrap_or(".twig.toml");

    maud::html! {
        div class="twig-stack" {
            h2 class="twig-section" { "Configuration" }
            p class="twig-hint" {
                "Repository configuration from " code class="twig-mono" { (config_filename) }
            }
            @if let Some(content) = twig_content {
                pre class="twig-code" tabindex="0" aria-label=(format!("{config_filename} contents")) {
                    code { (content) }
                }
            } @else {
                (render_empty(
                    "NO CONFIGURATION",
                    "No configuration file found. Create a .twig.toml file in the repository root to configure ignore patterns."
                ))
            }
        }
    }
}

pub(super) fn render_license_view(license_content: Option<&str>) -> Markup {
    maud::html! {
        div class="twig-stack" {
            @if let Some(content) = license_content {
                div class="twig-md twig-md--boxed" {
                    (maud::PreEscaped(content))
                }
            } @else {
                (render_empty("NO LICENSE", "No license information available."))
            }
        }
    }
}

pub(super) fn render_commit(commit: &Commit) -> Markup {
    let hash = commit.hash();
    let author = commit.author();
    let date = commit.date();
    let commit_message = commit.message();
    maud::html! {
        li class="twig-commit" {
            div class="twig-commit-meta" {
                code class="twig-commit-hash" {
                    (hash.chars().take(7).collect::<String>())
                }
                span class="twig-commit-author" { (author) }
                time class="twig-commit-date" datetime=(date.to_rfc3339()) {
                    (date.format("%Y-%m-%d %H:%M"))
                }
            }
            p class="twig-commit-msg" { (commit_message) }
        }
    }
}
