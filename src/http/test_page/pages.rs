use qrcode::QrCode;
use qrcode::render::svg;
use rand::TryRng;
use rand::rngs::SysRng;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TestPageQuery {
    pub pin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallerRole {
    Admin { username: String },
    Guest { pin: String },
}

impl CallerRole {
    #[must_use]
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Admin { .. })
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::Admin { username } => format!("ADMIN: {username}"),
            Self::Guest { pin } => format!("GUEST (PIN: {pin})"),
        }
    }

    #[must_use]
    pub fn username(&self) -> Option<&str> {
        match self {
            Self::Admin { username } => Some(username.as_str()),
            Self::Guest { .. } => None,
        }
    }
}

#[must_use]
pub fn generate_random_pin() -> String {
    let mut buf = [0u8; 4];
    SysRng
        .try_fill_bytes(&mut buf)
        .expect("operating system RNG should be available");
    let num = (u32::from_be_bytes(buf) % 900_000) + 100_000;
    format!("{num:06}")
}

pub fn generate_qr_svg(url: &str) -> Result<String, String> {
    let code = QrCode::new(url.as_bytes()).map_err(|e| e.to_string())?;
    let svg_str = code
        .render::<svg::Color>()
        .min_dimensions(180, 180)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Ok(svg_str)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestEndpoint {
    pub id: &'static str,
    pub category: &'static str,
    pub method: &'static str,
    pub url: String,
}

mod catalog;
mod handlers;
mod views;

pub use catalog::list_test_endpoints;
pub use handlers::{
    test_feature_check, test_page_alias, test_pin_create, test_pin_qr, test_pin_remove, test_ping,
    test_runner, test_stopped,
};
pub use views::{
    render_idle_content, render_pin_entry_page, render_pin_panel, render_runner_content,
    render_stopped_content, render_test_page,
};

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
