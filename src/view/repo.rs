use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use maud::Markup;
use pulldown_cmark::{Event, Parser, html};
use serde::Deserialize;

use crate::{
    auth::AuthState,
    config,
    git::{
        self,
        bare::{Commit, Depth},
    },
};

/// Helper function to get the username from the session cookie if logged in
async fn get_username_from_request(
    req: &HttpRequest,
    auth_state: &web::Data<AuthState>,
) -> Option<String> {
    let token = req.cookie("session")?;
    let user_id = auth_state.validate_token(token.value()).await?;
    let user = auth_state.db.get_user_by_id(&user_id).await.ok()??;
    Some(user.username)
}

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

#[get("/{namespace}/{repo}")]
pub async fn handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<AuthState>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let username = get_username_from_request(&req, &auth_state).await;

    // Get commits
    let commits_result =
        git::bare::get_commits(server.project_root(), namespace, repo, Depth::default());

    // Get README
    let readme_result = git::bare::read_readme(server.project_root(), namespace, repo);

    let content = match commits_result {
        Ok(commits) => {
            let readme_html = readme_result
                .ok()
                .flatten()
                .map(|(_, content)| markdown_to_html(&content));
            render_repo(namespace, repo, &commits, readme_html.as_deref())
        }
        Err(e) => render_git_error(e),
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(super::render_layout(&content, username.as_deref()))
    }
}

#[get("/{namespace}/{repo}/tab/{tab}")]
pub async fn tab_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<AuthState>,
    params: web::Path<TabParams>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let tab = &params.tab;
    let username = get_username_from_request(&req, &auth_state).await;

    // Get commits
    let commits_result =
        git::bare::get_commits(server.project_root(), namespace, repo, Depth::default());

    // Get README
    let readme_result = git::bare::read_readme(server.project_root(), namespace, repo);

    match commits_result {
        Ok(commits) => {
            let readme_html = readme_result
                .ok()
                .flatten()
                .map(|(_, content)| markdown_to_html(&content));

            if req.headers().get("HX-Request").is_some() {
                // For HTMX requests, only return the scrollable content (not tabs)
                let content = scrollable_container(render_tab_content_inner(
                    tab,
                    &commits,
                    readme_html.as_deref(),
                ));
                Ok(content)
            } else {
                // For full page loads, return tabs + content
                let content =
                    render_tab_content(namespace, repo, tab, &commits, readme_html.as_deref());
                Ok(super::render_layout(&content, username.as_deref()))
            }
        }
        Err(e) => {
            let content = render_git_error(e);
            if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(super::render_layout(&content, username.as_deref()))
            }
        }
    }
}

/// Converts markdown to HTML
fn markdown_to_html(markdown: &str) -> String {
    let parser = Parser::new(markdown);

    // Sanitize raw HTML by escaping it
    let parser = parser.map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        _ => event,
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

fn render_git_error(e: git2::Error) -> Markup {
    let code = e.code();
    let code = format!("{:?}", code);
    let klass = e.class();
    let klass = format!("{:?}", klass);
    let message = e.message();
    maud::html! {
        p { (message) }
        p { (code) }
        p { (klass) }
    }
}

/// A scrollable container for tab content
fn scrollable_container(content: Markup) -> Markup {
    maud::html! {
        div class="overflow-y-auto flex-auto" style="max-height: calc(100vh - 14rem);" {
            (content)
        }
    }
}

/// Renders a tab navigation bar
fn render_tabs(namespace: &str, repo: &str, active_tab: &str) -> Markup {
    let tabs = vec![("readme", "README"), ("commits", "Commits")];

    maud::html! {
        div class="flex bb b--white-20 mb3" {
            @for (tab_id, tab_label) in tabs {
                @let is_active = tab_id == active_tab;
                @let classes = if is_active {
                    "tab-active pa2 ph3 white fw6 no-underline pointer bg-white-10"
                } else {
                    "tab pa2 ph3 white-70 hover-white no-underline pointer hover-bg-white-10"
                };
                a
                    href=(format!("/{}/{}/tab/{}", namespace, repo, tab_id))
                    class=(classes)
                    hx-get=(format!("/{}/{}/tab/{}", namespace, repo, tab_id))
                    hx-target="#tab-content"
                    hx-push-url=(format!("/{}/{}", namespace, repo))
                {
                    (tab_label)
                }
            }
        }
    }
}

fn render_repo(
    namespace: &str,
    repo: &str,
    commits: &[Commit],
    readme_html: Option<&str>,
) -> Markup {
    // Default to "readme" tab if README exists, otherwise "commits"
    let default_tab = if readme_html.is_some() {
        "readme"
    } else {
        "commits"
    };

    maud::html! {
        // Breadcrumb navigation
        div class="mb4 f6 white-70" {
            a href="/" class="link white-70 hover-white no-underline" { "Namespaces" }
            span class="mh2" { "/" }
            a href=(format!("/{}", namespace)) class="link white-70 hover-white no-underline" { (namespace) }
            span class="mh2" { "/" }
            span class="white" { (repo) }
        }

        // Tab navigation
        (render_tabs(namespace, repo, default_tab))

        // Tab content container (scrollable)
        div id="tab-content" {
            (scrollable_container(render_tab_content_inner(default_tab, commits, readme_html)))
        }
    }
}

fn render_tab_content(
    namespace: &str,
    repo: &str,
    tab: &str,
    commits: &[Commit],
    readme_html: Option<&str>,
) -> Markup {
    maud::html! {
        // Tab navigation (update active state)
        (render_tabs(namespace, repo, tab))

        // Tab content container (scrollable)
        div id="tab-content" {
            (scrollable_container(render_tab_content_inner(tab, commits, readme_html)))
        }
    }
}

fn render_tab_content_inner(tab: &str, commits: &[Commit], readme_html: Option<&str>) -> Markup {
    match tab {
        "readme" => {
            if let Some(html) = readme_html {
                maud::html! {
                    div class="readme pa3 mb4 bg-dark-gray br2" {
                        h2 class="f4 fw6 mb3 white" { "README" }
                        div class="markdown-body white lh-copy" {
                            (maud::PreEscaped(html))
                        }
                    }
                }
            } else {
                maud::html! {
                    div class="pa3 white-50" {
                        "No README file found."
                    }
                }
            }
        }
        "commits" => {
            maud::html! {
                div {
                    h2 class="f4 fw6 mb3 white" { "Commits" }
                    ol class="list pl0" {
                        @for commit in commits {
                            li class="mb3" {
                                (render_commit(commit))
                            }
                        }
                    }
                }
            }
        }
        _ => {
            // Unknown tab - show README by default if available
            render_tab_content_inner(
                if readme_html.is_some() {
                    "readme"
                } else {
                    "commits"
                },
                commits,
                readme_html,
            )
        }
    }
}

fn render_commit(commit: &Commit) -> Markup {
    let hash = commit.hash();
    let author = commit.author();
    let date = commit.date();
    let commit_message = commit.commit_message();
    maud::html! {
        div class="commit ba b--white-20 br2 pa3 bg-black-20" {
            div class="flex items-center mb2" {
                code class="f7 mr2 ph2 pv1 bg-white-10 br1 white" {
                    (hash.chars().take(7).collect::<String>())
                }
                span class="f6 white-70" { (author) }
                span class="f6 white-50 ml2" { (date.format("%Y-%m-%d %H:%M")) }
            }
            p class="f5 white ma0" { (commit_message) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_to_html() {
        let md = "# Hello\n\nThis is **bold** text.";
        let html = markdown_to_html(md);
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }
}
