# Architecture: Namespace invitations

## Fit

- Add the User Management screen to the existing admin/tree area, using Actix handlers and maud rendering. Full-page navigation and HTMX partial responses follow the existing `src/http/tree/pages.rs` and `src/http/view.rs` patterns; no client-side framework is introduced.
- Add invitation records and membership operations to the existing lazy Turso/libSQL `Database` modules. Keep database construction lazy; the app must not connect eagerly as part of this work.
- Extend the existing `namespace_members` role model. Site administrator access remains the configured `ADMIN_USER`; namespace owner/contributor access is scoped to memberships.
- Keep the existing generic API-key signup invite flow and its records separate for compatibility. The new namespace invitation flow has an email, namespace, role, and expiry.
- Add Resend as an optional email service. If `RESEND_API_KEY` is absent, invitation creation must not instantiate/call Resend or make any email network request; it still creates the invite and shows a copyable link locally.

## Endpoints

- `GET /tree/users` — configured administrator sees global Users/Invites tabs; namespace owners see only their own namespace's users/invites.
- `POST /tree/invites` — create a namespace invitation; default role is Contributor; send email only when configured.
- `POST /tree/invites/{id}/resend` — retry delivery for a pending invite.
- `GET /auth/accept-invite/{token}` — validate an unexpired, unused invitation and render the account setup form with its email prefilled.
- `POST /auth/accept-invite/{token}` — create the account and namespace membership, then consume the invitation.
- Existing endpoints stay in place for compatibility; newly created namespace invitations use the new flow.

## Data

- Add a `namespace_invitations` table with a token, email, namespace ID, role, created/expiry timestamps, accepted timestamp, and optional accepted user ID. Index token uniquely and support listing pending/recent invitations by namespace and email. A null expiry represents the explicit “Never” option.
- Use the existing `namespace_members(namespace_id, user_id, role, added_at)` table for membership. Normalize legacy `member` values to `contributor`; new namespace creators remain `owner`.
- Account creation, membership insertion, and invitation consumption must be one database transaction so a token cannot produce an account without its namespace access, or be accepted twice.
- Existing `invites` records remain untouched, so old general signup links are not invalidated by the new namespace-scoped flow.

## Flow

1. A configured site administrator or namespace owner opens User Management. The site administrator may choose any namespace; an owner is limited to namespaces they own. Contributors are denied access to invite-management actions.
2. The actor submits email, namespace, role, and expiry via HTMX. The server validates authorization and inputs, stores the invitation, and constructs an absolute link using `PUBLIC_BASE_URL`.
3. If `RESEND_API_KEY` is set, send the invitation using Resend and the configured sender. If it is unset, skip the email client/network call entirely and show the copyable link. On a Resend error, retain the pending invitation and show the link plus a delivery error so the admin can recover or retry.
4. The recipient opens the link. The server checks token validity, expiry, and unused status, then renders the email, namespace, and role with username/password fields.
5. Submission revalidates the token and atomically creates the user, grants the selected membership role, and consumes the token. The account email comes from the invitation, not a changed form value.
6. Namespace permissions are enforced on every mutation route and Git write. Contributors may use existing repository operations, but cannot create namespaces/repositories or invite; a server-side receive hook rejects a contributor's non-fast-forward update or deletion of `main`. Owners retain owner actions.

## External

- Resend API via `resend-rs`; optional `RESEND_API_KEY` (set this to the real key, not the `re_xxxxxxxxx` placeholder), `RESEND_FROM` (a sender address allowed by the Resend account), and `PUBLIC_BASE_URL` (the canonical public URL used in links). No email attempt is made when the API key is absent.
- No other external services.
