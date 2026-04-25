use actix_web::HttpRequest;
use actix_web::web;

use crate::auth::FigContext;

/// Helper function to get the username from the session cookie if logged in
pub async fn get_username_from_request(
    req: &HttpRequest,
    auth_state: &web::Data<FigContext>,
) -> Option<String> {
    let token = req.cookie("session")?;
    let user_id = auth_state.validate_token(token.value()).await?;
    let user = auth_state.db.get_user_by_id(&user_id).await.ok()??;
    Some(user.username)
}
