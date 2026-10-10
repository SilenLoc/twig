# Slice Plan: Namespace invitations

## Slice 1 — Tracer bullet: mocked HTMX invite-to-setup path

- Add the protected User Management entry and Users/Invites tabs, a stub invitation form, an HTMX response containing a clearly marked demo link, and a stub account-setup page with prefilled demo email.
- No database schema, account creation, or email delivery in this slice; keep the stub isolated so later slices replace it rather than build on demo behavior.
- Prove it runs locally with `RESET_DB=true` and `ADMIN_USER=admin`: open User Management, submit the form, follow the returned link, and see the setup page. Add route/markup tests for full-page and HTMX responses.

## Slice 2 — Persist and manage real invitations

- Add the namespaced invitation migration/data operations, namespace/role/expiry form validation, admin-global and owner-scoped authorization, pending/expired/accepted list, and a real random token/link. The link resolves to its stored invitation details; account creation remains for Slice 3. Keep Resend out of this slice: creation returns the copyable link and makes no email attempt regardless of key.
- Prove the new migration works on a fresh DB and an existing DB; create an invite via HTMX, verify it persists and appears with the chosen namespace/role/expiry, and verify out-of-scope users are rejected.

## Slice 3 — Complete signup and membership

- Replace the setup stub with token lookup and validation; prefill/lock the invitation email; validate username/password; transactionally create the user, role membership, and consumed state; reject expired, used, or invalid tokens.
- Prove a local invite can be followed through to a real account that can access only the invited namespace. Run database and HTTP tests for expiry, one-use/concurrent acceptance, username collisions, and transaction rollback.

## Slice 4 — Resend delivery, optional and recoverable

- Add the Resend adapter, `RESEND_API_KEY`, `RESEND_FROM`, and `PUBLIC_BASE_URL` configuration/docs, plus delivery status and resend action. If the key is absent, do not instantiate/call the sender or perform network I/O; keep the copyable local link. If delivery fails, retain the pending link and allow retry.
- Prove no-key behavior with a sender/request spy that asserts zero requests. Test success and failure against a local Resend-compatible fake server; do not require a live API key or send real email in tests. Ask the user to set the real key outside the repository before testing actual delivery; never commit it.

## Slice 5 — Enforce membership permissions

- Normalize legacy `member` to Contributor; populate the scoped Users tab and enforce Owner/Contributor access in User Management, namespace/repository create routes, `/init`, Git auto-creation, and owner-only namespace/repository settings. Contributor pushes to existing repositories remain allowed.
- Prove both allowed and denied paths through HTTP and Git integration tests; verify the configured site admin and namespace owners retain their intended scopes.

## Slice 6 — Protect `main` from contributor history rewrites

- Install the role-aware server-side receive hook for managed repositories. Reject contributor non-fast-forward updates and deletion of `main`, allow normal/fast-forward pushes and owner rewrites, and preserve/chain any pre-existing custom `pre-receive` hook without silently overwriting it.
- Prove behavior with real local bare-repository and Smart HTTP pushes for contributor and owner credentials. Test custom-hook input, environment, executable state, failure propagation, and repeat-install idempotence.

## Slice boundaries

- After every slice, run its focused tests and relevant regression tests, start or exercise the local app, and report the observable result before checking the slice off in `00-status.md`.
- Stop after each completed slice and ask whether to continue or re-steer. Do not start the next slice without the user's direction.
- Local delivery stays disabled unless `RESEND_API_KEY` is explicitly configured. Never put the real key in tracked files or test output.
