# API

Twig's versioned JSON API is rooted at `/api/v1`. It provides namespace and repository management, Git branch/content access, binary-release transfers, and namespace member/invitation operations.

## Authentication and permissions

Every API request requires an `Authorization` header—including `GET /api/version` and reads of repositories whose web pages or Git clones are public. Browser session cookies do not authenticate API requests.

Use an account username/password with HTTP Basic authentication, or a personal access token as a Bearer credential. A personal access token can also be supplied as the Basic password; its username is ignored:

```sh
curl -u "$TWIG_USERNAME:$TWIG_PASSWORD" https://twig.example/api/v1/namespaces
curl -H "Authorization: Bearer $TWIG_TOKEN" https://twig.example/api/v1/me
```

Use HTTPS when sending credentials. Tokens are scoped to namespaces and permissions. Every namespace action checks the effective permission: a token's saved grant is intersected with its owner's current namespace rights, so a token cannot exceed either. API visibility is never made public by a repository's public UI setting.

Namespace permissions used by the API:

| Permission | Operations |
| --- | --- |
| `view_namespace` | Read namespaces, repositories, branches, commits, contents, and binary assets |
| `push_changes` | Commit file changes and upload/replace binary assets |
| `create_repositories` | Create repositories in the namespace |
| `manage_repositories` | Rename, move, or delete repositories |
| `manage_namespace` | Rename/delete namespaces; a scoped token also needs this grant in an existing namespace to create another namespace |
| `manage_members` | List members/invitations and create or resend invitations |

## Identity and version

### `GET /api/v1/me`

Returns the authenticated user's public identity fields (`id`, `username`, and `email`). It never returns password hashes or token data.

### `GET /api/version`

Returns the running application version:

```json
{"version":"0.3.17"}
```

The value comes from the application package at build time. This endpoint also requires authentication.

## Namespaces and repositories

| Method | Path | Required permission |
| --- | --- | --- |
| `GET` | `/api/v1/namespaces` | `view_namespace` for each returned namespace |
| `POST` | `/api/v1/namespaces` | Existing namespace-creation policy; scoped PATs also need `manage_namespace` on a namespace in their scope |
| `GET` | `/api/v1/namespaces/{namespace}` | `view_namespace` |
| `PATCH` | `/api/v1/namespaces/{namespace}` | `manage_namespace` |
| `DELETE` | `/api/v1/namespaces/{namespace}` | `manage_namespace`; namespace must be empty |
| `GET` | `/api/v1/namespaces/{namespace}/repositories?q={text}` | `view_namespace`; `q` is an optional case-insensitive repository-name filter |
| `POST` | `/api/v1/namespaces/{namespace}/repositories` | `create_repositories` |
| `GET` | `/api/v1/namespaces/{namespace}/repositories/{repo}` | `view_namespace` |
| `PATCH` | `/api/v1/namespaces/{namespace}/repositories/{repo}` | `manage_repositories`; rename with `{"name":"new-name"}` or move with `{"target_namespace":"other"}` |
| `DELETE` | `/api/v1/namespaces/{namespace}/repositories/{repo}` | `manage_repositories`; repository must be configured as deletable |

For example, create a repository with a chosen default branch:

```sh
curl -X POST https://twig.example/api/v1/namespaces/acme/repositories \
  -H "Authorization: Bearer $TWIG_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"tooling","default_branch":"main"}'
```

Repository creation requires an email address on the account, matching Twig's web workflow. A repository can be deleted only when its `.twig.toml` configuration has `deletable = true`.

## Branches, commits, trees, and files

| Method | Path | Required permission |
| --- | --- | --- |
| `GET` | `/api/v1/namespaces/{namespace}/repositories/{repo}/branches` | `view_namespace` |
| `GET` | `/api/v1/namespaces/{namespace}/repositories/{repo}/commits?ref={branch}&limit={n}` | `view_namespace` |
| `GET` | `/api/v1/namespaces/{namespace}/repositories/{repo}/tree?ref={branch}&path={path}` | `view_namespace` |
| `GET` | `/api/v1/namespaces/{namespace}/repositories/{repo}/contents?ref={branch}&path={path}` | `view_namespace` |
| `PUT` | `/api/v1/namespaces/{namespace}/repositories/{repo}/contents?ref={branch}&path={path}` | `push_changes` |
| `DELETE` | `/api/v1/namespaces/{namespace}/repositories/{repo}/contents?ref={branch}&path={path}` | `push_changes` |
| `POST` | `/api/v1/namespaces/{namespace}/repositories/{repo}/contents/rename` | `push_changes` |

