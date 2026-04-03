use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use maud::Markup;
use pulldown_cmark::{Event, Parser, html};
use serde::Deserialize;

use crate::{
    config,
    git::{
        self,
        bare::{Commit, Depth},
    },
};

#[derive(Deserialize)]
struct Params {
    namespace: String,
    repo: String,
}

#[get("/{namespace}/{repo}")]
pub async fn handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;

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
            render_repo(&commits, readme_html.as_deref())
        }
        Err(e) => render_git_error(e),
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(super::render_layout(&content))
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

fn render_repo(commits: &[Commit], readme_html: Option<&str>) -> Markup {
    maud::html! {
        // README section
        @if let Some(html) = readme_html {
            div class="readme pa3 mb4 bg-dark-gray br2" {
                h2 class="f4 fw6 mb3 white" { "README" }
                div class="markdown-body white lh-copy" {
                    (maud::PreEscaped(html))
                }
            }
        }

        // Commits section
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
