use actix_web::{HttpRequest, web};
use maud::Markup;

use crate::auth::{
    TwigContext,
    permissions::{self, NamespacePermission},
};
use crate::config;
use crate::git::bare::{RepoHandle, ScriptGroupNode, TwigConfigWithRaw};
use crate::http::auth::session::get_username_from_request;

pub(super) const TEXT_SIZES: [u16; 11] = [100, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200];
pub(super) const PRESENT_TEXT_SIZE_KEY: &str = "twig-present-text-size";
pub(super) const PAPER_TEXT_SIZE_KEY: &str = "twig-paper-text-size";

/// Mirrors the active size back to the browser so it survives a reload.
pub(super) fn text_size_persist(key: &str) -> String {
    format!("try {{ localStorage.setItem('{key}', String(data.twigTextSize)); }} catch (_) {{}}")
}

pub(super) fn text_size_list() -> String {
    TEXT_SIZES
        .iter()
        .map(|size| format!("'{size}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Restores a previously chosen size on load; the expression runs against the
/// owning container, where `data.twigTextSize` is the shared state.
pub(super) fn text_size_restore(key: &str) -> String {
    format!(
        "try {{ let size = localStorage.getItem('{key}'); if ([{}].includes(size)) data.twigTextSize = Number(size); }} catch (_) {{}}",
        text_size_list()
    )
}

pub(super) fn text_size_step(increase: bool, key: &str) -> String {
    let first = TEXT_SIZES[0];
    let last = TEXT_SIZES[TEXT_SIZES.len() - 1];
    let step = TEXT_SIZES[1] - TEXT_SIZES[0];
    let (bound, operator, clamp) = if increase {
        (last, '+', "Math.min")
    } else {
        (first, '-', "Math.max")
    };
    format!(
        "data.twigTextSize = {clamp}({bound}, data.twigTextSize {operator} {step}); {}",
        text_size_persist(key)
    )
}

/// Paper font choices. The active choice lives on `#paper-container` as
/// `data-twig-paper-font`; the stylesheet swaps the reading family from it.
pub(super) fn render_for_request(
    req: &HttpRequest,
    content: Markup,
    username: Option<&str>,
    page_title: &str,
) -> Markup {
    if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::http::view::render_layout(&content, username, Some(page_title))
    }
}

pub(super) fn render_repo_auth_error(req: &HttpRequest, page_title: &str) -> Markup {
    let content = crate::http::view::render_error_with_action(
        "Not logged in. Please log in first.",
        "/auth/login",
        "Log in",
    );
    render_for_request(req, content, None, page_title)
}

/// The opened repository plus everything a tab handler needs before loading its
/// own data: the parsed configuration, the signed-in username, and the page
/// title. Each handler borrows this to build only its own tab's body.
pub(super) struct RepoContext {
    pub(super) handle: RepoHandle,
    pub(super) twig_result: TwigConfigWithRaw,
    pub(super) namespace: String,
    pub(super) repo: String,
    pub(super) username: Option<String>,
    pub(super) page_title: String,
    pub(super) can_manage_hooks: bool,
    pub(super) paper_pages: Vec<String>,
    /// Absolute origin (`https://git.example.com`) the `curl` commands in the
    /// Scripts tab are built from.
    pub(super) base_url: String,
    /// The configured script groups, flattened and pruned. Empty when the
    /// repository has no `[scripts]` section, which hides the tab.
    pub(super) script_groups: Vec<ScriptGroupNode>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OptionalTab {
    Config,
    Present,
    Paper,
    Scripts,
}

/// The frame every tab shares: the breadcrumb, the tab bar, and the
/// configuration-error banner. None of it depends on the active tab.
pub(super) struct TabFrame<'a> {
    pub(super) namespace: &'a str,
    pub(super) repo: &'a str,
    pub(super) username: Option<&'a str>,
    pub(super) page_title: &'a str,
    pub(super) can_manage_hooks: bool,
    pub(super) twig_error: Option<&'a str>,
    pub(super) twig_filename: Option<&'a str>,
    pub(super) tabs_config: &'a [String],
    pub(super) optional_tabs: Vec<OptionalTab>,
}

impl TabFrame<'_> {
    pub(super) fn has_optional_tab(&self, tab: OptionalTab) -> bool {
        self.optional_tabs.contains(&tab)
    }
}

impl RepoContext {
    pub(super) fn frame(&self) -> TabFrame<'_> {
        let mut optional_tabs = Vec::with_capacity(4);
        if self.twig_result.raw.is_some() {
            optional_tabs.push(OptionalTab::Config);
        }
        if !self.twig_result.config.present.files.is_empty() {
            optional_tabs.push(OptionalTab::Present);
        }
        if !self.paper_pages.is_empty() {
            optional_tabs.push(OptionalTab::Paper);
        }
        if !self.script_groups.is_empty() {
            optional_tabs.push(OptionalTab::Scripts);
        }

        TabFrame {
            namespace: &self.namespace,
            repo: &self.repo,
            username: self.username.as_deref(),
            page_title: &self.page_title,
            can_manage_hooks: self.can_manage_hooks,
            twig_error: self.twig_result.error.as_deref(),
            twig_filename: self.twig_result.filename.as_deref(),
            tabs_config: &self.twig_result.config.tabs,
            optional_tabs,
        }
    }
}

