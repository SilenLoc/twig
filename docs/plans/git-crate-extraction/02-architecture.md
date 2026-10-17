# Architecture: Git crate extraction

## Fit

Add `crates/twig-git` as a workspace library and make the `twig` application depend on it. Move Git domain and repository behavior there: bare-repository access and config/history/listing, request classification and the CGI backend, repository initialization and receive hooks, path/name validation, and Git-specific tests. The crate owns its Git implementation dependencies and exposes the repository operations and value types used by Twig.

Keep Actix route registration, HTTP response construction, Basic-auth/database policy, and Twig's repository UI in the application. Those are application integration points: the current Git request handler needs `TwigContext`, database models, and namespace authorization, so moving it wholesale would make the reusable crate depend back on Twig. Keep this glue in an explicitly app-owned Git HTTP adapter, which calls `twig-git` for Git work. Move repository filesystem operations out of UI handlers behind crate APIs so Git storage behavior does not remain split across layers. The crate must not initialize or connect to Twig's database.

## Endpoints

None added or changed. Existing Git smart-HTTP routes and authenticated repository-init route keep their paths and behavior.

## Data

No new or changed tables. Repository files, refs, objects, and receive hooks retain their current on-disk format and locations. Existing `.twig.toml` configuration compatibility is preserved.

## Flow

For a Git smart-HTTP request, the app adapter parses the Actix request and resolves authentication/namespace permissions using the existing app services; it passes request data and the verified user/role to `twig-git`; the crate invokes the existing Git CGI backend and returns CGI headers/body; the adapter builds the Actix response.

For repository creation or management, the app route performs app-specific authorization and form handling, then calls `twig-git` to validate names and perform repository filesystem/Git operations. Repository browsing pages call the crate's read-only repository APIs and render the returned values.

## External

No new external services, APIs, environment variables, or webhooks. Continue using the installed `git` executable for repository plumbing and `git http-backend`. `git2` and other Git implementation dependencies belong to `twig-git`; app-wide dependencies such as `chrono` and `uuid` remain in `twig` where non-Git features still use them.
