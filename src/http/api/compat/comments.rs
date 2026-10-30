//! Conversation comments and emoji reactions on a request, in Forgejo's shapes.
//! Forgejo keeps request comments under `issues`, so the paths use `issues`.

use actix_web::{HttpRequest, HttpResponse, delete, get, patch, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{NamespacePermission, TwigContext},
    config,
    db::{
        Database,
        pull_requests::{
            PullRequest,
            comments::PullRequestComment,
            reactions::{ReactionContent, comment_key, pull_key},
        },
    },
    git::bare::RepoHandle,
    pulls::{self, Actor, RepoRef},
};

use super::{authorized_repo, compat_error, from_pull_error, json::UserJson, origin};

#[derive(Debug, Deserialize)]
pub struct IssuePath {
    pub owner: String,
    pub repo: String,
    pub index: i64,
}

#[derive(Debug, Deserialize)]
pub struct CommentPath {
    pub owner: String,
    pub repo: String,
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct BodyInput {
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct ReactionInput {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct CommentJson {
    pub id: i64,
    pub html_url: String,
    pub pull_request_url: String,
    pub issue_url: String,
    pub user: Option<UserJson>,
    pub original_author: String,
    pub original_author_id: i64,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ReactionJson {
    pub content: &'static str,
    pub user: UserJson,
}

async fn author(db: &Database, user_id: &str) -> Option<UserJson> {
    db.get_user_by_id(user_id)
        .await
        .ok()
        .flatten()
        .map(|user| UserJson::public(&user))
}

async fn comment_json(
    db: &Database,
    origin: &str,
    owner: &str,
    repo: &str,
    number: i64,
    comment: &PullRequestComment,
) -> CommentJson {
    let user = author(db, &comment.author_user_id).await;
    let login = user
        .as_ref()
        .map(|user| user.login.clone())
        .unwrap_or_default();
    let html_url = format!(
        "{origin}/{owner}/{repo}/pulls/{number}#issuecomment-{}",
        comment.id
    );
    CommentJson {
        id: comment.id,
        html_url,
        pull_request_url: format!("{origin}/api/v1/repos/{owner}/{repo}/pulls/{number}"),
        issue_url: format!("{origin}/api/v1/repos/{owner}/{repo}/issues/{number}"),
        original_author_id: user.as_ref().map(|user| user.id).unwrap_or_default(),
        original_author: login,
        user,
        body: comment.body.clone(),
        created_at: comment.created_at.clone(),
        updated_at: comment
            .updated_at
            .clone()
            .unwrap_or_else(|| comment.created_at.clone()),
    }
}

/// Opens the repository, returns the scoped request, and the caller. Shared by
/// every comment and reaction route.
async fn request_context(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    owner: &str,
    repo: &str,
    index: i64,
) -> Result<
    (
        RepoHandle,
        PullRequest,
        crate::auth::AuthenticatedPrincipal,
        crate::auth::PermissionSet,
    ),
    HttpResponse,
> {
    let compat = authorized_repo(
        req,
        server,
        auth_state,
        owner,
        repo,
        NamespacePermission::ViewNamespace,
    )
    .await?;
    let repo_id = compat.handle.repo_id().map_err(|error| {
        compat_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            &error.to_string(),
        )
    })?;
    let scope = RepoRef { repo_id: &repo_id };
    let pull_request = pulls::get(auth_state.db(), &scope, index)
        .await
        .map_err(from_pull_error)?;
    Ok((
        compat.handle,
        pull_request,
        compat.principal,
        compat.permissions,
    ))
}

/// `GET /api/v1/repos/{owner}/{repo}/issues/{index}/comments`
#[get("/api/v1/repos/{owner}/{repo}/issues/{index}/comments")]
pub async fn list_comments(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
) -> HttpResponse {
    let (_, pull_request, _, _) = match request_context(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.index,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };
    let db = auth_state.db();
    let comments = match db.list_pull_request_comments(&pull_request.id).await {
        Ok(comments) => comments,
        Err(error) => return from_pull_error(crate::pulls::PullError::Internal(error)),
    };
    let base = origin(&req);
    let mut items = Vec::with_capacity(comments.len());
    for comment in &comments {
        items.push(comment_json(db, &base, &path.owner, &path.repo, path.index, comment).await);
    }
    super::paginate(&req, items)
}

/// `POST /api/v1/repos/{owner}/{repo}/issues/{index}/comments`
#[post("/api/v1/repos/{owner}/{repo}/issues/{index}/comments")]
pub async fn create_comment(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    body: web::Json<BodyInput>,
) -> HttpResponse {
    let (_, pull_request, principal, permissions) = match request_context(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.index,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };
    let actor = Actor {
        user_id: &principal.user.id,
    };
    match pulls::add_comment(
        auth_state.db(),
        &pull_request,
        &actor,
        &permissions,
        &body.body,
    )
    .await
    {
        Ok(comment) => {
            let json = comment_json(
                auth_state.db(),
                &origin(&req),
                &path.owner,
                &path.repo,
                path.index,
                &comment,
            )
            .await;
            HttpResponse::Created().json(json)
        }
        Err(error) => from_pull_error(error),
    }
}

/// `PATCH /api/v1/repos/{owner}/{repo}/issues/comments/{id}`
#[patch("/api/v1/repos/{owner}/{repo}/issues/comments/{id}")]
pub async fn edit_comment(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CommentPath>,
    body: web::Json<BodyInput>,
) -> HttpResponse {
    let (_, pull_request, principal, permissions) =
        match comment_context(&req, &server, &auth_state, &path).await {
            Ok(context) => context,
            Err(response) => return response,
        };
    let actor = Actor {
        user_id: &principal.user.id,
    };
    match pulls::edit_comment(
        auth_state.db(),
        &pull_request,
        path.id,
        &actor,
        &permissions,
        &body.body,
    )
    .await
    {
        Ok(comment) => {
            let json = comment_json(
                auth_state.db(),
                &origin(&req),
                &path.owner,
                &path.repo,
                pull_request.number,
                &comment,
            )
            .await;
            HttpResponse::Ok().json(json)
        }
        Err(error) => from_pull_error(error),
    }
}

/// `DELETE /api/v1/repos/{owner}/{repo}/issues/comments/{id}`
#[delete("/api/v1/repos/{owner}/{repo}/issues/comments/{id}")]
pub async fn delete_comment(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CommentPath>,
) -> HttpResponse {
    let (_, pull_request, principal, permissions) =
        match comment_context(&req, &server, &auth_state, &path).await {
            Ok(context) => context,
            Err(response) => return response,
        };
    let actor = Actor {
        user_id: &principal.user.id,
    };
    match pulls::delete_comment(
        auth_state.db(),
        &pull_request,
        path.id,
        &actor,
        &permissions,
    )
    .await
    {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(error) => from_pull_error(error),
    }
}

/// Comments carry no request number in their path, so the request is found from
/// the comment itself.
async fn comment_context(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    path: &CommentPath,
) -> Result<
    (
        RepoHandle,
        PullRequest,
        crate::auth::AuthenticatedPrincipal,
        crate::auth::PermissionSet,
    ),
    HttpResponse,
> {
    let compat = authorized_repo(
        req,
        server,
        auth_state,
        &path.owner,
        &path.repo,
        NamespacePermission::ViewNamespace,
    )
    .await?;
    let repo_id = compat.handle.repo_id().map_err(|error| {
        compat_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            &error.to_string(),
        )
    })?;
    let db = auth_state.db();
    let comment = db
        .get_pull_request_comment(path.id)
        .await
        .map_err(|error| from_pull_error(crate::pulls::PullError::Internal(error)))?
        .ok_or_else(|| from_pull_error(crate::pulls::PullError::NotFound))?;
    let pull_request = db
        .get_pull_request_by_id(&comment.pull_request_id)
        .await
        .map_err(|error| from_pull_error(crate::pulls::PullError::Internal(error)))?
        .filter(|pull_request| pull_request.repo_id == repo_id)
        .ok_or_else(|| from_pull_error(crate::pulls::PullError::NotFound))?;
    Ok((
        compat.handle,
        pull_request,
        compat.principal,
        compat.permissions,
    ))
}

fn reaction_json(content: ReactionContent, user: UserJson) -> ReactionJson {
    ReactionJson {
        content: content.api_value(),
        user,
    }
}

async fn list_reactions_json(db: &Database, key: &str) -> Result<Vec<ReactionJson>, String> {
    let mut reactions = Vec::new();
    for (username, content) in db.list_reactions(key).await? {
        let user = db
            .get_user_by_username(&username)
            .await?
            .map(|user| UserJson::public(&user))
            .ok_or_else(|| format!("Reacting user '{username}' is missing"))?;
        reactions.push(reaction_json(content, user));
    }
    Ok(reactions)
}

async fn reactions_handler(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    owner: &str,
    repo: &str,
    index: i64,
    comment_id: Option<i64>,
    method: ReactionMethod,
    content: Option<&str>,
) -> HttpResponse {
    let (_, pull_request, principal, permissions) =
        match request_context(req, server, auth_state, owner, repo, index).await {
            Ok(context) => context,
            Err(response) => return response,
        };
    let db = auth_state.db();
    let key = match comment_id {
        Some(id) => comment_key(id),
        None => pull_key(&pull_request.id),
    };
    match method {
        ReactionMethod::List => match list_reactions_json(db, &key).await {
            Ok(items) => super::paginate(req, items),
            Err(error) => from_pull_error(crate::pulls::PullError::Internal(error)),
        },
        ReactionMethod::Set(on) => {
            let Some(content) = content.and_then(ReactionContent::parse) else {
                return compat_error(
                    actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
                    "Unknown reaction",
                );
            };
            let actor = Actor {
                user_id: &principal.user.id,
            };
            match pulls::react(
                db,
                &pull_request,
                comment_id,
                &actor,
                &permissions,
                content,
                on,
            )
            .await
            {
                Ok(()) if on => {
                    let user = author(db, &principal.user.id).await;
                    match user {
                        Some(user) => HttpResponse::Created().json(reaction_json(content, user)),
                        None => from_pull_error(crate::pulls::PullError::NotFound),
                    }
                }
                Ok(()) => HttpResponse::NoContent().finish(),
                Err(error) => from_pull_error(error),
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ReactionMethod {
    List,
    Set(bool),
}

#[get("/api/v1/repos/{owner}/{repo}/issues/{index}/reactions")]
pub async fn list_pull_reactions(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
) -> HttpResponse {
    reactions_handler(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.index,
        None,
        ReactionMethod::List,
        None,
    )
    .await
}

#[post("/api/v1/repos/{owner}/{repo}/issues/{index}/reactions")]
pub async fn add_pull_reaction(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    body: web::Json<ReactionInput>,
) -> HttpResponse {
    reactions_handler(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.index,
        None,
        ReactionMethod::Set(true),
        Some(&body.content),
    )
    .await
}

#[delete("/api/v1/repos/{owner}/{repo}/issues/{index}/reactions")]
pub async fn remove_pull_reaction(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<IssuePath>,
    body: web::Json<ReactionInput>,
) -> HttpResponse {
    reactions_handler(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        path.index,
        None,
        ReactionMethod::Set(false),
        Some(&body.content),
    )
    .await
}

#[get("/api/v1/repos/{owner}/{repo}/issues/comments/{id}/reactions")]
pub async fn list_comment_reactions(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CommentPath>,
) -> HttpResponse {
    let (index, _) = match comment_index(&req, &server, &auth_state, &path).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    reactions_handler(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        index,
        Some(path.id),
        ReactionMethod::List,
        None,
    )
    .await
}

#[post("/api/v1/repos/{owner}/{repo}/issues/comments/{id}/reactions")]
pub async fn add_comment_reaction(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CommentPath>,
    body: web::Json<ReactionInput>,
) -> HttpResponse {
    let (index, _) = match comment_index(&req, &server, &auth_state, &path).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    reactions_handler(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        index,
        Some(path.id),
        ReactionMethod::Set(true),
        Some(&body.content),
    )
    .await
}

#[delete("/api/v1/repos/{owner}/{repo}/issues/comments/{id}/reactions")]
pub async fn remove_comment_reaction(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    path: web::Path<CommentPath>,
    body: web::Json<ReactionInput>,
) -> HttpResponse {
    let (index, _) = match comment_index(&req, &server, &auth_state, &path).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    reactions_handler(
        &req,
        &server,
        &auth_state,
        &path.owner,
        &path.repo,
        index,
        Some(path.id),
        ReactionMethod::Set(false),
        Some(&body.content),
    )
    .await
}

/// The request number a comment belongs to, for building reaction routes.
async fn comment_index(
    req: &HttpRequest,
    server: &config::Server,
    auth_state: &TwigContext,
    path: &CommentPath,
) -> Result<(i64, ()), HttpResponse> {
    let (_, pull_request, _, _) = comment_context(req, server, auth_state, path).await?;
    Ok((pull_request.number, ()))
}
