//! Upload and serving of ticket image attachments.
//!
//! Attachments are content-addressed on disk outside the git repository, so an
//! upload can never conflict with another and a clone of the ticket repo stays
//! small. The stored type is decided by sniffing magic bytes, never by the
//! filename or `Content-Type` the client supplies.

use actix_multipart::Multipart;
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use futures_util::StreamExt;
use serde::Deserialize;

use crate::auth::FigContext;
use crate::config;
use crate::ticket::attachment::{self, MAX_ATTACHMENT_BYTES};
use crate::view::render_error;

#[derive(Deserialize)]
pub struct NamespaceParams {
    namespace: String,
}

#[derive(Deserialize)]
pub struct AttachmentParams {
    namespace: String,
    digest: String,
}

/// Accepts one image and returns the markdown snippet that embeds it.
#[post("/{namespace}/ticket/attachment")]
pub async fn upload_handler(
    req: HttpRequest,
    path: web::Path<NamespaceParams>,
    mut payload: Multipart,
    server: web::Data<config::Server>,
    auth_state: web::Data<FigContext>,
) -> impl Responder {
    let namespace = path.into_inner().namespace;

    let Some(cookie) = req.cookie("session") else {
        return HttpResponse::Unauthorized()
            .body(render_error("Log in to attach images").into_string());
    };
    let Some(user_id) = auth_state.validate_token(cookie.value()).await else {
        return HttpResponse::Unauthorized()
            .body(render_error("Session expired. Please log in again.").into_string());
    };

    match auth_state
        .db()
        .user_has_namespace_access(&user_id, &namespace)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Forbidden()
                .body(render_error("Access denied to namespace").into_string());
        }
        Err(e) => {
            log::error!("Database error checking namespace access: {e}");
            return HttpResponse::InternalServerError()
                .body(render_error("Database error").into_string());
        }
    }

    let mut bytes: Vec<u8> = Vec::new();
    let mut saw_field = false;

    while let Some(field) = payload.next().await {
        let mut field = match field {
            Ok(field) => field,
            Err(e) => {
                log::warn!("Malformed multipart upload: {e}");
                return HttpResponse::BadRequest()
                    .body(render_error("Malformed upload").into_string());
            }
        };

        saw_field = true;
        while let Some(chunk) = field.next().await {
            let Ok(chunk) = chunk else {
                return HttpResponse::BadRequest()
                    .body(render_error("Upload failed midway").into_string());
            };
            // Cap while streaming so an oversized upload is not buffered whole.
            if bytes.len() + chunk.len() > MAX_ATTACHMENT_BYTES {
                return HttpResponse::PayloadTooLarge().body(
                    render_error(&format!(
                        "Attachments are limited to {} MB",
                        MAX_ATTACHMENT_BYTES / (1024 * 1024)
                    ))
                    .into_string(),
                );
            }
            bytes.extend_from_slice(&chunk);
        }

        if !bytes.is_empty() {
            break; // first non-empty field wins
        }
    }

    if !saw_field || bytes.is_empty() {
        return HttpResponse::BadRequest().body(render_error("No file uploaded").into_string());
    }

    let root = server.attachment_root().to_string();
    let stored = web::block(move || attachment::store(&root, &namespace, &bytes)).await;

    match stored {
        Ok(Ok((digest, kind))) => {
            let path = path_for_response(&req, &digest);
            log::info!("Stored {} attachment {digest}", kind.extension());
            HttpResponse::Ok()
                .content_type("text/html")
                .body(render_upload_success(&path).into_string())
        }
        Ok(Err(message)) => HttpResponse::BadRequest().body(render_error(&message).into_string()),
        Err(e) => {
            log::error!("Attachment task failed: {e}");
            HttpResponse::InternalServerError()
                .body(render_error("Failed to store attachment").into_string())
        }
    }
}

fn path_for_response(req: &HttpRequest, digest: &str) -> String {
    let namespace = req.match_info().query("namespace").to_string();
    attachment::url(&namespace, digest)
}

fn render_upload_success(url: &str) -> maud::Markup {
    let snippet = format!("![image]({url})");
    maud::html! {
        div class="ba b--white-20 pa3 bg-black-20 mt3" {
            p class="tf-kicker white-50 mb2" { "Uploaded" }
            p class="f6 white-70 mb2" { "Paste this into the ticket body or a comment:" }
            code class="db pa2 bg-black-50 white f6" style="word-break: break-all;" { (snippet) }
            div class="mt3" {
                img src=(url) alt="Uploaded attachment" class="mw-100" style="max-height: 12rem;";
            }
        }
    }
}

/// Serves an attachment by content address.
#[get("/{namespace}/ticket/attachment/{digest}")]
pub async fn serve_handler(
    path: web::Path<AttachmentParams>,
    server: web::Data<config::Server>,
) -> impl Responder {
    let AttachmentParams { namespace, digest } = path.into_inner();
    let root = server.attachment_root().to_string();

    let loaded = web::block(move || attachment::load(&root, &namespace, &digest)).await;

    match loaded {
        Ok(Some((kind, bytes))) => HttpResponse::Ok()
            .content_type(kind.content_type())
            // Content-addressed, so the bytes behind this URL can never change.
            .insert_header(("Cache-Control", "public, max-age=31536000, immutable"))
            .body(bytes),
        Ok(None) => HttpResponse::NotFound().body("Not found"),
        Err(e) => {
            log::error!("Attachment read task failed: {e}");
            HttpResponse::InternalServerError().body("Failed to read attachment")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_upload_success_contains_markdown_snippet() {
        let html = render_upload_success("/acme/ticket/attachment/abc123").into_string();
        assert!(
            html.contains("![image](/acme/ticket/attachment/abc123)"),
            "{html}"
        );
        assert!(
            html.contains(r#"src="/acme/ticket/attachment/abc123""#),
            "{html}"
        );
    }

    #[test]
    fn test_attachment_params_deserialise() {
        let params: AttachmentParams =
            serde_urlencoded::from_str("namespace=acme&digest=deadbeef").unwrap();
        assert_eq!(params.namespace, "acme");
        assert_eq!(params.digest, "deadbeef");
    }
}
