# Margaret Project Complexity Report

Audit date (UTC): 2026-10-13

## Project summary

- Audit status: **CLEAR**
- Rust source files discovered: 126
- Files measured: 126
- Files skipped with read warnings: 0
- Total source size: 984029 bytes
- Total physical lines: 28397
- Functions analyzed: 1076
- Average estimated function complexity: 2.6
- Maximum estimated function complexity: 20
- Complexity review hotspots (>= 10): 45
- Large-file review hotspots (> 500 physical lines): 0
- Hard-blocking lint suppressions: 0
- Likely unused underscore-prefixed bindings: 13

## Review thresholds

- Function hotspot: estimated complexity of 10 or higher.
- Large file: more than 500 physical lines.

## Rust source file inventory

| File | Bytes | Physical lines | Review status |
| --- | ---: | ---: | --- |
| `src/http/repository/pages/presentation.rs` | 18659 | 489 | — |
| `src/http/overview/pages.rs` | 17804 | 488 | — |
| `src/auth/mod.rs` | 14295 | 485 | — |
| `src/http/view.rs` | 20523 | 477 | — |
| `src/config.rs` | 14719 | 469 | — |
| `src/http/repository/pages/tabs.rs` | 16204 | 468 | — |
| `crates/twig-highlight/src/lib.rs` | 13896 | 461 | — |
| `src/git/mod.rs` | 16257 | 458 | — |
| `src/http/namespace/pages.rs` | 16100 | 447 | — |
| `src/db/binaries.rs` | 14911 | 445 | — |
| `src/integration_tests/auth_and_invitations.rs` | 17079 | 443 | — |
| `src/http/auth/handlers.rs` | 14219 | 441 | — |
| `src/http/repo_edit/pages.rs` | 19337 | 440 | — |
| `src/http/info/pages.rs` | 15130 | 440 | — |
| `src/http/repository/pages/paper.rs` | 15298 | 427 | — |
| `src/http/auth/pages.rs` | 13503 | 411 | — |
| `src/db/migration.rs` | 13881 | 408 | — |
| `src/git/bare/handle/tests.rs` | 12391 | 407 | — |
| `src/http/settings/pages/tests.rs` | 12829 | 405 | — |
| `src/integration_tests/repo_access_and_routes.rs` | 15848 | 400 | — |
| `src/http/settings/pages/repositories.rs` | 15547 | 394 | — |
| `src/git/backend.rs` | 12539 | 392 | — |
| `src/http/namespace/pages/tests.rs` | 13137 | 390 | — |
| `src/http/repository/pages/tests/tabs.rs` | 13050 | 382 | — |
| `src/http/assets/tests.rs` | 11907 | 379 | — |
| `src/http/repository/pages/markdown.rs` | 12243 | 372 | — |
| `src/integration_tests/repo_workspace_ui.rs` | 14292 | 365 | — |
| `src/git/bare/mod.rs` | 12839 | 359 | — |
| `src/git/bare/handle/commit.rs` | 12420 | 351 | — |
| `src/email.rs` | 12342 | 350 | — |
| `src/db/namespaces.rs` | 11559 | 338 | — |
| `src/integration_tests/repo_binaries.rs` | 11413 | 337 | — |
| `src/integration_tests/repo_workspace_commit.rs` | 13381 | 332 | — |
| `src/http/auth/pages/tests.rs` | 10831 | 327 | — |
| `src/db/users.rs` | 11332 | 323 | — |
| `src/integration_tests/namespace_permissions.rs` | 11447 | 319 | — |
| `src/http/repository/pages/content.rs` | 11106 | 317 | — |
| `src/integration_tests/repo_content.rs` | 10492 | 300 | — |
| `src/git/repo/tests.rs` | 10482 | 300 | — |
| `src/db/invitations.rs` | 10616 | 299 | — |
| `src/http/tree/user_management/views.rs` | 12703 | 298 | — |
| `src/git/bare/listing.rs` | 11226 | 291 | — |
| `src/git/bare/config.rs` | 10239 | 291 | — |
| `src/http/test_page/pages/handlers.rs` | 8955 | 290 | — |
| `src/http/repository/pages/context.rs` | 9353 | 282 | — |
| `src/db/invitations/tests.rs` | 8353 | 277 | — |
| `src/db/namespaces/tests.rs` | 7760 | 276 | — |
| `src/git/bare/config/tests.rs` | 8213 | 274 | — |
| `src/http/repo_edit/handlers.rs` | 7869 | 271 | — |
| `src/http/repository/binaries.rs` | 8943 | 270 | — |
| `src/git/bare/handle/read.rs` | 8902 | 268 | — |
| `src/md/mod.rs` | 9145 | 266 | — |
| `src/http/auth/namespace_invitations.rs` | 10400 | 260 | — |
| `src/http/repository/pages/scripts.rs` | 10473 | 253 | — |
| `src/http/tree/api.rs` | 9485 | 247 | — |
| `src/http/repository/pages/tests/presentation.rs` | 9099 | 244 | — |
| `src/http/test_page/pages/views.rs` | 10429 | 243 | — |
| `src/integration_tests/test_page.rs` | 9465 | 239 | — |
| `src/http/tree/pages/namespaces.rs` | 11170 | 238 | — |
| `src/git/repo.rs` | 6854 | 235 | — |
| `src/integration_tests/admin_tree_settings.rs` | 8547 | 234 | — |
| `src/http/repo_edit/mod.rs` | 6493 | 232 | — |
| `src/http/tree/user_management/invitations.rs` | 7852 | 228 | — |
| `src/integration_tests/repo_management.rs` | 9146 | 226 | — |
| `src/integration_tests/repo_traversal.rs` | 7929 | 225 | — |
| `src/auth/permissions.rs` | 6808 | 221 | — |
| `src/http/tree/pages/data.rs` | 6818 | 197 | — |
| `src/http/repository/pages/tab_handlers.rs` | 4542 | 196 | — |
| `src/http/tree/user_management/tests.rs` | 6434 | 194 | — |
| `src/http/repository/pages/tests/markdown.rs` | 7809 | 193 | — |
| `src/http/settings/pages/namespaces.rs` | 6502 | 192 | — |
| `src/integration_tests/support.rs` | 6253 | 190 | — |
| `src/db/data.rs` | 6514 | 183 | — |
| `src/info/mod.rs` | 5453 | 180 | — |
| `src/http/routes.rs` | 7265 | 179 | — |
| `src/http/settings/pages/profile.rs` | 6101 | 179 | — |
| `src/http/tree/pages/repositories.rs` | 5993 | 179 | — |
| `src/http/repo_edit/commit.rs` | 5541 | 177 | — |
| `src/http/repository/pages/tests/paper.rs` | 5862 | 173 | — |
| `src/http/repository/pages/tests/content.rs` | 6131 | 168 | — |
| `src/http/repo_edit/operations.rs` | 5958 | 163 | — |
| `src/http/repository/pages/tests.rs` | 4772 | 161 | — |
| `src/auth/session_store.rs` | 4878 | 157 | — |
| `src/http/assets.rs` | 4695 | 156 | — |
| `src/http/repo_edit/validation.rs` | 5474 | 155 | — |
| `src/main.rs` | 5142 | 148 | — |
| `src/db/tokens.rs` | 4281 | 140 | — |
| `src/http/test_page/pages/catalog.rs` | 3494 | 137 | — |
| `src/db/invites.rs` | 4043 | 133 | — |
| `src/http/tree/pages/tests.rs` | 5235 | 132 | — |
| `src/http/tree/user_management/access.rs` | 4248 | 128 | — |
| `crates/twig-highlight/src/tests.rs` | 5383 | 121 | — |
| `src/git/repo/hooks.rs` | 4960 | 117 | — |
| `src/db/mod.rs` | 3282 | 116 | — |
| `src/git/reserved.rs` | 3116 | 107 | — |
| `src/db/test_pins.rs` | 3157 | 101 | — |
| `src/http/repository/pages.rs` | 1840 | 101 | — |
| `src/http/test_page/pages.rs` | 2142 | 86 | — |
| `src/db/sessions.rs` | 2564 | 85 | — |
| `src/http/repository/pages/tests/markup.rs` | 2829 | 75 | — |
| `src/http/repo_edit/tests.rs` | 2678 | 73 | — |
| `src/git/bare/handle.rs` | 1818 | 69 | — |
| `src/git/bare/history.rs` | 1666 | 67 | — |
| `src/http/settings/pages/shared.rs` | 1857 | 54 | — |
| `src/git/bare/path.rs` | 1745 | 52 | — |
| `src/http/settings/pages.rs` | 1028 | 52 | — |
| `src/http/tree/pages/hub.rs` | 1711 | 43 | — |
| `src/http/auth/session.rs` | 1469 | 43 | — |
| `src/http/test_page/pages/tests.rs` | 1513 | 42 | — |
| `crates/twig-highlight/build.rs` | 1339 | 38 | — |
| `src/http/tree/pages.rs` | 854 | 36 | — |
| `src/http/tree/user_management.rs` | 805 | 36 | — |
| `src/http/api.rs` | 1012 | 35 | — |
| `src/http/health.rs` | 959 | 33 | — |
| `src/integration_tests.rs` | 972 | 31 | — |
| `src/http/repository/pages/tests/scripts.rs` | 918 | 30 | — |
| `src/http/view/test_util.rs` | 833 | 22 | — |
| `src/http/mod.rs` | 230 | 14 | — |
| `src/http/auth/mod.rs` | 81 | 4 | — |
| `src/http/tree/mod.rs` | 53 | 3 | — |
| `src/http/repository/mod.rs` | 33 | 2 | — |
| `src/http/info/mod.rs` | 15 | 1 | — |
| `src/http/namespace/mod.rs` | 15 | 1 | — |
| `src/http/overview/mod.rs` | 15 | 1 | — |
| `src/http/settings/mod.rs` | 15 | 1 | — |
| `src/http/test_page/mod.rs` | 15 | 1 | — |

## Function complexity ranking

