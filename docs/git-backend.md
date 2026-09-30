# Git Backend

Twig serves repositories over Git smart HTTP and works with standard Git
commands. The web UI and Git endpoints use the same namespace/repository paths.

## Authentication

Git writes use **HTTP Basic Authentication** with your Twig username and
password. Reads of public repositories do not require credentials; repositories
marked `private = true` in `.twig.toml` require credentials for reads as well.

| Operation | Public repository | Private repository |
|-----------|-------------------|-------------------|
| Clone/fetch | No credentials | Basic Auth; user must own the namespace |
| Push | Basic Auth; user must own the namespace | Basic Auth; user must own the namespace |

## Clone and fetch

```sh
git clone http://your-twig-server/namespace/repo
cd repo
git fetch origin
```

The `.git` suffix is also accepted. If Git needs credentials, it will prompt;
you can use a credential helper to store them.

## Push

```sh
git remote add origin http://your-twig-server/namespace/repo
git push -u origin main
```

Avoid putting passwords in remote URLs, where they may be saved in shell history
or Git configuration. Only the namespace owner can push. A push can create a
missing namespace (owned by the pushing user) and repository.

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
`/assets/twig.schema.json`.

## Troubleshooting

### Authentication failed

Check your Twig username and password. For pushes, confirm that you own the
namespace. If Git has cached old credentials, clear or update them using your
credential helper.

### Access denied to namespace

Only the namespace owner can push or read private repositories.

### Repository not found

Check the namespace/repository path. A push can create a repository only when
the authenticated user owns that namespace, or when the namespace does not yet
exist.