`ref` names an exact local branch under `refs/heads/`, not an arbitrary Git revision. Slash-containing names must be URL encoded in the query string, for example `ref=feature%2Fdocs`. Content paths are repository-relative; traversal and Twig-ignored paths are refused.

Tree entries are JSON. File reads always return Base64 content, preserving arbitrary binary bytes:

```json
{
  "path": "src/main.rs",
  "reference": "main",
  "encoding": "base64",
  "content": "Zm4gbWFpbigpIHt9Cg==",
  "object_id": "…",
  "size_bytes": 13
}
```

Create or replace a file by sending JSON. `expected_head` is the current commit ID at the selected branch; stale updates return `409 Conflict` rather than overwriting newer work. `encoding` is `utf8` or `base64`. File contents are limited to 1 MiB per operation.

```sh
curl -X PUT 'https://twig.example/api/v1/namespaces/acme/repositories/tooling/contents?ref=main&path=README.md' \
  -H "Authorization: Bearer $TWIG_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"content":"Hello from Twig API\n","encoding":"utf8","message":"Update README","expected_head":"<current-commit-id>"}'
```

Delete sends `message` and `expected_head` as a JSON body. Rename sends `ref`, `from`, `to`, `message`, and `expected_head` as a JSON body to `/contents/rename`. Mutations create normal commits on the selected branch; they do not expose arbitrary Git refs or history rewrites.

## Binary releases

| Method | Path | Required permission |
| --- | --- | --- |
| `GET` | `/api/v1/namespaces/{namespace}/repositories/{repo}/binaries` | `view_namespace` |
| `GET` | `/api/v1/namespaces/{namespace}/repositories/{repo}/binaries/{version}/{filename}` | `view_namespace` |
| `PUT` | `/api/v1/namespaces/{namespace}/repositories/{repo}/binaries/{version}/{filename}` | `push_changes` |

Uploads use a raw `application/octet-stream` request body and return JSON. Downloads return the original bytes. Twig retains its three highest SemVer releases, supports `latest` for downloads, and enforces a 512 MiB upload ceiling and safe filenames.

```sh
curl -X PUT 'https://twig.example/api/v1/namespaces/acme/repositories/tooling/binaries/1.2.3/tool-linux-amd64' \
  -H "Authorization: Bearer $TWIG_TOKEN" \
  -H 'Content-Type: application/octet-stream' \
  --data-binary @./tool-linux-amd64
```

## Members and invitations

| Method | Path | Required permission |
| --- | --- | --- |
| `GET` | `/api/v1/namespaces/{namespace}/members` | `manage_members` |
| `GET` | `/api/v1/namespaces/{namespace}/invitations` | `manage_members` |
| `POST` | `/api/v1/namespaces/{namespace}/invitations` | `manage_members` |
| `POST` | `/api/v1/namespaces/{namespace}/invitations/{id}/resend` | `manage_members` in the invitation's namespace |

Invitation creation accepts `email`, `role` (`owner` or `contributor`), and `expires` (`1d`, `7d`, `30d`, or `never`). Its response contains the invitation URL so it can be shared; the invitation token is never included in list responses. Member and invitation data is limited to the authorized namespace.

## Errors and limits

Errors are JSON with a stable code and human-readable message:

```json
{"error":{"code":"forbidden","message":"The required permission is not granted"}}
```

Common statuses are `400` for invalid input, `401` for absent/invalid credentials, `403` for insufficient permissions, `404` for missing resources, `409` for conflicts/stale branch tips, `413` for oversize payloads, and `500` for internal failures. Internal database and filesystem details are logged server-side and are not returned.

Signup/login, account settings, personal-access-token creation/revocation, database administration, test endpoints, and Git Smart HTTP are not JSON API routes. Existing UI/public Git behavior is unchanged; the authentication requirement applies to `/api/` endpoints.

## Forgejo-compatible API

Pull requests, reviews, and the repository routes used by T3 Code and the
Forgejo and Gitea command-line tools live under `/api/v1/repos/{owner}/{repo}`,
`/api/v1/user`, and `/api/v1/version`. They use Forgejo's request and response
shapes and error body (`{"message": ...}`), and accept `Authorization: token`.
See [Pull Requests](pull-requests.md) for the endpoint table and the permission rules.