| File | Line | Function | Estimated complexity | Review status |
| --- | ---: | --- | ---: | --- |
| `src/git/bare/handle/commit.rs` | 15 | `commit_file` | 20 | Review hotspot |
| `src/git/bare/handle/commit.rs` | 81 | `commit_new_file` | 19 | Review hotspot |
| `src/http/repo_edit/operations.rs` | 51 | `rename_workspace_file` | 19 | Review hotspot |
| `src/git/repo/hooks.rs` | 12 | `install_pre_receive_hook` | 18 | Review hotspot |
| `src/http/auth/namespace_invitations.rs` | 139 | `accept_invitation_handler` | 18 | Review hotspot |
| `src/db/invitations.rs` | 30 | `validate_invitation_for_user` | 17 | Review hotspot |
| `src/http/settings/pages/namespaces.rs` | 16 | `rename_namespace` | 17 | Review hotspot |
| `crates/twig-highlight/src/lib.rs` | 285 | `try_from` | 16 | Review hotspot |
| `src/db/invitations.rs` | 258 | `list_namespace_invitations_for` | 16 | Review hotspot |
| `src/db/binaries.rs` | 201 | `store_binary_in_transaction` | 15 | Review hotspot |
| `src/db/invitations.rs` | 175 | `get_namespace_invitation` | 15 | Review hotspot |
| `src/git/mod.rs` | 139 | `authenticate_git_request` | 15 | Review hotspot |
| `src/http/settings/pages/repositories.rs` | 197 | `move_repo` | 15 | Review hotspot |
| `src/db/data.rs` | 37 | `read_table_page` | 14 | Review hotspot |
| `src/http/settings/pages/repositories.rs` | 301 | `rename_repo` | 14 | Review hotspot |
| `src/http/auth/handlers.rs` | 112 | `signup_ui_handler` | 13 | Review hotspot |
| `src/db/users.rs` | 33 | `list_users_for_management_in` | 12 | Review hotspot |
| `src/http/repo_edit/validation.rs` | 4 | `validate_new_path` | 12 | Review hotspot |
| `src/email.rs` | 69 | `invitation_url` | 11 | Review hotspot |
| `src/git/bare/config.rs` | 243 | `should_ignore` | 11 | Review hotspot |
| `src/git/bare/handle/read.rs` | 233 | `collect_markdown_files` | 11 | Review hotspot |
| `src/git/bare/listing.rs` | 19 | `extract_license_from_cargo_toml` | 11 | Review hotspot |
| `src/http/auth/handlers.rs` | 265 | `create_namespace_ui_handler` | 11 | Review hotspot |
| `src/http/namespace/pages.rs` | 96 | `format_date` | 11 | Review hotspot |
| `src/http/overview/pages.rs` | 18 | `render_index` | 11 | Review hotspot |
| `src/http/repo_edit/mod.rs` | 169 | `editor_file_fragment` | 11 | Review hotspot |
| `src/http/repo_edit/operations.rs` | 107 | `delete_workspace_file` | 11 | Review hotspot |
| `src/http/repository/binaries.rs` | 96 | `download_binary` | 11 | Review hotspot |
| `src/http/repository/pages/presentation.rs` | 50 | `slide_handler` | 11 | Review hotspot |
| `crates/twig-highlight/src/lib.rs` | 216 | `token_kind` | 10 | Review hotspot |
| `src/db/binaries.rs` | 89 | `list_binary_releases` | 10 | Review hotspot |
| `src/db/binaries.rs` | 165 | `get_latest_binary` | 10 | Review hotspot |
| `src/db/invites.rs` | 22 | `get_invite` | 10 | Review hotspot |
| `src/db/mod.rs` | 33 | `conn` | 10 | Review hotspot |
| `src/db/namespaces.rs` | 86 | `namespaces_with_owners` | 10 | Review hotspot |
| `src/db/users.rs` | 89 | `get_user_by_username` | 10 | Review hotspot |
| `src/db/users.rs` | 112 | `get_user_by_id` | 10 | Review hotspot |
| `src/git/bare/handle/commit.rs` | 137 | `commit_changes` | 10 | Review hotspot |
| `src/git/bare/handle/read.rs` | 160 | `list_dir` | 10 | Review hotspot |
| `src/git/repo.rs` | 101 | `create_repo` | 10 | Review hotspot |
| `src/http/namespace/pages.rs` | 124 | `render_namespace` | 10 | Review hotspot |
| `src/http/repo_edit/validation.rs` | 104 | `validate_draft_change` | 10 | Review hotspot |
| `src/http/repository/pages/scripts.rs` | 37 | `render_scripts_view` | 10 | Review hotspot |
| `src/http/tree/user_management/invitations.rs` | 166 | `resend_namespace_invitation` | 10 | Review hotspot |
| `src/md/mod.rs` | 95 | `render_mermaid_blocks` | 10 | Review hotspot |
| `crates/twig-highlight/src/lib.rs` | 33 | `class` | 9 | — |
| `crates/twig-highlight/src/lib.rs` | 341 | `match_at` | 9 | — |
| `src/auth/mod.rs` | 280 | `seed_dev_admin` | 9 | — |
| `src/db/binaries.rs` | 45 | `put_binary` | 9 | — |
| `src/db/namespaces.rs` | 52 | `get_namespace_by_name` | 9 | — |
| `src/db/namespaces.rs` | 179 | `get_owned_namespaces_for_user` | 9 | — |
| `src/db/namespaces.rs` | 255 | `get_namespaces_for_user` | 9 | — |
| `src/git/backend.rs` | 54 | `kind` | 9 | — |
| `src/git/repo.rs` | 142 | `bare_init` | 9 | — |
| `src/http/repo_edit/mod.rs` | 87 | `open_workspace_context` | 9 | — |
| `src/http/repository/binaries.rs` | 29 | `upload_binary` | 9 | — |
| `src/http/repository/pages/content.rs` | 17 | `raw_handler` | 9 | — |
| `src/http/repository/pages/tabs.rs` | 293 | `configured_tab` | 9 | — |
| `src/http/tree/pages/repositories.rs` | 83 | `repositories_page` | 9 | — |
| `src/http/tree/user_management/views.rs` | 34 | `render_user_management_content` | 9 | — |
| `src/db/namespaces.rs` | 6 | `create_namespace` | 8 | — |
| `src/db/namespaces.rs` | 295 | `delete_namespace` | 8 | — |
| `src/git/bare/handle/commit.rs` | 233 | `create_edit_tree` | 8 | — |
| `src/git/bare/handle/commit.rs` | 322 | `regular_text_file_mode` | 8 | — |
| `src/git/mod.rs` | 17 | `git_handler` | 8 | — |
| `src/git/mod.rs` | 239 | `is_authenticated` | 8 | — |
| `src/git/repo.rs` | 28 | `init` | 8 | — |
| `src/http/auth/handlers.rs` | 199 | `login_ui_handler` | 8 | — |
| `src/http/auth/namespace_invitations.rs` | 77 | `load_active_invitation` | 8 | — |
| `src/http/repo_edit/commit.rs` | 31 | `prepare_workspace_commit` | 8 | — |
| `src/http/repo_edit/validation.rs` | 36 | `validate_existing_text_path` | 8 | — |
| `src/http/repository/pages/paper.rs` | 277 | `paper_headings` | 8 | — |
| `src/http/repository/pages/presentation.rs` | 319 | `present_print_handler` | 8 | — |
| `src/http/repository/pages/tabs.rs` | 64 | `tab_body` | 8 | — |
| `src/http/tree/pages/data.rs` | 17 | `render_data_page` | 8 | — |
| `src/http/tree/pages/namespaces.rs` | 9 | `render_namespaces_page` | 8 | — |
| `src/http/tree/user_management/invitations.rs` | 83 | `create_namespace_invitation` | 8 | — |
| `src/http/tree/user_management/views.rs` | 194 | `render_invitation_list` | 8 | — |
| `src/main.rs` | 77 | `main` | 8 | — |
| `src/md/mod.rs` | 42 | `highlight_rust_code_blocks` | 8 | — |
| `crates/twig-highlight/src/lib.rs` | 378 | `delimited_end` | 7 | — |
| `crates/twig-highlight/src/lib.rs` | 407 | `hash_raw_string_end` | 7 | — |
| `crates/twig-highlight/src/lib.rs` | 437 | `escape_into` | 7 | — |
| `src/auth/permissions.rs` | 29 | `display_name` | 7 | — |
| `src/auth/permissions.rs` | 104 | `namespaces_for_user` | 7 | — |
| `src/db/migration.rs` | 155 | `prepare_migrations` | 7 | — |
| `src/db/namespaces.rs` | 209 | `user_can_create_namespace` | 7 | — |
| `src/db/users.rs` | 6 | `list_users_for_management` | 7 | — |
| `src/git/backend.rs` | 89 | `run_with_config` | 7 | — |
| `src/git/bare/handle/commit.rs` | 278 | `apply_upsert` | 7 | — |
| `src/git/mod.rs` | 391 | `ensure_repo_exists` | 7 | — |
| `src/http/namespace/pages.rs` | 345 | `create_repo_handler` | 7 | — |
| `src/http/repo_edit/handlers.rs` | 101 | `apply_operation` | 7 | — |
| `src/http/repo_edit/validation.rs` | 54 | `read_text_file` | 7 | — |
| `src/http/repo_edit/validation.rs` | 86 | `validate_draft_changes` | 7 | — |
| `src/http/repository/pages/content.rs` | 62 | `content_handler` | 7 | — |
| `src/http/repository/pages/context.rs` | 155 | `open_repo` | 7 | — |
| `src/http/settings/pages/namespaces.rs` | 126 | `delete_namespace` | 7 | — |
| `src/http/tree/user_management/access.rs` | 66 | `user_management_page` | 7 | — |
| `src/http/tree/user_management/views.rs` | 127 | `render_user_access_tree` | 7 | — |
| `src/integration_tests/repo_access_and_routes.rs` | 102 | `test_markdown_source_actions_are_only_in_content` | 7 | — |
| `src/auth/mod.rs` | 158 | `extract_basic_auth` | 6 | — |
| `src/auth/mod.rs` | 223 | `user_id_from_request` | 6 | — |
| `src/db/binaries.rs` | 134 | `get_binary` | 6 | — |
| `src/db/data.rs` | 16 | `list_table_names` | 6 | — |
| `src/db/migration.rs` | 248 | `create_tables` | 6 | — |
| `src/db/namespaces.rs` | 141 | `get_namespace_role` | 6 | — |
| `src/db/test_pins.rs` | 19 | `get_test_pin` | 6 | — |
| `src/db/tokens.rs` | 17 | `get_token_user` | 6 | — |
| `src/email.rs` | 200 | `read_http_request` | 6 | — |
| `src/git/backend.rs` | 176 | `prepare_cgi_env` | 6 | — |
| `src/git/bare/config.rs` | 95 | `collect_script_groups` | 6 | — |
| `src/git/bare/handle/read.rs` | 29 | `file_mode` | 6 | — |
| `src/git/bare/handle/read.rs` | 69 | `read_blob_bytes` | 6 | — |
| `src/git/bare/handle/read.rs` | 206 | `list_paper_pages` | 6 | — |
| `src/git/bare/listing.rs` | 174 | `is_repo_private` | 6 | — |
| `src/git/bare/path.rs` | 5 | `is_safe_component` | 6 | — |
| `src/http/overview/pages.rs` | 136 | `index` | 6 | — |
| `src/http/repo_edit/operations.rs` | 7 | `apply_file_operation` | 6 | — |
| `src/http/repo_edit/pages.rs` | 279 | `file_editor` | 6 | — |
| `src/http/repo_edit/validation.rs` | 70 | `validate_draft` | 6 | — |
| `src/http/repository/binaries.rs` | 173 | `require_namespace_writer` | 6 | — |
| `src/http/repository/binaries.rs` | 222 | `is_safe_filename` | 6 | — |
| `src/http/repository/pages/markdown.rs` | 111 | `rewrite_markdown_link` | 6 | — |
| `src/http/repository/pages/paper.rs` | 93 | `open_paper_page` | 6 | — |
| `src/http/repository/pages/paper.rs` | 243 | `heading_level` | 6 | — |
| `src/http/repository/pages/paper.rs` | 255 | `heading_slug` | 6 | — |
| `src/http/repository/pages/scripts.rs` | 236 | `script_base_url` | 6 | — |
| `src/http/repository/pages/tabs.rs` | 189 | `handler` | 6 | — |
| `src/http/settings/pages/repositories.rs` | 128 | `delete_repo` | 6 | — |
| `src/http/test_page/pages/handlers.rs` | 23 | `authenticate_caller` | 6 | — |
| `src/http/tree/pages/data.rs` | 119 | `data_page` | 6 | — |
| `src/http/tree/pages/repositories.rs` | 13 | `load_namespace_page_data` | 6 | — |
| `src/http/tree/user_management/access.rs` | 17 | `user_management_scope` | 6 | — |
| `src/auth/permissions.rs` | 76 | `for_user` | 5 | — |
| `src/db/invitations.rs` | 16 | `accept_invitation_in_transaction` | 5 | — |
| `src/db/invitations.rs` | 146 | `create_namespace_invitation` | 5 | — |
| `src/db/invitations.rs` | 229 | `accept_namespace_invitation` | 5 | — |
| `src/db/migration.rs` | 200 | `apply_migration_body` | 5 | — |
| `src/db/migration.rs` | 220 | `apply_migration` | 5 | — |
| `src/db/migration.rs` | 267 | `test_create_tables_runs_all_migrations` | 5 | — |
| `src/db/test_pins.rs` | 46 | `validate_test_pin` | 5 | — |
| `src/email.rs` | 124 | `send_invitation_to` | 5 | — |
| `src/git/bare/handle/read.rs` | 110 | `get_license_content` | 5 | — |
| `src/git/bare/listing.rs` | 90 | `last_commit_date` | 5 | — |
| `src/git/bare/mod.rs` | 87 | `build_tree` | 5 | — |
| `src/git/mod.rs` | 320 | `authenticate_git_user` | 5 | — |
| `src/git/repo/hooks.rs` | 96 | `set_hook_executable` | 5 | — |
| `src/git/reserved.rs` | 33 | `validate_repo_name` | 5 | — |
| `src/http/auth/handlers.rs` | 39 | `logout_ui_handler` | 5 | — |
| `src/http/auth/pages/tests.rs` | 78 | `test_auth_pages_label_and_describe_every_input` | 5 | — |
| `src/http/overview/pages.rs` | 271 | `test_search_cluster_preserves_the_get_form_contract` | 5 | — |
| `src/http/repo_edit/commit.rs` | 124 | `render_workspace_commit_result` | 5 | — |
| `src/http/repo_edit/pages.rs` | 209 | `file_tree` | 5 | — |
| `src/http/repository/pages/content.rs` | 231 | `render_content_dir` | 5 | — |
| `src/http/repository/pages/content.rs` | 269 | `render_content_file` | 5 | — |
| `src/http/repository/pages/context.rs` | 124 | `frame` | 5 | — |
| `src/http/repository/pages/markdown.rs` | 89 | `resolve_relative_path` | 5 | — |
| `src/http/repository/pages/tabs.rs` | 91 | `respond_tab` | 5 | — |
| `src/http/repository/pages/tests/paper.rs` | 4 | `test_paper_view_lazy_loads_pages_with_zoom_and_font_controls` | 5 | — |
| `src/http/repository/pages/tests/presentation.rs` | 82 | `test_print_export_is_a_chrome_free_document` | 5 | — |
| `src/http/settings/pages/profile.rs` | 86 | `settings_page` | 5 | — |
| `src/http/settings/pages/shared.rs` | 8 | `require_namespace_permission` | 5 | — |
| `src/http/test_page/pages/handlers.rs` | 114 | `handle_test_page` | 5 | — |
| `src/http/test_page/pages/views.rs` | 37 | `render_pin_panel` | 5 | — |
| `src/http/tree/pages/data.rs` | 170 | `data_rows` | 5 | — |
| `src/http/tree/user_management/invitations.rs` | 38 | `parse_expiry` | 5 | — |
| `src/integration_tests/repo_content.rs` | 68 | `tree` | 5 | — |
| `src/integration_tests/repo_workspace_ui.rs` | 10 | `init_repo_files` | 5 | — |
| `src/main.rs` | 43 | `spawn_database_init` | 5 | — |
| `crates/twig-highlight/src/lib.rs` | 126 | `highlight` | 4 | — |
| `src/auth/session_store.rs` | 60 | `save` | 4 | — |
| `src/auth/session_store.rs` | 77 | `update` | 4 | — |
| `src/config.rs` | 134 | `maybe_reset_database` | 4 | — |
| `src/config.rs` | 185 | `from_env` | 4 | — |
| `src/db/invitations.rs` | 71 | `ensure_username_available` | 4 | — |
| `src/db/invitations.rs` | 209 | `list_namespace_invitations` | 4 | — |
| `src/db/mod.rs` | 66 | `init_tables` | 4 | — |
| `src/db/namespaces.rs` | 120 | `user_has_namespace_access` | 4 | — |
| `src/db/namespaces.rs` | 239 | `user_has_any_namespaces` | 4 | — |
| `src/db/sessions.rs` | 4 | `load_actix_session` | 4 | — |
| `src/db/test_pins.rs` | 4 | `set_test_pin` | 4 | — |
| `src/db/users.rs` | 251 | `user_management_lists_global_users_and_scopes_members_by_namespace` | 4 | — |
| `src/git/bare/handle/commit.rs` | 166 | `head_branch_reference_name` | 4 | — |
| `src/git/bare/handle/commit.rs` | 175 | `parent_tree` | 4 | — |
| `src/git/bare/handle/commit.rs` | 207 | `advance_branch` | 4 | — |
| `src/git/bare/handle/commit.rs` | 263 | `apply_repo_edit` | 4 | — |
| `src/git/bare/handle/read.rs` | 41 | `get_commits` | 4 | — |
| `src/git/bare/handle/read.rs` | 141 | `list_files` | 4 | — |
| `src/git/bare/handle.rs` | 32 | `open` | 4 | — |
| `src/git/bare/listing.rs` | 98 | `list_repos_with_info` | 4 | — |
| `src/git/bare/path.rs` | 18 | `is_safe_repo_path` | 4 | — |
| `src/git/mod.rs` | 114 | `build_response` | 4 | — |
| `src/git/repo/hooks.rs` | 82 | `next_preserved_hook_path` | 4 | — |
| `src/http/auth/namespace_invitations.rs` | 16 | `render_invitation_setup` | 4 | — |
| `src/http/auth/pages.rs` | 20 | `render_field` | 4 | — |
| `src/http/info/pages.rs` | 73 | `render_docs_menu` | 4 | — |
| `src/http/info/pages.rs` | 420 | `test_page_carries_no_outgoing_classes_or_inline_styles` | 4 | — |
| `src/http/namespace/pages.rs` | 46 | `user_has_namespace_permission` | 4 | — |
| `src/http/namespace/pages.rs` | 225 | `handler` | 4 | — |
| `src/http/repo_edit/handlers.rs` | 12 | `workspace_page` | 4 | — |
| `src/http/repo_edit/handlers.rs` | 59 | `tree_fragment` | 4 | — |
| `src/http/repo_edit/handlers.rs` | 162 | `commit_workspace` | 4 | — |
| `src/http/repo_edit/handlers.rs` | 242 | `legacy_file_entry` | 4 | — |
| `src/http/repo_edit/mod.rs` | 152 | `encode_file_path` | 4 | — |
| `src/http/repo_edit/pages.rs` | 137 | `workspace_changes_panel` | 4 | — |
| `src/http/repo_edit/pages.rs` | 404 | `commit_result` | 4 | — |
| `src/http/repo_edit/validation.rs` | 137 | `validate_file_parent_conflicts` | 4 | — |
| `src/http/repository/pages/content.rs` | 141 | `breadcrumb_segments` | 4 | — |
| `src/http/repository/pages/content.rs` | 158 | `render_content_breadcrumbs` | 4 | — |
| `src/http/repository/pages/content.rs` | 195 | `render_content_view` | 4 | — |
| `src/http/repository/pages/markdown.rs` | 20 | `markdown_handler` | 4 | — |
| `src/http/repository/pages/markdown.rs` | 64 | `has_url_scheme` | 4 | — |
| `src/http/repository/pages/markdown.rs` | 196 | `render_paper_markdown` | 4 | — |
| `src/http/repository/pages/presentation.rs` | 203 | `render_slide_content` | 4 | — |
| `src/http/repository/pages/presentation.rs` | 388 | `render_pdf_document` | 4 | — |
| `src/http/repository/pages/tests/markup.rs` | 4 | `test_markdown_controls_are_rendered_only_for_content_files` | 4 | — |
| `src/http/repository/pages/tests/markup.rs` | 56 | `test_repo_markup_carries_no_inline_presentation` | 4 | — |
| `src/http/settings/pages/profile.rs` | 146 | `update_email` | 4 | — |
| `src/http/settings/pages/repositories.rs` | 15 | `render_repo_deletion_panel` | 4 | — |
| `src/http/settings/pages/repositories.rs` | 67 | `render_repo_rename_panel` | 4 | — |
| `src/http/tree/pages/hub.rs` | 5 | `render_tree_hub` | 4 | — |
| `src/http/tree/pages/namespaces.rs` | 186 | `namespaces_page` | 4 | — |
| `src/http/tree/user_management/views.rs` | 254 | `render_invitation_created` | 4 | — |
| `crates/twig-highlight/src/lib.rs` | 78 | `highlight` | 3 | — |
| `crates/twig-highlight/src/lib.rs` | 92 | `highlight_path` | 3 | — |
| `src/auth/mod.rs` | 61 | `parse` | 3 | — |
| `src/auth/mod.rs` | 203 | `create_session` | 3 | — |
| `src/auth/mod.rs` | 262 | `validate_test_pin` | 3 | — |
| `src/auth/session_store.rs` | 100 | `update_ttl` | 3 | — |
| `src/db/invites.rs` | 5 | `create_invite` | 3 | — |
| `src/db/invites.rs` | 46 | `mark_invite_used` | 3 | — |
| `src/db/migration.rs` | 182 | `migration_was_recorded` | 3 | — |
| `src/db/namespaces.rs` | 282 | `rename_namespace` | 3 | — |
| `src/db/sessions.rs` | 22 | `save_actix_session` | 3 | — |
| `src/db/sessions.rs` | 39 | `update_actix_session` | 3 | — |
| `src/db/sessions.rs` | 57 | `update_actix_session_ttl` | 3 | — |
| `src/db/sessions.rs` | 74 | `delete_actix_session` | 3 | — |
| `src/db/test_pins.rs` | 37 | `clear_test_pins` | 3 | — |
| `src/db/tokens.rs` | 4 | `create_token` | 3 | — |
| `src/db/tokens.rs` | 34 | `delete_token` | 3 | — |
| `src/db/tokens.rs` | 43 | `cleanup_expired_tokens` | 3 | — |
| `src/db/users.rs` | 78 | `create_user` | 3 | — |
| `src/db/users.rs` | 136 | `update_user_email` | 3 | — |
| `src/email.rs` | 253 | `missing_public_url_skips_delivery_without_network_request` | 3 | — |
| `src/git/backend.rs` | 160 | `parse_cgi_response` | 3 | — |
| `src/git/bare/config/tests.rs` | 142 | `test_config_parse_errors_are_reported_not_swallowed` | 3 | — |
| `src/git/bare/handle/commit.rs` | 187 | `create_edit_commit` | 3 | — |
| `src/git/bare/handle/commit.rs` | 306 | `apply_delete` | 3 | — |
| `src/git/bare/handle/read.rs` | 9 | `head_commit` | 3 | — |
| `src/git/bare/handle/read.rs` | 87 | `read_file` | 3 | — |
| `src/git/bare/handle/read.rs` | 132 | `load_config_with_raw` | 3 | — |
| `src/git/repo/tests.rs` | 7 | `run_pre_receive` | 3 | — |
| `src/http/assets/tests.rs` | 126 | `test_twig_css_confines_raw_colour_values_to_the_token_block` | 3 | — |
| `src/http/assets/tests.rs` | 161 | `test_twig_css_honours_the_depth_and_motion_bans` | 3 | — |
| `src/http/assets/tests.rs` | 185 | `test_twig_css_uses_only_the_two_authoritative_breakpoints` | 3 | — |
| `src/http/assets/tests.rs` | 271 | `test_assets_handler_known_files` | 3 | — |
| `src/http/assets.rs` | 113 | `find` | 3 | — |
| `src/http/assets.rs` | 135 | `assets` | 3 | — |
| `src/http/auth/handlers.rs` | 73 | `create_invite_ui_handler` | 3 | — |
| `src/http/auth/namespace_invitations.rs` | 118 | `accept_invitation_page` | 3 | — |
| `src/http/auth/pages/tests.rs` | 33 | `attr_value` | 3 | — |
| `src/http/auth/pages/tests.rs` | 48 | `test_auth_markup_uses_only_twig_design_system_classes` | 3 | — |
| `src/http/auth/pages/tests.rs` | 62 | `test_auth_markup_carries_no_inline_styles_or_glyph_decoration` | 3 | — |
| `src/http/namespace/pages/tests.rs` | 66 | `test_namespace_head_action_is_gated_on_namespace_access` | 3 | — |
| `src/http/namespace/pages/tests.rs` | 180 | `test_namespace_uses_only_twig_design_system_classes` | 3 | — |
| `src/http/namespace/pages/tests.rs` | 237 | `test_create_repo_form_is_a_panel_that_keeps_its_field_contract` | 3 | — |
| `src/http/namespace/pages.rs` | 272 | `render_create_repo_form` | 3 | — |
| `src/http/overview/pages.rs` | 467 | `test_index_uses_only_design_system_classes_and_no_inline_style` | 3 | — |
| `src/http/repo_edit/operations.rs` | 33 | `create_workspace_file` | 3 | — |
| `src/http/repo_edit/operations.rs` | 146 | `discard_workspace_file` | 3 | — |
| `src/http/repo_edit/pages.rs` | 392 | `operation_result` | 3 | — |
| `src/http/repository/pages/content.rs` | 214 | `render_content_row` | 3 | — |
| `src/http/repository/pages/markdown.rs` | 137 | `fix_link_tag` | 3 | — |
| `src/http/repository/pages/markdown.rs` | 174 | `markdown_to_html` | 3 | — |
| `src/http/repository/pages/markdown.rs` | 315 | `render_markdown_view` | 3 | — |
| `src/http/repository/pages/paper.rs` | 126 | `paper_handler` | 3 | — |
| `src/http/repository/pages/paper.rs` | 197 | `paper_path_slug` | 3 | — |
| `src/http/repository/pages/paper.rs` | 217 | `paper_page_anchors` | 3 | — |
| `src/http/repository/pages/paper.rs` | 323 | `render_paper_toolbar` | 3 | — |
| `src/http/repository/pages/paper.rs` | 369 | `render_paper_view` | 3 | — |
| `src/http/repository/pages/presentation.rs` | 138 | `load_present_slides` | 3 | — |
| `src/http/repository/pages/scripts.rs` | 12 | `scripts_body` | 3 | — |
| `src/http/repository/pages/tabs.rs` | 177 | `resolved_default_tab` | 3 | — |
| `src/http/repository/pages/tabs.rs` | 339 | `default_tab` | 3 | — |
| `src/http/repository/pages/tabs.rs` | 350 | `get_default_markdown_file` | 3 | — |
| `src/http/repository/pages/tabs.rs` | 369 | `render_commits_view` | 3 | — |
| `src/http/repository/pages/tests/markup.rs` | 40 | `test_repo_markup_uses_only_twig_design_system_classes` | 3 | — |
| `src/http/repository/pages/tests/presentation.rs` | 60 | `test_print_export_opens_the_dialog_once_loaded` | 3 | — |
| `src/http/repository/pages/tests/presentation.rs` | 164 | `test_presentation_controls_are_labelled_and_bounded` | 3 | — |
| `src/http/settings/pages/profile.rs` | 32 | `render_profile_panel` | 3 | — |
| `src/http/settings/pages/tests.rs` | 153 | `test_settings_delete_repo_rows_preserve_the_htmx_contract` | 3 | — |
| `src/http/settings/pages/tests.rs` | 183 | `test_settings_rename_repo_rows_preserve_the_htmx_contract` | 3 | — |
| `src/http/settings/pages/tests.rs` | 336 | `test_settings_uses_only_twig_design_system_classes` | 3 | — |
| `src/http/test_page/pages/handlers.rs` | 12 | `get_base_url` | 3 | — |
| `src/http/test_page/pages/handlers.rs` | 81 | `verify_admin_only` | 3 | — |
| `src/http/test_page/pages/handlers.rs` | 166 | `test_pin_create` | 3 | — |
| `src/http/test_page/pages/handlers.rs` | 187 | `test_pin_remove` | 3 | — |
| `src/http/test_page/pages/handlers.rs` | 207 | `test_pin_qr` | 3 | — |
| `src/http/test_page/pages/views.rs` | 110 | `render_pin_entry_page` | 3 | — |
| `src/http/tree/api.rs` | 31 | `tree_endpoint` | 3 | — |
| `src/http/tree/pages/data.rs` | 89 | `render_more_rows` | 3 | — |
| `src/http/tree/pages/namespaces.rs` | 84 | `render_namespace_rename_panel` | 3 | — |
| `src/http/tree/pages/namespaces.rs` | 141 | `render_namespace_deletion_panel` | 3 | — |
| `src/http/tree/user_management/invitations.rs` | 52 | `display_invite_url` | 3 | — |
| `src/http/view.rs` | 6 | `render_layout` | 3 | — |
| `src/http/view.rs` | 373 | `test_layout_uses_only_twig_design_system_classes` | 3 | — |
| `src/integration_tests/repo_binaries.rs` | 235 | `test_binary_uploads_retain_semver_releases_and_latest_resolves_by_asset` | 3 | — |
| `src/integration_tests/repo_content.rs` | 7 | `init_repo_with_paper` | 3 | — |
| `src/integration_tests/repo_traversal.rs` | 167 | `test_content_rejects_nul_byte_paths` | 3 | — |
| `src/integration_tests/repo_workspace_ui.rs` | 47 | `repo_edit_workspace_opens_from_overview_and_navigates_tree_through_htmx` | 3 | — |
| `src/integration_tests/repo_workspace_ui.rs` | 317 | `repo_edit_refuses_ignored_binary_invalid_and_non_regular_files` | 3 | — |
| `src/md/mod.rs` | 86 | `is_rust_block_start` | 3 | — |
| `crates/twig-highlight/build.rs` | 3 | `main` | 2 | — |
| `crates/twig-highlight/src/lib.rs` | 112 | `grammars` | 2 | — |
| `crates/twig-highlight/src/lib.rs` | 185 | `try_from` | 2 | — |
| `crates/twig-highlight/src/tests.rs` | 108 | `custom_grammars_use_the_shared_safe_writer` | 2 | — |
| `src/auth/mod.rs` | 47 | `as_str` | 2 | — |
| `src/auth/mod.rs` | 54 | `display_name` | 2 | — |
| `src/auth/mod.rs` | 99 | `hash_password` | 2 | — |
| `src/auth/mod.rs` | 108 | `verify_password` | 2 | — |
| `src/auth/mod.rs` | 125 | `create_user` | 2 | — |
| `src/auth/mod.rs` | 177 | `base64_decode` | 2 | — |
| `src/auth/mod.rs` | 244 | `get_test_pin` | 2 | — |
| `src/auth/permissions.rs` | 54 | `from_role` | 2 | — |
| `src/auth/permissions.rs` | 92 | `has` | 2 | — |
| `src/auth/permissions.rs` | 134 | `owner_has_every_namespace_permission` | 2 | — |
| `src/auth/permissions.rs` | 143 | `contributor_has_only_namespace_view_and_push` | 2 | — |
| `src/auth/session_store.rs` | 42 | `load` | 2 | — |
| `src/config.rs` | 168 | `effective_api_key` | 2 | — |
| `src/db/binaries.rs` | 35 | `version_order_desc` | 2 | — |
| `src/db/binaries.rs` | 371 | `binary_database_orders_semver_and_retains_the_top_three_versions` | 2 | — |
| `src/db/namespaces.rs` | 170 | `user_owns_namespace` | 2 | — |
| `src/db/users.rs` | 149 | `merge_user_entry` | 2 | — |
| `src/email.rs` | 27 | `with_base_url` | 2 | — |
| `src/email.rs` | 176 | `spawn_fake_resend` | 2 | — |
| `src/email.rs` | 231 | `missing_api_key_skips_sender_construction_and_network_request` | 2 | — |
| `src/email.rs` | 334 | `invitation_url_requires_a_safe_origin_and_token` | 2 | — |
| `src/git/backend.rs` | 37 | `fmt` | 2 | — |
| `src/git/backend.rs` | 263 | `test_git_request_kind_advertise_refs_upload` | 2 | — |
| `src/git/backend.rs` | 272 | `test_git_request_kind_advertise_refs_receive` | 2 | — |
| `src/git/backend.rs` | 281 | `test_git_request_kind_fetch_clone` | 2 | — |
| `src/git/backend.rs` | 295 | `test_git_request_kind_push` | 2 | — |
| `src/git/backend.rs` | 309 | `test_git_request_kind_dumb_get` | 2 | — |
| `src/git/bare/config.rs` | 82 | `label` | 2 | — |
| `src/git/bare/config.rs` | 207 | `from_source` | 2 | — |
| `src/git/bare/handle/commit.rs` | 349 | `is_utf8_text` | 2 | — |
| `src/git/bare/handle/read.rs` | 24 | `head_oid` | 2 | — |
| `src/git/bare/mod.rs` | 26 | `has_any_repository` | 2 | — |
| `src/git/mod.rs` | 370 | `ensure_namespace_exists` | 2 | — |
| `src/git/reserved.rs` | 66 | `test_namespace_level_routes_are_reserved` | 2 | — |
| `src/git/reserved.rs` | 76 | `test_ordinary_names_are_allowed` | 2 | — |
| `src/git/reserved.rs` | 93 | `test_path_traversal_is_refused` | 2 | — |
| `src/http/assets/tests.rs` | 6 | `body_of` | 2 | — |
| `src/http/assets/tests.rs` | 13 | `css_without_comments` | 2 | — |
| `src/http/assets/tests.rs` | 26 | `css_outside_root_blocks` | 2 | — |
| `src/http/assets/tests.rs` | 46 | `test_asset_constants_are_non_empty` | 2 | — |
| `src/http/assets/tests.rs` | 90 | `test_twig_schema_lists_every_config_key` | 2 | — |
| `src/http/assets/tests.rs` | 148 | `test_rich_editor_code_uses_theme_aware_contrast` | 2 | — |
| `src/http/assets/tests.rs` | 206 | `test_twig_css_drops_the_outgoing_system` | 2 | — |
| `src/http/assets.rs` | 127 | `url` | 2 | — |
| `src/http/auth/namespace_invitations.rs` | 237 | `invitation_setup_prefills_email_and_shows_namespace_and_role` | 2 | — |
| `src/http/auth/pages/tests.rs` | 40 | `field_tag` | 2 | — |
| `src/http/auth/pages/tests.rs` | 109 | `test_auth_pages_announce_their_result_targets` | 2 | — |
| `src/http/auth/pages/tests.rs` | 119 | `test_invite_page_preserves_its_form_contract` | 2 | — |
| `src/http/auth/pages/tests.rs` | 142 | `test_signup_page_preserves_its_form_contract` | 2 | — |
| `src/http/auth/pages/tests.rs` | 180 | `test_login_page_preserves_its_form_contract` | 2 | — |
| `src/http/auth/pages/tests.rs` | 210 | `test_namespace_page_preserves_its_form_contract` | 2 | — |
| `src/http/auth/pages/tests.rs` | 233 | `test_auth_submits_are_block_primary_buttons` | 2 | — |
| `src/http/auth/pages/tests.rs` | 245 | `test_auth_copy_says_an_invite` | 2 | — |
| `src/http/auth/pages/tests.rs` | 255 | `test_success_notices_are_announced_status_notices` | 2 | — |
| `src/http/auth/pages/tests.rs` | 291 | `test_render_signup_success_offers_login_and_namespace` | 2 | — |
| `src/http/auth/pages/tests.rs` | 318 | `test_success_notices_escape_untrusted_values` | 2 | — |
| `src/http/auth/pages.rs` | 117 | `invite_page` | 2 | — |
| `src/http/auth/pages.rs` | 211 | `signup_page` | 2 | — |
| `src/http/auth/pages.rs` | 279 | `login_page` | 2 | — |
| `src/http/auth/pages.rs` | 344 | `namespace_page` | 2 | — |
| `src/http/auth/session.rs` | 5 | `get_username_from_request` | 2 | — |
| `src/http/info/pages.rs` | 36 | `from_query` | 2 | — |
| `src/http/info/pages.rs` | 43 | `as_str` | 2 | — |
| `src/http/info/pages.rs` | 51 | `render_tabs` | 2 | — |
| `src/http/info/pages.rs` | 203 | `index` | 2 | — |
| `src/http/info/pages.rs` | 228 | `anchor_containing` | 2 | — |
| `src/http/info/pages.rs` | 328 | `test_render_docs_tab_pairs_the_rail_with_measured_prose` | 2 | — |
| `src/http/info/pages.rs` | 352 | `test_every_doc_page_offers_a_copy_button_carrying_its_markdown` | 2 | — |
| `src/http/info/pages.rs` | 401 | `test_copy_script_uses_the_clipboard_with_a_selection_fallback` | 2 | — |
| `src/http/namespace/pages/tests.rs` | 90 | `test_namespace_search_cluster_uses_the_field_primitives` | 2 | — |
| `src/http/namespace/pages/tests.rs` | 147 | `test_namespace_empty_states_preserve_their_messages` | 2 | — |
| `src/http/namespace/pages/tests.rs` | 203 | `test_create_repo_form_preserves_the_htmx_contract` | 2 | — |
| `src/http/namespace/pages/tests.rs` | 319 | `test_format_date_singular_units` | 2 | — |
| `src/http/namespace/pages.rs` | 329 | `create_repo_form_handler` | 2 | — |
| `src/http/overview/pages.rs` | 114 | `search_repo_hits` | 2 | — |
| `src/http/overview/pages.rs` | 342 | `test_rows_keep_owner_metadata_visible_and_labelled_at_every_width` | 2 | — |
| `src/http/overview/pages.rs` | 427 | `test_empty_states_name_their_condition_and_keep_their_copy` | 2 | — |
| `src/http/repo_edit/handlers.rs` | 212 | `file_fragment` | 2 | — |
| `src/http/repo_edit/pages.rs` | 7 | `workspace_shell` | 2 | — |
| `src/http/repo_edit/pages.rs` | 68 | `workspace_files_panel` | 2 | — |
| `src/http/repo_edit/pages.rs` | 115 | `workspace_document_panel` | 2 | — |
| `src/http/repo_edit/tests.rs` | 4 | `workspace_shell_loads_the_file_tree_through_htmx` | 2 | — |
| `src/http/repository/pages/content.rs` | 124 | `content_urls` | 2 | — |
| `src/http/repository/pages/content.rs` | 133 | `parent_path` | 2 | — |
| `src/http/repository/pages/content.rs` | 309 | `raw_content_type` | 2 | — |
| `src/http/repository/pages/context.rs` | 38 | `text_size_step` | 2 | — |
| `src/http/repository/pages/context.rs` | 55 | `render_for_request` | 2 | — |
| `src/http/repository/pages/context.rs` | 223 | `can_view_namespace` | 2 | — |
| `src/http/repository/pages/context.rs` | 271 | `render_not_found_for_request` | 2 | — |
| `src/http/repository/pages/markdown.rs` | 79 | `split_link_suffix` | 2 | — |
| `src/http/repository/pages/markdown.rs` | 237 | `render_slide_markdown` | 2 | — |
| `src/http/repository/pages/markdown.rs` | 354 | `render_markdown_content_only` | 2 | — |
| `src/http/repository/pages/paper.rs` | 83 | `page_in_dir` | 2 | — |
| `src/http/repository/pages/paper.rs` | 185 | `paper_relative_path` | 2 | — |
| `src/http/repository/pages/paper.rs` | 407 | `render_paper_content_only` | 2 | — |
| `src/http/repository/pages/presentation.rs` | 176 | `render_slide_nav_button` | 2 | — |
| `src/http/repository/pages/presentation.rs` | 286 | `render_present_view` | 2 | — |
| `src/http/repository/pages/presentation.rs` | 438 | `render_pdf_pagehead` | 2 | — |
| `src/http/repository/pages/presentation.rs` | 476 | `render_pdf_license_page` | 2 | — |
| `src/http/repository/pages/scripts.rs` | 117 | `render_script_group_tabs` | 2 | — |
| `src/http/repository/pages/scripts.rs` | 148 | `render_script_row` | 2 | — |
| `src/http/repository/pages/tab_handlers.rs` | 187 | `tab_handler` | 2 | — |
| `src/http/repository/pages/tabs.rs` | 47 | `commits_body` | 2 | — |
| `src/http/repository/pages/tabs.rs` | 135 | `render_tab_shell` | 2 | — |
| `src/http/repository/pages/tabs.rs` | 152 | `render_tab_response` | 2 | — |
| `src/http/repository/pages/tabs.rs` | 274 | `available_tabs` | 2 | — |
| `src/http/repository/pages/tabs.rs` | 313 | `render_tabs` | 2 | — |
| `src/http/repository/pages/tabs.rs` | 385 | `render_config_view` | 2 | — |
| `src/http/repository/pages/tabs.rs` | 434 | `render_license_view` | 2 | — |
| `src/http/repository/pages/tests/markdown.rs` | 159 | `test_markdown_controls_use_the_raw_route_and_keep_the_original_source` | 2 | — |
| `src/http/repository/pages/tests/markdown.rs` | 183 | `test_markdown_copy_script_uses_clipboard_and_manual_fallback` | 2 | — |
| `src/http/repository/pages/tests/presentation.rs` | 20 | `test_print_export_renders_one_page_per_slide_with_the_license_last` | 2 | — |
| `src/http/repository/pages/tests/presentation.rs` | 115 | `test_presentation_targets_its_container_and_scopes_the_keyboard` | 2 | — |
| `src/http/repository/pages/tests/tabs.rs` | 56 | `test_tabs_preserve_every_htmx_navigation_attribute` | 2 | — |
| `src/http/repository/pages/tests/tabs.rs` | 89 | `test_tabs_mark_exactly_one_current_tab` | 2 | — |
| `src/http/repository/pages/tests/tabs.rs` | 321 | `test_tab_content_shows_a_config_error_on_every_tab` | 2 | — |
| `src/http/routes.rs` | 122 | `test_is_git_guard_matches_git_user_agent` | 2 | — |
| `src/http/routes.rs` | 142 | `test_is_git_guard_rejects_non_git_user_agent` | 2 | — |
| `src/http/routes.rs` | 162 | `test_is_git_guard_rejects_missing_user_agent` | 2 | — |
| `src/http/settings/pages/shared.rs` | 47 | `render_settings_auth_error` | 2 | — |
| `src/http/settings/pages/tests.rs` | 48 | `test_settings_leads_with_crumbs_and_page_head` | 2 | — |
| `src/http/settings/pages/tests.rs` | 249 | `test_settings_email_form_preserves_the_htmx_contract` | 2 | — |
| `src/http/test_page/pages/handlers.rs` | 234 | `test_runner` | 2 | — |
| `src/http/test_page/pages/handlers.rs` | 248 | `test_stopped` | 2 | — |
| `src/http/test_page/pages/handlers.rs` | 262 | `test_ping` | 2 | — |
| `src/http/test_page/pages/handlers.rs` | 276 | `test_feature_check` | 2 | — |
| `src/http/test_page/pages/tests.rs` | 27 | `test_endpoint_catalog_uses_current_api_and_info_routes` | 2 | — |
| `src/http/test_page/pages/views.rs` | 210 | `render_runner_content` | 2 | — |
| `src/http/test_page/pages.rs` | 25 | `display_name` | 2 | — |
| `src/http/test_page/pages.rs` | 33 | `username` | 2 | — |
| `src/http/test_page/pages.rs` | 51 | `generate_qr_svg` | 2 | — |
| `src/http/tree/api.rs` | 17 | `msgpack_responder` | 2 | — |
| `src/http/tree/pages/data.rs` | 67 | `render_data_row` | 2 | — |
| `src/http/tree/pages/data.rs` | 100 | `require_admin` | 2 | — |
| `src/http/tree/pages/hub.rs` | 30 | `tree_page` | 2 | — |
| `src/http/tree/user_management/invitations.rs` | 28 | `invitations_for_scope` | 2 | — |
| `src/http/tree/user_management/tests.rs` | 51 | `htmx_user_management_response_contains_content_without_tabs` | 2 | — |
| `src/http/tree/user_management/tests.rs` | 62 | `invite_tab_renders_htmx_form_with_contributor_default_and_expiry` | 2 | — |
| `src/http/tree/user_management/tests.rs` | 87 | `user_memberships_render_as_namespace_role_permission_trees` | 2 | — |
| `src/http/tree/user_management/tests.rs` | 115 | `site_admin_tree_shows_all_namespace_permissions` | 2 | — |
| `src/http/tree/user_management/tests.rs` | 141 | `expiry_values_map_to_expected_expiration` | 2 | — |
| `src/http/tree/user_management/tests.rs` | 166 | `delivery_notices_preserve_link_and_distinguish_sent_skipped_and_failed` | 2 | — |
| `src/http/tree/user_management/views.rs` | 117 | `render_permission_list` | 2 | — |
| `src/http/view/test_util.rs` | 19 | `index_of` | 2 | — |
| `src/http/view.rs` | 92 | `render_workspace_layout` | 2 | — |
| `src/http/view.rs` | 193 | `test_workspace_layout_omits_site_chrome_and_page_width_wrapper` | 2 | — |
| `src/http/view.rs` | 232 | `test_layout_declares_document_language_and_landmarks` | 2 | — |
| `src/http/view.rs` | 288 | `test_layout_loads_space_grotesk_and_tachyons_before_twig_css` | 2 | — |
| `src/http/view.rs` | 308 | `test_layout_offers_theme_switch_before_rendering_styles` | 2 | — |
| `src/http/view.rs` | 340 | `test_layout_links_versioned_assets` | 2 | — |
| `src/http/view.rs` | 397 | `test_layout_authenticated_nav_exposes_user_actions` | 2 | — |
| `src/http/view.rs` | 418 | `test_layout_anonymous_nav_offers_login_and_signup` | 2 | — |
| `src/http/view.rs` | 466 | `test_notices_escape_message_markup` | 2 | — |
| `src/info/mod.rs` | 103 | `list_doc_pages` | 2 | — |
| `src/info/mod.rs` | 150 | `test_repository_config_page_documents_every_section` | 2 | — |
| `src/integration_tests/admin_tree_settings.rs` | 6 | `test_tree_hub_and_admin_data_view_are_gated_and_paginated` | 2 | — |
| `src/integration_tests/repo_access_and_routes.rs` | 37 | `test_raw_route_serves_repo_files_and_refuses_ignored_paths` | 2 | — |
| `src/integration_tests/repo_traversal.rs` | 119 | `test_content_rejects_dotdot_segments` | 2 | — |
| `src/integration_tests/repo_traversal.rs` | 146 | `test_content_rejects_namespace_and_repo_traversal` | 2 | — |
| `src/integration_tests/repo_traversal.rs` | 185 | `test_markdown_view_rejects_traversal` | 2 | — |
| `src/integration_tests/repo_workspace_commit.rs` | 139 | `repo_edit_commit_rejects_invalid_message_noop_and_oversized_file` | 2 | — |
| `src/integration_tests/test_page.rs` | 3 | `create_test_service_with_test_user` | 2 | — |
| `src/md/mod.rs` | 5 | `replace_mustache` | 2 | — |
| `src/md/mod.rs` | 16 | `is_markdown` | 2 | — |
| `src/md/mod.rs` | 136 | `is_mermaid_block_start` | 2 | — |
| `crates/twig-highlight/src/lib.rs` | 55 | `text` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 60 | `token` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 99 | `highlight_with` | 1 | — |
| `crates/twig-highlight/src/tests.rs` | 4 | `unknown_languages_are_not_highlighted` | 1 | — |
| `crates/twig-highlight/src/tests.rs` | 9 | `extension_lookup_is_case_insensitive` | 1 | — |
| `crates/twig-highlight/src/tests.rs` | 15 | `source_is_escaped_even_when_it_is_not_a_token` | 1 | — |
| `crates/twig-highlight/src/tests.rs` | 20 | `rust_grammar_is_loaded_from_the_toml_registry` | 1 | — |
| `crates/twig-highlight/src/tests.rs` | 29 | `javascript_grammar_is_loaded_from_toml_and_maps_its_extensions` | 1 | — |
| `crates/twig-highlight/src/tests.rs` | 40 | `shell_grammar_highlights_script_tokens_and_maps_shell_extensions` | 1 | — |
| `crates/twig-highlight/src/tests.rs` | 55 | `css_grammar_highlights_styles_and_maps_css_files` | 1 | — |
| `crates/twig-highlight/src/tests.rs` | 72 | `grammar_rules_highlight_tokens_and_escape_source` | 1 | — |
| `crates/twig-highlight/src/tests.rs` | 88 | `handles_nested_comments_raw_strings_raw_identifiers_and_lifetimes` | 1 | — |
| `crates/twig-highlight/src/tests.rs` | 101 | `unclosed_constructs_are_still_safely_rendered` | 1 | — |
| `crates/twig-highlight/src/tests.rs` | 112 | `highlight` | 1 | — |
| `src/auth/mod.rs` | 117 | `generate_token` | 1 | — |
| `src/auth/mod.rs` | 142 | `generate_invite` | 1 | — |
| `src/auth/mod.rs` | 146 | `create_namespace` | 1 | — |
| `src/auth/mod.rs` | 191 | `new` | 1 | — |
| `src/auth/mod.rs` | 195 | `db` | 1 | — |
| `src/auth/mod.rs` | 199 | `validate_api_key` | 1 | — |
| `src/auth/mod.rs` | 213 | `validate_token` | 1 | — |
| `src/auth/mod.rs` | 218 | `invalidate_token` | 1 | — |
| `src/auth/mod.rs` | 254 | `set_test_pin` | 1 | — |
| `src/auth/mod.rs` | 258 | `clear_test_pin` | 1 | — |
| `src/auth/mod.rs` | 310 | `test_hash_and_verify_password` | 1 | — |
| `src/auth/mod.rs` | 317 | `test_verify_password_invalid_hash` | 1 | — |
| `src/auth/mod.rs` | 322 | `test_generate_token_is_hex` | 1 | — |
| `src/auth/mod.rs` | 329 | `test_generate_invite_is_uuid` | 1 | — |
| `src/auth/mod.rs` | 335 | `test_generate_tokens_are_unique` | 1 | — |
| `src/auth/mod.rs` | 342 | `test_create_user_fields` | 1 | — |
| `src/auth/mod.rs` | 356 | `test_create_user_password_is_hashed` | 1 | — |
| `src/auth/mod.rs` | 368 | `test_create_namespace_fields` | 1 | — |
| `src/auth/mod.rs` | 376 | `test_base64_decode_valid` | 1 | — |
| `src/auth/mod.rs` | 382 | `test_base64_decode_invalid` | 1 | — |
| `src/auth/mod.rs` | 388 | `test_base64_decode_empty` | 1 | — |
| `src/auth/mod.rs` | 394 | `test_twig_context_validate_api_key` | 1 | — |
| `src/auth/mod.rs` | 402 | `test_twig_context_test_pin` | 1 | — |
| `src/auth/mod.rs` | 424 | `test_seed_dev_admin_creates_user_namespace_and_session` | 1 | — |
| `src/auth/mod.rs` | 460 | `test_seed_dev_admin_is_idempotent` | 1 | — |
| `src/auth/permissions.rs` | 46 | `new` | 1 | — |
| `src/auth/permissions.rs` | 50 | `all` | 1 | — |
| `src/auth/permissions.rs` | 65 | `contains` | 1 | — |
| `src/auth/permissions.rs` | 69 | `iter` | 1 | — |
| `src/auth/permissions.rs` | 163 | `configured_admin_gets_all_permissions_without_membership` | 1 | — |
| `src/auth/session_store.rs` | 15 | `new` | 1 | — |
| `src/auth/session_store.rs` | 19 | `ttl_seconds` | 1 | — |
| `src/auth/session_store.rs` | 23 | `new_session_key` | 1 | — |
| `src/auth/session_store.rs` | 30 | `middleware` | 1 | — |
| `src/auth/session_store.rs` | 117 | `delete` | 1 | — |
| `src/auth/session_store.rs` | 130 | `sql_session_store_saves_loads_updates_and_deletes_sessions` | 1 | — |
| `src/config.rs` | 24 | `new` | 1 | — |
| `src/config.rs` | 49 | `with_cache_control` | 1 | — |
| `src/config.rs` | 54 | `with_test_user` | 1 | — |
| `src/config.rs` | 59 | `with_admin_user` | 1 | — |
| `src/config.rs` | 64 | `with_resend_settings` | 1 | — |
| `src/config.rs` | 76 | `resend_api_key` | 1 | — |
| `src/config.rs` | 80 | `resend_from` | 1 | — |
| `src/config.rs` | 84 | `public_base_url` | 1 | — |
| `src/config.rs` | 88 | `admin_user` | 1 | — |
| `src/config.rs` | 92 | `is_configured_admin` | 1 | — |
| `src/config.rs` | 96 | `address` | 1 | — |
| `src/config.rs` | 100 | `log_level` | 1 | — |
| `src/config.rs` | 104 | `project_root` | 1 | — |
| `src/config.rs` | 108 | `db_path` | 1 | — |
| `src/config.rs` | 113 | `reset_db` | 1 | — |
| `src/config.rs` | 118 | `test_user` | 1 | — |
| `src/config.rs` | 123 | `is_test_user_enabled` | 1 | — |
| `src/config.rs` | 128 | `is_test_user` | 1 | — |
| `src/config.rs` | 153 | `cache_control` | 1 | — |
| `src/config.rs` | 157 | `session_key` | 1 | — |
| `src/config.rs` | 161 | `with_session_key` | 1 | — |
| `src/config.rs` | 180 | `fmt` | 1 | — |
| `src/config.rs` | 250 | `ascii` | 1 | — |
| `src/config.rs` | 278 | `test_server_new` | 1 | — |
| `src/config.rs` | 298 | `test_server_cache_control_round_trip` | 1 | — |
| `src/config.rs` | 316 | `test_server_clone` | 1 | — |
| `src/config.rs` | 331 | `test_effective_api_key_with_set_key` | 1 | — |
| `src/config.rs` | 344 | `test_effective_api_key_generates_when_empty` | 1 | — |
| `src/config.rs` | 359 | `test_maybe_reset_database_no_flag` | 1 | — |
| `src/config.rs` | 372 | `test_address_returns_clone` | 1 | — |
| `src/config.rs` | 386 | `test_server_test_user` | 1 | — |
| `src/config.rs` | 407 | `test_server_admin_user_is_independent_from_test_user` | 1 | — |
| `src/config.rs` | 425 | `test_blank_admin_user_is_not_configured` | 1 | — |
| `src/config.rs` | 440 | `test_resend_settings_are_optional_and_keep_sender_configuration` | 1 | — |
| `src/db/binaries.rs` | 300 | `test_db_path` | 1 | — |
| `src/db/binaries.rs` | 304 | `version` | 1 | — |
| `src/db/binaries.rs` | 309 | `binary_database_round_trips_arbitrary_blob_bytes` | 1 | — |
| `src/db/binaries.rs` | 333 | `binary_database_replaces_and_lists_asset_metadata` | 1 | — |
| `src/db/binaries.rs` | 425 | `latest_binary_resolves_highest_release_containing_filename` | 1 | — |
| `src/db/data.rs` | 91 | `table_reader_lists_tables_and_pages_values` | 1 | — |
| `src/db/data.rs` | 134 | `table_reader_rejects_an_oversized_page_loudly` | 1 | — |
| `src/db/data.rs` | 148 | `table_reader_returns_an_empty_page_past_the_end_of_the_table` | 1 | — |
| `src/db/invitations/tests.rs` | 4 | `setup_db` | 1 | — |
| `src/db/invitations/tests.rs` | 27 | `create_get_and_scope_list_namespace_invitations` | 1 | — |
| `src/db/invitations/tests.rs` | 79 | `pending_invitation` | 1 | — |
| `src/db/invitations/tests.rs` | 96 | `invited_user` | 1 | — |
| `src/db/invitations/tests.rs` | 102 | `acceptance_creates_account_membership_and_consumes_invite_atomically` | 1 | — |
| `src/db/invitations/tests.rs` | 160 | `failed_acceptance_rolls_back_account_membership_and_invite_state` | 1 | — |
| `src/db/invitations/tests.rs` | 206 | `expired_or_mismatched_invitation_cannot_be_accepted` | 1 | — |
| `src/db/invitations/tests.rs` | 248 | `concurrent_acceptance_only_creates_one_account_and_membership` | 1 | — |
| `src/db/invitations.rs` | 90 | `insert_invited_user` | 1 | — |
| `src/db/invitations.rs` | 107 | `add_invited_user_to_namespace` | 1 | — |
| `src/db/invitations.rs` | 128 | `mark_invitation_accepted` | 1 | — |
| `src/db/invites.rs` | 65 | `setup_db` | 1 | — |
| `src/db/invites.rs` | 72 | `test_invite` | 1 | — |
| `src/db/invites.rs` | 83 | `test_create_and_get_invite` | 1 | — |
| `src/db/invites.rs` | 101 | `test_mark_invite_used` | 1 | — |
| `src/db/invites.rs` | 121 | `test_get_invite_not_found` | 1 | — |
| `src/db/migration.rs` | 346 | `test_legacy_member_roles_migrate_to_contributors` | 1 | — |
| `src/db/mod.rs` | 26 | `new` | 1 | — |
| `src/db/mod.rs` | 84 | `test_database_new_stores_path` | 1 | — |
| `src/db/mod.rs` | 90 | `test_database_conn_creates_file` | 1 | — |
| `src/db/mod.rs` | 106 | `test_database_init_tables_succeeds` | 1 | — |
| `src/db/namespaces/tests.rs` | 4 | `setup_db_with_user` | 1 | — |
| `src/db/namespaces/tests.rs` | 22 | `test_namespace` | 1 | — |
| `src/db/namespaces/tests.rs` | 32 | `test_create_and_get_namespace_by_name` | 1 | — |
| `src/db/namespaces/tests.rs` | 52 | `test_get_all_namespaces_with_owners` | 1 | — |
| `src/db/namespaces/tests.rs` | 73 | `test_user_has_namespace_access_owner` | 1 | — |
| `src/db/namespaces/tests.rs` | 92 | `test_user_has_namespace_access_denied` | 1 | — |
| `src/db/namespaces/tests.rs` | 112 | `namespace_roles_control_owner_scopes_and_creation_permissions` | 1 | — |
| `src/db/namespaces/tests.rs` | 203 | `test_search_namespaces_with_owners` | 1 | — |
| `src/db/namespaces/tests.rs` | 228 | `test_rename_namespace` | 1 | — |
| `src/db/namespaces/tests.rs` | 257 | `test_delete_namespace` | 1 | — |
| `src/db/namespaces.rs` | 75 | `get_all_namespaces_with_owners` | 1 | — |
| `src/db/namespaces.rs` | 79 | `search_namespaces_with_owners` | 1 | — |
| `src/db/test_pins.rs` | 68 | `setup_db` | 1 | — |
| `src/db/test_pins.rs` | 76 | `test_set_and_get_test_pin` | 1 | — |
| `src/db/tokens.rs` | 61 | `setup_db_with_user` | 1 | — |
| `src/db/tokens.rs` | 80 | `test_create_and_get_token_user` | 1 | — |
| `src/db/tokens.rs` | 96 | `test_delete_token` | 1 | — |
| `src/db/tokens.rs` | 113 | `test_cleanup_expired_tokens_removes_old_tokens` | 1 | — |
| `src/db/users.rs` | 165 | `setup_db` | 1 | — |
| `src/db/users.rs` | 172 | `test_user` | 1 | — |
| `src/db/users.rs` | 183 | `test_create_and_get_user_by_username` | 1 | — |
| `src/db/users.rs` | 201 | `test_get_user_by_id` | 1 | — |
| `src/db/users.rs` | 216 | `test_get_user_by_username_not_found` | 1 | — |
| `src/db/users.rs` | 230 | `test_update_user_email` | 1 | — |
| `src/email.rs` | 22 | `new` | 1 | — |
| `src/email.rs` | 38 | `send_namespace_invitation` | 1 | — |
| `src/email.rs` | 91 | `send_invitation_if_configured` | 1 | — |
| `src/email.rs` | 276 | `resend_sends_invitation_payload_to_the_configured_api` | 1 | — |
| `src/email.rs` | 309 | `resend_failure_is_reported_for_retry_and_recovery` | 1 | — |
| `src/git/backend.rs` | 11 | `new` | 1 | — |
| `src/git/backend.rs` | 17 | `project_root` | 1 | — |
| `src/git/backend.rs` | 74 | `new` | 1 | — |
| `src/git/backend.rs` | 254 | `sh` | 1 | — |
| `src/git/backend.rs` | 318 | `test_git_service_display` | 1 | — |
| `src/git/backend.rs` | 324 | `test_git_request_new` | 1 | — |
| `src/git/backend.rs` | 333 | `test_config_new` | 1 | — |
| `src/git/backend.rs` | 339 | `test_parse_cgi_response_with_headers_and_body` | 1 | — |
| `src/git/backend.rs` | 348 | `test_parse_cgi_response_with_lf_only` | 1 | — |
| `src/git/backend.rs` | 356 | `test_parse_cgi_response_no_separator` | 1 | — |
| `src/git/backend.rs` | 364 | `test_parse_cgi_response_empty_body` | 1 | — |
| `src/git/backend.rs` | 372 | `test_prepare_cgi_env_sets_vars` | 1 | — |
| `src/git/backend.rs` | 385 | `test_prepare_cgi_env_sets_protected_branch_permission` | 1 | — |
| `src/git/bare/config/tests.rs` | 4 | `test_twig_config_parse` | 1 | — |
| `src/git/bare/config/tests.rs` | 16 | `test_present_config_parse` | 1 | — |
| `src/git/bare/config/tests.rs` | 36 | `test_paper_config_parse` | 1 | — |
| `src/git/bare/config/tests.rs` | 56 | `test_paper_config_empty_dir_is_not_configured` | 1 | — |
| `src/git/bare/config/tests.rs` | 62 | `test_scripts_config_parses_groups_and_entries` | 1 | — |
| `src/git/bare/config/tests.rs` | 110 | `test_scripts_config_absent_or_empty_yields_no_groups` | 1 | — |
| `src/git/bare/config/tests.rs` | 161 | `test_twig_config_should_ignore_folder` | 1 | — |
| `src/git/bare/config/tests.rs` | 178 | `test_twig_config_should_ignore_with_slash_pattern` | 1 | — |
| `src/git/bare/config/tests.rs` | 195 | `test_twig_config_should_ignore_multiple_patterns` | 1 | — |
| `src/git/bare/config/tests.rs` | 213 | `test_twig_config_empty` | 1 | — |
| `src/git/bare/config/tests.rs` | 220 | `test_twig_config_should_ignore_folder_with_trailing_slash` | 1 | — |
| `src/git/bare/config/tests.rs` | 237 | `test_twig_config_should_ignore_exact_file` | 1 | — |
| `src/git/bare/config/tests.rs` | 254 | `test_twig_config_toml_format_parsing` | 1 | — |
| `src/git/bare/config/tests.rs` | 265 | `test_twig_config_parse_private` | 1 | — |
| `src/git/bare/config.rs` | 36 | `default_script_shell` | 1 | — |
| `src/git/bare/config.rs` | 157 | `is_configured` | 1 | — |
| `src/git/bare/config.rs` | 162 | `default_paper_dir` | 1 | — |
| `src/git/bare/config.rs` | 229 | `load` | 1 | — |
| `src/git/bare/config.rs` | 238 | `parse` | 1 | — |
| `src/git/bare/config.rs` | 280 | `script_groups` | 1 | — |
| `src/git/bare/handle/read.rs` | 95 | `path_exists` | 1 | — |
| `src/git/bare/handle/tests.rs` | 7 | `new` | 1 | — |
| `src/git/bare/handle/tests.rs` | 13 | `handle` | 1 | — |
| `src/git/bare/handle/tests.rs` | 42 | `empty_handle` | 1 | — |
| `src/git/bare/handle/tests.rs` | 51 | `drop` | 1 | — |
| `src/git/bare/handle/tests.rs` | 57 | `commit_file_writes_nested_content_and_preserves_other_entries_and_mode` | 1 | — |
| `src/git/bare/handle/tests.rs` | 100 | `commit_file_refuses_to_overwrite_a_newer_branch_tip` | 1 | — |
| `src/git/bare/handle/tests.rs` | 137 | `commit_new_file_adds_a_nested_path_and_refuses_overwrites_and_stale_heads` | 1 | — |
| `src/git/bare/handle/tests.rs` | 218 | `commit_changes_applies_multiple_file_operations_in_one_commit` | 1 | — |
| `src/git/bare/handle/tests.rs` | 290 | `commit_changes_rejects_stale_head_and_invalid_or_no_op_batches` | 1 | — |
| `src/git/bare/handle/tests.rs` | 378 | `commit_changes_can_create_the_first_commit_in_an_empty_repository` | 1 | — |
| `src/git/bare/handle.rs` | 28 | `from_repository` | 1 | — |
| `src/git/bare/history.rs` | 14 | `new` | 1 | — |
| `src/git/bare/history.rs` | 23 | `hash` | 1 | — |
| `src/git/bare/history.rs` | 27 | `author` | 1 | — |
| `src/git/bare/history.rs` | 31 | `date` | 1 | — |
| `src/git/bare/history.rs` | 35 | `message` | 1 | — |
| `src/git/bare/history.rs` | 46 | `chrono` | 1 | — |
| `src/git/bare/history.rs` | 55 | `test_chrono_handles_valid_timestamp` | 1 | — |
| `src/git/bare/history.rs` | 62 | `test_chrono_handles_current_time` | 1 | — |
| `src/git/bare/listing.rs` | 66 | `get_non_commercial_license` | 1 | — |
| `src/git/bare/listing.rs` | 141 | `get_repos_with_info` | 1 | — |
| `src/git/bare/listing.rs` | 145 | `search_repos_with_info` | 1 | — |
| `src/git/bare/listing.rs` | 154 | `search_repo_names` | 1 | — |
| `src/git/bare/listing.rs` | 196 | `test_repo_info_has_name_and_date` | 1 | — |
| `src/git/bare/listing.rs` | 210 | `test_config_filenames_still_cover_the_historic_names` | 1 | — |
| `src/git/bare/listing.rs` | 219 | `fake_bare_repo` | 1 | — |
| `src/git/bare/listing.rs` | 232 | `scratch_root` | 1 | — |
| `src/git/bare/listing.rs` | 243 | `test_search_repo_names_matches_case_insensitively_and_sorts` | 1 | — |
| `src/git/bare/listing.rs` | 272 | `test_search_repo_names_skips_non_repository_directories` | 1 | — |
| `src/git/bare/listing.rs` | 285 | `test_search_repo_names_handles_missing_namespaces` | 1 | — |
| `src/git/bare/mod.rs` | 48 | `drop` | 1 | — |
| `src/git/bare/mod.rs` | 53 | `create_temp_dir` | 1 | — |
| `src/git/bare/mod.rs` | 66 | `init_bare_repo` | 1 | — |
| `src/git/bare/mod.rs` | 79 | `commit_files` | 1 | — |
| `src/git/bare/mod.rs` | 156 | `test_init_branch_parameter` | 1 | — |
| `src/git/bare/mod.rs` | 178 | `test_config_fallback_reads_fig_toml` | 1 | — |
| `src/git/bare/mod.rs` | 194 | `test_config_fallback_reads_dot_fig_toml` | 1 | — |
| `src/git/bare/mod.rs` | 209 | `test_config_prefers_current_name_over_fig` | 1 | — |
| `src/git/bare/mod.rs` | 232 | `test_config_prefers_twig_short_name_over_fig` | 1 | — |
| `src/git/bare/mod.rs` | 254 | `test_is_repo_private` | 1 | — |
| `src/git/bare/mod.rs` | 278 | `test_list_dir_walks_head_tree` | 1 | — |
| `src/git/bare/mod.rs` | 309 | `test_list_paper_pages_reads_sorted_markdown_under_dir` | 1 | — |
| `src/git/bare/mod.rs` | 346 | `test_open_rejects_parent_directory_components` | 1 | — |
| `src/git/bare/path.rs` | 33 | `test_is_safe_repo_path` | 1 | — |
| `src/git/mod.rs` | 428 | `test_build_response_status_header` | 1 | — |
| `src/git/mod.rs` | 436 | `test_build_response_default_status` | 1 | — |
| `src/git/mod.rs` | 444 | `test_build_response_content_type` | 1 | — |
| `src/git/mod.rs` | 452 | `test_build_response_multiple_headers` | 1 | — |
| `src/git/repo/tests.rs` | 41 | `test_default_branch_is_main` | 1 | — |
| `src/git/repo/tests.rs` | 46 | `test_create_repo_creates_directory_structure` | 1 | — |
| `src/git/repo/tests.rs` | 63 | `test_bare_init_creates_valid_repository` | 1 | — |
| `src/git/repo/tests.rs` | 99 | `contributor_hook_allows_fast_forwards_and_rejects_main_rewrites_and_deletions` | 1 | — |
| `src/git/repo/tests.rs` | 178 | `receive_hook_installation_preserves_chains_and_is_idempotent` | 1 | — |
| `src/git/repo/tests.rs` | 276 | `receive_hook_installation_upgrades_previous_managed_hook_in_place` | 1 | — |
| `src/git/repo.rs` | 23 | `default_branch` | 1 | — |
| `src/git/repo.rs` | 229 | `sh` | 1 | — |
| `src/git/reserved.rs` | 25 | `is_reserved_repo_name` | 1 | — |
| `src/git/reserved.rs` | 59 | `test_reserved_check_is_case_insensitive` | 1 | — |
| `src/git/reserved.rs` | 87 | `test_empty_name_is_refused` | 1 | — |
| `src/git/reserved.rs` | 103 | `test_dotfile_names_are_refused` | 1 | — |
| `src/http/api.rs` | 11 | `version_endpoint` | 1 | — |
| `src/http/api.rs` | 23 | `test_version_endpoint_returns_package_version` | 1 | — |
| `src/http/assets/tests.rs` | 41 | `starts_a_hex_colour` | 1 | — |
| `src/http/assets/tests.rs` | 70 | `test_urls_carry_the_version` | 1 | — |
| `src/http/assets/tests.rs` | 110 | `test_twig_schema_describes_nested_script_groups` | 1 | — |
| `src/http/assets/tests.rs` | 215 | `test_versioned_assets_handler_uses_configured_cache_control` | 1 | — |
| `src/http/assets/tests.rs` | 242 | `test_unversioned_assets_must_revalidate` | 1 | — |
| `src/http/assets/tests.rs` | 332 | `test_assets_handler_unknown_file_returns_404` | 1 | — |
| `src/http/assets/tests.rs` | 355 | `test_assets_handler_rejects_another_version` | 1 | — |
| `src/http/assets.rs` | 23 | `name` | 1 | — |
| `src/http/assets.rs` | 27 | `versioned_name` | 1 | — |
| `src/http/auth/handlers.rs` | 363 | `test_generate_token_format` | 1 | — |
| `src/http/auth/handlers.rs` | 372 | `test_invite_struct_with_none_user_id` | 1 | — |
| `src/http/auth/handlers.rs` | 386 | `test_invite_struct_with_some_user_id` | 1 | — |
| `src/http/auth/handlers.rs` | 400 | `test_create_invite_with_null_user_id` | 1 | — |
| `src/http/auth/namespace_invitations.rs` | 63 | `render_account_created` | 1 | — |
| `src/http/auth/namespace_invitations.rs` | 222 | `invitation` | 1 | — |
| `src/http/auth/namespace_invitations.rs` | 253 | `account_created_notice_offers_login_and_names_the_namespace` | 1 | — |
| `src/http/auth/pages/tests.rs` | 4 | `auth_pages` | 1 | — |
| `src/http/auth/pages/tests.rs` | 13 | `success_notices` | 1 | — |
| `src/http/auth/pages/tests.rs` | 24 | `input_tags` | 1 | — |
| `src/http/auth/pages/tests.rs` | 277 | `test_render_invite_success_shows_the_invite_as_code` | 1 | — |
| `src/http/auth/pages/tests.rs` | 304 | `test_render_login_success_offers_namespace_creation` | 1 | — |
| `src/http/auth/pages.rs` | 47 | `render_submit` | 1 | — |
| `src/http/auth/pages.rs` | 56 | `wrap_auth_content` | 1 | — |
| `src/http/auth/pages.rs` | 67 | `render_invite_page` | 1 | — |
| `src/http/auth/pages.rs` | 135 | `render_signup_page` | 1 | — |
| `src/http/auth/pages.rs` | 229 | `render_login_page` | 1 | — |
| `src/http/auth/pages.rs` | 293 | `render_namespace_page` | 1 | — |
| `src/http/auth/pages.rs` | 363 | `render_invite_success` | 1 | — |
| `src/http/auth/pages.rs` | 379 | `render_signup_success` | 1 | — |
| `src/http/auth/pages.rs` | 395 | `render_login_success` | 1 | — |
| `src/http/auth/session.rs` | 25 | `test_get_username_from_request_without_cookie` | 1 | — |
| `src/http/auth/session.rs` | 34 | `test_get_username_from_request_with_invalid_cookie` | 1 | — |
| `src/http/health.rs` | 4 | `health` | 1 | — |
| `src/http/health.rs` | 9 | `up` | 1 | — |
| `src/http/health.rs` | 19 | `test_health_endpoint` | 1 | — |
| `src/http/health.rs` | 27 | `test_up_endpoint` | 1 | — |
| `src/http/info/pages.rs` | 106 | `render_copy_bar` | 1 | — |
| `src/http/info/pages.rs` | 154 | `render_docs_tab` | 1 | — |
| `src/http/info/pages.rs` | 180 | `render_about_tab` | 1 | — |
| `src/http/info/pages.rs` | 195 | `render_page` | 1 | — |
| `src/http/info/pages.rs` | 236 | `escaped` | 1 | — |
| `src/http/info/pages.rs` | 244 | `test_tab_from_query_defaults_to_docs` | 1 | — |
| `src/http/info/pages.rs` | 251 | `test_tab_from_query_about` | 1 | — |
| `src/http/info/pages.rs` | 256 | `test_render_tabs_is_a_labelled_navigation_landmark` | 1 | — |
| `src/http/info/pages.rs` | 266 | `test_render_tabs_marks_only_the_active_tab` | 1 | — |
| `src/http/info/pages.rs` | 279 | `test_render_docs_menu_is_a_labelled_rail` | 1 | — |
| `src/http/info/pages.rs` | 293 | `test_render_docs_menu_marks_only_the_active_page` | 1 | — |
| `src/http/info/pages.rs` | 309 | `test_render_docs_menu_shows_section_heading_once` | 1 | — |
| `src/http/info/pages.rs` | 316 | `test_render_docs_tab_falls_back_to_first_page_on_unknown_slug` | 1 | — |
| `src/http/info/pages.rs` | 341 | `test_render_about_tab_contains_about_content` | 1 | — |
| `src/http/info/pages.rs` | 377 | `test_about_tab_offers_the_same_copy_button` | 1 | — |
| `src/http/info/pages.rs` | 387 | `test_copy_source_is_hidden_but_not_removed` | 1 | — |
| `src/http/info/pages.rs` | 435 | `test_query_deserialization` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 5 | `repo` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 14 | `test_namespace_breadcrumb_trail_is_semantic` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 39 | `test_namespace_is_headed_by_its_identifier_inside_the_trail` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 58 | `test_namespace_does_not_repeat_the_repository_section_heading` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 115 | `test_namespace_rows_keep_last_commit_labelled_when_stacked` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 222 | `test_create_repo_form_cancel_is_plain_text_with_its_dismiss_behaviour` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 263 | `test_format_date_years` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 273 | `test_format_date_months` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 283 | `test_format_date_days` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 293 | `test_format_date_hours` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 303 | `test_format_date_minutes` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 313 | `test_format_date_just_now` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 344 | `test_missing_email_response_is_a_bad_request_fragment_with_account_link` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 361 | `test_params_deserialization` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 367 | `test_create_repo_form_deserialization` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 374 | `test_create_repo_form_default_branch` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 381 | `test_search_query_deserialization` | 1 | — |
| `src/http/namespace/pages/tests.rs` | 387 | `test_search_query_empty` | 1 | — |
| `src/http/namespace/pages.rs` | 17 | `user_can_create_repositories` | 1 | — |
| `src/http/namespace/pages.rs` | 33 | `user_has_namespace_permission_for_request` | 1 | — |
| `src/http/namespace/pages.rs` | 434 | `missing_email_response` | 1 | — |
| `src/http/overview/pages.rs` | 201 | `namespace` | 1 | — |
| `src/http/overview/pages.rs` | 211 | `render` | 1 | — |
| `src/http/overview/pages.rs` | 219 | `render_with_repo_hits` | 1 | — |
| `src/http/overview/pages.rs` | 229 | `test_search_query_deserialization` | 1 | — |
| `src/http/overview/pages.rs` | 235 | `test_search_query_empty` | 1 | — |
| `src/http/overview/pages.rs` | 241 | `test_head_carries_the_single_create_action_for_signed_in_users` | 1 | — |
| `src/http/overview/pages.rs` | 258 | `test_create_action_stays_hidden_without_a_session` | 1 | — |
| `src/http/overview/pages.rs` | 305 | `test_clear_returns_to_the_unfiltered_index_only_while_filtering` | 1 | — |
| `src/http/overview/pages.rs` | 319 | `test_namespace_list_is_a_flush_technical_panel` | 1 | — |
| `src/http/overview/pages.rs` | 374 | `test_rows_keep_their_namespace_hrefs` | 1 | — |
| `src/http/overview/pages.rs` | 385 | `test_repo_hits_render_the_namespace_beneath_the_repository` | 1 | — |
| `src/http/overview/pages.rs` | 416 | `test_repo_hits_respect_the_private_filter_and_missing_roots` | 1 | — |
| `src/http/repo_edit/commit.rs` | 19 | `workspace_commit_preparation_error` | 1 | — |
| `src/http/repo_edit/commit.rs` | 109 | `render_commit_result` | 1 | — |
| `src/http/repo_edit/mod.rs` | 146 | `html_response` | 1 | — |
| `src/http/repo_edit/mod.rs` | 165 | `workspace_file_url` | 1 | — |
| `src/http/repo_edit/pages.rs` | 49 | `workspace_document_url` | 1 | — |
| `src/http/repo_edit/pages.rs` | 56 | `workspace_navigation_header` | 1 | — |
| `src/http/repo_edit/pages.rs` | 190 | `workspace_restore_form` | 1 | — |
| `src/http/repo_edit/pages.rs` | 382 | `file_unavailable` | 1 | — |
| `src/http/repo_edit/pages.rs` | 422 | `file_placeholder` | 1 | — |
| `src/http/repo_edit/pages.rs` | 431 | `editor_assets` | 1 | — |
| `src/http/repo_edit/tests.rs` | 54 | `workspace_paths_are_encoded_for_query_and_route_components` | 1 | — |
| `src/http/repo_edit/tests.rs` | 63 | `editor_operation_forms_override_implicit_get_inheritance` | 1 | — |
| `src/http/repo_edit/validation.rs` | 153 | `parent_path` | 1 | — |
| `src/http/repository/binaries.rs` | 217 | `parse_release_version` | 1 | — |
| `src/http/repository/binaries.rs` | 232 | `is_binary_size_allowed` | 1 | — |
| `src/http/repository/binaries.rs` | 236 | `binary_response` | 1 | — |
| `src/http/repository/binaries.rs` | 252 | `binary_versions_accept_optional_v_and_reject_latest_for_upload` | 1 | — |
| `src/http/repository/binaries.rs` | 262 | `binary_filename_and_size_validation_reject_unsafe_inputs` | 1 | — |
| `src/http/repository/pages/context.rs` | 17 | `text_size_persist` | 1 | — |
| `src/http/repository/pages/context.rs` | 21 | `text_size_list` | 1 | — |
| `src/http/repository/pages/context.rs` | 31 | `text_size_restore` | 1 | — |
| `src/http/repository/pages/context.rs` | 68 | `render_repo_auth_error` | 1 | — |
| `src/http/repository/pages/context.rs` | 118 | `has_optional_tab` | 1 | — |
| `src/http/repository/pages/context.rs` | 245 | `render_empty` | 1 | — |
| `src/http/repository/pages/context.rs` | 254 | `render_git_error` | 1 | — |
| `src/http/repository/pages/markdown.rs` | 166 | `markdown_options` | 1 | — |
| `src/http/repository/pages/markdown.rs` | 283 | `render_markdown_controls` | 1 | — |
| `src/http/repository/pages/paper.rs` | 70 | `paper_body` | 1 | — |
| `src/http/repository/pages/paper.rs` | 156 | `load_paper_pages` | 1 | — |
| `src/http/repository/pages/paper.rs` | 168 | `render_paper_page` | 1 | — |
| `src/http/repository/pages/paper.rs` | 211 | `paper_page_anchor` | 1 | — |
| `src/http/repository/pages/presentation.rs` | 39 | `present_body` | 1 | — |
| `src/http/repository/pages/presentation.rs` | 163 | `slide_counter` | 1 | — |
| `src/http/repository/pages/presentation.rs` | 452 | `render_pdf_slide_page` | 1 | — |
| `src/http/repository/pages/scripts.rs` | 139 | `script_command` | 1 | — |
| `src/http/repository/pages/scripts.rs` | 176 | `render_script_group_row` | 1 | — |
| `src/http/repository/pages/tab_handlers.rs` | 13 | `markdown_tab_handler` | 1 | — |
| `src/http/repository/pages/tab_handlers.rs` | 32 | `content_tab_handler` | 1 | — |
| `src/http/repository/pages/tab_handlers.rs` | 51 | `commits_tab_handler` | 1 | — |
| `src/http/repository/pages/tab_handlers.rs` | 70 | `config_tab_handler` | 1 | — |
| `src/http/repository/pages/tab_handlers.rs` | 89 | `present_tab_handler` | 1 | — |
| `src/http/repository/pages/tab_handlers.rs` | 108 | `paper_tab_handler` | 1 | — |
| `src/http/repository/pages/tab_handlers.rs` | 127 | `license_tab_handler` | 1 | — |
| `src/http/repository/pages/tab_handlers.rs` | 146 | `scripts_tab_handler` | 1 | — |
| `src/http/repository/pages/tab_handlers.rs` | 165 | `scripts_group_handler` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 14 | `markdown_files` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 22 | `markdown_body` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 26 | `markdown_body_from` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 39 | `content_body` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 52 | `config_body` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 59 | `license_body` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 130 | `tab_nav` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 244 | `render_repo_crumbs` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 415 | `render_config_error` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 449 | `render_commit` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 4 | `test_raw_content_type_is_text_for_scripts_and_neutral_otherwise` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 24 | `test_parent_path` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 31 | `test_breadcrumb_segments` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 45 | `test_content_rows_mark_directories_with_a_trailing_slash` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 72 | `test_content_dir_offers_a_parent_row_below_the_root` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 85 | `test_content_breadcrumbs_mark_the_final_segment_as_current` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 112 | `test_content_view_reports_missing_paths_and_binary_files` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 4 | `test_markdown_to_html` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 17 | `test_markdown_link_fixing` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 26 | `test_markdown_links_resolve_against_containing_directory` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 37 | `test_markdown_non_markdown_links_target_content_route` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 44 | `test_markdown_link_preserves_fragment_and_query` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 54 | `test_markdown_link_leaves_absolute_and_scheme_links_alone` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 62 | `test_slide_markdown_anchors_relative_links_to_the_slide_directory` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 73 | `test_slide_markdown_substitutes_template_variables` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 86 | `test_resolve_relative_path` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 95 | `test_has_url_scheme` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 104 | `test_markdown_tables_rendering` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 116 | `test_markdown_rail_appears_only_for_multiple_files` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 144 | `test_markdown_content_reports_a_missing_file` | 1 | — |
| `src/http/repository/pages/tests/markup.rs` | 25 | `test_render_repo_auth_error` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 76 | `test_paper_headings_slug_anchor_and_dedupe` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 100 | `test_render_paper_markdown_puts_heading_ids_on_the_headings` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 116 | `test_render_paper_content_only_anchors_headings` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 134 | `test_paper_page_anchors_are_path_relative_and_unique` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 156 | `test_paper_view_without_pages_states_the_condition` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 166 | `test_page_in_dir_accepts_only_paths_below_the_paper_directory` | 1 | — |
| `src/http/repository/pages/tests/presentation.rs` | 4 | `test_presentation_offers_the_pdf_export` | 1 | — |
| `src/http/repository/pages/tests/presentation.rs` | 227 | `test_slide_counter_pads_to_the_width_of_the_deck` | 1 | — |
| `src/http/repository/pages/tests/presentation.rs` | 233 | `test_present_view_without_slides_states_the_condition` | 1 | — |
| `src/http/repository/pages/tests/scripts.rs` | 4 | `test_base_url_schemes_follow_the_request` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 4 | `test_get_default_markdown_file` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 22 | `test_default_tab_prefers_configuration_then_markdown` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 30 | `test_tabs_are_an_out_of_band_labelled_nav` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 115 | `test_tabs_render_only_available_views_in_order` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 167 | `test_repo_page_leads_with_a_breadcrumb_head_then_the_tabs` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 187 | `test_repo_page_offers_workspace_from_the_repository_heading` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 204 | `test_repo_page_announces_its_tab_panel` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 212 | `test_tab_response_differs_for_htmx_and_direct_navigation` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 246 | `test_commit_row_is_a_dense_machine_record` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 284 | `test_commits_view_falls_back_to_an_empty_state` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 291 | `test_config_error_is_a_diagnostic_with_a_help_hint` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 342 | `test_config_and_license_views_render_code_and_empty_states` | 1 | — |
| `src/http/repository/pages/tests/tabs.rs` | 367 | `test_git_error_is_an_announced_danger_notice` | 1 | — |
| `src/http/repository/pages/tests.rs` | 6 | `base_frame` | 1 | — |
| `src/http/repository/pages/tests.rs` | 20 | `frame_with` | 1 | — |
| `src/http/repository/pages/tests.rs` | 26 | `slides` | 1 | — |
| `src/http/repository/pages/tests.rs` | 33 | `entry` | 1 | — |
| `src/http/repository/pages/tests.rs` | 40 | `count_of` | 1 | — |
| `src/http/repository/pages/tests.rs` | 43 | `tab_labels` | 1 | — |
| `src/http/repository/pages/tests.rs` | 55 | `representative_markup` | 1 | — |
| `src/http/routes.rs` | 12 | `configure_routes` | 1 | — |
| `src/http/routes.rs` | 106 | `is_git` | 1 | — |
| `src/http/settings/pages/profile.rs` | 14 | `render_settings` | 1 | — |
| `src/http/settings/pages/tests.rs` | 5 | `user_fixture` | 1 | — |
| `src/http/settings/pages/tests.rs` | 15 | `repo_fixture` | 1 | — |
| `src/http/settings/pages/tests.rs` | 23 | `populated_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 27 | `empty_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 31 | `repo_panel_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 35 | `empty_repo_panel_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 39 | `rename_panel_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 43 | `empty_rename_panel_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 71 | `test_account_page_renders_account_panel_only` | 1 | — |
| `src/http/settings/pages/tests.rs` | 112 | `test_settings_does_not_render_repository_moves` | 1 | — |
| `src/http/settings/pages/tests.rs` | 120 | `test_settings_deletion_panels_carry_danger_and_caution_affordances` | 1 | — |
| `src/http/settings/pages/tests.rs` | 229 | `test_settings_rename_panel_empty_state_offers_no_targets` | 1 | — |
| `src/http/settings/pages/tests.rs` | 242 | `test_settings_does_not_render_namespace_deletion_controls` | 1 | — |
| `src/http/settings/pages/tests.rs` | 280 | `test_settings_result_targets_announce_swaps` | 1 | — |
| `src/http/settings/pages/tests.rs` | 289 | `test_settings_keeps_identity_visible_and_email_break_safe` | 1 | — |
| `src/http/settings/pages/tests.rs` | 313 | `test_settings_empty_states_name_their_condition_and_offer_no_targets` | 1 | — |
| `src/http/settings/pages/tests.rs` | 361 | `test_namespace_form_deserialization` | 1 | — |
| `src/http/settings/pages/tests.rs` | 367 | `test_update_email_form_deserialization` | 1 | — |
| `src/http/settings/pages/tests.rs` | 373 | `test_delete_repo_form_deserialization` | 1 | — |
| `src/http/settings/pages/tests.rs` | 380 | `test_move_repo_form_deserialization` | 1 | — |
| `src/http/settings/pages/tests.rs` | 391 | `test_rename_repo_form_deserialization` | 1 | — |
| `src/http/settings/pages/tests.rs` | 400 | `test_rename_namespace_form_deserialization` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 3 | `core_system_endpoints` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 26 | `docs_endpoints` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 67 | `asset_endpoints` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 96 | `ui_endpoints` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 113 | `feature_endpoints` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 130 | `list_test_endpoints` | 1 | — |
| `src/http/test_page/pages/handlers.rs` | 110 | `is_hx_request` | 1 | — |
| `src/http/test_page/pages/handlers.rs` | 156 | `test_page_alias` | 1 | — |
| `src/http/test_page/pages/tests.rs` | 4 | `test_render_stopped_content` | 1 | — |
| `src/http/test_page/pages/tests.rs` | 11 | `test_render_test_page_is_idle_until_started` | 1 | — |
| `src/http/test_page/pages/views.rs` | 3 | `render_pagehead` | 1 | — |
| `src/http/test_page/pages/views.rs` | 155 | `render_test_page` | 1 | — |
| `src/http/test_page/pages/views.rs` | 177 | `render_idle_content` | 1 | — |
| `src/http/test_page/pages/views.rs` | 194 | `render_stopped_content` | 1 | — |
| `src/http/test_page/pages.rs` | 20 | `is_admin` | 1 | — |
| `src/http/test_page/pages.rs` | 42 | `generate_random_pin` | 1 | — |
| `src/http/tree/api.rs` | 77 | `test_config` | 1 | — |
| `src/http/tree/api.rs` | 89 | `test_tree_round_trips_as_named_messagepack` | 1 | — |
| `src/http/tree/api.rs` | 104 | `test_tree_endpoint_returns_namespaces_and_repositories` | 1 | — |
| `src/http/tree/api.rs` | 158 | `test_tree_endpoint_filters_private_repositories` | 1 | — |
| `src/http/tree/pages/data.rs` | 77 | `render_load_more` | 1 | — |
| `src/http/tree/pages/tests.rs` | 6 | `tree_hub_only_renders_enabled_sections` | 1 | — |
| `src/http/tree/pages/tests.rs` | 30 | `namespace_page_lists_repositories_with_confirmed_namespace_buttons` | 1 | — |
| `src/http/tree/pages/tests.rs` | 65 | `namespace_page_explains_when_no_namespace_can_be_deleted` | 1 | — |
| `src/http/tree/pages/tests.rs` | 78 | `namespace_page_lists_owned_namespaces_for_rename_with_the_htmx_contract` | 1 | — |
| `src/http/tree/pages/tests.rs` | 109 | `namespace_rename_panel_empty_state_offers_no_targets` | 1 | — |
| `src/http/tree/pages/tests.rs` | 120 | `data_page_uses_the_infinite_scroll_placeholder` | 1 | — |
| `src/http/tree/user_management/invitations.rs` | 12 | `is_expired` | 1 | — |
| `src/http/tree/user_management/invitations.rs` | 21 | `format_expiry` | 1 | — |
| `src/http/tree/user_management/invitations.rs` | 66 | `deliver_invitation` | 1 | — |
| `src/http/tree/user_management/tests.rs` | 6 | `scope` | 1 | — |
| `src/http/tree/user_management/tests.rs` | 25 | `invitation` | 1 | — |
| `src/http/tree/user_management/tests.rs` | 40 | `full_user_management_render_has_one_link_per_tab` | 1 | — |
| `src/http/tree/user_management/tests.rs` | 80 | `users_tab_is_reachable_with_htmx_and_renders_scoped_placeholder` | 1 | — |
| `src/http/tree/user_management/tests.rs` | 156 | `pending_invitation_lists_have_an_htmx_resend_action` | 1 | — |
| `src/http/tree/user_management/views.rs` | 9 | `render_user_management_tabs` | 1 | — |
| `src/http/tree/user_management/views.rs` | 180 | `render_user_management` | 1 | — |
| `src/http/tree/user_management.rs` | 17 | `default_invitation_role` | 1 | — |
| `src/http/view/test_util.rs` | 4 | `classes_in` | 1 | — |
| `src/http/view.rs` | 134 | `render_theme_toggle` | 1 | — |
| `src/http/view.rs` | 152 | `render_error` | 1 | — |
| `src/http/view.rs` | 161 | `render_error_with_action` | 1 | — |
| `src/http/view.rs` | 173 | `render_success` | 1 | — |
| `src/http/view.rs` | 187 | `layout_html` | 1 | — |
| `src/http/view.rs` | 254 | `test_layout_prefixes_named_page_title` | 1 | — |
| `src/http/view.rs` | 262 | `test_layout_escapes_named_page_title` | 1 | — |
| `src/http/view.rs` | 274 | `test_layout_skip_link_is_the_first_focusable_element` | 1 | — |
| `src/http/view.rs` | 322 | `test_layout_preserves_htmx_wiring` | 1 | — |
| `src/http/view.rs` | 360 | `test_layout_keeps_all_styling_in_stylesheets` | 1 | — |
| `src/http/view.rs` | 388 | `test_layout_optic_rule_is_decorative` | 1 | — |
| `src/http/view.rs` | 430 | `test_render_error_is_an_announced_danger_notice` | 1 | — |
| `src/http/view.rs` | 442 | `test_render_error_with_action_uses_a_real_recovery_link` | 1 | — |
| `src/http/view.rs` | 454 | `test_render_success_is_an_announced_status_notice` | 1 | — |
| `src/info/mod.rs` | 33 | `render_page_html` | 1 | — |
| `src/info/mod.rs` | 115 | `load_doc_page` | 1 | — |
| `src/info/mod.rs` | 127 | `test_about_page_has_content` | 1 | — |
| `src/info/mod.rs` | 133 | `test_list_doc_pages_sorted_by_order` | 1 | — |
| `src/info/mod.rs` | 143 | `test_environment_variables_page_is_grouped_under_self_hosting` | 1 | — |
| `src/info/mod.rs` | 170 | `test_load_doc_page_unknown_slug_is_none` | 1 | — |
| `src/info/mod.rs` | 175 | `test_render_page_html_produces_html` | 1 | — |
| `src/integration_tests/admin_tree_settings.rs` | 175 | `test_settings_page_without_login_is_an_htmx_recovery_fragment` | 1 | — |
| `src/integration_tests/admin_tree_settings.rs` | 190 | `test_settings_page_with_expired_session_is_a_full_recovery_page` | 1 | — |
| `src/integration_tests/admin_tree_settings.rs` | 208 | `test_settings_page_without_login` | 1 | — |
| `src/integration_tests/admin_tree_settings.rs` | 221 | `test_update_email_invalid_format` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 6 | `extract_invite` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 13 | `signup_and_login` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 62 | `create_namespace` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 85 | `test_health_endpoint` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 92 | `test_invite_page_loads` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 104 | `test_signup_page_loads` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 116 | `test_user_management_renders_admin_page_and_htmx_tabs` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 192 | `test_created_invitation_completes_account_and_membership_setup` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 322 | `expired_namespace_invitation_link_returns_gone` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 352 | `test_login_page_loads` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 364 | `test_namespace_page_loads` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 375 | `test_generate_invite_with_invalid_api_key` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 388 | `test_generate_invite_with_valid_api_key` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 401 | `test_signup_with_invalid_invite` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 419 | `test_login_with_wrong_password` | 1 | — |
| `src/integration_tests/auth_and_invitations.rs` | 432 | `test_create_namespace_without_session` | 1 | — |
| `src/integration_tests/namespace_permissions.rs` | 8 | `namespace_owner_can_invite_only_to_owned_namespace` | 1 | — |
| `src/integration_tests/namespace_permissions.rs` | 46 | `contributor_cannot_create_namespaces_or_repositories_but_can_access_existing_repo` | 1 | — |
| `src/integration_tests/namespace_permissions.rs` | 200 | `configured_admin_has_all_git_permissions_without_namespace_membership` | 1 | — |
| `src/integration_tests/repo_access_and_routes.rs` | 8 | `init_private_repo` | 1 | — |
| `src/integration_tests/repo_access_and_routes.rs` | 28 | `basic_auth` | 1 | — |
| `src/integration_tests/repo_access_and_routes.rs` | 178 | `test_legacy_tab_url_redirects_to_the_tab_route` | 1 | — |
| `src/integration_tests/repo_access_and_routes.rs` | 215 | `test_private_repository_ui_visibility_gated_on_login` | 1 | — |
| `src/integration_tests/repo_access_and_routes.rs` | 298 | `test_private_repository_git_read_gated_behind_auth` | 1 | — |
| `src/integration_tests/repo_binaries.rs` | 8 | `test_http_binary_upload_and_download_work_for_public_and_private_repos` | 1 | — |
| `src/integration_tests/repo_content.rs` | 56 | `init_repo_with_files` | 1 | — |
| `src/integration_tests/repo_content.rs` | 132 | `get_body` | 1 | — |
| `src/integration_tests/repo_content.rs` | 145 | `test_paper_tab_renders_lazily_loaded_pages` | 1 | — |
| `src/integration_tests/repo_content.rs` | 192 | `test_scripts_tab_is_hidden_until_configured` | 1 | — |
| `src/integration_tests/repo_content.rs` | 214 | `test_scripts_tab_shows_groups_with_curl_commands` | 1 | — |
| `src/integration_tests/repo_content.rs` | 278 | `test_invalid_twig_toml_shows_a_diagnostic` | 1 | — |
| `src/integration_tests/repo_management.rs` | 8 | `test_move_repo_between_owned_namespaces` | 1 | — |
| `src/integration_tests/repo_management.rs` | 37 | `test_move_repo_rejects_namespace_owned_by_another_user` | 1 | — |
| `src/integration_tests/repo_management.rs` | 70 | `test_rename_repo_within_owned_namespace` | 1 | — |
| `src/integration_tests/repo_management.rs` | 98 | `test_rename_repo_rejects_foreign_namespace_and_existing_name` | 1 | — |
| `src/integration_tests/repo_management.rs` | 143 | `test_rename_namespace_moves_directory_and_record` | 1 | — |
| `src/integration_tests/repo_management.rs` | 177 | `test_rename_namespace_rejects_foreign_owner_and_existing_name` | 1 | — |
| `src/integration_tests/repo_management.rs` | 209 | `test_cannot_create_namespace_with_reserved_prefix` | 1 | — |
| `src/integration_tests/repo_traversal.rs` | 18 | `drop` | 1 | — |
| `src/integration_tests/repo_traversal.rs` | 27 | `setup_traversal_fixture` | 1 | — |
| `src/integration_tests/repo_traversal.rs` | 53 | `create_traversal_service` | 1 | — |
| `src/integration_tests/repo_traversal.rs` | 87 | `test_content_tab_lists_repo_files` | 1 | — |
| `src/integration_tests/repo_traversal.rs` | 101 | `test_content_file_view_serves_repo_files_only` | 1 | — |
| `src/integration_tests/repo_traversal.rs` | 202 | `test_content_cannot_reach_sibling_namespace_via_dots` | 1 | — |
| `src/integration_tests/repo_workspace_commit.rs` | 9 | `repo_edit_commits_the_complete_draft_as_one_commit_and_keeps_the_workspace` | 1 | — |
| `src/integration_tests/repo_workspace_commit.rs` | 86 | `repo_edit_commit_conflict_preserves_draft_and_concurrent_head` | 1 | — |
| `src/integration_tests/repo_workspace_commit.rs` | 239 | `repo_edit_commit_accepts_drafts_larger_than_the_default_form_limit` | 1 | — |
| `src/integration_tests/repo_workspace_commit.rs` | 287 | `repo_edit_commit_requires_an_author_email` | 1 | — |
| `src/integration_tests/repo_workspace_ui.rs` | 7 | `init_text_repo` | 1 | — |
| `src/integration_tests/repo_workspace_ui.rs` | 184 | `repo_edit_operations_stage_multiple_file_changes_without_writing_git` | 1 | — |
| `src/integration_tests/repo_workspace_ui.rs` | 298 | `repo_edit_legacy_file_entry_denies_authenticated_users_without_namespace_access` | 1 | — |
| `src/integration_tests/support.rs` | 3 | `create_test_service` | 1 | — |
| `src/integration_tests/support.rs` | 13 | `create_test_service_in` | 1 | — |
| `src/integration_tests/support.rs` | 22 | `create_test_service_with_db_in` | 1 | — |
| `src/integration_tests/support.rs` | 68 | `create_admin_test_service` | 1 | — |
| `src/integration_tests/support.rs` | 177 | `body_of` | 1 | — |
| `src/integration_tests/test_page.rs` | 48 | `test_test_page_404_when_test_user_unset` | 1 | — |
| `src/integration_tests/test_page.rs` | 55 | `test_test_page_forbidden_when_unauthenticated` | 1 | — |
| `src/integration_tests/test_page.rs` | 62 | `test_test_page_and_rapid_runner_when_authenticated_as_admin` | 1 | — |
| `src/integration_tests/test_page.rs` | 133 | `test_test_page_pin_and_qr_code_multi_user_lifecycle` | 1 | — |
| `src/main.rs` | 21 | `init_logging` | 1 | — |
| `src/md/mod.rs` | 22 | `markdown_to_ast` | 1 | — |
| `src/md/mod.rs` | 29 | `ast_to_html` | 1 | — |
| `src/md/mod.rs` | 78 | `highlighted_rust_event` | 1 | — |
| `src/md/mod.rs` | 141 | `process_markdown` | 1 | — |
| `src/md/mod.rs` | 152 | `test_replace_mustache_simple` | 1 | — |
| `src/md/mod.rs` | 160 | `test_replace_mustache_multiple` | 1 | — |
| `src/md/mod.rs` | 169 | `test_replace_mustache_no_match` | 1 | — |
| `src/md/mod.rs` | 176 | `test_process_markdown` | 1 | — |
| `src/md/mod.rs` | 186 | `test_markdown_to_ast` | 1 | — |
| `src/md/mod.rs` | 193 | `test_ast_to_html` | 1 | — |
| `src/md/mod.rs` | 201 | `test_process_markdown_renders_mermaid_fences_as_svg` | 1 | — |
| `src/md/mod.rs` | 219 | `test_invalid_mermaid_fence_falls_back_to_code` | 1 | — |
| `src/md/mod.rs` | 231 | `test_non_mermaid_fences_remain_code` | 1 | — |
| `src/md/mod.rs` | 246 | `test_rust_fences_are_highlighted_and_other_fences_are_unchanged` | 1 | — |

