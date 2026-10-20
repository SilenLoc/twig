use actix_web::Result as AwResult;
use actix_web::{HttpRequest, get, web};
use maud::{DOCTYPE, Markup};

use crate::auth::TwigContext;
use crate::config;
use crate::git::bare::{PresentConfig, RepoHandle};
use crate::http::auth::session::get_username_from_request;

use super::context::{
    PRESENT_TEXT_SIZE_KEY, RepoContext, TEXT_SIZES, can_view_namespace, render_empty,
    render_for_request, render_git_error, render_repo_auth_error, text_size_restore,
    text_size_step,
};
use super::markdown::render_slide_markdown;
use super::{Params, SlideParams};

const PDF_PRINT_SCRIPT: &str = r"(function(){
var hint=document.getElementById('twig-pdf-hint');
var ready=function(){
if(hint){hint.setAttribute('data-twig-pdf-ready','true');}
window.print();
};
var fonts='fonts' in document ? document.fonts.ready : Promise.resolve();
var images=Promise.all([].slice.call(document.images).map(function(img){
return img.complete ? null : new Promise(function(resolve){ img.onload=img.onerror=resolve; });
}));
var page=document.readyState==='complete' ? null : new Promise(function(resolve){
window.addEventListener('load',resolve,{once:true});
});
Promise.all([fonts,images,page]).then(ready).catch(ready);
})();";

/// A rendered slide for the presentation view
pub(super) struct PresentSlide {
    pub(super) html: String,
}

pub(super) fn present_body(ctx: &RepoContext) -> Markup {
    let slides = load_present_slides(
        &ctx.handle,
        &ctx.twig_result.config.present,
        &ctx.namespace,
        &ctx.repo,
    );
    render_present_view(&ctx.namespace, &ctx.repo, &slides)
}

#[get("/{namespace}/{repo}/slide/{index}")]
pub async fn slide_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<SlideParams>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let index = params.index;
    let page_title = format!("{namespace}/{repo}");
    let username = get_username_from_request(&req, &auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(h) => h,
        Err(e) => {
            let content = render_git_error(&e);
            return if req.headers().get("HX-Request").is_some() {
                Ok(content)
            } else {
                Ok(crate::http::view::render_layout(
                    &content,
                    username.as_deref(),
                    Some(&page_title),
                ))
            };
        }
    };

    let twig_result = handle.load_config_with_raw();
    if twig_result.config.private {
        match can_view_namespace(&req, &server, &auth_state, namespace).await {
            Ok(true) => {}
            Ok(false) if username.is_none() => {
                return Ok(render_repo_auth_error(&req, &page_title));
            }
            Ok(false) => {
                return Ok(render_for_request(
                    &req,
                    crate::http::view::render_error(
                        "You do not have permission to view this namespace.",
                    ),
                    username.as_deref(),
                    &page_title,
                ));
            }
            Err(error) => {
                log::error!("Failed to authorize private slide read: {error}");
                return Ok(render_for_request(
                    &req,
                    crate::http::view::render_error("Failed to verify namespace permissions."),
                    username.as_deref(),
                    &page_title,
                ));
            }
        }
    }

    let present_slides = load_present_slides(&handle, &twig_result.config.present, namespace, repo);

    if index >= present_slides.len() {
        let content = render_empty("NOT FOUND", "Slide not found");
        if req.headers().get("HX-Request").is_some() {
            Ok(content)
        } else {
            Ok(crate::http::view::render_layout(
                &content,
                username.as_deref(),
                Some(&page_title),
            ))
        }
    } else {
        let content = render_slide_content(namespace, repo, index, &present_slides);
        if req.headers().get("HX-Request").is_some() {
            Ok(content)
        } else {
            Ok(crate::http::view::render_layout(
                &content,
                username.as_deref(),
                Some(&page_title),
            ))
        }
    }
}

