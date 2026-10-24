use std::collections::{BTreeSet, HashMap};

use actix_web::{HttpRequest, HttpResponse, get, post, web};

use crate::auth::permissions::{self, NamespacePermission, PermissionSet};
use crate::auth::{Namespace, PersonalAccessToken, TwigContext, User};
use crate::config;
use crate::http::view::render_error;

use super::shared::{render_account_tabs, render_settings_auth_error, render_settings_navigation};

fn render_permission_checkbox(permission: NamespacePermission) -> maud::Markup {
    let label = match permission {
        NamespacePermission::ViewNamespace => "View namespace and repositories",
        _ => permission.display_name(),
    };
    maud::html! {
        label {
            input type="checkbox" name=(format!("permission_{}", permission.as_str())) value="true";
            " " (label)
        }
    }
}

fn render_permission_choices() -> maud::Markup {
    maud::html! {
        fieldset class="twig-stack twig-stack--tight" {
            legend class="twig-subsection" { "2. Choose permissions" }
            p class="twig-body-sm twig-ink-tertiary" {
                "These permissions apply to every selected namespace. You can only grant actions you already have access to in all selected namespaces."
            }
            label {
                input type="checkbox" name="all_read" value="true";
                " Select all read permissions"
            }
            @for permission in NamespacePermission::READ {
                (render_permission_checkbox(permission))
            }
            label {
                input type="checkbox" name="all_write" value="true";
                " Select all write permissions"
            }
            @for permission in NamespacePermission::WRITE {
                (render_permission_checkbox(permission))
            }
        }
    }
}

fn render_token_tabs(active: &str) -> maud::Markup {
    maud::html! {
        nav class="twig-tabs twig-tabs--nested" aria-label="Access token views" {
            a
                class="twig-tab"
                aria-current=[(active == "create").then_some("page")]
                href="/settings/tokens/new"
            { "Create token" }
            a
                class="twig-tab"
                aria-current=[(active == "existing").then_some("page")]
                href="/settings/tokens"
            { "Existing tokens" }
        }
    }
}

