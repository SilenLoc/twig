use super::*;

#[test]
fn test_presentation_offers_the_pdf_export() {
    let deck = slides(3);
    let html = render_slide_content("acme", "my-project", 0, &deck).into_string();

    assert!(
        html.contains(r#"href="/acme/my-project/present/print""#),
        "the toolbar links to the print view: {html}"
    );
    assert!(
        html.contains(
            r#"<a class="twig-btn twig-btn--quiet" href="/acme/my-project/present/print""#
        ),
        "Download PDF is a plain navigation link, not an htmx swap: {html}"
    );
}

#[test]
fn test_print_export_renders_one_page_per_slide_with_the_license_last() {
    let deck = slides(3);
    let html = render_pdf_document("acme", "my-project", &deck, Some("<p>MIT</p>")).into_string();

    // One page per slide in deck order, each labelled and counted, plus
    // the license page at the end.
    for i in 0..3 {
        assert!(
            html.contains(&format!(r#"aria-label="Slide {} of 3""#, i + 1)),
            "every slide gets its own page: {html}"
        );
        assert!(
            html.contains(&format!("<h1>Slide {i}</h1>")),
            "slide {i} renders from the deck data: {html}"
        );
        assert!(
            html.contains(&format!(
                r#"<span class="twig-present-count">{} / 3</span>"#,
                i + 1
            )),
            "the export reuses the live deck's zero-padded counter: {html}"
        );
    }
    assert_eq!(
        count_of(&html, r#"class="twig-pdf-page""#),
        3,
        "exactly one page per slide: {html}"
    );
    assert!(
        index_of(&html, r#"aria-label="Slide 3 of 3""#)
            < index_of(&html, r#"class="twig-pdf-page twig-pdf-page--license""#),
        "the license page follows the last slide: {html}"
    );
    assert!(html.contains("<p>MIT</p>"), "{html}");

    // The license page starts on its own sheet, and the last page never
    // forces a trailing blank sheet.
    assert!(html.contains(r#"class="twig-pdf-page twig-pdf-page--license""#),);
}

#[test]
fn test_print_export_opens_the_dialog_once_loaded() {
    let deck = slides(2);
    let html = render_pdf_document("acme", "my-project", &deck, Some("<p>MIT</p>")).into_string();

    assert!(
        html.contains("document.fonts.ready") && html.contains("window.print()"),
        "the export auto-prints after the fonts settle: {html}"
    );
    assert!(
        html.contains("img.complete"),
        "images join the wait before the dialog opens: {html}"
    );
    assert!(
        html.contains(r#"id="twig-pdf-hint""#) && html.contains("data-twig-pdf-ready"),
        "the save hint flips to ready when the dialog opens: {html}"
    );
    assert!(
        html.contains("Save as PDF"),
        "the hint names the dialog option to pick: {html}"
    );
}

#[test]
fn test_print_export_is_a_chrome_free_document() {
    let deck = slides(2);
    let html = render_pdf_document("acme", "my-project", &deck, Some("<p>MIT</p>")).into_string();

    assert!(html.starts_with("<!DOCTYPE html>"), "{html}");
    assert!(
        html.contains("<title>acme/my-project slides · Twig</title>"),
        "{html}"
    );
    assert!(
        !html.contains("twig-masthead") && !html.contains("twig-tabs"),
        "the export carries no application chrome: {html}"
    );
    assert!(
        !html.contains("hx-get=") && !html.contains("/assets/h.js"),
        "the export is a static document: {html}"
    );
    assert!(
        html.contains(r#"class="twig-pdf""#) && html.contains(r#"class="twig-pdf-binder""#),
        "{html}"
    );

    let empty = render_pdf_document("acme", "my-project", &[], None).into_string();
    assert!(
        empty.contains("NO SLIDES") && empty.contains("No license information available."),
        "a deck without slides still names both conditions: {empty}"
    );
    assert!(
        !empty.contains("window.print()"),
        "an empty binder has nothing to print: {empty}"
    );
}

#[test]
fn test_presentation_targets_its_container_and_scopes_the_keyboard() {
    let deck = slides(3);
    let html = render_present_view("acme", "my-project", &deck).into_string();

    assert!(
        html.contains(r#"<section id="present-container" class="twig-present" "#)
            && html.contains(
                r#"tabindex="-1" aria-roledescription="carousel" aria-label="Presentation""#
            ),
        "{html}"
    );
    assert!(
        html.contains(r#"data-twig-fullscreen="false""#),
        "fullscreen is a CSS overlay state, not the Fullscreen API: {html}"
    );
    assert_eq!(
        count_of(&html, "hx-target=\"#present-container\""),
        4,
        "next plus three ticks all swap the container: {html}"
    );
    assert_eq!(count_of(&html, "hx-swap=\"innerHTML\""), 4, "{html}");

    // Keyboard navigation is declarative htmx scoped to the container, so a
    // key press only advances the deck while focus is inside it. The first
    // slide disables Previous, so only the right arrow is wired there; the
    // left arrow appears once there is a slide to return to.
    assert!(
        html.contains(r"keyup[key=='ArrowRight'] from:#present-container"),
        "the right arrow advances the next slide via htmx: {html}"
    );
    let middle = render_slide_content("acme", "my-project", 1, &deck).into_string();
    assert!(
        middle.contains(r"keyup[key=='ArrowLeft'] from:#present-container"),
        "the left arrow returns via htmx: {middle}"
    );
    assert!(
        !html.contains("document.addEventListener"),
        "nothing is bound to the document: {html}"
    );
    assert!(
        !html.contains("<script>"),
        "the presentation carries no inline script: {html}"
    );
    assert!(
        !html.contains("requestFullscreen"),
        "fullscreen no longer depends on the Fullscreen API: {html}"
    );
}

#[test]
fn test_presentation_controls_are_labelled_and_bounded() {
    let deck = slides(3);
    let html = render_slide_content("acme", "my-project", 0, &deck).into_string();

    assert!(
        html.contains("twig-theme-toggle"),
        "the theme switch must remain accessible in fullscreen and after slide swaps: {html}"
    );
    for control in [
        r#"id="decrease-text-size" class="twig-btn twig-btn--quiet" type="button" aria-label="Decrease presentation font size" title="Decrease font size""#,
        r#"id="increase-text-size" class="twig-btn twig-btn--quiet" type="button" aria-label="Increase presentation font size" title="Increase font size""#,
        r#"<span id="present-text-size" class="twig-present-count" aria-live="polite" hx-live:text="data.twigTextSize + '%'">100%</span>"#,
    ] {
        assert!(
            html.contains(control),
            "missing presentation font control: {html}"
        );
    }
    assert!(
        html.contains(
            r#"<button id="prev-slide" class="twig-btn twig-btn--quiet" type="button" disabled>"#
        ),
        "the first slide disables Previous rather than swapping in a span: {html}"
    );
    assert!(
            html.contains(r#"<button id="next-slide" class="twig-btn twig-btn--quiet" type="button" hx-get="/acme/my-project/slide/1""#),
            "{html}"
        );
    assert!(
        html.contains(r#"<div class="twig-ticks" role="group" aria-label="Go to slide">"#),
        "{html}"
    );
    for i in 1..=3 {
        assert!(
            html.contains(&format!(r#"aria-label="Slide {i} of 3""#)),
            "every tick names its slide: {html}"
        );
    }
    assert_eq!(
        count_of(&html, "aria-current=\"true\""),
        1,
        "one tick is current: {html}"
    );
    assert!(
            html.contains(
                r#"<button id="fullscreen-toggle" class="twig-btn twig-btn--quiet" type="button" aria-pressed="false" hx-on:click="data.twigFullscreen = !data.twigFullscreen""#
            ),
            "fullscreen is a real toggle button with a visible label: {html}"
        );
    assert!(
        html.contains(r#"hx-live:text="data.twigFullscreen ? 'Exit fullscreen' : 'Fullscreen'""#),
        "the fullscreen label reflects the toggle state: {html}"
    );

    let last = render_slide_content("acme", "my-project", 2, &deck).into_string();
    assert!(
        last.contains(
            r#"<button id="next-slide" class="twig-btn twig-btn--quiet" type="button" disabled>"#
        ),
        "the last slide disables Next: {last}"
    );
}

#[test]
fn test_slide_counter_pads_to_the_width_of_the_deck() {
    assert_eq!(slide_counter(0, 3), "1 / 3");
    assert_eq!(slide_counter(2, 12), "03 / 12");
    assert_eq!(slide_counter(11, 12), "12 / 12");
}

#[test]
fn test_present_view_without_slides_states_the_condition() {
    let html = render_present_view("acme", "my-project", &[]).into_string();
    assert!(html.contains("twig-empty"), "{html}");
    assert!(html.contains(">NO SLIDES<"), "{html}");
    assert!(
            html.contains(
                "No presentation slides configured. Add a [present] section with files to your .twig.toml."
            ),
            "{html}"
        );
    assert!(!html.contains("present-container"), "{html}");
}
