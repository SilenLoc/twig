use actix_web::{HttpRequest, web};

use crate::auth::TwigContext;

pub fn device_return_to(user_code: &str) -> Option<String> {
    let normalized = crate::auth::device::normalize_user_code(user_code).ok()?;
    Some(format!(
        "/auth/device?user_code={}",
        crate::auth::device::format_user_code(&normalized)
    ))
}

pub fn safe_device_return_to(value: &str) -> Option<&str> {
    let user_code = value.strip_prefix("/auth/device?user_code=")?;
    let normalized = crate::auth::device::normalize_user_code(user_code).ok()?;
    (crate::auth::device::format_user_code(&normalized) == user_code).then_some(value)
}

pub async fn get_username_from_request(
    req: &HttpRequest,
    auth_state: &web::Data<TwigContext>,
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
    use crate::{auth::TwigContext, db::Database};
    use actix_web::test;

    #[tokio::test]
    async fn test_get_username_from_request_without_cookie() {
        let db = Database::new("/tmp/test_twig_session_auth_no_cookie.db");
        let auth_state = web::Data::new(TwigContext::new(db, "key".to_string()));
        let req = test::TestRequest::default().to_http_request();
        let result = get_username_from_request(&req, &auth_state).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_username_from_request_with_invalid_cookie() {
        let db = Database::new("/tmp/test_twig_session_auth_invalid_cookie.db");
        let auth_state = web::Data::new(TwigContext::new(db, "key".to_string()));
        let req = test::TestRequest::default()
            .cookie(actix_web::cookie::Cookie::new("session", "invalid-token"))
            .to_http_request();
        let result = get_username_from_request(&req, &auth_state).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn login_return_target_only_accepts_canonical_device_codes() {
        let valid = "/auth/device?user_code=WQJH-8K2M";
        assert_eq!(safe_device_return_to(valid), Some(valid));
        assert_eq!(device_return_to("wqjh8k2m"), Some(valid.to_string()));
        for invalid in [
            "https://attacker.example/",
            "//attacker.example/",
            "/settings",
            "/auth/device?user_code=WQJH-8K2M&next=https://attacker.example",
            "/auth/device?user_code=wqjh-8k2m",
        ] {
            assert_eq!(safe_device_return_to(invalid), None, "accepted {invalid}");
        }
    }
}
