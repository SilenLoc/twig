# Program Design: Namespace invitations

## Files

- `Cargo.toml`, `Cargo.lock` — add the Resend client dependency and lockfile update.
- `docs/environment-variables.md` — document optional Resend settings and canonical invitation URL.
- `src/config.rs`, `src/main.rs` — load optional email/link configuration and provide it to handlers without opening a database connection eagerly.
- `src/email.rs` — wrap Resend delivery and build the invitation email.
- `src/auth/mod.rs` — define namespace roles and invitation domain types while preserving the existing generic invite type/flow.
- `src/db/migration.rs`, `src/db/mod.rs`, `src/db/invitations.rs` — add the namespaced invite table and its lazy database operations.
- `src/db/namespaces.rs`, `src/db/users.rs` — query namespace roles/memberships and users for scoped User Management views.
- `src/http/mod.rs`, `src/http/routes.rs` — register the User Management and namespace-invitation handlers.
- `src/http/tree/mod.rs`, `src/http/tree/user_management.rs`, `src/http/tree/pages.rs` — implement Users/Invites HTMX tabs, invitation actions, scoped lists, and navigation.
- `src/http/auth/mod.rs`, `src/http/auth/namespace_invitations.rs` — render and handle the token-based account setup flow.
- `src/http/namespace/pages.rs`, `src/http/settings/pages.rs` — enforce contributor restrictions and recognize all namespace owners for existing management actions.
- `src/git/mod.rs`, `src/git/backend.rs`, `src/git/repo.rs` — enforce membership/role checks for Git operations, provide role context to receive hooks, and install the hook for managed repositories.
- `src/git/hooks/pre-receive` — reject contributor non-fast-forward updates/deletion of `main` on the server and chain any preserved repository hook.
- `src/integration_tests.rs` — exercise the complete invitation flow and permissions over HTTP/Git.

## Types & signatures

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamespaceRole {
    Owner,
    Contributor,
}

pub struct NamespaceInvitation {
    pub token: String,
    pub email: String,
    pub namespace_id: String,
    pub namespace_name: String,
    pub role: NamespaceRole,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub accepted_at: Option<String>,
    pub accepted_user_id: Option<String>,
}

pub struct NewNamespaceInvitation {
    pub email: String,
    pub namespace_id: String,
    pub role: NamespaceRole,
    pub expires_at: Option<String>,
}

pub enum InvitationDelivery {
    Sent,
    SkippedNoApiKey,
    Failed(String),
}
```

```rust
impl Database {
    pub async fn create_namespace_invitation(
        &self,
        invitation: &NewNamespaceInvitation,
    ) -> Result<NamespaceInvitation, String>;
    pub async fn get_namespace_invitation(
        &self,
        token: &str,
    ) -> Result<Option<NamespaceInvitation>, String>;
    pub async fn list_namespace_invitations(
        &self,
        namespace_ids: Option<&[String]>,
    ) -> Result<Vec<NamespaceInvitation>, String>;
    pub async fn accept_namespace_invitation(
        &self,
        token: &str,
        user: &User,
    ) -> Result<(), String>;
    pub async fn get_namespace_role(
        &self,
        user_id: &str,
        namespace_name: &str,
    ) -> Result<Option<NamespaceRole>, String>;
    pub async fn user_owns_namespace(
        &self,
        user_id: &str,
        namespace_name: &str,
    ) -> Result<bool, String>;
}
```

```rust
pub struct ResendMailer { /* configured Resend client and sender */ }

impl ResendMailer {
    pub async fn send_namespace_invitation(
        &self,
        recipient: &str,
        namespace: &str,
        role: NamespaceRole,
        invite_url: &str,
    ) -> Result<(), String>;
}

pub async fn create_namespace_invitation(
    actor: &User,
    request: NewNamespaceInvitation,
    public_base_url: &str,
    mailer: Option<&ResendMailer>,
    db: &Database,
) -> Result<(NamespaceInvitation, String, InvitationDelivery), String>;
```

```rust
#[get("/tree/users")]
async fn user_management_page(/* request, server, auth state, tab query */) -> HttpResponse;
#[post("/tree/invites")]
async fn create_namespace_invitation_handler(/* request, form, app state */) -> HttpResponse;
#[post("/tree/invites/{id}/resend")]
async fn resend_namespace_invitation_handler(/* request, path, app state */) -> HttpResponse;
#[get("/auth/accept-invite/{token}")]
async fn accept_invitation_page(/* request, path, app state */) -> HttpResponse;
#[post("/auth/accept-invite/{token}")]
async fn accept_invitation_handler(/* request, path, form, app state */) -> HttpResponse;
```

```rust
pub async fn authenticate_git_request(
    /* request, config, auth state, namespace, repo, request kind */
) -> Result<Option<AuthenticatedGitUser>, HttpResponse>;

