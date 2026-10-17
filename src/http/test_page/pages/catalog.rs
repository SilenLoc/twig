use super::TestEndpoint;

fn core_system_endpoints() -> Vec<TestEndpoint> {
    vec![
        TestEndpoint {
            id: "health",
            category: "System",
            method: "GET",
            url: "/health".to_string(),
        },
        TestEndpoint {
            id: "up",
            category: "System",
            method: "GET",
            url: "/up".to_string(),
        },
        TestEndpoint {
            id: "tree",
            category: "API",
            method: "GET",
            url: "/api/tree".to_string(),
        },
    ]
}

fn docs_endpoints() -> Vec<TestEndpoint> {
    vec![
        TestEndpoint {
            id: "info",
            category: "Docs",
            method: "GET",
            url: "/_info".to_string(),
        },
        TestEndpoint {
            id: "info-about",
            category: "Docs",
            method: "GET",
            url: "/_info?tab=about".to_string(),
        },
        TestEndpoint {
            id: "info-docs",
            category: "Docs",
            method: "GET",
            url: "/_info?tab=docs".to_string(),
        },
        TestEndpoint {
            id: "info-git",
            category: "Docs",
            method: "GET",
            url: "/_info?tab=docs&page=git-backend".to_string(),
        },
        TestEndpoint {
            id: "info-ui",
            category: "Docs",
            method: "GET",
            url: "/_info?tab=docs&page=ui".to_string(),
        },
        TestEndpoint {
            id: "info-env",
            category: "Docs",
            method: "GET",
            url: "/_info?tab=docs&page=environment-variables".to_string(),
        },
    ]
}

fn asset_endpoints() -> Vec<TestEndpoint> {
    vec![
        TestEndpoint {
            id: "css-twig",
            category: "Assets",
            method: "GET",
            url: crate::http::assets::url("twig.css"),
        },
        TestEndpoint {
            id: "css-t",
            category: "Assets",
            method: "GET",
            url: crate::http::assets::url("t.css"),
        },
        TestEndpoint {
            id: "js-h",
            category: "Assets",
            method: "GET",
            url: crate::http::assets::url("h.js"),
        },
        TestEndpoint {
            id: "svg-twig",
            category: "Assets",
            method: "GET",
            url: crate::http::assets::url("twig.svg"),
        },
    ]
}

fn ui_endpoints() -> Vec<TestEndpoint> {
    vec![
        TestEndpoint {
            id: "overview",
            category: "UI",
            method: "GET",
            url: "/".to_string(),
        },
        TestEndpoint {
            id: "overview-search",
            category: "UI",
            method: "GET",
            url: "/?q=test".to_string(),
        },
    ]
}

fn feature_endpoints() -> Vec<TestEndpoint> {
    vec![
        TestEndpoint {
            id: "test-ping",
            category: "Features",
            method: "GET",
            url: "/_test/ping".to_string(),
        },
        TestEndpoint {
            id: "test-feature-check",
            category: "Features",
            method: "GET",
            url: "/_test/feature-check".to_string(),
        },
    ]
}

pub fn list_test_endpoints() -> Vec<TestEndpoint> {
    let mut endpoints = core_system_endpoints();
    endpoints.extend(docs_endpoints());
    endpoints.extend(asset_endpoints());
    endpoints.extend(ui_endpoints());
    endpoints.extend(feature_endpoints());
    endpoints
}