/// Converts markdown to HTML, fixing relative links to point to repo root
/// A destination is external when it carries a URL scheme such as `https:` or
/// `mailto:`. A bare relative path never does, because a scheme cannot contain
/// a `/`.
pub(super) fn load_present_slides(
    handle: &RepoHandle,
    present_config: &PresentConfig,
    namespace: &str,
    repo: &str,
) -> Vec<PresentSlide> {
    present_config
        .files
        .iter()
        .filter_map(|file| {
            let content = handle.read_file(file).ok()??;
            let html = render_slide_markdown(
                &content,
                namespace,
                repo,
                file,
                &present_config.template_vars,
            );
            Some(PresentSlide { html })
        })
        .collect()
}

/// The configured paper pages, sorted for reading. Empty when `[paper]` is
/// absent, unconfigured, or its directory is missing from HEAD.
pub(super) fn slide_counter(current_index: usize, slide_count: usize) -> String {
    let width = slide_count.to_string().len();
    format!(
        "{:0width$} / {slide_count}",
        current_index + 1,
        width = width
    )
}

/// A Previous/Next slide control. The button swaps `#present-container` via
/// htmx on click, and the matching arrow key drives the same request while
/// focus stays inside the container — a keyboard shortcut without a global
/// handler, since `keyup` only reaches the container from its own subtree.
pub(super) fn render_slide_nav_button(
    id: &str,
    label: &str,
    target: Option<&str>,
    key: &str,
) -> Markup {
    maud::html! {
        @if let Some(href) = target {
            button
                id=(id)
                class="twig-btn twig-btn--quiet"
                type="button"
                hx-get=(href)
                hx-target="#present-container"
                hx-swap="innerHTML"
                hx-trigger=(format!("click, keyup[key=='{key}'] from:#present-container"))
            {
                (label)
            }
        } @else {
            button id=(id) class="twig-btn twig-btn--quiet" type="button" disabled {
                (label)
            }
        }
    }
}

pub(super) fn render_slide_content(
    namespace: &str,
    repo: &str,
    current_index: usize,
    slides: &[PresentSlide],
) -> Markup {
    let slide_count = slides.len();
    let slide = &slides[current_index];
    let prev =
        (current_index > 0).then(|| format!("/{namespace}/{repo}/slide/{}", current_index - 1));
    let next = (current_index + 1 < slide_count)
        .then(|| format!("/{namespace}/{repo}/slide/{}", current_index + 1));

    maud::html! {
        header class="twig-present-bar" {
            p class="twig-eyebrow" { "PRESENTATION" }
            div class="twig-present-tools" {
                span class="twig-present-count" aria-live="polite" {
                    (slide_counter(current_index, slide_count))
                }
                button id="decrease-text-size" class="twig-btn twig-btn--quiet" type="button"
                    aria-label="Decrease presentation font size" title="Decrease font size"
                    "hx-on:click"=(text_size_step(false, PRESENT_TEXT_SIZE_KEY))
                    "hx-live:disabled"=(format!("data.twigTextSize === {}", TEXT_SIZES[0]))
                {
                    "A−"
                }
                span id="present-text-size" class="twig-present-count" aria-live="polite"
                    "hx-live:text"="data.twigTextSize + '%'" { "100%" }
                button id="increase-text-size" class="twig-btn twig-btn--quiet" type="button"
                    aria-label="Increase presentation font size" title="Increase font size"
                    "hx-on:click"=(text_size_step(true, PRESENT_TEXT_SIZE_KEY))
                    "hx-live:disabled"=(format!(
                        "data.twigTextSize === {}",
                        TEXT_SIZES[TEXT_SIZES.len() - 1]
                    ))
                {
                    "A+"
                }
                button id="fullscreen-toggle" class="twig-btn twig-btn--quiet" type="button"
                    aria-pressed="false"
                    "hx-on:click"="data.twigFullscreen = !data.twigFullscreen"
                    "hx-live:text"="data.twigFullscreen ? 'Exit fullscreen' : 'Fullscreen'"
                    "hx-live:aria-pressed"="data.twigFullscreen"
                {
                    "Fullscreen"
                }
                a class="twig-btn twig-btn--quiet"
                    href=(format!("/{namespace}/{repo}/present/print"))
                    title="Open the print view with every slide and the license page"
                {
                    "Download PDF"
                }
                (crate::http::view::render_theme_toggle())
            }
        }

        div id="slides-wrapper" class="twig-present-stage" {
            article class="twig-md twig-md--slide" aria-live="polite" {
                (maud::PreEscaped(&slide.html))
            }
        }

        nav class="twig-present-controls" aria-label="Slide navigation" {
            (render_slide_nav_button("prev-slide", "\u{2190} Previous", prev.as_deref(), "ArrowLeft"))
            div class="twig-ticks" role="group" aria-label="Go to slide" {
                @for i in 0..slide_count {
                    button
                        class="twig-tick"
                        type="button"
                        aria-label=(format!("Slide {} of {slide_count}", i + 1))
                        aria-current=[(i == current_index).then_some("true")]
                        hx-get=(format!("/{namespace}/{repo}/slide/{i}"))
                        hx-target="#present-container"
                        hx-swap="innerHTML"
                    {}
                }
            }
            (render_slide_nav_button("next-slide", "Next \u{2192}", next.as_deref(), "ArrowRight"))
        }
    }
}

