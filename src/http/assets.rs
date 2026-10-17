use actix_web::{
    HttpRequest, HttpResponse, Responder, get,
    http::header::{CACHE_CONTROL, HeaderValue},
    web,
};

use crate::config;

/// Version segment embedded in every asset URL the UI renders.
pub const ASSET_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Unversioned URLs share one cache entry across releases, so they must revalidate.
pub const UNVERSIONED_CACHE_CONTROL: &str = "no-cache";

struct Asset {
    stem: &'static str,
    extension: &'static str,
    content_type: &'static str,
    body: &'static str,
}

impl Asset {
    fn name(&self) -> String {
        format!("{}.{}", self.stem, self.extension)
    }

    fn versioned_name(&self) -> String {
        format!("{}-{}.{}", self.stem, ASSET_VERSION, self.extension)
    }
}

const ASSETS: &[Asset] = &[
    Asset {
        stem: "t",
        extension: "css",
        content_type: "text/css; charset=utf-8",
        body: include_str!("../../assets/t.css"),
    },
    Asset {
        stem: "twig",
        extension: "css",
        content_type: "text/css; charset=utf-8",
        body: include_str!("../../assets/twig.css"),
    },
    Asset {
        stem: "theme",
        extension: "js",
        content_type: "application/javascript; charset=utf-8",
        body: include_str!("../../assets/theme.js"),
    },
    Asset {
        stem: "h",
        extension: "js",
        content_type: "application/javascript; charset=utf-8",
        body: include_str!("../../assets/h.js"),
    },
    Asset {
        stem: "hx-live",
        extension: "js",
        content_type: "application/javascript; charset=utf-8",
        body: include_str!("../../assets/hx-live.js"),
    },
    Asset {
        stem: "repository-editor",
        extension: "js",
        content_type: "application/javascript; charset=utf-8",
        body: include_str!("../../assets/repository-editor.js"),
    },
    Asset {
        stem: "quill",
        extension: "js",
        content_type: "application/javascript; charset=utf-8",
        body: include_str!("../../assets/vendor/quill/quill.min.js"),
    },
    Asset {
        stem: "quill.snow",
        extension: "css",
        content_type: "text/css; charset=utf-8",
        body: include_str!("../../assets/vendor/quill/quill.snow.css"),
    },
    Asset {
        stem: "marked",
        extension: "js",
        content_type: "application/javascript; charset=utf-8",
        body: include_str!("../../assets/vendor/marked/marked.umd.js"),
    },
    Asset {
        stem: "purify",
        extension: "js",
        content_type: "application/javascript; charset=utf-8",
        body: include_str!("../../assets/vendor/dompurify/purify.min.js"),
    },
    Asset {
        stem: "turndown",
        extension: "js",
        content_type: "application/javascript; charset=utf-8",
        body: include_str!("../../assets/vendor/turndown/turndown.js"),
    },
    Asset {
        stem: "twig",
        extension: "svg",
        content_type: "image/svg+xml",
        body: include_str!("../../assets/twig.svg"),
    },
    Asset {
        stem: "twig.schema",
        extension: "json",
        content_type: "application/schema+json; charset=utf-8",
        body: include_str!("../../assets/twig.schema.json"),
    },
];

fn find(filename: &str) -> Option<(&'static Asset, bool)> {
    ASSETS.iter().find_map(|asset| {
        if filename == asset.versioned_name() {
            Some((asset, true))
        } else if filename == asset.name() {
            Some((asset, false))
        } else {
            None
        }
    })
}

/// Path of a versioned asset, e.g. `/assets/twig-<version>.css`, so a browser only
/// reuses its cached copy while the version it was fetched under still matches.
pub fn url(filename: &str) -> String {
    match find(filename) {
        Some((asset, _)) => format!("/assets/{}", asset.versioned_name()),
        None => format!("/assets/{filename}"),
    }
}

#[get("/assets/{filename:.*}")]
pub async fn assets(req: HttpRequest, config: web::Data<config::Server>) -> impl Responder {
    let path = req.match_info().query("filename");

    match find(path) {
        Some((asset, true)) => HttpResponse::Ok()
            .content_type(asset.content_type)
            .insert_header((CACHE_CONTROL, config.cache_control().clone()))
            .body(asset.body),
        Some((asset, false)) => HttpResponse::Ok()
            .content_type(asset.content_type)
            .insert_header((
                CACHE_CONTROL,
                HeaderValue::from_static(UNVERSIONED_CACHE_CONTROL),
            ))
            .body(asset.body),
        None => HttpResponse::NotFound().body("Not found"),
    }
}

#[cfg(test)]
#[path = "assets/tests.rs"]
mod tests;
