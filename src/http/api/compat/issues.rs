//! `GET /repos/{owner}/{repo}/issues`, the list Forgejo clients use to search a
//! repository's requests (`type=pulls`). Twig has no issues, so `type=issues`
//! returns nothing. Requests are shown in the issue shape, with a `pull_request` marker.

use actix_web::{HttpRequest, HttpResponse, get, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{NamespacePermission, TwigContext},
    config,
    db::pull_requests::{PullRequestFilter, PullRequestState},
    pulls::{self, RepoRef},
};

use super::{
    RepoPath, authorized_repo, compat_error, from_pull_error,
    json::{PullRequestJson, UserJson},
    origin,
};

#[derive(Debug, Deserialize)]
pub struct IssueListQuery {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub state: Option<String>,
    pub q: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct PullRequestMarker {
    pub merged: bool,
    pub merged_at: Option<String>,
    pub draft: bool,
    pub html_url: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct IssueJson {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub state: &'static str,
    pub user: Option<UserJson>,
    pub html_url: String,
    pub url: String,
    pub comments: i64,
    pub is_locked: bool,
    pub labels: Vec<serde_json::Value>,
    pub assignees: Vec<UserJson>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub pull_request: PullRequestMarker,
}

impl IssueJson {
    fn from_pull(pull: PullRequestJson) -> Self {
        Self {
            id: pull.id,
            number: pull.number,
            title: pull.title,
            body: pull.body,
            state: pull.state,
            user: pull.user,
            html_url: pull.html_url.clone(),
            url: pull.url.clone(),
            comments: pull.comments,
            is_locked: pull.is_locked,
            labels: pull.labels,
            assignees: pull.assignees,
            created_at: pull.created_at,
            updated_at: pull.updated_at,
            closed_at: pull.closed_at,
            pull_request: PullRequestMarker {
                merged: pull.merged,
                merged_at: pull.merged_at,
                draft: pull.draft,
                html_url: pull.html_url,
                url: pull.url,
            },
        }
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/issues`
#[get("/api/v1/repos/{owner}/{repo}/issues")]
pub async fn list_issues(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<RepoPath>,
    query: web::Query<IssueListQuery>,
) -> HttpResponse {
    let repo = match authorized_repo(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(repo) => repo,
        Err(response) => return response,
    };
    if query.kind.as_deref() == Some("issues") {
        return super::paginate(&req, Vec::<IssueJson>::new());
    }
    let state = match query.state.as_deref().unwrap_or("open") {
        "all" => None,
        other => match PullRequestState::parse(other) {
            Some(state) => Some(state),
            None => {
                return compat_error(
                    actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                    "Invalid state",
                );
            }
        },
    };
    let repo_id = match repo.handle.repo_id() {
        Ok(id) => id,
        Err(error) => {
            log::error!("Failed to read repository identity: {error}");
            return compat_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "The request could not be completed",
            );
        }
    };
    let limit = query.limit.unwrap_or(30).clamp(1, 50);
    let page = query.page.unwrap_or(1).max(1);
    let filter = PullRequestFilter {
        state,
        head_branch: None,
        query: query.q.clone(),
        recently_updated: false,
        limit,
        offset: (page - 1).saturating_mul(limit),
    };
    let (rows, has_more) =
        match pulls::list(auth_state.db(), &RepoRef { repo_id: &repo_id }, &filter).await {
            Ok(found) => found,
            Err(error) => return from_pull_error(error),
        };
    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        match super::pulls::render_for_list(
            auth_state.db(),
            &repo.handle,
            &origin(&req),
            &path.owner,
            &path.repo,
            row,
        )
        .await
        {
            Ok(pull) => items.push(IssueJson::from_pull(pull)),
            Err(response) => return response,
        }
    }
    let total = match auth_state.db().count_pull_requests(&repo_id, &filter).await {
        Ok(total) => total,
        Err(error) => return from_pull_error(crate::pulls::PullError::Internal(error)),
    };
    let mut response = HttpResponse::Ok();
    response.insert_header(("X-Total-Count", total.to_string()));
    if has_more {
        response.insert_header(("Link", super::next_page_link(&req, page + 1)));
    }
    response.json(items)
}
