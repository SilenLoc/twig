# Git Backend

Twig serves repositories over Git smart HTTP and works with standard Git
commands. The web UI and Git endpoints use the same namespace/repository paths.

## Authentication

Git writes use **HTTP Basic Authentication** with your Twig username and
password, or with a Twig access token as the password. Token credentials are
also accepted as Bearer credentials. Public repository reads do not require
credentials; repositories marked `private = true` in `.twig.toml` require a
token with `View namespace` or normal account credentials for reads.

| Operation | Public repository | Private repository |
|-----------|-------------------|-------------------|
| Clone/fetch | No credentials | Basic Auth or Bearer; token needs `View namespace` |
| Push | Basic Auth or Bearer; account needs `Push changes` | Basic Auth or Bearer; token needs `Push changes` |

Token permissions are limited to their selected namespaces and actions, then
intersected with the user's current Twig permissions. A token cannot grant more
access than its owner currently has.

## Request body limits

Git read request bodies and repository-edit forms are limited to 1 MiB. Git
pushes and binary uploads are limited to 512 MiB. Write credentials and
permissions are checked before Twig consumes those bodies; oversized bodies are
rejected with `413 Payload Too Large`. Large Git-write and binary-upload body
processing shares one in-flight slot, while small request bodies share eight.
These application limits complement, but do not replace, reverse-proxy ingress
limits.

## Clone and fetch

```sh
git clone https://your-twig-server/namespace/repo
cd repo
git fetch origin
```

The `.git` suffix is also accepted. If Git needs credentials, it will prompt;
you can use a credential helper to store them.

## Sign in with Tree

Run `tree auth` to finish Twig sign-in in your browser, choose token namespaces
and actions, and configure Git for that Twig URL. Tree defaults to
`https://twig.silenlocatelli.ch/`; pass `--url` for another instance. The token
is stored in the operating system's credential store, not in the remote URL or
Git configuration. Revoke the `Tree CLI` token from Account Settings → Access
Tokens.

## Push

```sh
git remote add origin https://your-twig-server/namespace/repo
git push -u origin main
```

Avoid putting passwords or tokens in remote URLs, where they may be saved in
shell history or Git configuration. A push needs the account's current
`Push changes` permission for an existing repository. A push can create a new
repository in an existing namespace when the credential grants
`Create repositories`. Password-authenticated users who can create namespaces
may also bootstrap a missing namespace; scoped tokens cannot access namespaces
that were not selected when the token was created.

## Account and repository setup

1. Get a one-time signup invite from the Twig administrator using the invite
   page.
2. Sign up at `http://your-twig-server/auth/signup` with the invite, username,
   email address, and password.
3. Log in and create a namespace at `http://your-twig-server/auth/namespace`,
   or let your first authenticated push create it.
4. Create a repository in the web UI at `http://your-twig-server/namespace`
   (the account needs an email), or push to a new repository path in your
   namespace.

For scripts and integrations, `POST /init` also creates a repository in an
existing namespace the authenticated user owns. Submit `namespace`, `repo`,
and optionally `branch` as form fields; the branch defaults to `main`.

## Repository configuration

Twig reads `.twig.toml` from the repository's current commit. Set `private = true`
to require authenticated access for reads. Other options control which files
appear in the browser, which repository tabs are shown, whether the repository
can be deleted from Settings, and which Markdown files appear in the
presentation view. New UI-created repositories include a commented
configuration template. The schema is available at
`/assets/twig.schema.json`, and every option is documented in the
Repository Config docs.

## Troubleshooting

### Authentication failed

Check the Twig account credentials or access token. For pushes, confirm the
account and token have `Push changes` for the namespace. If Git has cached old
credentials, clear or update them using the configured credential helper.

### Access denied to namespace

Only the namespace owner can push or read private repositories.

### Repository not found

Check the namespace/repository path. A push can create a repository only when
the authenticated user owns that namespace, or when the namespace does not yet
exist.
