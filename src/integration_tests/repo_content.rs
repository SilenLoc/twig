use super::*;

use super::auth_and_invitations::{create_namespace, signup_and_login};
use super::support::*;

async fn assert_repository_markup_is_inert<S>(app: &S, uri: &str, use_htmx: bool, marker: &str)
where
    S: actix_web::dev::Service<
            Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
{
    let mut req = test::TestRequest::get().uri(uri);
    if use_htmx {
        req = req.insert_header(("HX-Request", "true"));
    }
    let response = test::call_service(app, req.to_request()).await;
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    let html = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();
    assert!(
        html.contains(marker),
        "repository text should remain visible: {html}"
    );
    assert!(
        html.contains("&lt;"),
        "repository tags must be escaped: {html}"
    );
    assert!(
        !html.contains("<div data-security-marker=")
            && !html.contains("<span data-security-marker="),
        "repository markup must not become an HTML element: {html}"
    );
}

/// Builds a bare repository whose tree contains the given `.twig.toml` and,
/// when non-empty, a `paper/` directory holding the supplied pages.
pub(super) fn init_repo_with_paper(
    repo_path: &std::path::Path,
    twig: &str,
    paper_pages: &[(&str, &str)],
) {
    use std::fmt::Write as _;

    std::fs::create_dir_all(repo_path).unwrap();
    crate::git::repo::bare_init(repo_path, "main", "Test", "test@example.com").unwrap();
    let sh = xshell::Shell::new().unwrap();
    let _p = sh.push_dir(repo_path);

    let hash = |content: &str| -> String {
        xshell::cmd!(sh, "git hash-object -w --stdin")
            .stdin(content)
            .read()
            .unwrap()
    };

    let twig_blob = hash(twig);
    let mut entries = format!("100644 blob {twig_blob}\t.twig.toml\n");

    if !paper_pages.is_empty() {
        let mut paper_entries = String::new();
        for (name, content) in paper_pages {
            let blob = hash(content);
            writeln!(paper_entries, "100644 blob {blob}\t{name}").unwrap();
        }
        let paper_tree = xshell::cmd!(sh, "git mktree")
            .stdin(paper_entries)
            .read()
            .unwrap();
        writeln!(entries, "040000 tree {paper_tree}\tpaper").unwrap();
    }

    let tree = xshell::cmd!(sh, "git mktree")
        .stdin(entries)
        .read()
        .unwrap();
    let commit = xshell::cmd!(sh, "git commit-tree {tree} -m 'paper'")
        .read()
        .unwrap();
    xshell::cmd!(sh, "git update-ref refs/heads/main {commit}")
        .run()
        .unwrap();
}
/// Builds a bare repository with the given `.twig.toml` plus arbitrary
/// extra files. Nested paths build their subtrees bottom-up, since
/// `mktree` rejects slashes.
pub(super) fn init_repo_with_files(
    repo_path: &std::path::Path,
    twig: &str,
    files: &[(&str, &str)],
) {
    use std::fmt::Write as _;
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    // Builds the tree holding `files`, whose paths all live under one
    // directory: blobs straight into `mktree`, deeper paths into a
    // recursive subtree named by their first path segment.
    fn tree(
        files: &[(&str, &str)],
        blob: &impl Fn(&str) -> String,
        mktree: &impl Fn(&str) -> String,
    ) -> String {
        let mut entries = String::new();
        let mut nested: Vec<(&str, Vec<(&str, &str)>)> = Vec::new();
        for (name, content) in files {
            match name.split_once('/') {
                None => {
                    let hash = blob(content);
                    let _ = writeln!(entries, "100644 blob {hash}\t{name}");
                }
                Some((dir, rest)) => match nested.iter_mut().find(|(d, _)| *d == dir) {
                    Some((_, group)) => group.push((rest, *content)),
                    None => nested.push((dir, vec![(rest, *content)])),
                },
            }
        }
        for (dir, group) in &nested {
            let subtree = tree(group, blob, mktree);
            let _ = writeln!(entries, "040000 tree {subtree}\t{dir}");
        }
        mktree(&entries)
    }

    std::fs::create_dir_all(repo_path).unwrap();
    crate::git::repo::bare_init(repo_path, "main", "Test", "test@example.com").unwrap();
    let sh = xshell::Shell::new().unwrap();
    let _p = sh.push_dir(repo_path);

    let git_pipe = |args: &[&str], input: &str| -> String {
        let mut child = Command::new("git")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .current_dir(repo_path)
            .spawn()
            .unwrap();
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "{args:?}: {out:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };

    let blob = |content: &str| git_pipe(&["hash-object", "-w", "--stdin"], content);
    let mktree = |input: &str| git_pipe(&["mktree"], input);

    let mut all: Vec<(&str, &str)> = vec![(".twig.toml", twig)];
    all.extend_from_slice(files);
    let root = tree(&all, &blob, &mktree);

    let commit = xshell::cmd!(sh, "git commit-tree {root} -m 'files'")
        .read()
        .unwrap();
    xshell::cmd!(sh, "git update-ref refs/heads/main {commit}")
        .run()
        .unwrap();
}
pub(super) async fn get_body(
    app: &impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    uri: &str,
) -> String {
    let req = test::TestRequest::get().uri(uri).to_request();
    let resp = test::call_service(app, req).await;
    String::from_utf8(test::read_body(resp).await.to_vec()).unwrap()
}
#[actix_web::test]
async fn test_paper_tab_renders_lazily_loaded_pages() {
    let root = format!("/tmp/test_twig_paper_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    init_repo_with_paper(
        &std::path::Path::new(&root).join("pub/book"),
        "[paper]\ndir = \"paper\"\n",
        &[("01.md", "# First\n"), ("02.md", "# Second\n")],
    );

    let body = get_body(&app, "/pub/book").await;
    assert!(body.contains(">Paper<"), "paper tab missing: {body}");
    assert!(
        body.contains("href=\"/pub/book/paper\""),
        "the tab points at the paper view: {body}"
    );

    let body = get_body(&app, "/pub/book/paper").await;
    assert!(body.contains("paper-container"), "{body}");
    assert!(body.contains("data-twig-paper-font=\"sans\""), "{body}");
    assert!(
        body.contains(r#"hx-trigger="revealed""#),
        "pages must lazy load on scroll: {body}"
    );
    assert!(
        body.contains("/pub/book/paper/paper/01.md"),
        "the page endpoint is wired: {body}"
    );
    assert!(
        body.contains(r#"id="paper-01.md""#),
        "each page carries an anchor: {body}"
    );

    let body = get_body(&app, "/pub/book/paper/paper/01.md").await;
    assert!(
        body.contains(r#"<h1 id="paper-01.md--first">First</h1>"#),
        "headings are anchored: {body}"
    );

    let body = get_body(&app, "/pub/book/paper/.twig.toml").await;
    assert!(
        body.contains("Paper page not found."),
        "files outside the paper dir are refused: {body}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
#[actix_web::test]
async fn test_scripts_tab_is_hidden_until_configured() {
    let root = format!("/tmp/test_twig_scripts_hidden_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    init_repo_with_files(
        &std::path::Path::new(&root).join("pub/plain"),
        "tabs = []\n",
        &[("README.md", "# Plain\n")],
    );

    let body = get_body(&app, "/pub/plain").await;
    assert!(
        !body.contains(">Scripts<"),
        "no [scripts] section means no Scripts tab: {body}"
    );

    // Even a direct visit stays inert: no groups render.
    let body = get_body(&app, "/pub/plain/scripts").await;
    assert!(body.contains("NO SCRIPTS"), "{body}");

    let _ = std::fs::remove_dir_all(&root);
}
#[actix_web::test]
async fn test_scripts_tab_shows_groups_with_curl_commands() {
    let root = format!("/tmp/test_twig_scripts_tab_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    init_repo_with_files(
        &std::path::Path::new(&root).join("pub/toolbox"),
        r#"
[scripts.linux]
name = "Linux"
scripts = [{ name = "Install", path = "scripts/install.sh", shell = "sh" }]

[scripts.linux.maintenance]
scripts = [{ name = "Cleanup", path = "scripts/cleanup.sh" }]
"#,
        &[
            ("README.md", "# Toolbox\n"),
            ("scripts/install.sh", "#!/bin/sh\necho install\n"),
            ("scripts/cleanup.sh", "#!/bin/bash\necho cleanup\n"),
        ],
    );

    // The tab appears on the repository home and points at its own route.
    let body = get_body(&app, "/pub/toolbox").await;
    assert!(body.contains(">Scripts<"), "Scripts tab missing: {body}");
    assert!(
        body.contains("href=\"/pub/toolbox/scripts\""),
        "the tab points at the scripts view: {body}"
    );

    // The default group renders with its sub-tab bar and run command.
    let body = get_body(&app, "/pub/toolbox/scripts").await;
    assert!(body.contains("scripts-container"), "{body}");
    assert!(body.contains(">Linux<"), "the group is a sub-tab: {body}");
    assert!(
        body.contains("href=\"/pub/toolbox/scripts/linux\""),
        "sub-tabs link by group key: {body}"
    );
    assert!(
        body.contains("curl -fsSL 'http://localhost:8080/pub/toolbox/raw/scripts/install.sh' | sh"),
        "the run command names the raw route and the configured shell: {body}"
    );
    assert!(
        body.contains("data-twig-copy"),
        "every entry carries a copy button: {body}"
    );

    // A nested group renders its own scripts under its key.
    let body = get_body(&app, "/pub/toolbox/scripts/linux/maintenance").await;
    assert!(
        body.contains(
            "curl -fsSL 'http://localhost:8080/pub/toolbox/raw/scripts/cleanup.sh' | bash"
        ),
        "nested groups resolve and default to bash: {body}"
    );

    // An unknown group key falls back to the first configured group.
    let body = get_body(&app, "/pub/toolbox/scripts/bogus").await;
    assert!(
        body.contains("curl -fsSL 'http://localhost:8080/pub/toolbox/raw/scripts/install.sh'"),
        "unknown keys fall back to the first group: {body}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
#[actix_web::test]
async fn test_invalid_twig_toml_shows_a_diagnostic() {
    let root = format!("/tmp/test_twig_badconfig_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    init_repo_with_paper(
        &std::path::Path::new(&root).join("pub/broken"),
        "this = = not valid\n",
        &[],
    );

    let body = get_body(&app, "/pub/broken").await;
    assert!(body.contains("CONFIG ERROR"), "{body}");
    assert!(
        body.contains("error: could not parse configuration"),
        "the parse failure is surfaced, not swallowed: {body}"
    );
    assert!(body.contains("help: fix the syntax"), "{body}");
    assert!(
        body.contains(".twig.toml"),
        "the offending file is named: {body}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn repository_license_and_presentation_markup_are_inert_on_all_views() {
    let root = format!("/tmp/test_twig_stored_markup_{}", uuid::Uuid::new_v4());
    let app = create_test_service_in(&root).await;
    let license_md = r#"<div data-security-marker="license-md">primary license marker</div>"#;
    let license_fallback =
        r#"<span data-security-marker="license-fallback">fallback license marker</span>"#;
    let slide = "# Slide title\n\n**Supported Markdown**\n\n<div data-security-marker=\"slide\">slide marker</div>\n";
    init_repo_with_files(
        &std::path::Path::new(&root).join("pub/primary"),
        "[present]\nfiles = [\"slides/one.md\"]\n",
        &[
            ("LICENSE.md", license_md),
            ("LICENSE", license_fallback),
            ("slides/one.md", slide),
        ],
    );
    init_repo_with_files(
        &std::path::Path::new(&root).join("pub/fallback"),
        "",
        &[("LICENSE", license_fallback)],
    );

    for (uri, htmx, marker) in [
        ("/pub/primary/license", false, "primary license marker"),
        ("/pub/primary/license", true, "primary license marker"),
        ("/pub/primary/present", false, "slide marker"),
        ("/pub/primary/present", true, "slide marker"),
        ("/pub/primary/slide/0", false, "slide marker"),
        ("/pub/primary/slide/0", true, "slide marker"),
        (
            "/pub/primary/present/print",
            false,
            "primary license marker",
        ),
        ("/pub/primary/present/print", false, "slide marker"),
        ("/pub/fallback/license", false, "fallback license marker"),
        (
            "/pub/fallback/present/print",
            false,
            "fallback license marker",
        ),
    ] {
        assert_repository_markup_is_inert(&app, uri, htmx, marker).await;
    }

    let present_html = get_body(&app, "/pub/primary/present").await;
    assert!(
        present_html.contains("<h1>Slide title</h1>"),
        "{present_html}"
    );
    assert!(
        present_html.contains("<strong>Supported Markdown</strong>"),
        "{present_html}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn private_repository_markup_is_inert_for_an_authorized_reader() {
    let root = format!("/tmp/test_twig_private_markup_{}", uuid::Uuid::new_v4());
    let (app, _) = create_test_service_with_db_in(&root).await;
    let session = signup_and_login(&app, "markupreader").await;
    create_namespace(&app, session.clone(), "private-space").await;
    init_repo_with_files(
        &std::path::Path::new(&root).join("private-space/private-repo"),
        "private = true\n[present]\nfiles = [\"slides/one.md\"]\n",
        &[
            (
                "LICENSE.md",
                r#"<div data-security-marker="private-license">private license marker</div>"#,
            ),
            (
                "slides/one.md",
                "<div data-security-marker=\"private-slide\">private slide marker</div>",
            ),
        ],
    );

    for (uri, marker) in [
        (
            "/private-space/private-repo/license",
            "private license marker",
        ),
        (
            "/private-space/private-repo/present",
            "private slide marker",
        ),
        (
            "/private-space/private-repo/slide/0",
            "private slide marker",
        ),
        (
            "/private-space/private-repo/present/print",
            "private license marker",
        ),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(uri)
                .cookie(session.clone())
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        let html = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();
        assert!(html.contains(marker), "{html}");
        assert!(
            html.contains("&lt;"),
            "repository tags must be escaped: {html}"
        );
        assert!(
            !html.contains("<div data-security-marker=")
                && !html.contains("<span data-security-marker="),
            "repository markup must not become an HTML element: {html}"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
}