pub fn install_pre_receive_hook(repo_path: &Path) -> Result<(), String>;
```

## Call stack

- **Create invite:** HTMX form → scoped User Management handler → authorize configured admin or owner of selected namespace → validate email/role/expiry → insert pending invite → if mailer exists, call Resend → return updated invitation list and copyable canonical link/delivery state.
- **Accept invite:** invitation link → load token and reject missing/used/expired invite → render email/namespace/role and username/password → validate form and hash password → transactionally insert user, insert namespace membership, and consume token → show account-created response.
- **Manage users/invites:** `/tree/users` → authenticate session → determine global admin or owned namespace scope → load only rows inside that scope → render Users and Invites tabs; HTMX tab/action responses return fragments.
- **Repository creation:** UI create-repository, `/init`, and Git auto-create paths → authenticate → read namespace role → allow owner creation; reject contributor creation; allow contributor pushes only to existing repositories.
- **Protect main:** Git HTTP handler → authenticate user and resolve namespace role → set trusted CGI role context → receive-pack invokes managed pre-receive hook → contributors may create/fast-forward `main`, but a non-fast-forward update or deletion is rejected; owners are not blocked by this rule. When installing over an existing custom `pre-receive`, preserve it as a sidecar and have the Twig wrapper run it after Twig's policy check with the original ref-update input; never silently overwrite it.

## Test plan

- `migration_creates_namespace_invitations_and_normalizes_legacy_member_role` — migration is repeatable; legacy `member` rows become `contributor`; existing generic invites remain intact.
- `namespace_invitation_crud_and_expiry` — persisted fields round-trip, pending/accepted rows list by scope, null expiry stays valid, expired invites are rejected.
- `accept_namespace_invitation_is_atomic_and_single_use` — one acceptance creates one user and membership; a repeated or concurrent acceptance cannot create another account/access grant.
- `user_management_scope_is_global_for_admin_and_owned_only_for_owner` — site admin sees global rows, namespace owners see only their own namespace, contributors/anonymous users cannot manage invites.
- `invite_form_defaults_to_contributor_and_accepts_owner` — omitted role resolves to Contributor; explicit Owner is retained.
- `invite_without_resend_key_returns_link_without_sending` — no mailer/network call is made and a usable local link is returned.
- `resend_failure_keeps_pending_invitation_and_link` — delivery failure does not consume/delete the invite and the UI exposes retry/recovery.
- `accept_invite_page_prefills_locked_email_and_shows_scope` — email is prefilled and cannot be replaced; namespace and role are clear.
- `user_management_htmx_tabs_and_actions_render_expected_fragments` — Users/Invites navigation and invite/resend forms use HTMX targets and accessible result regions.
- `contributor_cannot_create_namespace_or_repository` — UI namespace/repository creation, Git namespace auto-creation, and `/init` are forbidden to contributors.
- `contributor_can_push_to_existing_repository` — ordinary writes to an existing namespace repository remain allowed.
- `contributor_cannot_rewrite_or_delete_main_but_owner_can` — hook rejects non-fast-forward/deletion of `main` for contributors and does not reject an owner for that role rule.
- `non_owner_cannot_manage_namespace_settings` — contributor cannot rename/delete namespace or move/rename/delete repositories through owner-only settings actions.

## Least confident decisions

1. The hook-chain behavior must preserve the existing custom hook's executable permissions, environment, input, and exit status across repeated installs; its integration test should cover failure and idempotence.
2. Owner invitations can grant another Owner, matching the role selector. If that is too broad, constrain Owner assignment to the configured site administrator.
3. Keeping the old API-key generic signup invitation path preserves outstanding links, but leaves a second invitation mechanism available. Remove it only in a separately approved scope if desired.
4. `PUBLIC_BASE_URL` and `RESEND_FROM` need production values; local configuration should make clear that no email is sent when `RESEND_API_KEY` is absent, rather than encouraging a real key in checked-in files.
