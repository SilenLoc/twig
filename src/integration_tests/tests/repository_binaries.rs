use super::*;

#[actix_web::test]
async fn test_http_binary_upload_and_download_work_for_public_and_private_repos() {
    use base64::Engine as _;

    let root = format!("/tmp/test_twig_binary_upload_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let owner = auth::create_user(
        "binary-owner".to_string(),
        "binary-owner@example.com".to_string(),
        "correct horse battery staple",
    )
    .expect("create upload user");
    db.create_user(&owner).await.expect("store upload user");
    let outsider = auth::create_user(
        "binary-outsider".to_string(),
        "binary-outsider@example.com".to_string(),
        "correct horse battery staple",
    )
    .expect("create unrelated user");
    db.create_user(&outsider)
        .await
        .expect("store unrelated user");
    let namespace = auth::create_namespace("pub".to_string(), owner.id.clone());
    db.create_namespace(&namespace)
        .await
        .expect("create upload namespace");

    init_repo_with_files(
        &std::path::Path::new(&root).join("pub/toolbox"),
        r#"
[scripts.linux]
scripts = [{ name = "Install", path = "scripts/install.sh" }]
"#,
        &[("scripts/install.sh", "#!/bin/sh\necho install\n")],
    );
    init_repo_with_files(
        &std::path::Path::new(&root).join("pub/plain"),
        "\n",
        &[("README.md", "No Scripts tab here.\n")],
    );
    init_repo_with_files(
        &std::path::Path::new(&root).join("pub/private-tool"),
        r#"
private = true
[scripts.linux]
scripts = [{ name = "Install", path = "install.sh" }]
"#,
        &[("install.sh", "#!/bin/sh\n")],
    );

    let basic = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD
            .encode("binary-owner:correct horse battery staple")
    );
    let outsider_basic = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD
            .encode("binary-outsider:correct horse battery staple")
    );
    let anonymous_upload = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/pub/toolbox/binaries/1.4.2/example-linux-x86_64")
            .set_payload(b"unauthorized".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(anonymous_upload.status(), StatusCode::UNAUTHORIZED);
    let unrelated_upload = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/pub/toolbox/binaries/1.4.2/example-linux-x86_64")
            .insert_header(("Authorization", outsider_basic.as_str()))
            .set_payload(b"not allowed".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(unrelated_upload.status(), StatusCode::FORBIDDEN);

    let linux_bytes = b"locally built linux binary\0bytes";
    let upload_linux = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/pub/toolbox/binaries/v1.4.2/example-linux-x86_64")
            .insert_header(("Authorization", basic.as_str()))
            .set_payload(linux_bytes.to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(upload_linux.status(), StatusCode::CREATED);
    let replacement_bytes = b"replacement linux binary\0bytes";
    let replace_linux = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/pub/toolbox/binaries/1.4.2/example-linux-x86_64")
            .insert_header(("Authorization", basic.as_str()))
            .set_payload(replacement_bytes.to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(replace_linux.status(), StatusCode::OK);

    let mac_bytes = b"locally built macOS binary";
    let upload_mac = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/pub/toolbox/binaries/1.4.2/example-darwin-arm64")
            .insert_header(("Authorization", basic.as_str()))
            .set_payload(mac_bytes.to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(upload_mac.status(), StatusCode::CREATED);

    let page = get_body(&app, "/pub/toolbox/scripts").await;
    assert!(page.contains("Available binaries"), "{page}");
    assert!(page.contains("v1.4.2"), "{page}");
    assert!(page.contains("example-linux-x86_64"), "{page}");
    assert!(page.contains("example-darwin-arm64"), "{page}");
    assert!(
        page.contains("data-twig-copy"),
        "install command needs a Copy button: {page}"
    );
    assert!(
        page.contains("/pub/toolbox/binaries/latest/…"),
        "the page explains the direct script URL: {page}"
    );

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/toolbox/binaries/1.4.2/example-linux-x86_64")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/octet-stream"
    );
    assert_eq!(
        response.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    assert!(
        response
            .headers()
            .get("content-disposition")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("example-linux-x86_64")
    );
    assert_eq!(test::read_body(response).await.as_ref(), replacement_bytes);

    let latest = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/toolbox/binaries/latest/example-linux-x86_64")
            .to_request(),
    )
    .await;
    assert_eq!(latest.status(), StatusCode::OK);
    assert_eq!(test::read_body(latest).await.as_ref(), replacement_bytes);

    // Direct downloads remain available when the repository has no Scripts tab.
    let plain_upload = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/pub/plain/binaries/1.0.0/tool")
            .insert_header(("Authorization", basic.as_str()))
            .set_payload(b"plain repo asset".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(plain_upload.status(), StatusCode::CREATED);
    let direct = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/plain/binaries/1.0.0/tool")
            .to_request(),
    )
    .await;
    assert_eq!(direct.status(), StatusCode::OK);
    assert_eq!(test::read_body(direct).await.as_ref(), b"plain repo asset");

    let private_upload = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/pub/private-tool/binaries/1.0.0/private-linux")
            .insert_header(("Authorization", basic.as_str()))
            .set_payload(b"private asset".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(private_upload.status(), StatusCode::CREATED);
    let private_anonymous = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/private-tool/binaries/1.0.0/private-linux")
            .to_request(),
    )
    .await;
    assert_eq!(private_anonymous.status(), StatusCode::UNAUTHORIZED);
    db.create_token("binary-private-session", &owner.id)
        .await
        .expect("create authenticated session token");
    let private_authenticated = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/private-tool/binaries/1.0.0/private-linux")
            .cookie(actix_web::cookie::Cookie::new(
                "session",
                "binary-private-session",
            ))
            .to_request(),
    )
    .await;
    assert_eq!(private_authenticated.status(), StatusCode::OK);
    assert_eq!(
        test::read_body(private_authenticated).await.as_ref(),
        b"private asset"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[actix_web::test]
async fn test_binary_uploads_retain_semver_releases_and_latest_resolves_by_asset() {
    use base64::Engine as _;

    let root = format!("/tmp/test_twig_binary_semver_{}", uuid::Uuid::new_v4());
    let (app, db) = create_test_service_with_db_in(&root).await;
    let owner = auth::create_user(
        "semver-owner".to_string(),
        "semver-owner@example.com".to_string(),
        "correct horse battery staple",
    )
    .expect("create upload user");
    db.create_user(&owner).await.expect("store upload user");
    db.create_namespace(&auth::create_namespace("pub".to_string(), owner.id.clone()))
        .await
        .expect("create upload namespace");
    init_repo_with_files(
        &std::path::Path::new(&root).join("pub/versions"),
        r#"
[scripts.linux]
scripts = [{ name = "Install", path = "install.sh" }]
"#,
        &[("install.sh", "#!/bin/sh\n")],
    );

    let basic = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD
            .encode("semver-owner:correct horse battery staple")
    );
    for (version, filename, bytes) in [
        ("v1.9.0", "linux", &b"linux-1.9"[..]),
        ("1.10.0", "macos", &b"macos-1.10"[..]),
        ("1.8.0", "linux", &b"linux-1.8"[..]),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!("/pub/versions/binaries/{version}/{filename}"))
                .insert_header(("Authorization", basic.as_str()))
                .set_payload(bytes.to_vec())
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let too_old = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/pub/versions/binaries/1.7.0/linux")
            .insert_header(("Authorization", basic.as_str()))
            .set_payload(b"too old".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(too_old.status(), StatusCode::CONFLICT);

    let invalid = test::call_service(
        &app,
        test::TestRequest::put()
            .uri("/pub/versions/binaries/latest/linux")
            .insert_header(("Authorization", basic.as_str()))
            .set_payload(b"reserved".to_vec())
            .to_request(),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    let page = get_body(&app, "/pub/versions/scripts").await;
    let v110 = page.find("v1.10.0").expect("newest version rendered");
    let v19 = page.find("v1.9.0").expect("second version rendered");
    let v18 = page.find("v1.8.0").expect("third version rendered");
    assert!(
        v110 < v19 && v19 < v18,
        "versions are in SemVer order: {page}"
    );
    assert!(
        !page.contains("v1.7.0"),
        "too-old release was not retained: {page}"
    );

    let latest_linux = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/versions/binaries/latest/linux")
            .to_request(),
    )
    .await;
    assert_eq!(latest_linux.status(), StatusCode::OK);
    assert_eq!(test::read_body(latest_linux).await.as_ref(), b"linux-1.9");

    let latest_macos = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/pub/versions/binaries/latest/macos")
            .to_request(),
    )
    .await;
    assert_eq!(latest_macos.status(), StatusCode::OK);
    assert_eq!(test::read_body(latest_macos).await.as_ref(), b"macos-1.10");

    let _ = std::fs::remove_dir_all(&root);
}