> Complexity is a lexical estimate: start at 1 per function, add 1 for each `if`, loop, `&&`, `||`, and `?`, and add one for each `match` arm after the first. Use it to find functions worth simplifying; do not mechanically minimize totals or split clear code into unnecessary helpers. Macros are not expanded; decision-like tokens inside invocations may be counted lexically. Syntax edge cases can make these values differ from AST-based tools.

## Hard-blocking lint suppressions

_No supported lint-suppression mechanisms found._

## Underscore-prefixed binding candidates

| File | Line | Function | Binding | Occurrences | Assessment |
| --- | ---: | --- | --- | ---: | --- |
| `src/git/bare/mod.rs` | 158 | `test_init_branch_parameter` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 180 | `test_config_fallback_reads_fig_toml` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 196 | `test_config_fallback_reads_dot_fig_toml` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 211 | `test_config_prefers_current_name_over_fig` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 234 | `test_config_prefers_twig_short_name_over_fig` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 256 | `test_is_repo_private` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 280 | `test_list_dir_walks_head_tree` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 311 | `test_list_paper_pages_reads_sorted_markdown_under_dir` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 348 | `test_open_rejects_parent_directory_components` | `_cleanup` | 1 | Likely unused |
| `src/http/tree/api.rs` | 172 | `test_tree_endpoint_filters_private_repositories` | `_p` | 1 | Likely unused |
| `src/integration_tests/repo_access_and_routes.rs` | 12 | `init_private_repo` | `_p` | 1 | Likely unused |
| `src/integration_tests/repo_content.rs` | 17 | `init_repo_with_paper` | `_p` | 1 | Likely unused |
| `src/integration_tests/repo_content.rs` | 97 | `init_repo_with_files` | `_p` | 1 | Likely unused |

> Underscore-prefixed names are lexical candidates, not compiler diagnostics. A name seen once in its function is marked likely unused; shadowing, macros, and complex Rust patterns can affect the result.