pub(super) fn render_present_view(namespace: &str, repo: &str, slides: &[PresentSlide]) -> Markup {
    if slides.is_empty() {
        return render_empty(
            "NO SLIDES",
            "No presentation slides configured. Add a [present] section with files to your .twig.toml.",
        );
    }

    maud::html! {
        section
            id="present-container"
            class="twig-present"
            data-twig-text-size=(TEXT_SIZES[0])
            hx-live=(text_size_restore(PRESENT_TEXT_SIZE_KEY))
            tabindex="-1"
            aria-roledescription="carousel"
            aria-label="Presentation"
            data-twig-fullscreen="false"
            "hx-on:keydown"="if (event.key === 'Escape') data.twigFullscreen = false"
            "hx-on:htmx:after:swap"="this.focus({ preventScroll: true })"
        {
            (render_slide_content(namespace, repo, 0, slides))
        }
    }
}

/// The PDF export route: the entire deck, one A4 page per slide, with the
/// license page last. It serves a standalone document with no application
/// chrome, so the print dialog captures slides only. Rendering reads the slide
/// files again from the repository, so the output never depends on what the
/// live view currently shows — scroll position, text size, and the active
/// slide have no effect on it.
#[get("/{namespace}/{repo}/present/print")]
pub async fn present_print_handler(
    req: HttpRequest,
    server: web::Data<config::Server>,
    auth_state: web::Data<TwigContext>,
    params: web::Path<Params>,
) -> AwResult<Markup> {
    let namespace = &params.namespace;
    let repo = &params.repo;
    let page_title = format!("{namespace}/{repo}");
    let username = get_username_from_request(&req, &auth_state).await;

    let handle = match RepoHandle::open(server.project_root(), namespace, repo) {
        Ok(handle) => handle,
        Err(e) => {
            let content = render_git_error(&e);
            return Ok(render_for_request(
                &req,
                content,
                username.as_deref(),
                &page_title,
            ));
        }
    };

    let twig_result = handle.load_config_with_raw();
    if twig_result.config.private {
        match can_view_namespace(&req, &server, &auth_state, namespace).await {
            Ok(true) => {}
            Ok(false) if username.is_none() => {
                return Ok(render_repo_auth_error(&req, &page_title));
            }
            Ok(false) => {
                return Ok(render_for_request(
                    &req,
                    crate::http::view::render_error(
                        "You do not have permission to view this namespace.",
                    ),
                    username.as_deref(),
                    &page_title,
                ));
            }
            Err(error) => {
                log::error!("Failed to authorize private presentation read: {error}");
                return Ok(render_for_request(
                    &req,
                    crate::http::view::render_error("Failed to verify namespace permissions."),
                    username.as_deref(),
                    &page_title,
                ));
            }
        }
    }

    let slides = load_present_slides(&handle, &twig_result.config.present, namespace, repo);
    // The license comes from the same repository metadata the License tab reads.
    let license_content = handle.get_license_content();
    let license = if license_content.trim().is_empty() {
        None
    } else {
        Some(license_content.as_str())
    };

    Ok(render_pdf_document(namespace, repo, &slides, license))
}

