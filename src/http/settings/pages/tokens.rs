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
                p class="twig-empty-body" { "You have not created any access tokens." }
            }
        } @else {
            div class="twig-list" {
                @for token in tokens {
                    div class="twig-row" {
                        div class="twig-row-id" { (token.name) }
                        p class="twig-row-meta" { "twigt_••••" (token.token_hint) }
                        p class="twig-row-meta" {
                            "Created " (token.created_at)
                            " · Last used " (token.last_used_at.as_deref().unwrap_or("Never"))
                            " · Expires " (token.expires_at.as_deref().unwrap_or("Never"))
                        }
                        @for scope in &token.scopes {
                            p class="twig-row-meta" {
                                (scope.namespace_name) ": "
                                @for (index, permission) in scope.permissions.iter().enumerate() {
                                    @if index > 0 { ", " }
                                    (permission.display_name())
                                }
                            }
                        }
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

fn render_token_panel(
    namespaces: &[Namespace],
    tokens: &[PersonalAccessToken],
    created_secret: Option<&str>,
) -> maud::Markup {
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

                    @if let Some(secret) = created_secret {
                        (render_created_secret(secret))
                    }

                    form
                        class="twig-form"
                        hx-post="/settings/tokens"
                        hx-target="#token-create-result"
                        hx-swap="innerHTML"
                        "hx-status:4xx"="swap:innerHTML target:#token-create-result"
                    {
                        h3 class="twig-subsection" { "Create a token" }
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

                    section aria-labelledby="existing-tokens-heading" {
                        h3 class="twig-subsection" id="existing-tokens-heading" { "Existing tokens" }
                        (render_token_list(tokens, None))
                    }
                }
            }
        }
    }
}

#[get("/settings/tokens")]
pub async fn personal_access_tokens_page(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
) -> actix_web::Result<maud::Markup> {
    let Some(user_id) = auth_state.user_id_from_request(&req).await else {
        let message = if req.cookie("session").is_some() {
            "Session expired. Please log in again."
        } else {
            "Not logged in. Please log in first."
        };
        return Ok(render_settings_auth_error(&req, message));
    };

    let db = auth_state.db();
    let Ok(Some(user)) = db.get_user_by_id(&user_id).await else {
        return Ok(render_error("Failed to load user."));
    };
    let namespaces = match accessible_namespaces(db, &server, &user).await {
        Ok(namespaces) => namespaces,
        Err(error) => {
            log::error!("Failed to load accessible namespaces for token settings: {error}");
            return Ok(render_error("Failed to load namespaces."));
        }
    };
    let tokens = match db.list_personal_access_tokens(&user_id).await {
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
        (render_token_panel(&namespaces, &tokens, None))
    };

    if req.headers().get("HX-Request").is_some() {
        Ok(content)
    } else {
        Ok(crate::http::view::render_layout(
            &content,
            Some(&user.username),
            Some("Access Tokens"),
        ))
    }
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
    let mut tokens = match db.list_personal_access_tokens(&user_id).await {
        Ok(tokens) => tokens,
        Err(error) => {
            log::error!("Failed to load current tokens before creation: {error}");
            return HttpResponse::InternalServerError()
                .body(render_error("Failed to load access tokens.").into_string());
        }
    };
    let (token, secret) = match auth_state
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
    tokens.insert(0, token);

    let content = maud::html! {
        (render_created_secret(&secret))
        div id="token-list" hx-swap-oob="outerHTML" {
            (render_token_list_contents(&tokens, None))
        }
    };
    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
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
