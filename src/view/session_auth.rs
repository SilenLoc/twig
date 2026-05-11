use actix_web::HttpRequest;
use actix_web::web;

use crate::auth::FigContext;

pub async fn get_username_from_request(
    req: &HttpRequest,
    auth_state: &web::Data<FigContext>,
) -> Option<String> {
    let token = req.cookie("session")?;
    let db = auth_state.db();
    db.get_username_by_token(token.value()).await.ok().flatten()
}