/// The export binder: a complete HTML document whose head mirrors the app
/// layout (same webfont and stylesheet, so slides look identical), whose body
/// is one page per slide plus the license page, and whose only script waits
/// for fonts and images before opening the print dialog.
pub(super) fn render_pdf_document(
    namespace: &str,
    repo: &str,
    slides: &[PresentSlide],
    license: Option<&str>,
) -> Markup {
    maud::html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (format!("{namespace}/{repo} slides")) " · Twig" }
                link rel="icon" type="image/svg+xml" href=(crate::http::assets::url("twig.svg"));
                script src=(crate::http::assets::url("theme.js")) {}
                link rel="preconnect" href="https://fonts.googleapis.com";
                link rel="preconnect" href="https://fonts.gstatic.com" crossorigin;
                link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@300..700&display=swap";
                link rel="stylesheet" href=(crate::http::assets::url("twig.css"));
            }
            body class="twig-pdf" {
                main class="twig-pdf-binder" {
                    @if slides.is_empty() {
                        section class="twig-pdf-page" aria-label="No slides" {
                            (render_empty(
                                "NO SLIDES",
                                "No presentation slides configured. Add a [present] section with files to your .twig.toml.",
                            ))
                        }
                    } @else {
                        @for (index, slide) in slides.iter().enumerate() {
                            (render_pdf_slide_page(index, slides.len(), slide))
                        }
                    }
                    (render_pdf_license_page(license))
                }
                @if !slides.is_empty() {
                    div id="twig-pdf-hint" class="twig-pdf-hint" role="status" {
                        p class="twig-eyebrow" { "PRINT" }
                        p class="twig-pdf-hint-body" { "Choose “Save as PDF” in the dialog to download the deck." }
                    }
                    script { (maud::PreEscaped(PDF_PRINT_SCRIPT)) }
                }
            }
        }
    }
}

/// The header strip every export page carries: the section name and, on slide
/// pages, the same zero-padded counter the live deck shows.
pub(super) fn render_pdf_pagehead(eyebrow: &str, count: Option<&str>) -> Markup {
    maud::html! {
        header class="twig-pdf-pagehead" {
            p class="twig-eyebrow" { (eyebrow) }
            @if let Some(count) = count {
                span class="twig-present-count" { (count) }
            }
        }
    }
}

/// One slide on one A4 page. The slide reuses the live view's `twig-md--slide`
/// article inside a static, print-safe stage, so the export styling matches
/// the deck at its default 100% text size.
pub(super) fn render_pdf_slide_page(
    index: usize,
    slide_count: usize,
    slide: &PresentSlide,
) -> Markup {
    maud::html! {
        section
            class="twig-pdf-page"
            aria-label=(format!("Slide {} of {slide_count}", index + 1))
        {
            (render_pdf_pagehead("PRESENTATION", Some(&slide_counter(index, slide_count))))
            div class="twig-pdf-stage" {
                article class="twig-md twig-md--slide" {
                    (maud::PreEscaped(&slide.html))
                }
            }
        }
    }
}

/// The final export page: the repository license, always starting on its own
/// sheet. It reads through the same `get_license_content` fallback chain the
/// License tab uses; a repository without any license text gets the same
/// empty state the License tab shows.
pub(super) fn render_pdf_license_page(license: Option<&str>) -> Markup {
    maud::html! {
        section class="twig-pdf-page twig-pdf-page--license" aria-label="License" {
            (render_pdf_pagehead("LICENSE", None))
            @if let Some(license) = license {
                div class="twig-md twig-md--boxed" {
                    (license)
                }
            } @else {
                (render_empty("NO LICENSE", "No license information available."))
            }
        }
    }
}
