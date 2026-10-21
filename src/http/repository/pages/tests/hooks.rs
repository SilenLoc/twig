use super::super::hooks::render_hooks_view;
use twig_git::hooks::{
    CustomHookInfo, CustomHookState, HookInventory, HookRun, HookRunStatus, ManagedHookState,
};

#[test]
fn hooks_view_shows_real_status_and_escapes_script_source() {
    let inventory = HookInventory {
        managed_state: ManagedHookState::Healthy,
        custom_hooks: vec![CustomHookInfo {
            name: "pre-receive.twig-user".to_string(),
            state: CustomHookState::Enabled,
            source: Some("#!/bin/sh\nprintf '<not markup>'\n".to_string()),
        }],
        recent_runs: Vec::new(),
        warnings: Vec::new(),
    };
    let html =
        render_hooks_view("acme", "hook-demo", &inventory, Some("Hook disabled.")).into_string();

    assert!(html.contains(">Git server hooks</h2>"), "{html}");
    assert!(html.contains("acme/hook-demo"), "{html}");
    assert!(html.contains(">HEALTHY<"), "{html}");
    assert!(html.contains("pre-receive.twig-user"), "{html}");
    assert!(html.contains("Enabled"), "{html}");
    assert!(
        html.contains("&lt;not markup&gt;"),
        "source is escaped: {html}"
    );
    assert!(
        !html.contains("<not markup>"),
        "source is never markup: {html}"
    );
    assert!(html.contains("Hook disabled."), "{html}");
    assert!(html.contains("NO RECORDED RUNS"), "{html}");
    assert!(
        html.contains("/acme/hook-demo/hooks/pre-receive.twig-user/disable"),
        "enabled hooks offer a disable action: {html}"
    );
    assert!(
        html.contains("Confirm permanent deletion"),
        "permanent removal requires confirmation: {html}"
    );
}

#[test]
fn hooks_view_explains_missing_policy_hook_and_empty_custom_chain() {
    let inventory = HookInventory {
        managed_state: ManagedHookState::Missing,
        custom_hooks: Vec::new(),
        recent_runs: Vec::new(),
        warnings: vec!["The managed pre-receive hook is not installed.".to_string()],
    };
    let html = render_hooks_view("acme", "empty", &inventory, None).into_string();

    assert!(html.contains(">MISSING<"), "{html}");
    assert!(html.contains("NO CUSTOM HOOKS"), "{html}");
    assert!(html.contains("HOOK WARNING"), "{html}");
}

#[test]
fn disabled_hooks_can_be_deleted_but_cannot_be_reenabled_from_the_page() {
    let inventory = HookInventory {
        managed_state: ManagedHookState::Healthy,
        custom_hooks: vec![CustomHookInfo {
            name: "pre-receive.twig-user.1".to_string(),
            state: CustomHookState::Disabled,
            source: Some("#!/bin/sh\nexit 0\n".to_string()),
        }],
        recent_runs: Vec::new(),
        warnings: Vec::new(),
    };
    let html = render_hooks_view("acme", "hook-demo", &inventory, None).into_string();

    assert!(!html.contains("/pre-receive.twig-user.1/enable"), "{html}");
    assert!(
        !html.contains(">Disable</button>"),
        "disabled hooks stay disabled: {html}"
    );
    assert!(
        html.contains("/acme/hook-demo/hooks/pre-receive.twig-user.1/delete"),
        "disabled hooks can be removed: {html}"
    );
}

#[test]
fn hooks_view_renders_recent_run_details_as_escaped_text() {
    let inventory = HookInventory {
        managed_state: ManagedHookState::Healthy,
        custom_hooks: Vec::new(),
        recent_runs: vec![HookRun {
            id: "run-1".to_string(),
            hook_name: "pre-receive.twig-user".to_string(),
            started_at: "2026-10-21T12:34:56Z".to_string(),
            actor: Some("jules".to_string()),
            ref_updates: vec!["0000 1234 refs/heads/main".to_string()],
            status: HookRunStatus::Failed,
            stdout: "client-visible output".to_string(),
            stderr: "<script>not markup</script>".to_string(),
            output_truncated: true,
        }],
        warnings: Vec::new(),
    };
    let html = render_hooks_view("acme", "hook-demo", &inventory, None).into_string();

    assert!(html.contains(">FAILED<"), "{html}");
    assert!(html.contains("2026-10-21T12:34:56Z"), "{html}");
    assert!(html.contains("jules"), "{html}");
    assert!(html.contains("refs/heads/main"), "{html}");
    assert!(html.contains("Recent custom hook runs"), "{html}");
    assert!(html.contains("client-visible output"), "{html}");
    assert!(
        html.contains("&lt;script&gt;not markup&lt;/script&gt;"),
        "{html}"
    );
    assert!(!html.contains("<script>not markup</script>"), "{html}");
    assert!(html.contains("Output was truncated"), "{html}");
}
