# Vertical Slices: Smaller, clearer source modules

1. **Tracer — Git repository tests:** move the existing tests in `src/git/repo.rs` into `src/git/repo/tests.rs`; preserve their parent-module access and names. Run the focused test module, the full suite, format, and diff checks.
2. **Database unit tests:** extract the tests from `src/db/namespaces.rs` and `src/db/invitations.rs` into child test files. Run both focused test modules and verify test totals.
3. **Git and highlighter tests:** extract tests from `src/git/bare/handle.rs`, `src/git/bare/config.rs`, and `crates/twig-highlight/src/lib.rs`. Run the Git/highlighter tests and verify test totals.
4. **Auth and namespace page tests:** extract tests from `src/http/auth/pages.rs` and `src/http/namespace/pages.rs`. Run their focused tests and relevant integration tests.
5. **Other oversized HTTP tests:** extract tests from `src/http/settings/pages.rs`, `src/http/test_page/pages.rs`, `src/http/tree/pages.rs`, `src/http/tree/user_management.rs`, `src/http/repository/editor.rs`, and `src/http/assets.rs`. Keep tests beside their implementation modules and run focused tests.
6. **Integration-test organization:** split `src/integration_tests.rs` into behavior-focused test modules with only common fixtures in support; run the full service-level suite and confirm no test is lost.
7. **Feature module splits:** split account/repository/namespace settings, test-page catalog/rendering, tree data browsing, user-management invitations, and editor rendering into cohesive modules. Preserve handler exports and run focused plus full tests.
8. **Repository page split:** move Markdown, content, Scripts, Paper, and presentation/PDF behavior into focused modules; split the large page-test module by those same concerns. Preserve route paths, handlers, and rendering contracts; run all repository unit and integration tests.
9. **Final review:** extract inline tests from the four near-threshold files where tests are already a clean boundary; inspect the remaining production code without splitting for line count alone. Run `cargo fmt --check`, `git diff --check`, workspace tests, Marg, and record the final file inventory.

After each slice, compare the test count and behavior, update `00-status.md`, and stop for the user's direction before continuing. No slice changes database lifecycle, routes, or intended UI/API behavior.
