use super::super::*;

pub(in crate::integration_tests::tests) fn init_text_repo(
    repo_path: &std::path::Path,
    path: &str,
    content: &[u8],
) -> twig_git::Oid {
    init_repo_files(repo_path, &[(path, content, 0o100_644)])
}

pub(in crate::integration_tests::tests) fn init_repo_files(
    repo_path: &std::path::Path,
    files: &[(&str, &[u8], i32)],
) -> twig_git::Oid {
    use std::fmt::Write as _;
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    fn git_pipe(repo_path: &std::path::Path, args: &[&str], input: &[u8]) -> String {
        let mut child = Command::new("git")
            .args(args)
            .current_dir(repo_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.as_mut().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn build_tree(repo_path: &std::path::Path, files: &[(&str, &[u8], i32)]) -> String {
        let mut entries = String::new();
        let mut nested: Vec<(&str, Vec<(&str, &[u8], i32)>)> = Vec::new();
        for (path, content, mode) in files {
            match path.split_once('/') {
                Some((directory, rest)) => match nested
                    .iter_mut()
                    .find(|(existing, _)| *existing == directory)
                {
                    Some((_, group)) => group.push((rest, content, *mode)),
                    None => nested.push((directory, vec![(rest, content, *mode)])),
                },
                None => {
                    let blob = git_pipe(repo_path, &["hash-object", "-w", "--stdin"], content);
                    let mode = match mode {
                        0o100_644 => "100644",
                        0o100_755 => "100755",
                        0o120_000 => "120000",
                        _ => panic!("unsupported test file mode: {mode:o}"),
                    };
                    writeln!(entries, "{mode} blob {blob}\t{path}").unwrap();
                }
            }
        }
        for (directory, children) in &nested {
            let subtree = build_tree(repo_path, children);
            writeln!(entries, "040000 tree {subtree}\t{directory}").unwrap();
        }
        git_pipe(repo_path, &["mktree"], entries.as_bytes())
    }

    std::fs::create_dir_all(repo_path).unwrap();
    let init = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    assert!(init.status.success(), "{init:?}");
    let tree = build_tree(repo_path, files);
    let commit = Command::new("git")
        .args(["commit-tree", &tree, "-m", "seed file"])
        .current_dir(repo_path)
        .env("GIT_AUTHOR_NAME", "Seed")
        .env("GIT_AUTHOR_EMAIL", "seed@example.com")
        .env("GIT_COMMITTER_NAME", "Seed")
        .env("GIT_COMMITTER_EMAIL", "seed@example.com")
        .output()
        .unwrap();
    assert!(commit.status.success(), "{commit:?}");
    let commit = String::from_utf8_lossy(&commit.stdout).trim().to_owned();
    let update = Command::new("git")
        .args(["update-ref", "refs/heads/main", &commit])
        .current_dir(repo_path)
        .output()
        .unwrap();
    assert!(update.status.success(), "{update:?}");
    twig_git::Oid::from_str(&commit).unwrap()
}

pub(in crate::integration_tests::tests) fn git_repo_output(
    repo_path: &std::path::Path,
    args: &[&str],
) -> String {
    use std::process::Command;

    let output = Command::new("git")
        .arg(format!("--git-dir={}", repo_path.display()))
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

pub(in crate::integration_tests::tests) fn commit_deleting_file(
    repo_path: &std::path::Path,
    file_path: &str,
    author_name: &str,
    author_email: &str,
    message: &str,
) {
    use std::io::Write as _;
    use std::process::Command;

    let index_path = repo_path.join(format!("twig-test-index-{}", uuid::Uuid::new_v4()));
    let git = |arguments: &[&str]| {
        let output = Command::new("git")
            .arg(format!("--git-dir={}", repo_path.display()))
            .args(arguments)
            .env("GIT_INDEX_FILE", &index_path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{arguments:?}: {output:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    git(&["read-tree", "HEAD"]);
    let object_format = git(&["rev-parse", "--show-object-format"]);
    let zero_oid = "0".repeat(if object_format == "sha256" { 64 } else { 40 });
    let mut remove_file = Command::new("git")
        .arg(format!("--git-dir={}", repo_path.display()))
        .args(["update-index", "--index-info"])
        .env("GIT_INDEX_FILE", &index_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(
        remove_file.stdin.as_mut().unwrap(),
        "0 {zero_oid}\t{file_path}"
    )
    .unwrap();
    let remove_file = remove_file.wait_with_output().unwrap();
    assert!(remove_file.status.success(), "{remove_file:?}");
    let tree = git(&["write-tree"]);
    let parent = git(&["rev-parse", "HEAD"]);
    let commit = Command::new("git")
        .arg(format!("--git-dir={}", repo_path.display()))
        .args(["commit-tree", &tree, "-p", &parent, "-m", message])
        .env("GIT_AUTHOR_NAME", author_name)
        .env("GIT_AUTHOR_EMAIL", author_email)
        .env("GIT_COMMITTER_NAME", author_name)
        .env("GIT_COMMITTER_EMAIL", author_email)
        .output()
        .unwrap();
    assert!(commit.status.success(), "{commit:?}");
    let commit = String::from_utf8_lossy(&commit.stdout).trim().to_owned();
    git(&["update-ref", "refs/heads/main", &commit, &parent]);
    let _ = std::fs::remove_file(index_path);
}

/// Marker embedded in a secret file placed OUTSIDE the project root.
/// No response may ever contain this string.
pub(in crate::integration_tests::tests) const SECRET_MARKER: &str =
    "TOP_SECRET_TRAVERSAL_MARKER_9f3a";

pub(in crate::integration_tests::tests) struct TraversalFixture {
    pub(in crate::integration_tests::tests) root: std::path::PathBuf,
    pub(in crate::integration_tests::tests) outside: std::path::PathBuf,
    pub(in crate::integration_tests::tests) db_path: String,
}

impl Drop for TraversalFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
        let _ = std::fs::remove_dir_all(&self.outside);
        let _ = std::fs::remove_file(&self.db_path);
    }
}

/// Creates a project root containing `public/repo` (a real bare repo with
/// an initial commit) plus a secret directory OUTSIDE the project root.
pub(in crate::integration_tests::tests) fn setup_traversal_fixture() -> TraversalFixture {
    use std::path::PathBuf;

    let id = uuid::Uuid::new_v4().to_string();
    let root = PathBuf::from(format!("/tmp/twig_trav_root_{id}"));
    let repo_dir = root.join("public").join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    twig_git::repo::bare_init(&repo_dir, "main", "Test", "t@example.com")
        .expect("bare_init failed");

    // Secret file outside the project root, reachable only via traversal
    let outside = PathBuf::from(format!("/tmp/twig_trav_secret_{id}"));
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(
        outside.join("secret.txt"),
        format!("{SECRET_MARKER}\nroot:x:0:0:root\n"),
    )
    .unwrap();

    TraversalFixture {
        root,
        outside,
        db_path: format!("/tmp/twig_trav_{id}.db"),
    }
}

pub(in crate::integration_tests::tests) async fn create_traversal_service(
    fixture: &TraversalFixture,
) -> impl actix_web::dev::Service<
    Request,
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
> {
    let config = config::Server::new(
        ("127.0.0.1".to_string(), 8080),
        "error".to_string(),
        fixture.root.to_string_lossy().into_owned(),
        fixture.db_path.clone(),
        "secure".to_string(),
        true,
    );

    let db = Database::new(config.db_path());
    let auth_state = web::Data::new(auth::TwigContext::new(db, "secure".to_string()));
    auth_state.db().init_tables().await.expect("init tables");

    test::init_service(
        App::new()
            .app_data(web::Data::new(config))
            .app_data(auth_state)
            .service(repo_pages::handler)
            .service(repo_pages::tab_handler)
            .service(repo_pages::content_tab_handler)
            .service(repo_pages::commits_tab_handler)
            .service(repo_pages::content_handler)
            .service(repo_pages::markdown_handler),
    )
    .await
}

pub(in crate::integration_tests::tests) async fn body_of(
    app: &impl actix_web::dev::Service<
        Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    uri: &str,
) -> (StatusCode, String) {
    let req = test::TestRequest::get().uri(uri).to_request();
    let resp = test::call_service(app, req).await;
    let status = resp.status();
    let body = test::read_body(resp).await;
    (status, String::from_utf8(body.to_vec()).unwrap())
}

pub(in crate::integration_tests::tests) fn init_private_repo(repo_path: &std::path::Path) {
    std::fs::create_dir_all(repo_path).unwrap();
    twig_git::repo::bare_init(repo_path, "main", "Test", "test@example.com").unwrap();
    let sh = xshell::Shell::new().unwrap();
    let _p = sh.push_dir(repo_path);
    let blob = xshell::cmd!(sh, "git hash-object -w --stdin")
        .stdin("private = true\n")
        .read()
        .unwrap();
    let tree = xshell::cmd!(sh, "git mktree")
        .stdin(format!("100644 blob {blob}\t.twig.toml\n"))
        .read()
        .unwrap();
    let commit = xshell::cmd!(sh, "git commit-tree {tree} -m 'make private'")
        .read()
        .unwrap();
    xshell::cmd!(sh, "git update-ref refs/heads/main {commit}")
        .run()
        .unwrap();
}

/// Builds a bare repository whose tree contains the given `.twig.toml` and,
/// when non-empty, a `paper/` directory holding the supplied pages.
pub(in crate::integration_tests::tests) fn init_repo_with_paper(
    repo_path: &std::path::Path,
    twig: &str,
    paper_pages: &[(&str, &str)],
) {
    use std::fmt::Write as _;

    std::fs::create_dir_all(repo_path).unwrap();
    twig_git::repo::bare_init(repo_path, "main", "Test", "test@example.com").unwrap();
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
pub(in crate::integration_tests::tests) fn init_repo_with_files(
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
    twig_git::repo::bare_init(repo_path, "main", "Test", "test@example.com").unwrap();
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

pub(in crate::integration_tests::tests) async fn get_body(
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
