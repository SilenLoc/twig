//! Reviews, inline review comments, and requested reviewers, in Forgejo's shapes.

use actix_web::{HttpRequest, HttpResponse, delete, get, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{NamespacePermission, TwigContext},
    config,
    db::{
        Database,
        pull_requests::{
            PullRequest,
            comments::PullRequestComment,
            reviews::{PullRequestReview, ReviewState},
        },
    },
    pulls::{self, Actor, InlineInput, RepoRef},
};

use super::{authorized_repo, compat_error, from_pull_error, json::UserJson, origin};

#[derive(Debug, Deserialize)]
pub struct PullPath {
    pub owner: String,
    pub repo: String,
    pub index: i64,
}

#[derive(Debug, Deserialize)]
pub struct ReviewPath {
    pub owner: String,
    pub repo: String,
    pub index: i64,
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct InlineBody {
    pub path: String,
    pub body: String,
    #[serde(default)]
    pub old_position: i64,
    #[serde(default)]
    pub new_position: i64,
}

#[derive(Debug, Deserialize)]
pub struct SubmitReviewBody {
    pub event: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub commit_id: String,
    #[serde(default)]
    pub comments: Vec<InlineBody>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewersBody {
    pub reviewers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReviewJson {
    pub id: i64,
    pub user: Option<UserJson>,
    pub body: String,
    pub state: &'static str,
    pub commit_id: String,
    pub comments_count: i64,
    pub dismissed: bool,
    pub stale: bool,
    pub official: bool,
    pub html_url: String,
    pub pull_request_url: String,
    pub submitted_at: String,
}

#[derive(Debug, Serialize)]
pub struct ReviewCommentJson {
    pub id: i64,
    pub body: String,
    pub path: String,
    pub position: i64,
    pub original_position: i64,
    pub commit_id: String,
    pub original_commit_id: String,
    pub user: Option<UserJson>,
    pub created_at: String,
    pub updated_at: String,
    pub pull_request_review_id: i64,
    pub html_url: String,
    pub resolver: Option<UserJson>,
    pub diff_hunk: String,
}

/// Forgejo's review states: the stored value, not the event name, is reported.
fn state_name(state: ReviewState) -> &'static str {
    match state {
        ReviewState::Approved => "APPROVED",
        ReviewState::RequestChanges => "REQUEST_CHANGES",
        ReviewState::Comment => "COMMENT",
    }
}

async fn user_of(db: &Database, user_id: &str) -> Option<UserJson> {
    db.get_user_by_id(user_id)
        .await
        .ok()
        .flatten()
        .map(|user| UserJson::public(&user))
}

async fn review_json(
    db: &Database,
    origin: &str,
    owner: &str,
    repo: &str,
    number: i64,
    review: &PullRequestReview,
    comments_count: i64,
) -> ReviewJson {
    ReviewJson {
        id: review.id,
        user: user_of(db, &review.author_user_id).await,
        body: review.body.clone(),
        state: state_name(review.state),
        commit_id: review.commit_oid.clone(),
        comments_count,
        dismissed: false,
        stale: false,
        official: false,
        html_url: format!(
            "{origin}/{owner}/{repo}/pulls/{number}#review-{}",
            review.id
        ),
        pull_request_url: format!("{origin}/api/v1/repos/{owner}/{repo}/pulls/{number}"),
        submitted_at: review.submitted_at.clone(),
    }
}

async fn comment_json(
    db: &Database,
    comment: &PullRequestComment,
    review_id: i64,
) -> ReviewCommentJson {
    let old = comment.old_position.unwrap_or_default();
    let new = comment.new_position.unwrap_or_default();
    let commit = comment.commit_oid.clone().unwrap_or_default();
    ReviewCommentJson {
        id: comment.id,
        body: comment.body.clone(),
        path: comment.path.clone().unwrap_or_default(),
        position: new,
        original_position: old,
        commit_id: commit.clone(),
        original_commit_id: commit,
        user: user_of(db, &comment.author_user_id).await,
        created_at: comment.created_at.clone(),
        updated_at: comment
            .updated_at
            .clone()
            .unwrap_or_else(|| comment.created_at.clone()),
        pull_request_review_id: review_id,
        html_url: String::new(),
        resolver: None,
        diff_hunk: String::new(),
    }
}

/// Opens the repository and the request for one review route.
async fn request_for(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    owner: &str,
    repo: &str,
    index: i64,
    permission: NamespacePermission,
) -> Result<
    (
        crate::git::bare::RepoHandle,
        PullRequest,
        crate::auth::AuthenticatedPrincipal,
        crate::auth::PermissionSet,
    ),
    HttpResponse,
> {
    let compat = authorized_repo(req, server, auth_state, owner, repo, permission).await?;
    let repo_id = compat.handle.repo_id().map_err(|error| {
        compat_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            &error.to_string(),
        )
    })?;
    let pull_request = pulls::get(auth_state.db(), &RepoRef { repo_id: &repo_id }, index)
        .await
        .map_err(from_pull_error)?;
    Ok((
        compat.handle,
        pull_request,
        compat.principal,
        compat.permissions,
    ))
}

/// `GET /api/v1/repos/{owner}/{repo}/pulls/{index}/reviews`
#[get("/api/v1/repos/{owner}/{repo}/pulls/{index}/reviews")]
pub async fn list_reviews(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
) -> HttpResponse {
    let (_, pull_request, _, _) = match request_for(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.index,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let db = auth_state.db();
    let reviews = match db.list_pull_request_reviews(&pull_request.id).await {
        Ok(reviews) => reviews,
        Err(error) => return from_pull_error(crate::pulls::PullError::Internal(error)),
    };
    let inline = db
        .list_review_comments(&pull_request.id)
        .await
        .unwrap_or_default();
    let base = origin(&req);
    let mut items = Vec::with_capacity(reviews.len());
    for review in &reviews {
        let count = inline
            .iter()
            .filter(|comment| comment.review_id == Some(review.id))
            .count();
        items.push(
            review_json(
                db,
                &base,
                &path.owner,
                &path.repo,
                path.index,
                review,
                i64::try_from(count).unwrap_or_default(),
            )
            .await,
        );
    }
    super::paginate(&req, items)
}

/// `GET /api/v1/repos/{owner}/{repo}/pulls/{index}/reviews/{id}`
#[get("/api/v1/repos/{owner}/{repo}/pulls/{index}/reviews/{id}")]
pub async fn get_review(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<ReviewPath>,
) -> HttpResponse {
    let (_, pull_request, _, _) = match request_for(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.index,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let db = auth_state.db();
    match db.get_pull_request_review(&pull_request.id, path.id).await {
        Ok(Some(review)) => {
            let count = db
                .list_review_comments(&pull_request.id)
                .await
                .map(|items| {
                    items
                        .iter()
                        .filter(|comment| comment.review_id == Some(review.id))
                        .count()
                })
                .unwrap_or_default();
            HttpResponse::Ok().json(
                review_json(
                    db,
                    &origin(&req),
                    &path.owner,
                    &path.repo,
                    path.index,
                    &review,
                    i64::try_from(count).unwrap_or_default(),
                )
                .await,
            )
        }
        Ok(None) => from_pull_error(crate::pulls::PullError::NotFound),
        Err(error) => from_pull_error(crate::pulls::PullError::Internal(error)),
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/pulls/{index}/reviews/{id}/comments`
#[get("/api/v1/repos/{owner}/{repo}/pulls/{index}/reviews/{id}/comments")]
pub async fn list_review_comments(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<ReviewPath>,
) -> HttpResponse {
    let (_, pull_request, _, _) = match request_for(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.index,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let db = auth_state.db();
    let comments = match db.list_review_comments(&pull_request.id).await {
        Ok(comments) => comments,
        Err(error) => return from_pull_error(crate::pulls::PullError::Internal(error)),
    };
    let mut items = Vec::new();
    for comment in comments
        .iter()
        .filter(|comment| comment.review_id == Some(path.id))
    {
        items.push(comment_json(db, comment, path.id).await);
    }
    super::paginate(&req, items)
}

/// `POST /api/v1/repos/{owner}/{repo}/pulls/{index}/reviews`
#[post("/api/v1/repos/{owner}/{repo}/pulls/{index}/reviews")]
pub async fn submit_review(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    body: web::Json<SubmitReviewBody>,
) -> HttpResponse {
    let (handle, pull_request, principal, permissions) = match request_for(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.index,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Some(state) = ReviewState::parse(&body.event) else {
        return compat_error(
            actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            "Unknown review event",
        );
    };
    // A review is pinned to the head it was written against, unless the client names another commit.
    let commit = if body.commit_id.is_empty() {
        handle
            .branch_tip(&pull_request.head_branch)
            .ok()
            .flatten()
            .map(|oid| oid.to_string())
            .unwrap_or_default()
    } else {
        body.commit_id.clone()
    };
    let inline: Vec<InlineInput<'_>> = body
        .comments
        .iter()
        .map(|comment| InlineInput {
            path: &comment.path,
            body: &comment.body,
            old_position: comment.old_position,
            new_position: comment.new_position,
        })
        .collect();
    let actor = Actor {
        user_id: &principal.user.id,
    };
    match pulls::submit_review(
        auth_state.db(),
        &pull_request,
        &actor,
        &permissions,
        state,
        &body.body,
        &commit,
        &inline,
    )
    .await
    {
        Ok(review) => {
            let db = auth_state.db();
            let count = inline.len();
            let json = review_json(
                db,
                &origin(&req),
                &path.owner,
                &path.repo,
                path.index,
                &review,
                i64::try_from(count).unwrap_or_default(),
            )
            .await;
            HttpResponse::Ok().json(json)
        }
        Err(error) => from_pull_error(error),
    }
}

/// `GET /api/v1/repos/{owner}/{repo}/pulls/{index}/requested_reviewers`
#[get("/api/v1/repos/{owner}/{repo}/pulls/{index}/requested_reviewers")]
pub async fn list_requested_reviewers(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
) -> HttpResponse {
    let (_, pull_request, _, _) = match request_for(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.index,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    match requested_reviewers(auth_state.db(), &pull_request).await {
        Ok(users) => HttpResponse::Ok().json(users),
        Err(error) => from_pull_error(crate::pulls::PullError::Internal(error)),
    }
}

/// Reviewers as Forgejo users. Usernames that no longer resolve are skipped.
pub async fn requested_reviewers(
    db: &Database,
    pull_request: &PullRequest,
) -> Result<Vec<UserJson>, String> {
    let mut users = Vec::new();
    for username in db.list_requested_reviewers(&pull_request.id).await? {
        if let Some(user) = db.get_user_by_username(&username).await? {
            users.push(UserJson::public(&user));
        }
    }
    Ok(users)
}

/// `POST /api/v1/repos/{owner}/{repo}/pulls/{index}/requested_reviewers`
#[post("/api/v1/repos/{owner}/{repo}/pulls/{index}/requested_reviewers")]
pub async fn request_reviewers(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    body: web::Json<ReviewersBody>,
) -> HttpResponse {
    change_reviewers(req, server, auth_state, path, body, true).await
}

/// `DELETE /api/v1/repos/{owner}/{repo}/pulls/{index}/requested_reviewers`
#[delete("/api/v1/repos/{owner}/{repo}/pulls/{index}/requested_reviewers")]
pub async fn withdraw_reviewers(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    body: web::Json<ReviewersBody>,
) -> HttpResponse {
    change_reviewers(req, server, auth_state, path, body, false).await
}

async fn change_reviewers(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<PullPath>,
    body: web::Json<ReviewersBody>,
    requested: bool,
) -> HttpResponse {
    let (_, pull_request, principal, permissions) = match request_for(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.index,
        NamespacePermission::ViewNamespace,
    )
    .await
    {
        Ok(found) => found,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &principal.user.id,
    };
    for username in &body.reviewers {
        if let Err(error) = pulls::set_requested_reviewer(
            auth_state.db(),
            &path.owner,
            &pull_request,
            &actor,
            &permissions,
            username,
            requested,
        )
        .await
        {
            return from_pull_error(error);
        }
    }
    if requested {
        match requested_reviewers(auth_state.db(), &pull_request).await {
            Ok(users) => HttpResponse::Created().json(users),
            Err(error) => from_pull_error(crate::pulls::PullError::Internal(error)),
        }
    } else {
        HttpResponse::NoContent().finish()
    }
}