/// Opens the repository, loads its configuration, and gates private reads on a
/// session. On failure it returns the already-rendered response, so every tab
/// handler shares one prologue.
pub(super) async fn open_repo(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
    repo: &str,
) -> Result<RepoContext, Markup> {
    let page_title = format!("{namespace}/{repo}");
    let username = get_username_from_request(req, auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(handle) => handle,
        Err(e) => {
            let content = render_git_error(&e);
            return Err(render_for_request(
                req,
                content,
                username.as_deref(),
                &page_title,
            ));
        }
    };

    let twig_result = handle.load_config_with_raw();
    if twig_result.config.private {
        match can_view_namespace(req, server, auth_state, namespace).await {
            Ok(true) => {}
            Ok(false) if username.is_none() => {
                return Err(render_repo_auth_error(req, &page_title));
            }
            Ok(false) => {
                return Err(render_for_request(
                    req,
                    crate::http::view::render_error(
                        "You do not have permission to view this namespace.",
                    ),
                    username.as_deref(),
                    &page_title,
                ));
            }
            Err(error) => {
                log::error!("Failed to authorize private repository read: {error}");
                return Err(render_for_request(
                    req,
                    crate::http::view::render_error("Failed to verify namespace permissions."),
                    username.as_deref(),
                    &page_title,
                ));
            }
        }
    }

    let can_manage_hooks = match can_manage_hooks(req, server, auth_state, namespace).await {
        Ok(can_manage_hooks) => can_manage_hooks,
        Err(error) => {
            log::error!("Failed to resolve hook-management permission: {error}");
            false
        }
    };

    let paper_pages = super::paper::load_paper_pages(&handle, &twig_result.config);
    let script_groups = twig_result.config.script_groups();

    Ok(RepoContext {
        handle,
        twig_result,
        namespace: namespace.to_string(),
        repo: repo.to_string(),
        username,
        page_title,
        can_manage_hooks,
        paper_pages,
        base_url: super::scripts::script_base_url(req),
        script_groups,
    })
}

pub(super) async fn can_manage_hooks(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
) -> Result<bool, String> {
    let Some(user_id) = auth_state.user_id_from_request(req).await else {
        return Ok(false);
    };
    let Some(user) = auth_state.db().get_user_by_id(&user_id).await? else {
        return Ok(false);
    };
    permissions::has(
        auth_state.db(),
        &user_id,
        namespace,
        server.is_configured_admin(&user.username),
        NamespacePermission::ManageRepositories,
    )
    .await
}

pub(super) async fn can_view_namespace(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &web::Data<TwigContext>,
    namespace: &str,
) -> Result<bool, String> {
    let Some(user_id) = auth_state.user_id_from_request(req).await else {
        return Ok(false);
    };
    let Some(user) = auth_state.db().get_user_by_id(&user_id).await? else {
        return Ok(false);
    };
    permissions::has(
        auth_state.db(),
        &user_id,
        namespace,
        server.is_configured_admin(&user.username),
        NamespacePermission::ViewNamespace,
    )
    .await
}

pub(super) fn render_empty(eyebrow: &str, body: &str) -> Markup {
    maud::html! {
        div class="twig-empty" {
            p class="twig-eyebrow" { (eyebrow) }
            p class="twig-empty-body" { (body) }
        }
    }
}

pub(super) fn render_git_error(e: &git2::Error) -> Markup {
    let code = e.code();
    let code = format!("{code:?}");
    let klass = e.class();
    let klass = format!("{klass:?}");
    let message = e.message();
    maud::html! {
        div class="twig-notice twig-notice--danger" role="alert" {
            p class="twig-eyebrow" { "ERROR" }
            p class="twig-notice-body" { (message) }
            p class="twig-notice-body twig-mono twig-ink-tertiary" { (code) " / " (klass) }
        }
    }
}

/// Renders a "Path not found" response, wrapping it in the full layout for
/// non-HTMX requests.
pub(super) fn render_not_found_for_request(
    req: &HttpRequest,
    username: Option<&str>,
    page_title: &str,
) -> Markup {
    let content = render_empty("NOT FOUND", "Path not found.");
    if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::http::view::render_layout(&content, username, Some(page_title))
    }
}
