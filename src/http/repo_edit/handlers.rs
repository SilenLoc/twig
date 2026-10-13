use actix_web::get;

use super::commit::{
    prepare_workspace_commit, render_commit_result, render_workspace_commit_result,
};
use super::operations::apply_file_operation;
use super::validation::validate_draft;
use super::*;
use crate::git::bare::is_safe_repo_path;

#[get("/{namespace}/{repo}/edit")]
pub async fn workspace_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<RepoParams>,
    query: web::Query<WorkspaceQuery>,
) -> HttpResponse {
    let params = params.into_inner();
    let context = match open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };
    let draft = WorkspaceDraft {
        base_head: context
            .head
            .map_or_else(|| ZERO_OID.to_string(), |head| head.to_string()),
        active_path: query.path.clone(),
        tree_path: String::new(),
        changes: Vec::new(),
    };
    let page = pages::workspace_shell(
        &params.namespace,
        &params.repo,
        query.path.as_deref(),
        &draft,
        context.user.email.is_some(),
    );
    if req.headers().contains_key("HX-Request") {
        return html_response(page);
    }

    html_response(crate::http::view::render_workspace_layout(
        &page,
        Some("Repository workspace"),
    ))
}

#[get("/{namespace}/{repo}/edit/tree/{path:.*}")]
pub async fn tree_fragment(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<TreeParams>,
) -> HttpResponse {
    let params = params.into_inner();
    if !is_safe_repo_path(&params.path) {
        return HttpResponse::NotFound().finish();
    }
    let context = match open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };
    let entries = match context.handle.list_dir(&params.path, Some(&context.config)) {
        Ok(entries) => entries,
        Err(error) => {
            log::error!(
                "Failed to list repository editor path '{}': {error}",
                params.path
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

    html_response(pages::file_tree(
        &params.namespace,
        &params.repo,
        &params.path,
        &entries,
    ))
}

pub async fn apply_operation(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<RepoParams>,
    input: web::Form<WorkspaceOperationInput>,
) -> HttpResponse {
    let params = params.into_inner();
    let context = match open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };
    let mut draft: WorkspaceDraft = match serde_json::from_str(&input.draft) {
        Ok(draft) => draft,
        Err(error) => {
            log::warn!("Invalid repository editor draft submitted: {error}");
            return html_response(pages::operation_result(
                None,
                "The workspace draft is invalid.",
                false,
            ));
        }
    };

    let current_head = context
        .head
        .map_or_else(|| ZERO_OID.to_string(), |head| head.to_string());
    if draft.base_head != current_head {
        let serialized = serde_json::to_string(&draft).unwrap_or_default();
        return html_response(pages::operation_result(
            Some(&serialized),
            "The repository changed since this workspace was opened. The draft remains in this tab.",
            false,
        ));
    }

    let message = match apply_file_operation(&context, &mut draft, &input) {
        Ok(message) => message,
        Err(message) => {
            let serialized = serde_json::to_string(&draft).unwrap_or_default();
            return html_response(pages::operation_result(Some(&serialized), &message, false));
        }
    };
    if let Err(message) = validate_draft(&context, &draft) {
        let serialized = serde_json::to_string(&draft).unwrap_or_default();
        return html_response(pages::operation_result(Some(&serialized), &message, false));
    }

    let serialized = serde_json::to_string(&draft).unwrap_or_default();
    html_response(pages::operation_result(Some(&serialized), &message, true))
}

pub async fn commit_workspace(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<RepoParams>,
    input: web::Form<CommitWorkspaceInput>,
) -> HttpResponse {
    let params = params.into_inner();
    let context = match open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };
    let draft: WorkspaceDraft = match serde_json::from_str(&input.draft) {
        Ok(draft) => draft,
        Err(error) => {
            log::warn!("Invalid repository editor draft submitted for commit: {error}");
            return render_commit_result(None, "The workspace draft is invalid.", false, false);
        }
    };
    let prepared = match prepare_workspace_commit(&context, draft, &input.message) {
        Ok(prepared) => prepared,
        Err(error) => {
            return render_commit_result(
                error.draft.as_deref(),
                &error.message,
                false,
                error.reload,
            );
        }
    };
    let result = context.handle.commit_changes(
        &prepared.changes,
        prepared.expected_head,
        &context.user.username,
        &prepared.email,
        &prepared.message,
    );
    render_workspace_commit_result(&params, &context, prepared, result)
}

#[get("/{namespace}/{repo}/edit/file/{path:.*}")]
pub async fn file_fragment(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<FileParams>,
) -> HttpResponse {
    let params = params.into_inner();
    let context = match open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        Ok(context) => context,
        Err(response) => return response,
    };

    html_response(editor_file_fragment(
        &context,
        &params.namespace,
        &params.repo,
        &params.path,
    ))
}

#[get("/{namespace}/{repo}/edit/{path:.*}")]
pub async fn legacy_file_entry(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<FileParams>,
) -> HttpResponse {
    let params = params.into_inner();
    if !is_safe_repo_path(&params.path) || params.path.is_empty() {
        return HttpResponse::NotFound().finish();
    }
    if let Err(response) = open_workspace_context(
        &req,
        &server,
        &auth_state,
        &params.namespace,
        &params.repo,
        NamespacePermission::PushChanges,
    )
    .await
    {
        return response;
    }

    HttpResponse::Found()
        .insert_header((
            "Location",
            workspace_file_url(&params.namespace, &params.repo, &params.path),
        ))
        .finish()
}
