# Status: Namespace invitations

- Gate 1 — Product: APPROVED 2026-10-10
- Gate 2 — Architecture: APPROVED 2026-10-10
- Gate 3 — Program Design: APPROVED 2026-10-10
- Gate 4 — Slice plan: APPROVED 2026-10-10

## Slices

- [x] Slice 1 — HTMX User Management → mocked invite link → mocked setup page, runnable locally.
- [x] Slice 2 — persist namespace invites; authorize admin/owner scope; list them and create copyable links with chosen expiry.
- [x] Slice 3 — accept real invitations atomically; create account and namespace membership; enforce single-use and expiry.
- [x] Slice 4 — optional Resend delivery and retry; no key means no email attempt, with local fake-server tests.
- [x] Slice 5 — enforce owner/contributor permissions across management UI, namespace/repository creation, and Git writes.
- [x] Slice 6 — enforce contributor main-branch rewrite protection with a safe, chained receive hook.

## Notes for a fresh session

- Administrators manage this under User Management, alongside Users and Invites.
- Invitations are email links; select the namespace and role per invite, with Contributor as the default, and choose an expiry.
- Invitees set a username and password; their email is prefilled.
- Contributor can do what an owner can within a namespace, but cannot invite people or create namespaces/repositories. In repositories, contributors cannot force-push to main.
- The user requested Resend for email and HTMX for UI work. Make deployment setup explicit and ask them to replace `re_xxxxxxxxx` with their real Resend API key when appropriate.
- Local runs must not attempt to send email unless a Resend API key is configured.
- Slice 1 complete: full test suite passes (389 tests). Local click-through verified with a throwaway DB/root under `/tmp/opencode/twig-invite-slice1`; User Management returned 200 after login, demo invite returned a setup link, setup showed prefilled email, and submit explicitly created no account. No Resend key was set and this stub contains no email sender.
- Slice 2 complete: full test suite passes (392 tests). Local app on port `18081`, using a throwaway DB/root under `/tmp/opencode/twig-invite-slice2` and with `RESEND_API_KEY` unset, created a real persisted invite, returned a shareable link, and opened setup showing the email, namespace, and role. Invite lists show stored invitations and expiry; expired links return 410. Owner scope can invite only into its own namespace. Account creation is still intentionally deferred to Slice 3, and no email is sent in Slice 2.
- Slice 3 complete: full test suite passes (397 tests). Local app on port `18082`, using `/tmp/opencode/twig-invite-slice3` and with `RESEND_API_KEY` unset, completed an invite through account creation, login, and access to the invited namespace; the accepted link then returned 410. Tests cover atomic account/membership/consumption, duplicate usernames, email binding, expiry, rollback, and concurrent single-use acceptance. Contributor-versus-owner restrictions are still pending Slice 5; this partial workflow should not be treated as production-ready before those rules land.
- Slice 4 complete: full test suite passes (405 tests). `RESEND_API_KEY`, `RESEND_FROM`, and `PUBLIC_BASE_URL` are documented; email attempts require both a non-empty API key and a valid canonical URL. Local app on port `18083` ran with the key unset: creating and retrying an invite both returned a copyable link and made no email request. Fake Resend HTTP tests cover request payload, success, failure, and zero requests without required configuration. No real key was used and no real email was sent.
- Slice 5 complete: full test suite passes (409 tests). Legacy `member` rows migrate to Contributor; the Users tab now lists global users for the configured admin and only namespace members for owners. Contributor creation is blocked in namespace UI, namespace creation, `/init`, Git auto-creation, and owner-only management routes; existing repository access and Git write advertisement remain allowed. Local verification at `http://localhost:18085` confirmed the Users list, hidden create-repo button, 403s for create actions, 200 for an existing-repo Git write handshake, and 403 for a new-repo handshake. Main-branch rewrite protection remains Slice 6.
- Slice 6 complete: full test suite passes (412 tests). The receive hook rejects contributor deletion or non-fast-forward updates to `main`, permits creation/fast-forward updates and owner rewrites, and chains preserved custom hooks with original input and environment. Hook tests cover executable modes, failure propagation, and repeat-install idempotence. Local Smart HTTP verification at `http://localhost:18086` allowed a Contributor fast-forward, rejected their force-push to `main`, and allowed an Owner force-push. No Resend key was set and no email was sent.
