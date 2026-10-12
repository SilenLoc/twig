# Program Design: Users and invite tabs

## Files

- `src/http/tree/user_management.rs` — split tab-bar rendering from tab-content rendering and return only target contents for htmx requests.
- `src/integration_tests.rs` — assert the initial page has one link for each tab and each htmx response excludes tab navigation.

## Types & signatures

```rust
fn render_user_management_tabs(active_tab: &str) -> Markup;

fn render_user_management_content(
    scope: &UserManagementScope,
    active_tab: &str,
    invitations: &[NamespaceInvitation],
    users: &[UserManagementEntry],
) -> Markup;

fn render_user_management(
    scope: &UserManagementScope,
    active_tab: &str,
    invitations: &[NamespaceInvitation],
    users: &[UserManagementEntry],
) -> Markup;
```

`render_user_management` composes the tabs and `#user-management-content` wrapper for a full page. The htmx branch calls `render_user_management_content` directly so its response is the inner HTML for the existing target.

## Call stack

- Initial page: `user_management_page` → load scoped data → `render_user_management` → `render_user_management_tabs` + `render_user_management_content` → layout.
- Tab update: htmx `GET /tree/users?tab=...` → `user_management_page` → load scoped data → `render_user_management_content` → swap into `#user-management-content`.

## Test plan

- `full_user_management_render_has_one_link_per_tab` — each tab URL appears once in the full markup.
- `htmx_user_management_response_contains_content_without_tabs` — the tab contents render, but neither tab link nor tab navigation is included in fragment markup.
- `test_user_management_renders_admin_page_and_htmx_tabs` — the authenticated full page contains one link to each tab, and htmx requests for both Users and Invites return content without tab links.
- Existing user-management, invitation, scoping, and UI integration tests remain green.

## Least confident decisions

1. Return the target's inner HTML, not a second `#user-management-content` wrapper; this best matches the current default htmx swap behavior.
2. Keep the existing `hx-target` and default swap mode rather than changing the client-side target to a larger wrapper.
3. Keep the htmx request detection based on the existing `HX-Request` header; request-header hardening is outside this fix.