fn render_token_list_contents(
    tokens: &[PersonalAccessToken],
    notice: Option<&str>,
) -> maud::Markup {
    maud::html! {
        @if let Some(notice) = notice {
            div class="twig-notice twig-notice--success" role="status" {
                p class="twig-notice-body" { (notice) }
            }
        }
        @if tokens.is_empty() {
            div class="twig-empty" {
                p class="twig-eyebrow" { "NO TOKENS" }
                p class="twig-empty-body" {
                    "You have not created any access tokens. "
                    a href="/settings/tokens/new" { "Create a token" }
                    "."
                }
            }
        } @else {
            div class="twig-table-scroll" {
                table class="twig-table" {
                    thead {
                        tr {
                            th scope="col" { "Name" }
                            th scope="col" { "Token" }
                            th scope="col" { "Last used" }
                            th scope="col" { "Actions" }
                        }
                    }
                    tbody {
                        @for token in tokens {
                            tr {
                                td {
                                    a href=(format!("/settings/tokens/{}", token.id)) { (token.name) }
                                }
                                td class="twig-data" { "twigt_••••" (token.token_hint) }
                                td { (token.last_used_at.as_deref().unwrap_or("Never")) }
                                td {
                                    form
                                        class="twig-form-actions"
                                        hx-post=(format!("/settings/tokens/{}/revoke", token.id))
                                        hx-target="#token-list"
                                        hx-swap="outerHTML"
                                        hx-confirm=(format!("Revoke '{}' permanently?", token.name))
                                        "hx-status:4xx"="swap:innerHTML target:#token-list"
                                    {
                                        button class="twig-btn twig-btn--danger" type="submit" { "Revoke" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_token_list(tokens: &[PersonalAccessToken], notice: Option<&str>) -> maud::Markup {
    maud::html! {
        div id="token-list" {
            (render_token_list_contents(tokens, notice))
        }
    }
}

fn render_created_secret(secret: &str) -> maud::Markup {
    maud::html! {
        div class="twig-notice twig-notice--warning" role="alert" {
            p class="twig-eyebrow" { "COPY THIS TOKEN NOW" }
            p class="twig-notice-body" { "This secret is shown only once. Store it somewhere safe; it cannot be recovered later." }
            code class="twig-code" { (secret) }
        }
    }
}

fn render_create_token_form(namespaces: &[Namespace]) -> maud::Markup {
    maud::html! {
        form
            class="twig-form"
            hx-post="/settings/tokens"
            hx-target="#token-create-result"
            hx-swap="innerHTML"
            "hx-status:4xx"="swap:innerHTML target:#token-create-result"
        {
            div class="twig-field" {
                label class="twig-label" for="token-name" { "Token name" }
                input
                    class="twig-input"
                    type="text"
                    name="name"
                    id="token-name"
                    required
                    maxlength="80"
                    placeholder="e.g. Build server";
            }

            fieldset class="twig-stack twig-stack--tight" {
                legend class="twig-subsection" { "1. Choose namespaces" }
                p class="twig-body-sm twig-ink-tertiary" {
                    "All namespaces selects the namespaces you can access now; namespaces you gain later are not added automatically."
                }
                label {
                    input type="checkbox" name="all_namespaces" value="true";
                    " All namespaces I can access"
                }
                @for namespace in namespaces {
                    label {
                        input
                            type="checkbox"
                            name=(format!("namespace_{}", namespace.id))
                            value="true";
                        " " (namespace.name)
                    }
                }
            }

            (render_permission_choices())

            div class="twig-form-actions" {
                button class="twig-btn twig-btn--primary" type="submit" { "Create token" }
            }
            div id="token-create-result" aria-live="polite" {}
        }
    }
}

fn render_token_panel(active: &str, content: &maud::Markup) -> maud::Markup {
    maud::html! {
        div class="twig-bento" {
            section class="twig-panel" aria-labelledby="settings-tokens-heading" {
                header class="twig-panel-head" {
                    h2 class="twig-eyebrow" id="settings-tokens-heading" { "Personal access tokens" }
                }
                div class="twig-panel-body twig-stack" {
                    p class="twig-body-sm twig-ink-secondary" {
                        "Create a token for Git and API access. Tokens do not expire and can be revoked at any time."
                    }
                    (render_token_tabs(active))
                    (content)
                }
            }
        }
    }
}

fn render_token_details(token: &PersonalAccessToken) -> maud::Markup {
    maud::html! {
        div class="twig-bento" {
            section class="twig-panel" aria-labelledby="token-details-heading" {
                header class="twig-panel-head" {
                    a class="twig-btn twig-btn--quiet" href="/settings/tokens" { "Back to tokens" }
                    h2 class="twig-eyebrow" id="token-details-heading" { (token.name) }
                }
                div class="twig-panel-body twig-stack" {
                    div class="twig-field" {
                        span class="twig-label" { "Token" }
                        p class="twig-body twig-break" { "twigt_••••" (token.token_hint) }
                    }
                    div class="twig-field" {
                        span class="twig-label" { "Created" }
                        p class="twig-body" { (token.created_at) }
                    }
                    div class="twig-field" {
                        span class="twig-label" { "Last used" }
                        p class="twig-body" { (token.last_used_at.as_deref().unwrap_or("Never")) }
                    }
                    div class="twig-field" {
                        span class="twig-label" { "Expires" }
                        p class="twig-body" { (token.expires_at.as_deref().unwrap_or("Never")) }
                    }

                    section aria-labelledby="token-access-heading" {
                        h3 class="twig-subsection" id="token-access-heading" { "Namespace access" }
                        @if token.scopes.is_empty() {
                            p class="twig-body-sm twig-ink-tertiary" { "This token has no namespace access." }
                        } @else {
                            div class="twig-table-scroll" {
                                table class="twig-table" {
                                    thead {
                                        tr {
                                            th scope="col" { "Namespace" }
                                            th scope="col" { "Permissions" }
                                        }
                                    }
                                    tbody {
                                        @for scope in &token.scopes {
                                            tr {
                                                td { (scope.namespace_name) }
                                                td {
                                                    @for (index, permission) in scope.permissions.iter().enumerate() {
                                                        @if index > 0 { ", " }
                                                        (permission.display_name())
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_tokens_response(
    req: &HttpRequest,
    username: &str,
    content: maud::Markup,
) -> maud::Markup {
    if req.headers().get("HX-Request").is_some() {
        content
    } else {
        crate::http::view::render_layout(&content, Some(username), Some("Access Tokens"))
    }
}

async fn load_signed_in_user(
    req: &HttpRequest,
    auth_state: &TwigContext,
) -> Result<User, maud::Markup> {
    let Some(user_id) = auth_state.user_id_from_request(req).await else {
        let message = if req.cookie("session").is_some() {
            "Session expired. Please log in again."
        } else {
            "Not logged in. Please log in first."
        };
        return Err(render_settings_auth_error(req, message));
    };

    match auth_state.db().get_user_by_id(&user_id).await {
        Ok(Some(user)) => Ok(user),
        _ => Err(render_error("Failed to load user.")),
    }
}

#[get("/settings/tokens")]
pub async fn personal_access_tokens_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> actix_web::Result<maud::Markup> {
    let user = match load_signed_in_user(&req, &auth_state).await {
        Ok(user) => user,
        Err(markup) => return Ok(markup),
    };

    let db = auth_state.db();
    let tokens = match db.list_personal_access_tokens(&user.id).await {
        Ok(tokens) => tokens,
        Err(error) => {
            log::error!("Failed to list personal access tokens: {error}");
            return Ok(render_error("Failed to load access tokens."));
        }
    };

    let navigation = render_settings_navigation(&server, db, &user).await;
    let content = maud::html! {
        (navigation)
        (render_account_tabs("tokens"))
        (render_token_panel("existing", &render_token_list(&tokens, None)))
    };

    Ok(render_tokens_response(&req, &user.username, content))
}

#[get("/settings/tokens/new")]
pub async fn new_personal_access_token_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> actix_web::Result<maud::Markup> {
    let user = match load_signed_in_user(&req, &auth_state).await {
        Ok(user) => user,
        Err(markup) => return Ok(markup),
    };

    let db = auth_state.db();
    let namespaces = match accessible_namespaces(db, &server, &user).await {
        Ok(namespaces) => namespaces,
        Err(error) => {
            log::error!("Failed to load accessible namespaces for token settings: {error}");
            return Ok(render_error("Failed to load namespaces."));
        }
    };

    let navigation = render_settings_navigation(&server, db, &user).await;
    let content = maud::html! {
        (navigation)
        (render_account_tabs("tokens"))
        (render_token_panel("create", &render_create_token_form(&namespaces)))
    };

    Ok(render_tokens_response(&req, &user.username, content))
}

#[get("/settings/tokens/{token_id}")]
pub async fn personal_access_token_details_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    token_id: web::Path<String>,
) -> actix_web::Result<maud::Markup> {
    let user = match load_signed_in_user(&req, &auth_state).await {
        Ok(user) => user,
        Err(markup) => return Ok(markup),
    };

    let db = auth_state.db();
    let tokens = match db.list_personal_access_tokens(&user.id).await {
        Ok(tokens) => tokens,
        Err(error) => {
            log::error!("Failed to load personal access token details: {error}");
            return Ok(render_error("Failed to load access tokens."));
        }
    };
    let Some(token) = tokens.iter().find(|token| token.id == *token_id) else {
        return Ok(render_error("Token not found."));
    };

    let navigation = render_settings_navigation(&server, db, &user).await;
    let content = maud::html! {
        (navigation)
        (render_account_tabs("tokens"))
        (render_token_details(token))
    };

    Ok(render_tokens_response(&req, &user.username, content))
}

#[post("/settings/tokens")]
pub async fn create_personal_access_token(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    form: web::Form<HashMap<String, String>>,
) -> HttpResponse {
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };
    let db = auth_state.db();
    let user = match db.get_user_by_id(&user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return HttpResponse::Unauthorized()
                .body(render_error("Session expired. Please log in again.").into_string());
        }
        Err(error) => {
            log::error!("Failed to load user for token creation: {error}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to load user.").into_string());
        }
    };

    let name = form.get("name").map_or("", String::as_str).trim();
    if name.is_empty() || name.chars().count() > 80 {
        return HttpResponse::BadRequest()
            .body(render_error("Token names must be between 1 and 80 characters.").into_string());
    }

    let namespaces = match accessible_namespaces(db, &server, &user).await {
        Ok(namespaces) => namespaces,
        Err(error) => {
            log::error!("Failed to load accessible namespaces for token creation: {error}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to load namespaces.").into_string());
        }
    };
    let selected_namespaces = match select_namespaces(&form, &namespaces) {
        Ok(namespaces) => namespaces,
        Err(message) => {
            return HttpResponse::BadRequest().body(render_error(&message).into_string());
        }
    };
    let selected_permissions = match select_permissions(&form) {
        Ok(permissions) => permissions,
        Err(message) => {
            return HttpResponse::BadRequest().body(render_error(&message).into_string());
        }
    };
    if let Err(message) = permissions::validate_selected_permissions(
        db,
        &user.id,
        server.is_configured_admin(&user.username),
        &selected_namespaces,
        &selected_permissions,
    )
    .await
    {
        return HttpResponse::BadRequest().body(render_error(&message).into_string());
    }

    let namespace_ids = selected_namespaces
        .iter()
        .map(|namespace| namespace.id.clone())
        .collect::<Vec<_>>();
    let (_, secret) = match auth_state
        .issue_personal_access_token(&user_id, name, &namespace_ids, &selected_permissions, None)
        .await
    {
        Ok(result) => result,
        Err(error) => {
            log::error!("Failed to create personal access token: {error}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to create access token.").into_string());
        }
    };

    HttpResponse::Ok()
        .content_type("text/html")
        .body(render_created_secret(&secret).into_string())
}

#[post("/settings/tokens/{token_id}/revoke")]
pub async fn revoke_personal_access_token(
    req: HttpRequest,
    auth_state: web::Data<TwigContext>,
    token_id: web::Path<String>,
) -> HttpResponse {
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Not logged in. Please log in first.").into_string());
    };

    match auth_state
        .db()
        .revoke_personal_access_token(&user_id, &token_id)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::NotFound().body(render_error("Token not found.").into_string());
        }
        Err(error) => {
            log::error!("Failed to revoke personal access token: {error}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to revoke access token.").into_string());
        }
    }

    match auth_state.db().list_personal_access_tokens(&user_id).await {
        Ok(tokens) => HttpResponse::Ok()
            .content_type("text/html")
            .body(render_token_list(&tokens, Some("Access token revoked.")).into_string()),
        Err(error) => {
            log::error!("Failed to refresh token list after revocation: {error}");
            HttpResponse::InternalServerError().body(
                render_error("Token was revoked, but the list could not be refreshed.")
                    .into_string(),
            )
        }
    }
}

async fn accessible_namespaces(
    db: &crate::db::Database,
    server: &config::Server,
    user: &User,
) -> Result<Vec<Namespace>, String> {
    permissions::namespaces_for_user(
        db,
        &user.id,
        server.is_configured_admin(&user.username),
        NamespacePermission::ViewNamespace,
    )
    .await
}

fn select_namespaces(
    form: &HashMap<String, String>,
    accessible: &[Namespace],
) -> Result<Vec<Namespace>, String> {
    let selected_ids = form
        .iter()
        .filter_map(|(key, value)| {
            (value == "true")
                .then(|| key.strip_prefix("namespace_"))
                .flatten()
                .map(str::to_owned)
        })
        .collect::<BTreeSet<_>>();
    let select_all = form
        .get("all_namespaces")
        .is_some_and(|value| value == "true");
    permissions::select_namespaces(accessible, select_all, &selected_ids)
}

fn select_permissions(form: &HashMap<String, String>) -> Result<PermissionSet, String> {
    let requested = form
        .iter()
        .filter_map(|(key, value)| {
            (value == "true")
                .then(|| key.strip_prefix("permission_"))
                .flatten()
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    let all_read = form.get("all_read").is_some_and(|value| value == "true");
    let all_write = form.get("all_write").is_some_and(|value| value == "true");
    permissions::select_permissions(all_read, all_write, &requested)
}
