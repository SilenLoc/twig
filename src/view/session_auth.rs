use actix_web::{HttpRequest, web};

use crate::auth::FigContext;

pub async fn get_username_from_request(
    req: &HttpRequest,
    auth_state: &web::Data<FigContext>,
) -> Option<String> {
    let db = auth_state.db();
    let user_id = auth_state.user_id_from_request(req).await?;
    db.get_user_by_id(&user_id)
        .await
        .ok()
        .flatten()
        .map(|user| user.username)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auth::FigContext, db::Database};
    use actix_web::test;

    #[tokio::test]
    async fn test_get_username_from_request_without_cookie() {
        let db = Database::new("/tmp/test_fig_session_auth_no_cookie.db");
        let auth_state = web::Data::new(FigContext::new(db, "key".to_string()));
        let req = test::TestRequest::default().to_http_request();
        let result = get_username_from_request(&req, &auth_state).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_username_from_request_with_invalid_cookie() {
        let db = Database::new("/tmp/test_fig_session_auth_invalid_cookie.db");
        let auth_state = web::Data::new(FigContext::new(db, "key".to_string()));
        let req = test::TestRequest::default()
            .cookie(actix_web::cookie::Cookie::new("session", "invalid-token"))
            .to_http_request();
        let result = get_username_from_request(&req, &auth_state).await;
        assert!(result.is_none());
    }
}
