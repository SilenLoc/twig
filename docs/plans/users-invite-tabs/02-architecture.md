# Architecture: Users and invite tabs

## Fit

The existing user-management page is rendered by `src/http/tree/user_management.rs`. Its tab links request `/tree/users?tab=...` and swap into `#user-management-content`. Keep the tabs outside that swap target and make the htmx response match the target's contents, so a partial update cannot insert another copy of the tabs.

## Endpoints

- `GET /tree/users` — unchanged; serves the complete page for normal requests and tab content for htmx requests.

## Data

No data changes. The existing handler continues loading scoped invitations and, for the Users tab, scoped users.

## Flow

1. A normal `GET /tree/users` renders the tree navigation, one Users/Invites tab bar, and the active tab content.
2. Clicking a Users or Invites tab sends an htmx `GET` to the same route.
3. The handler renders only the content expected by `#user-management-content`; htmx replaces that target without adding another tab bar.

## External

None.
