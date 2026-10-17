# Margaret Project Complexity Report

Audit date (UTC): 2026-10-17

## Project summary

- Audit status: **CLEAR**
- Rust source files discovered: 114
- Files measured: 114
- Files skipped with read warnings: 0
- Total source size: 879520 bytes
- Total physical lines: 25356
- Functions analyzed: 979
- Average estimated function complexity: 2.5
- Maximum estimated function complexity: 27
- Complexity review hotspots (>= 10): 42
- Large-file review hotspots (> 500 physical lines): 0
- Hard-blocking lint suppressions: 0
- Likely unused underscore-prefixed bindings: 18

## Review thresholds

- Function hotspot: estimated complexity of 10 or higher.
- Large file: more than 500 physical lines.

## Rust source file inventory

| File | Bytes | Physical lines | Review status |
| --- | ---: | ---: | --- |
| `crates/twig-highlight/src/lib.rs` | 13896 | 461 | — |
| `src/http/info/pages.rs` | 15130 | 440 | — |
| `src/http/repository/pages/presentation.rs` | 16844 | 439 | — |
| `src/http/repository/pages/tabs.rs` | 12714 | 437 | — |
| `src/git/bare/handle.rs` | 14853 | 436 | — |
| `src/http/repository/pages.rs` | 14822 | 433 | — |
| `src/db/binaries.rs` | 15013 | 425 | — |
| `src/http/repository/pages/paper.rs` | 14980 | 422 | — |
| `src/git/mod.rs` | 14995 | 421 | — |
| `src/integration_tests/tests/repository_editor.rs` | 16568 | 414 | — |
| `src/http/repository/editor.rs` | 13387 | 413 | — |
| `src/http/auth/pages.rs` | 13586 | 411 | — |
| `src/http/settings/pages/tests.rs` | 13150 | 404 | — |
| `src/http/view.rs` | 17426 | 396 | — |
| `src/http/tree/pages.rs` | 16923 | 393 | — |
| `src/http/namespace/pages.rs` | 14799 | 392 | — |
| `src/git/backend.rs` | 12681 | 392 | — |
| `src/db/migration.rs` | 13618 | 390 | — |
| `src/http/namespace/pages/tests.rs` | 13157 | 390 | — |
| `src/integration_tests/tests/namespace.rs` | 14731 | 385 | — |
| `src/http/settings/pages/repositories.rs` | 15649 | 384 | — |
| `src/http/assets/tests.rs` | 11418 | 366 | — |
| `src/http/test_page/pages.rs` | 10777 | 366 | — |
| `src/http/auth/handlers.rs` | 11758 | 361 | — |
| `src/git/bare/mod.rs` | 12772 | 357 | — |
| `src/http/repository/pages/markdown.rs` | 11362 | 346 | — |
| `src/http/repository/pages/tests/navigation.rs` | 11133 | 342 | — |
| `src/email.rs` | 11885 | 337 | — |
| `src/http/tree/user_management/invitations.rs` | 12493 | 336 | — |
| `src/integration_tests/tests/auth.rs` | 12483 | 336 | — |
| `src/integration_tests/tests/repository_binaries.rs` | 11330 | 334 | — |
| `src/git/repo.rs` | 11065 | 330 | — |
| `src/http/repository/pages/scripts.rs` | 13000 | 329 | — |
| `src/http/auth/pages/tests.rs` | 10831 | 327 | — |
| `src/db/users.rs` | 11290 | 322 | — |
| `src/integration_tests/tests/support/repository.rs` | 10738 | 306 | — |
| `src/db/namespaces.rs` | 10344 | 306 | — |
| `src/auth/mod.rs` | 8101 | 305 | — |
| `src/git/bare/listing.rs` | 11226 | 291 | — |
| `src/git/bare/config.rs` | 10239 | 291 | — |
| `src/http/overview/pages/tests.rs` | 9376 | 289 | — |
| `src/http/tree/user_management.rs` | 11482 | 288 | — |
| `src/integration_tests/tests/admin.rs` | 10792 | 286 | — |
| `src/http/repository/pages/content.rs` | 9229 | 280 | — |
| `src/db/invitations/tests.rs` | 8353 | 277 | — |
| `src/db/namespaces/tests.rs` | 7764 | 276 | — |
| `src/config.rs` | 8657 | 275 | — |
| `src/git/bare/config/tests.rs` | 8213 | 274 | — |
| `src/md/mod.rs` | 9145 | 266 | — |
| `src/http/auth/namespace_invitations.rs` | 10400 | 260 | — |
| `src/git/repo/tests.rs` | 8552 | 256 | — |
| `src/http/repository/pages/tests/presentation.rs` | 9158 | 251 | — |
| `src/db/invitations.rs` | 9872 | 248 | — |
| `src/http/tree/api.rs` | 9485 | 247 | — |
| `src/http/test_page/pages/render.rs` | 10440 | 243 | — |
| `src/http/repository/binaries.rs` | 7634 | 232 | — |
| `src/http/repository/pages/tests/mod.rs` | 7225 | 231 | — |
| `src/integration_tests/tests/support/app.rs` | 7394 | 224 | — |
| `src/git/bare/handle/edit_tests.rs` | 6272 | 208 | — |
| `src/integration_tests/tests/repository_tabs.rs` | 6815 | 200 | — |
| `src/integration_tests/tests/repository_content.rs` | 6820 | 199 | — |
| `src/integration_tests/tests/test_page.rs` | 8024 | 198 | — |
| `src/http/overview/pages.rs` | 7356 | 198 | — |
| `src/http/tree/pages/data.rs` | 6817 | 195 | — |
| `src/config/tests.rs` | 5379 | 194 | — |
| `src/integration_tests/tests/repository_auth.rs` | 8459 | 190 | — |
| `src/http/repository/pages/tests/markdown.rs` | 6804 | 186 | — |
| `src/db/data.rs` | 6514 | 183 | — |
| `src/http/settings/pages/account.rs` | 6261 | 181 | — |
| `src/info/mod.rs` | 5453 | 180 | — |
| `src/http/repository/pages/tests/paper.rs` | 5888 | 179 | — |
| `src/auth/tests.rs` | 5554 | 178 | — |
| `src/http/settings/pages/namespaces.rs` | 6300 | 175 | — |
| `src/http/routes.rs` | 6728 | 166 | — |
| `src/auth/session_store.rs` | 4878 | 157 | — |
| `src/http/assets.rs` | 4711 | 156 | — |
| `src/http/repository/pages/tests/content.rs` | 5542 | 150 | — |
| `src/main.rs` | 5142 | 148 | — |
| `src/db/tokens.rs` | 4281 | 140 | — |
| `src/http/test_page/pages/catalog.rs` | 3494 | 137 | — |
| `src/db/invites.rs` | 4043 | 133 | — |
| `src/http/repository/editor/render.rs` | 6339 | 132 | — |
| `src/http/tree/pages/tests.rs` | 5175 | 129 | — |
| `src/http/tree/user_management/tests.rs` | 4405 | 127 | — |
| `crates/twig-highlight/src/tests.rs` | 5391 | 121 | — |
| `src/db/mod.rs` | 3282 | 116 | — |
| `src/git/reserved.rs` | 3116 | 107 | — |
| `src/db/test_pins.rs` | 3157 | 101 | — |
| `src/integration_tests/tests/support/auth.rs` | 2916 | 92 | — |
| `src/http/repository/editor/tests.rs` | 2754 | 85 | — |
| `src/db/sessions.rs` | 2564 | 85 | — |
| `src/http/auth/handlers/tests.rs` | 2250 | 82 | — |
| `src/git/bare/history.rs` | 1666 | 67 | — |
| `src/git/bare/path.rs` | 1745 | 52 | — |
| `src/http/settings/pages.rs` | 1450 | 50 | — |
| `src/http/auth/session.rs` | 1469 | 43 | — |
| `src/http/repository/pages/tests/scripts.rs` | 1286 | 43 | — |
| `src/http/test_page/pages/tests.rs` | 1513 | 42 | — |
| `crates/twig-highlight/build.rs` | 1339 | 38 | — |
| `src/http/api.rs` | 1012 | 35 | — |
| `src/http/health.rs` | 959 | 33 | — |
| `src/http/view/test_util.rs` | 833 | 22 | — |
| `src/integration_tests/tests/mod.rs` | 377 | 16 | — |
| `src/http/mod.rs` | 211 | 13 | — |
| `src/integration_tests/tests/support/mod.rs` | 113 | 7 | — |
| `src/http/auth/mod.rs` | 81 | 4 | — |
| `src/integration_tests.rs` | 67 | 3 | — |
| `src/http/tree/mod.rs` | 53 | 3 | — |
| `src/http/repository/mod.rs` | 49 | 3 | — |
| `src/http/info/mod.rs` | 15 | 1 | — |
| `src/http/namespace/mod.rs` | 15 | 1 | — |
| `src/http/overview/mod.rs` | 15 | 1 | — |
| `src/http/settings/mod.rs` | 15 | 1 | — |
| `src/http/test_page/mod.rs` | 15 | 1 | — |

## Function complexity ranking

| File | Line | Function | Estimated complexity | Review status |
| --- | ---: | --- | ---: | --- |
| `src/db/invitations.rs` | 95 | `accept_namespace_invitation` | 27 | Review hotspot |
| `src/http/repository/editor.rs` | 138 | `open_editor_context` | 27 | Review hotspot |
| `src/db/binaries.rs` | 45 | `put_binary` | 22 | Review hotspot |
| `src/db/migration.rs` | 156 | `create_tables` | 21 | Review hotspot |
| `src/git/bare/handle.rs` | 81 | `commit_file` | 20 | Review hotspot |
| `src/http/repository/pages.rs` | 336 | `render_tabs` | 20 | Review hotspot |
| `src/git/bare/handle.rs` | 145 | `commit_new_file` | 19 | Review hotspot |
| `src/http/auth/namespace_invitations.rs` | 139 | `accept_invitation_handler` | 18 | Review hotspot |
| `src/git/repo.rs` | 222 | `install_pre_receive_hook` | 17 | Review hotspot |
| `src/http/settings/pages/namespaces.rs` | 15 | `rename_namespace` | 17 | Review hotspot |
| `crates/twig-highlight/src/lib.rs` | 285 | `try_from` | 16 | Review hotspot |
| `src/db/invitations.rs` | 207 | `list_namespace_invitations_for` | 16 | Review hotspot |
| `src/db/invitations.rs` | 41 | `get_namespace_invitation` | 15 | Review hotspot |
| `src/http/settings/pages/repositories.rs` | 203 | `move_repo` | 15 | Review hotspot |
| `src/db/data.rs` | 37 | `read_table_page` | 14 | Review hotspot |
| `src/http/settings/pages/repositories.rs` | 299 | `rename_repo` | 14 | Review hotspot |
| `src/git/mod.rs` | 134 | `authenticate_git_request` | 13 | Review hotspot |
| `src/http/auth/handlers.rs` | 113 | `signup_ui_handler` | 13 | Review hotspot |
| `src/http/tree/user_management.rs` | 99 | `render_user_management` | 13 | Review hotspot |
| `src/db/users.rs` | 33 | `list_users_for_management_in` | 12 | Review hotspot |
| `src/email.rs` | 69 | `invitation_url` | 11 | Review hotspot |
| `src/git/bare/config.rs` | 243 | `should_ignore` | 11 | Review hotspot |
| `src/git/bare/handle.rs` | 397 | `collect_markdown_files` | 11 | Review hotspot |
| `src/git/bare/listing.rs` | 19 | `extract_license_from_cargo_toml` | 11 | Review hotspot |
| `src/git/mod.rs` | 214 | `is_authenticated` | 11 | Review hotspot |
| `src/http/auth/handlers.rs` | 267 | `create_namespace_ui_handler` | 11 | Review hotspot |
| `src/http/namespace/pages.rs` | 51 | `format_date` | 11 | Review hotspot |
| `src/http/overview/pages.rs` | 18 | `render_index` | 11 | Review hotspot |
| `crates/twig-highlight/src/lib.rs` | 216 | `token_kind` | 10 | Review hotspot |
| `src/db/binaries.rs` | 163 | `list_binary_releases` | 10 | Review hotspot |
| `src/db/binaries.rs` | 240 | `get_latest_binary` | 10 | Review hotspot |
| `src/db/invites.rs` | 22 | `get_invite` | 10 | Review hotspot |
| `src/db/mod.rs` | 33 | `conn` | 10 | Review hotspot |
| `src/db/namespaces.rs` | 86 | `namespaces_with_owners` | 10 | Review hotspot |
| `src/db/users.rs` | 88 | `get_user_by_username` | 10 | Review hotspot |
| `src/db/users.rs` | 111 | `get_user_by_id` | 10 | Review hotspot |
| `src/git/bare/handle.rs` | 318 | `list_dir` | 10 | Review hotspot |
| `src/git/repo.rs` | 91 | `create_repo` | 10 | Review hotspot |
| `src/http/namespace/pages.rs` | 79 | `render_namespace` | 10 | Review hotspot |
| `src/http/repository/pages/scripts.rs` | 104 | `render_scripts_view` | 10 | Review hotspot |
| `src/http/tree/user_management/invitations.rs` | 274 | `resend_namespace_invitation` | 10 | Review hotspot |
| `src/md/mod.rs` | 95 | `render_mermaid_blocks` | 10 | Review hotspot |
| `crates/twig-highlight/src/lib.rs` | 33 | `class` | 9 | — |
| `crates/twig-highlight/src/lib.rs` | 341 | `match_at` | 9 | — |
| `src/auth/mod.rs` | 278 | `seed_dev_admin` | 9 | — |
| `src/db/namespaces.rs` | 52 | `get_namespace_by_name` | 9 | — |
| `src/db/namespaces.rs` | 176 | `get_owned_namespaces_for_user` | 9 | — |
| `src/git/backend.rs` | 56 | `kind` | 9 | — |
| `src/git/repo.rs` | 132 | `bare_init` | 9 | — |
| `src/http/repository/binaries.rs` | 25 | `upload_binary` | 9 | — |
| `src/http/repository/editor.rs` | 269 | `commit_file_handler` | 9 | — |
| `src/http/tree/user_management/invitations.rs` | 188 | `create_namespace_invitation` | 9 | — |
| `src/db/namespaces.rs` | 6 | `create_namespace` | 8 | — |
| `src/db/namespaces.rs` | 263 | `delete_namespace` | 8 | — |
| `src/git/mod.rs` | 13 | `git_handler` | 8 | — |
| `src/git/repo.rs` | 26 | `init` | 8 | — |
| `src/http/auth/handlers.rs` | 201 | `login_ui_handler` | 8 | — |
| `src/http/auth/namespace_invitations.rs` | 77 | `load_active_invitation` | 8 | — |
| `src/http/namespace/pages.rs` | 290 | `create_repo_handler` | 8 | — |
| `src/http/repository/binaries.rs` | 90 | `download_binary` | 8 | — |
| `src/http/repository/pages/paper.rs` | 272 | `paper_headings` | 8 | — |
| `src/http/repository/pages/presentation.rs` | 52 | `slide_handler` | 8 | — |
| `src/http/repository/pages/tabs.rs` | 42 | `tab_body` | 8 | — |
| `src/http/tree/pages/data.rs` | 15 | `render_data_page` | 8 | — |
| `src/http/tree/pages.rs` | 48 | `render_namespaces_page` | 8 | — |
| `src/http/tree/pages.rs` | 317 | `repositories_page` | 8 | — |
| `src/http/tree/user_management/invitations.rs` | 8 | `render_invitation_list` | 8 | — |
| `src/http/tree/user_management.rs` | 45 | `user_management_scope` | 8 | — |
| `src/main.rs` | 77 | `main` | 8 | — |
| `src/md/mod.rs` | 42 | `highlight_rust_code_blocks` | 8 | — |
| `crates/twig-highlight/src/lib.rs` | 378 | `delimited_end` | 7 | — |
| `crates/twig-highlight/src/lib.rs` | 407 | `hash_raw_string_end` | 7 | — |
| `crates/twig-highlight/src/lib.rs` | 437 | `escape_into` | 7 | — |
| `src/db/binaries.rs` | 208 | `get_binary` | 7 | — |
| `src/db/namespaces.rs` | 206 | `user_can_create_namespace` | 7 | — |
| `src/db/users.rs` | 6 | `list_users_for_management` | 7 | — |
| `src/git/backend.rs` | 91 | `run_with_config` | 7 | — |
| `src/git/mod.rs` | 354 | `ensure_repo_exists` | 7 | — |
| `src/http/repository/editor.rs` | 53 | `conflict_copy_candidate` | 7 | — |
| `src/http/repository/pages/content.rs` | 31 | `content_handler` | 7 | — |
| `src/http/repository/pages/scripts.rs` | 69 | `raw_handler` | 7 | — |
| `src/http/settings/pages/namespaces.rs` | 117 | `delete_namespace` | 7 | — |
| `src/auth/mod.rs` | 156 | `extract_basic_auth` | 6 | — |
| `src/auth/mod.rs` | 221 | `user_id_from_request` | 6 | — |
| `src/db/data.rs` | 16 | `list_table_names` | 6 | — |
| `src/db/namespaces.rs` | 140 | `get_namespace_role` | 6 | — |
| `src/db/test_pins.rs` | 19 | `get_test_pin` | 6 | — |
| `src/db/tokens.rs` | 17 | `get_token_user` | 6 | — |
| `src/email.rs` | 188 | `read_http_request` | 6 | — |
| `src/git/bare/config.rs` | 95 | `collect_script_groups` | 6 | — |
| `src/git/bare/handle.rs` | 66 | `file_mode` | 6 | — |
| `src/git/bare/handle.rs` | 227 | `read_blob_bytes` | 6 | — |
| `src/git/bare/handle.rs` | 364 | `list_paper_pages` | 6 | — |
| `src/git/bare/listing.rs` | 174 | `is_repo_private` | 6 | — |
| `src/git/bare/path.rs` | 5 | `is_safe_component` | 6 | — |
| `src/http/overview/pages.rs` | 136 | `index` | 6 | — |
| `src/http/repository/binaries.rs` | 140 | `require_namespace_writer` | 6 | — |
| `src/http/repository/binaries.rs` | 184 | `is_safe_filename` | 6 | — |
| `src/http/repository/editor/render.rs` | 4 | `render_editor_page` | 6 | — |
| `src/http/repository/pages/markdown.rs` | 123 | `rewrite_markdown_link` | 6 | — |
| `src/http/repository/pages/paper.rs` | 88 | `open_paper_page` | 6 | — |
| `src/http/repository/pages/paper.rs` | 238 | `heading_level` | 6 | — |
| `src/http/repository/pages/paper.rs` | 250 | `heading_slug` | 6 | — |
| `src/http/repository/pages/scripts.rs` | 300 | `script_base_url` | 6 | — |
| `src/http/repository/pages/tabs.rs` | 131 | `handler` | 6 | — |
| `src/http/settings/pages/repositories.rs` | 142 | `delete_repo` | 6 | — |
| `src/http/test_page/pages.rs` | 95 | `authenticate_caller` | 6 | — |
| `src/http/tree/pages/data.rs` | 117 | `data_page` | 6 | — |
| `src/http/tree/pages.rs` | 237 | `namespaces_page` | 6 | — |
| `src/http/tree/user_management.rs` | 224 | `user_management_page` | 6 | — |
| `src/db/invitations.rs` | 12 | `create_namespace_invitation` | 5 | — |
| `src/db/migration.rs` | 249 | `test_create_tables_runs_all_migrations` | 5 | — |
| `src/db/test_pins.rs` | 46 | `validate_test_pin` | 5 | — |
| `src/email.rs` | 113 | `send_invitation_to` | 5 | — |
| `src/git/backend.rs` | 172 | `prepare_cgi_env` | 5 | — |
| `src/git/bare/handle.rs` | 268 | `get_license_content` | 5 | — |
| `src/git/bare/listing.rs` | 90 | `last_commit_date` | 5 | — |
| `src/git/bare/mod.rs` | 85 | `build_tree` | 5 | — |
| `src/git/repo.rs` | 301 | `set_hook_executable` | 5 | — |
| `src/git/reserved.rs` | 33 | `validate_repo_name` | 5 | — |
| `src/http/auth/handlers.rs` | 39 | `logout_ui_handler` | 5 | — |
| `src/http/auth/pages/tests.rs` | 78 | `test_auth_pages_label_and_describe_every_input` | 5 | — |
| `src/http/overview/pages/tests.rs` | 74 | `test_search_cluster_preserves_the_get_form_contract` | 5 | — |
| `src/http/repository/editor.rs` | 92 | `conflict_copy_path` | 5 | — |
| `src/http/repository/pages/content.rs` | 201 | `render_content_dir` | 5 | — |
| `src/http/repository/pages/content.rs` | 239 | `render_content_file` | 5 | — |
| `src/http/repository/pages/markdown.rs` | 101 | `resolve_relative_path` | 5 | — |
| `src/http/repository/pages/presentation.rs` | 293 | `present_print_handler` | 5 | — |
| `src/http/repository/pages/tabs.rs` | 69 | `respond_tab` | 5 | — |
| `src/http/repository/pages/tests/paper.rs` | 4 | `test_paper_view_lazy_loads_pages_with_zoom_and_font_controls` | 5 | — |
| `src/http/repository/pages/tests/presentation.rs` | 85 | `test_print_export_is_a_chrome_free_document` | 5 | — |
| `src/http/settings/pages/account.rs` | 83 | `settings_page` | 5 | — |
| `src/http/test_page/pages/render.rs` | 37 | `render_pin_panel` | 5 | — |
| `src/http/test_page/pages.rs` | 186 | `handle_test_page` | 5 | — |
| `src/http/tree/pages/data.rs` | 168 | `data_rows` | 5 | — |
| `src/http/tree/user_management/invitations.rs` | 93 | `parse_expiry` | 5 | — |
| `src/integration_tests/tests/support/repository.rs` | 11 | `init_repo_files` | 5 | — |
| `src/integration_tests/tests/support/repository.rs` | 230 | `tree` | 5 | — |
| `src/main.rs` | 43 | `spawn_database_init` | 5 | — |
| `crates/twig-highlight/src/lib.rs` | 126 | `highlight` | 4 | — |
| `src/auth/session_store.rs` | 60 | `save` | 4 | — |
| `src/auth/session_store.rs` | 77 | `update` | 4 | — |
| `src/config.rs` | 134 | `maybe_reset_database` | 4 | — |
| `src/config.rs` | 185 | `from_env` | 4 | — |
| `src/db/invitations.rs` | 75 | `list_namespace_invitations` | 4 | — |
| `src/db/mod.rs` | 66 | `init_tables` | 4 | — |
| `src/db/namespaces.rs` | 119 | `user_has_namespace_access` | 4 | — |
| `src/db/namespaces.rs` | 236 | `user_has_any_namespaces` | 4 | — |
| `src/db/sessions.rs` | 4 | `load_actix_session` | 4 | — |
| `src/db/test_pins.rs` | 4 | `set_test_pin` | 4 | — |
| `src/db/users.rs` | 250 | `user_management_lists_global_users_and_scopes_members_by_namespace` | 4 | — |
| `src/git/bare/handle.rs` | 35 | `open` | 4 | — |
| `src/git/bare/handle.rs` | 199 | `get_commits` | 4 | — |
| `src/git/bare/handle.rs` | 299 | `list_files` | 4 | — |
| `src/git/bare/listing.rs` | 98 | `list_repos_with_info` | 4 | — |
| `src/git/bare/path.rs` | 18 | `is_safe_repo_path` | 4 | — |
| `src/git/mod.rs` | 109 | `build_response` | 4 | — |
| `src/git/repo.rs` | 287 | `next_preserved_hook_path` | 4 | — |
| `src/http/auth/namespace_invitations.rs` | 16 | `render_invitation_setup` | 4 | — |
| `src/http/auth/pages.rs` | 20 | `render_field` | 4 | — |
| `src/http/info/pages.rs` | 73 | `render_docs_menu` | 4 | — |
| `src/http/info/pages.rs` | 420 | `test_page_carries_no_outgoing_classes_or_inline_styles` | 4 | — |
| `src/http/namespace/pages.rs` | 180 | `handler` | 4 | — |
| `src/http/repository/editor.rs` | 20 | `encode_file_path` | 4 | — |
| `src/http/repository/editor.rs` | 33 | `sanitized_username` | 4 | — |
| `src/http/repository/editor.rs` | 357 | `save_conflict_copy` | 4 | — |
| `src/http/repository/pages/content.rs` | 111 | `breadcrumb_segments` | 4 | — |
| `src/http/repository/pages/content.rs` | 128 | `render_content_breadcrumbs` | 4 | — |
| `src/http/repository/pages/content.rs` | 165 | `render_content_view` | 4 | — |
| `src/http/repository/pages/markdown.rs` | 29 | `markdown_handler` | 4 | — |
| `src/http/repository/pages/markdown.rs` | 76 | `has_url_scheme` | 4 | — |
| `src/http/repository/pages/markdown.rs` | 208 | `render_paper_markdown` | 4 | — |
| `src/http/repository/pages/presentation.rs` | 177 | `render_slide_content` | 4 | — |
| `src/http/repository/pages/presentation.rs` | 338 | `render_pdf_document` | 4 | — |
| `src/http/repository/pages/tests/mod.rs` | 212 | `test_repo_markup_carries_no_inline_presentation` | 4 | — |
| `src/http/repository/pages.rs` | 194 | `open_repo` | 4 | — |
| `src/http/settings/pages/account.rs` | 148 | `update_email` | 4 | — |
| `src/http/settings/pages/repositories.rs` | 29 | `render_repo_deletion_panel` | 4 | — |
| `src/http/settings/pages/repositories.rs` | 81 | `render_repo_rename_panel` | 4 | — |
| `src/http/tree/pages.rs` | 24 | `render_tree_hub` | 4 | — |
| `src/http/tree/user_management/invitations.rs` | 107 | `render_invitation_created` | 4 | — |
| `crates/twig-highlight/src/lib.rs` | 78 | `highlight` | 3 | — |
| `crates/twig-highlight/src/lib.rs` | 92 | `highlight_path` | 3 | — |
| `src/auth/mod.rs` | 60 | `parse` | 3 | — |
| `src/auth/mod.rs` | 201 | `create_session` | 3 | — |
| `src/auth/mod.rs` | 260 | `validate_test_pin` | 3 | — |
| `src/auth/session_store.rs` | 100 | `update_ttl` | 3 | — |
| `src/db/invites.rs` | 5 | `create_invite` | 3 | — |
| `src/db/invites.rs` | 46 | `mark_invite_used` | 3 | — |
| `src/db/namespaces.rs` | 250 | `rename_namespace` | 3 | — |
| `src/db/sessions.rs` | 22 | `save_actix_session` | 3 | — |
| `src/db/sessions.rs` | 39 | `update_actix_session` | 3 | — |
| `src/db/sessions.rs` | 57 | `update_actix_session_ttl` | 3 | — |
| `src/db/sessions.rs` | 74 | `delete_actix_session` | 3 | — |
| `src/db/test_pins.rs` | 37 | `clear_test_pins` | 3 | — |
| `src/db/tokens.rs` | 4 | `create_token` | 3 | — |
| `src/db/tokens.rs` | 34 | `delete_token` | 3 | — |
| `src/db/tokens.rs` | 43 | `cleanup_expired_tokens` | 3 | — |
| `src/db/users.rs` | 77 | `create_user` | 3 | — |
| `src/db/users.rs` | 135 | `update_user_email` | 3 | — |
| `src/email.rs` | 241 | `missing_public_url_skips_delivery_without_network_request` | 3 | — |
| `src/git/backend.rs` | 156 | `parse_cgi_response` | 3 | — |
| `src/git/bare/config/tests.rs` | 142 | `test_config_parse_errors_are_reported_not_swallowed` | 3 | — |
| `src/git/bare/handle.rs` | 46 | `head_commit` | 3 | — |
| `src/git/bare/handle.rs` | 245 | `read_file` | 3 | — |
| `src/git/bare/handle.rs` | 290 | `load_config_with_raw` | 3 | — |
| `src/http/assets/tests.rs` | 126 | `test_twig_css_confines_raw_colour_values_to_the_token_block` | 3 | — |
| `src/http/assets/tests.rs` | 148 | `test_twig_css_honours_the_depth_and_motion_bans` | 3 | — |
| `src/http/assets/tests.rs` | 172 | `test_twig_css_uses_only_the_two_authoritative_breakpoints` | 3 | — |
| `src/http/assets/tests.rs` | 258 | `test_assets_handler_known_files` | 3 | — |
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
| `src/http/namespace/pages.rs` | 218 | `render_create_repo_form` | 3 | — |
| `src/http/overview/pages/tests.rs` | 269 | `test_index_uses_only_design_system_classes_and_no_inline_style` | 3 | — |
| `src/http/repository/pages/content.rs` | 184 | `render_content_row` | 3 | — |
| `src/http/repository/pages/markdown.rs` | 149 | `fix_link_tag` | 3 | — |
| `src/http/repository/pages/markdown.rs` | 186 | `markdown_to_html` | 3 | — |
| `src/http/repository/pages/markdown.rs` | 270 | `get_default_markdown_file` | 3 | — |
| `src/http/repository/pages/markdown.rs` | 289 | `render_markdown_view` | 3 | — |
| `src/http/repository/pages/paper.rs` | 121 | `paper_handler` | 3 | — |
| `src/http/repository/pages/paper.rs` | 192 | `paper_path_slug` | 3 | — |
| `src/http/repository/pages/paper.rs` | 212 | `paper_page_anchors` | 3 | — |
| `src/http/repository/pages/paper.rs` | 318 | `render_paper_toolbar` | 3 | — |
| `src/http/repository/pages/paper.rs` | 364 | `render_paper_view` | 3 | — |
| `src/http/repository/pages/presentation.rs` | 112 | `load_present_slides` | 3 | — |
| `src/http/repository/pages/scripts.rs` | 26 | `scripts_body` | 3 | — |
| `src/http/repository/pages/tabs.rs` | 116 | `resolved_default_tab` | 3 | — |
| `src/http/repository/pages/tabs.rs` | 352 | `default_tab` | 3 | — |
| `src/http/repository/pages/tabs.rs` | 362 | `render_commits_view` | 3 | — |
| `src/http/repository/pages/tests/mod.rs` | 195 | `test_repo_markup_uses_only_twig_design_system_classes` | 3 | — |
| `src/http/repository/pages/tests/presentation.rs` | 62 | `test_print_export_opens_the_dialog_once_loaded` | 3 | — |
| `src/http/repository/pages/tests/presentation.rs` | 169 | `test_presentation_controls_are_labelled_and_bounded` | 3 | — |
| `src/http/settings/pages/account.rs` | 30 | `render_profile_panel` | 3 | — |
| `src/http/settings/pages/tests.rs` | 152 | `test_settings_delete_repo_rows_preserve_the_htmx_contract` | 3 | — |
| `src/http/settings/pages/tests.rs` | 182 | `test_settings_rename_repo_rows_preserve_the_htmx_contract` | 3 | — |
| `src/http/settings/pages/tests.rs` | 335 | `test_settings_uses_only_twig_design_system_classes` | 3 | — |
| `src/http/settings/pages.rs` | 31 | `require_namespace_owner` | 3 | — |
| `src/http/test_page/pages/render.rs` | 110 | `render_pin_entry_page` | 3 | — |
| `src/http/test_page/pages.rs` | 84 | `get_base_url` | 3 | — |
| `src/http/test_page/pages.rs` | 153 | `verify_admin_only` | 3 | — |
| `src/http/test_page/pages.rs` | 238 | `test_pin_create` | 3 | — |
| `src/http/test_page/pages.rs` | 259 | `test_pin_remove` | 3 | — |
| `src/http/test_page/pages.rs` | 279 | `test_pin_qr` | 3 | — |
| `src/http/tree/api.rs` | 31 | `tree_endpoint` | 3 | — |
| `src/http/tree/pages/data.rs` | 87 | `render_more_rows` | 3 | — |
| `src/http/tree/pages.rs` | 119 | `render_namespace_rename_panel` | 3 | — |
| `src/http/tree/pages.rs` | 176 | `render_namespace_deletion_panel` | 3 | — |
| `src/http/tree/user_management/invitations.rs` | 153 | `display_invite_url` | 3 | — |
| `src/http/view.rs` | 6 | `render_layout` | 3 | — |
| `src/http/view.rs` | 292 | `test_layout_uses_only_twig_design_system_classes` | 3 | — |
| `src/integration_tests/tests/repository_binaries.rs` | 232 | `test_binary_uploads_retain_semver_releases_and_latest_resolves_by_asset` | 3 | — |
| `src/integration_tests/tests/repository_content.rs` | 88 | `test_content_rejects_nul_byte_paths` | 3 | — |
| `src/integration_tests/tests/support/repository.rs` | 168 | `init_repo_with_paper` | 3 | — |
| `src/md/mod.rs` | 86 | `is_rust_block_start` | 3 | — |
| `crates/twig-highlight/build.rs` | 3 | `main` | 2 | — |
| `crates/twig-highlight/src/lib.rs` | 112 | `grammars` | 2 | — |
| `crates/twig-highlight/src/lib.rs` | 185 | `try_from` | 2 | — |
| `crates/twig-highlight/src/tests.rs` | 108 | `custom_grammars_use_the_shared_safe_writer` | 2 | — |
| `src/auth/mod.rs` | 46 | `as_str` | 2 | — |
| `src/auth/mod.rs` | 53 | `display_name` | 2 | — |
| `src/auth/mod.rs` | 97 | `hash_password` | 2 | — |
| `src/auth/mod.rs` | 106 | `verify_password` | 2 | — |
| `src/auth/mod.rs` | 123 | `create_user` | 2 | — |
| `src/auth/mod.rs` | 175 | `base64_decode` | 2 | — |
| `src/auth/mod.rs` | 242 | `get_test_pin` | 2 | — |
| `src/auth/session_store.rs` | 42 | `load` | 2 | — |
| `src/config.rs` | 168 | `effective_api_key` | 2 | — |
| `src/db/binaries.rs` | 35 | `version_order_desc` | 2 | — |
| `src/db/binaries.rs` | 351 | `binary_database_orders_semver_and_retains_the_top_three_versions` | 2 | — |
| `src/db/namespaces.rs` | 168 | `user_owns_namespace` | 2 | — |
| `src/db/users.rs` | 148 | `merge_user_entry` | 2 | — |
| `src/email.rs` | 27 | `with_base_url` | 2 | — |
| `src/email.rs` | 164 | `spawn_fake_resend` | 2 | — |
| `src/email.rs` | 219 | `missing_api_key_skips_sender_construction_and_network_request` | 2 | — |
| `src/email.rs` | 321 | `invitation_url_requires_a_safe_origin_and_token` | 2 | — |
| `src/git/backend.rs` | 39 | `fmt` | 2 | — |
| `src/git/backend.rs` | 257 | `test_git_request_kind_advertise_refs_upload` | 2 | — |
| `src/git/backend.rs` | 266 | `test_git_request_kind_advertise_refs_receive` | 2 | — |
| `src/git/backend.rs` | 275 | `test_git_request_kind_fetch_clone` | 2 | — |
| `src/git/backend.rs` | 289 | `test_git_request_kind_push` | 2 | — |
| `src/git/backend.rs` | 303 | `test_git_request_kind_dumb_get` | 2 | — |
| `src/git/bare/config.rs` | 82 | `label` | 2 | — |
| `src/git/bare/config.rs` | 207 | `from_source` | 2 | — |
| `src/git/bare/handle.rs` | 61 | `head_oid` | 2 | — |
| `src/git/bare/mod.rs` | 24 | `has_any_repository` | 2 | — |
| `src/git/mod.rs` | 333 | `ensure_namespace_exists` | 2 | — |
| `src/git/reserved.rs` | 66 | `test_namespace_level_routes_are_reserved` | 2 | — |
| `src/git/reserved.rs` | 76 | `test_ordinary_names_are_allowed` | 2 | — |
| `src/git/reserved.rs` | 93 | `test_path_traversal_is_refused` | 2 | — |
| `src/http/assets/tests.rs` | 6 | `body_of` | 2 | — |
| `src/http/assets/tests.rs` | 13 | `css_without_comments` | 2 | — |
| `src/http/assets/tests.rs` | 26 | `css_outside_root_blocks` | 2 | — |
| `src/http/assets/tests.rs` | 46 | `test_asset_constants_are_non_empty` | 2 | — |
| `src/http/assets/tests.rs` | 90 | `test_twig_schema_lists_every_config_key` | 2 | — |
| `src/http/assets/tests.rs` | 193 | `test_twig_css_drops_the_outgoing_system` | 2 | — |
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
| `src/http/namespace/pages.rs` | 14 | `user_can_create_repositories` | 2 | — |
| `src/http/namespace/pages.rs` | 275 | `create_repo_form_handler` | 2 | — |
| `src/http/overview/pages/tests.rs` | 142 | `test_rows_keep_owner_metadata_visible_and_labelled_at_every_width` | 2 | — |
| `src/http/overview/pages/tests.rs` | 229 | `test_empty_states_name_their_condition_and_keep_their_copy` | 2 | — |
| `src/http/overview/pages.rs` | 114 | `search_repo_hits` | 2 | — |
| `src/http/repository/editor/render.rs` | 122 | `render_editor_scripts` | 2 | — |
| `src/http/repository/editor.rs` | 236 | `edit_file_page` | 2 | — |
| `src/http/repository/pages/content.rs` | 94 | `content_urls` | 2 | — |
| `src/http/repository/pages/content.rs` | 103 | `parent_path` | 2 | — |
| `src/http/repository/pages/markdown.rs` | 91 | `split_link_suffix` | 2 | — |
| `src/http/repository/pages/markdown.rs` | 249 | `render_slide_markdown` | 2 | — |
| `src/http/repository/pages/markdown.rs` | 328 | `render_markdown_content_only` | 2 | — |
| `src/http/repository/pages/paper.rs` | 78 | `page_in_dir` | 2 | — |
| `src/http/repository/pages/paper.rs` | 180 | `paper_relative_path` | 2 | — |
| `src/http/repository/pages/paper.rs` | 402 | `render_paper_content_only` | 2 | — |
| `src/http/repository/pages/presentation.rs` | 150 | `render_slide_nav_button` | 2 | — |
| `src/http/repository/pages/presentation.rs` | 260 | `render_present_view` | 2 | — |
| `src/http/repository/pages/presentation.rs` | 388 | `render_pdf_pagehead` | 2 | — |
| `src/http/repository/pages/presentation.rs` | 426 | `render_pdf_license_page` | 2 | — |
| `src/http/repository/pages/scripts.rs` | 184 | `render_script_group_tabs` | 2 | — |
| `src/http/repository/pages/scripts.rs` | 215 | `render_script_row` | 2 | — |
| `src/http/repository/pages/scripts.rs` | 321 | `raw_content_type` | 2 | — |
| `src/http/repository/pages/tabs.rs` | 22 | `commits_body` | 2 | — |
| `src/http/repository/pages/tabs.rs` | 339 | `tab_handler` | 2 | — |
| `src/http/repository/pages/tabs.rs` | 378 | `render_config_view` | 2 | — |
| `src/http/repository/pages/tabs.rs` | 404 | `render_license_view` | 2 | — |
| `src/http/repository/pages/tests/navigation.rs` | 40 | `test_tabs_preserve_every_htmx_navigation_attribute` | 2 | — |
| `src/http/repository/pages/tests/navigation.rs` | 74 | `test_tabs_mark_exactly_one_current_tab` | 2 | — |
| `src/http/repository/pages/tests/navigation.rs` | 297 | `test_tab_content_shows_a_config_error_on_every_tab` | 2 | — |
| `src/http/repository/pages/tests/presentation.rs` | 21 | `test_print_export_renders_one_page_per_slide_with_the_license_last` | 2 | — |
| `src/http/repository/pages/tests/presentation.rs` | 119 | `test_presentation_targets_its_container_and_scopes_the_keyboard` | 2 | — |
| `src/http/repository/pages.rs` | 87 | `text_size_step` | 2 | — |
| `src/http/repository/pages.rs` | 102 | `render_for_request` | 2 | — |
| `src/http/repository/pages.rs` | 243 | `render_tab_shell` | 2 | — |
| `src/http/repository/pages.rs` | 260 | `render_tab_response` | 2 | — |
| `src/http/repository/pages.rs` | 305 | `render_not_found_for_request` | 2 | — |
| `src/http/routes.rs` | 109 | `test_is_git_guard_matches_git_user_agent` | 2 | — |
| `src/http/routes.rs` | 129 | `test_is_git_guard_rejects_non_git_user_agent` | 2 | — |
| `src/http/routes.rs` | 149 | `test_is_git_guard_rejects_missing_user_agent` | 2 | — |
| `src/http/settings/pages/account.rs` | 138 | `render_settings_auth_error` | 2 | — |
| `src/http/settings/pages/tests.rs` | 47 | `test_settings_leads_with_crumbs_and_page_head` | 2 | — |
| `src/http/settings/pages/tests.rs` | 248 | `test_settings_email_form_preserves_the_htmx_contract` | 2 | — |
| `src/http/test_page/pages/render.rs` | 210 | `render_runner_content` | 2 | — |
| `src/http/test_page/pages/tests.rs` | 27 | `test_endpoint_catalog_uses_current_api_and_info_routes` | 2 | — |
| `src/http/test_page/pages.rs` | 39 | `display_name` | 2 | — |
| `src/http/test_page/pages.rs` | 47 | `username` | 2 | — |
| `src/http/test_page/pages.rs` | 65 | `generate_qr_svg` | 2 | — |
| `src/http/test_page/pages.rs` | 306 | `test_runner` | 2 | — |
| `src/http/test_page/pages.rs` | 320 | `test_stopped` | 2 | — |
| `src/http/test_page/pages.rs` | 334 | `test_ping` | 2 | — |
| `src/http/test_page/pages.rs` | 348 | `test_feature_check` | 2 | — |
| `src/http/tree/api.rs` | 17 | `msgpack_responder` | 2 | — |
| `src/http/tree/pages/data.rs` | 65 | `render_data_row` | 2 | — |
| `src/http/tree/pages/data.rs` | 98 | `require_admin` | 2 | — |
| `src/http/tree/pages.rs` | 221 | `tree_page` | 2 | — |
| `src/http/tree/user_management/invitations.rs` | 83 | `invitations_for_scope` | 2 | — |
| `src/http/tree/user_management/tests.rs` | 37 | `invite_tab_renders_htmx_form_with_contributor_default_and_expiry` | 2 | — |
| `src/http/tree/user_management/tests.rs` | 79 | `expiry_values_map_to_expected_expiration` | 2 | — |
| `src/http/tree/user_management/tests.rs` | 104 | `delivery_notices_preserve_link_and_distinguish_sent_skipped_and_failed` | 2 | — |
| `src/http/view/test_util.rs` | 19 | `index_of` | 2 | — |
| `src/http/view.rs` | 151 | `test_layout_declares_document_language_and_landmarks` | 2 | — |
| `src/http/view.rs` | 207 | `test_layout_loads_space_grotesk_and_tachyons_before_twig_css` | 2 | — |
| `src/http/view.rs` | 227 | `test_layout_offers_theme_switch_before_rendering_styles` | 2 | — |
| `src/http/view.rs` | 259 | `test_layout_links_versioned_assets` | 2 | — |
| `src/http/view.rs` | 316 | `test_layout_authenticated_nav_exposes_user_actions` | 2 | — |
| `src/http/view.rs` | 337 | `test_layout_anonymous_nav_offers_login_and_signup` | 2 | — |
| `src/http/view.rs` | 385 | `test_notices_escape_message_markup` | 2 | — |
| `src/info/mod.rs` | 103 | `list_doc_pages` | 2 | — |
| `src/info/mod.rs` | 150 | `test_repository_config_page_documents_every_section` | 2 | — |
| `src/integration_tests/tests/admin.rs` | 55 | `test_tree_hub_and_admin_data_view_are_gated_and_paginated` | 2 | — |
| `src/integration_tests/tests/repository_content.rs` | 38 | `test_content_rejects_dotdot_segments` | 2 | — |
| `src/integration_tests/tests/repository_content.rs` | 66 | `test_content_rejects_namespace_and_repo_traversal` | 2 | — |
| `src/integration_tests/tests/repository_content.rs` | 107 | `test_markdown_view_rejects_traversal` | 2 | — |
| `src/integration_tests/tests/repository_content.rs` | 151 | `test_raw_route_serves_repo_files_and_refuses_ignored_paths` | 2 | — |
| `src/integration_tests/tests/repository_editor.rs` | 317 | `repository_editor_refuses_ignored_binary_invalid_and_non_regular_files` | 2 | — |
| `src/integration_tests/tests/repository_editor.rs` | 361 | `repository_editor_rejects_empty_or_unchanged_commits_without_history_changes` | 2 | — |
| `src/integration_tests/tests/support/app.rs` | 181 | `create_test_service_with_test_user` | 2 | — |
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
| `src/auth/mod.rs` | 115 | `generate_token` | 1 | — |
| `src/auth/mod.rs` | 140 | `generate_invite` | 1 | — |
| `src/auth/mod.rs` | 144 | `create_namespace` | 1 | — |
| `src/auth/mod.rs` | 189 | `new` | 1 | — |
| `src/auth/mod.rs` | 193 | `db` | 1 | — |
| `src/auth/mod.rs` | 197 | `validate_api_key` | 1 | — |
| `src/auth/mod.rs` | 211 | `validate_token` | 1 | — |
| `src/auth/mod.rs` | 216 | `invalidate_token` | 1 | — |
| `src/auth/mod.rs` | 252 | `set_test_pin` | 1 | — |
| `src/auth/mod.rs` | 256 | `clear_test_pin` | 1 | — |
| `src/auth/session_store.rs` | 15 | `new` | 1 | — |
| `src/auth/session_store.rs` | 19 | `ttl_seconds` | 1 | — |
| `src/auth/session_store.rs` | 23 | `new_session_key` | 1 | — |
| `src/auth/session_store.rs` | 30 | `middleware` | 1 | — |
| `src/auth/session_store.rs` | 117 | `delete` | 1 | — |
| `src/auth/session_store.rs` | 130 | `sql_session_store_saves_loads_updates_and_deletes_sessions` | 1 | — |
| `src/auth/tests.rs` | 4 | `test_hash_and_verify_password` | 1 | — |
| `src/auth/tests.rs` | 11 | `test_verify_password_invalid_hash` | 1 | — |
| `src/auth/tests.rs` | 16 | `test_generate_token_is_hex` | 1 | — |
| `src/auth/tests.rs` | 23 | `test_generate_invite_is_uuid` | 1 | — |
| `src/auth/tests.rs` | 29 | `test_generate_tokens_are_unique` | 1 | — |
| `src/auth/tests.rs` | 36 | `test_create_user_fields` | 1 | — |
| `src/auth/tests.rs` | 50 | `test_create_user_password_is_hashed` | 1 | — |
| `src/auth/tests.rs` | 62 | `test_create_namespace_fields` | 1 | — |
| `src/auth/tests.rs` | 70 | `test_base64_decode_valid` | 1 | — |
| `src/auth/tests.rs` | 76 | `test_base64_decode_invalid` | 1 | — |
| `src/auth/tests.rs` | 82 | `test_base64_decode_empty` | 1 | — |
| `src/auth/tests.rs` | 88 | `test_twig_context_validate_api_key` | 1 | — |
| `src/auth/tests.rs` | 96 | `test_twig_context_test_pin` | 1 | — |
| `src/auth/tests.rs` | 118 | `test_seed_dev_admin_creates_user_namespace_and_session` | 1 | — |
| `src/auth/tests.rs` | 154 | `test_seed_dev_admin_is_idempotent` | 1 | — |
| `src/config/tests.rs` | 4 | `test_server_new` | 1 | — |
| `src/config/tests.rs` | 24 | `test_server_cache_control_round_trip` | 1 | — |
| `src/config/tests.rs` | 42 | `test_server_clone` | 1 | — |
| `src/config/tests.rs` | 57 | `test_effective_api_key_with_set_key` | 1 | — |
| `src/config/tests.rs` | 70 | `test_effective_api_key_generates_when_empty` | 1 | — |
| `src/config/tests.rs` | 85 | `test_maybe_reset_database_no_flag` | 1 | — |
| `src/config/tests.rs` | 98 | `test_address_returns_clone` | 1 | — |
| `src/config/tests.rs` | 112 | `test_server_test_user` | 1 | — |
| `src/config/tests.rs` | 133 | `test_server_admin_user_is_independent_from_test_user` | 1 | — |
| `src/config/tests.rs` | 151 | `test_blank_admin_user_is_not_configured` | 1 | — |
| `src/config/tests.rs` | 166 | `test_resend_settings_are_optional_and_keep_sender_configuration` | 1 | — |
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
| `src/db/binaries.rs` | 280 | `test_db_path` | 1 | — |
| `src/db/binaries.rs` | 284 | `version` | 1 | — |
| `src/db/binaries.rs` | 289 | `binary_database_round_trips_arbitrary_blob_bytes` | 1 | — |
| `src/db/binaries.rs` | 313 | `binary_database_replaces_and_lists_asset_metadata` | 1 | — |
| `src/db/binaries.rs` | 405 | `latest_binary_resolves_highest_release_containing_filename` | 1 | — |
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
| `src/db/invites.rs` | 65 | `setup_db` | 1 | — |
| `src/db/invites.rs` | 72 | `test_invite` | 1 | — |
| `src/db/invites.rs` | 83 | `test_create_and_get_invite` | 1 | — |
| `src/db/invites.rs` | 101 | `test_mark_invite_used` | 1 | — |
| `src/db/invites.rs` | 121 | `test_get_invite_not_found` | 1 | — |
| `src/db/migration.rs` | 328 | `test_legacy_member_roles_migrate_to_contributors` | 1 | — |
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
| `src/db/users.rs` | 164 | `setup_db` | 1 | — |
| `src/db/users.rs` | 171 | `test_user` | 1 | — |
| `src/db/users.rs` | 182 | `test_create_and_get_user_by_username` | 1 | — |
| `src/db/users.rs` | 200 | `test_get_user_by_id` | 1 | — |
| `src/db/users.rs` | 215 | `test_get_user_by_username_not_found` | 1 | — |
| `src/db/users.rs` | 229 | `test_update_user_email` | 1 | — |
| `src/email.rs` | 22 | `new` | 1 | — |
| `src/email.rs` | 38 | `send_namespace_invitation` | 1 | — |
| `src/email.rs` | 91 | `send_invitation_if_configured` | 1 | — |
| `src/email.rs` | 264 | `resend_sends_invitation_payload_to_the_configured_api` | 1 | — |
| `src/email.rs` | 297 | `resend_failure_is_reported_for_retry_and_recovery` | 1 | — |
| `src/git/backend.rs` | 13 | `new` | 1 | — |
| `src/git/backend.rs` | 19 | `project_root` | 1 | — |
| `src/git/backend.rs` | 76 | `new` | 1 | — |
| `src/git/backend.rs` | 248 | `sh` | 1 | — |
| `src/git/backend.rs` | 312 | `test_git_service_display` | 1 | — |
| `src/git/backend.rs` | 318 | `test_git_request_new` | 1 | — |
| `src/git/backend.rs` | 327 | `test_config_new` | 1 | — |
| `src/git/backend.rs` | 333 | `test_parse_cgi_response_with_headers_and_body` | 1 | — |
| `src/git/backend.rs` | 342 | `test_parse_cgi_response_with_lf_only` | 1 | — |
| `src/git/backend.rs` | 350 | `test_parse_cgi_response_no_separator` | 1 | — |
| `src/git/backend.rs` | 358 | `test_parse_cgi_response_empty_body` | 1 | — |
| `src/git/backend.rs` | 366 | `test_prepare_cgi_env_sets_vars` | 1 | — |
| `src/git/backend.rs` | 379 | `test_prepare_cgi_env_sets_the_verified_namespace_role` | 1 | — |
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
| `src/git/bare/handle/edit_tests.rs` | 7 | `new` | 1 | — |
| `src/git/bare/handle/edit_tests.rs` | 13 | `handle` | 1 | — |
| `src/git/bare/handle/edit_tests.rs` | 44 | `drop` | 1 | — |
| `src/git/bare/handle/edit_tests.rs` | 50 | `commit_file_writes_nested_content_and_preserves_other_entries_and_mode` | 1 | — |
| `src/git/bare/handle/edit_tests.rs` | 93 | `commit_file_refuses_to_overwrite_a_newer_branch_tip` | 1 | — |
| `src/git/bare/handle/edit_tests.rs` | 130 | `commit_new_file_adds_a_nested_path_and_refuses_overwrites_and_stale_heads` | 1 | — |
| `src/git/bare/handle.rs` | 31 | `from_repository` | 1 | — |
| `src/git/bare/handle.rs` | 253 | `path_exists` | 1 | — |
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
| `src/git/bare/mod.rs` | 46 | `drop` | 1 | — |
| `src/git/bare/mod.rs` | 51 | `create_temp_dir` | 1 | — |
| `src/git/bare/mod.rs` | 64 | `init_bare_repo` | 1 | — |
| `src/git/bare/mod.rs` | 77 | `commit_files` | 1 | — |
| `src/git/bare/mod.rs` | 154 | `test_init_branch_parameter` | 1 | — |
| `src/git/bare/mod.rs` | 176 | `test_config_fallback_reads_fig_toml` | 1 | — |
| `src/git/bare/mod.rs` | 192 | `test_config_fallback_reads_dot_fig_toml` | 1 | — |
| `src/git/bare/mod.rs` | 207 | `test_config_prefers_current_name_over_fig` | 1 | — |
| `src/git/bare/mod.rs` | 230 | `test_config_prefers_twig_short_name_over_fig` | 1 | — |
| `src/git/bare/mod.rs` | 252 | `test_is_repo_private` | 1 | — |
| `src/git/bare/mod.rs` | 276 | `test_list_dir_walks_head_tree` | 1 | — |
| `src/git/bare/mod.rs` | 307 | `test_list_paper_pages_reads_sorted_markdown_under_dir` | 1 | — |
| `src/git/bare/mod.rs` | 344 | `test_open_rejects_parent_directory_components` | 1 | — |
| `src/git/bare/path.rs` | 33 | `test_is_safe_repo_path` | 1 | — |
| `src/git/mod.rs` | 391 | `test_build_response_status_header` | 1 | — |
| `src/git/mod.rs` | 399 | `test_build_response_default_status` | 1 | — |
| `src/git/mod.rs` | 407 | `test_build_response_content_type` | 1 | — |
| `src/git/mod.rs` | 415 | `test_build_response_multiple_headers` | 1 | — |
| `src/git/repo/tests.rs` | 4 | `run_pre_receive` | 1 | — |
| `src/git/repo/tests.rs` | 30 | `test_default_branch_is_main` | 1 | — |
| `src/git/repo/tests.rs` | 35 | `test_create_repo_creates_directory_structure` | 1 | — |
| `src/git/repo/tests.rs` | 52 | `test_bare_init_creates_valid_repository` | 1 | — |
| `src/git/repo/tests.rs` | 88 | `contributor_hook_allows_fast_forwards_and_rejects_main_rewrites_and_deletions` | 1 | — |
| `src/git/repo/tests.rs` | 161 | `receive_hook_installation_preserves_chains_and_is_idempotent` | 1 | — |
| `src/git/repo.rs` | 21 | `default_branch` | 1 | — |
| `src/git/repo.rs` | 324 | `sh` | 1 | — |
| `src/git/reserved.rs` | 25 | `is_reserved_repo_name` | 1 | — |
| `src/git/reserved.rs` | 59 | `test_reserved_check_is_case_insensitive` | 1 | — |
| `src/git/reserved.rs` | 87 | `test_empty_name_is_refused` | 1 | — |
| `src/git/reserved.rs` | 103 | `test_dotfile_names_are_refused` | 1 | — |
| `src/http/api.rs` | 11 | `version_endpoint` | 1 | — |
| `src/http/api.rs` | 23 | `test_version_endpoint_returns_package_version` | 1 | — |
| `src/http/assets/tests.rs` | 41 | `starts_a_hex_colour` | 1 | — |
| `src/http/assets/tests.rs` | 70 | `test_urls_carry_the_version` | 1 | — |
| `src/http/assets/tests.rs` | 110 | `test_twig_schema_describes_nested_script_groups` | 1 | — |
| `src/http/assets/tests.rs` | 202 | `test_versioned_assets_handler_uses_configured_cache_control` | 1 | — |
| `src/http/assets/tests.rs` | 229 | `test_unversioned_assets_must_revalidate` | 1 | — |
| `src/http/assets/tests.rs` | 319 | `test_assets_handler_unknown_file_returns_404` | 1 | — |
| `src/http/assets/tests.rs` | 342 | `test_assets_handler_rejects_another_version` | 1 | — |
| `src/http/assets.rs` | 23 | `name` | 1 | — |
| `src/http/assets.rs` | 27 | `versioned_name` | 1 | — |
| `src/http/auth/handlers/tests.rs` | 5 | `test_generate_token_format` | 1 | — |
| `src/http/auth/handlers/tests.rs` | 14 | `test_invite_struct_with_none_user_id` | 1 | — |
| `src/http/auth/handlers/tests.rs` | 28 | `test_invite_struct_with_some_user_id` | 1 | — |
| `src/http/auth/handlers/tests.rs` | 42 | `test_create_invite_with_null_user_id` | 1 | — |
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
| `src/http/namespace/pages.rs` | 379 | `missing_email_response` | 1 | — |
| `src/http/overview/pages/tests.rs` | 4 | `namespace` | 1 | — |
| `src/http/overview/pages/tests.rs` | 14 | `render` | 1 | — |
| `src/http/overview/pages/tests.rs` | 22 | `render_with_repo_hits` | 1 | — |
| `src/http/overview/pages/tests.rs` | 32 | `test_search_query_deserialization` | 1 | — |
| `src/http/overview/pages/tests.rs` | 38 | `test_search_query_empty` | 1 | — |
| `src/http/overview/pages/tests.rs` | 44 | `test_head_carries_the_single_create_action_for_signed_in_users` | 1 | — |
| `src/http/overview/pages/tests.rs` | 61 | `test_create_action_stays_hidden_without_a_session` | 1 | — |
| `src/http/overview/pages/tests.rs` | 107 | `test_clear_returns_to_the_unfiltered_index_only_while_filtering` | 1 | — |
| `src/http/overview/pages/tests.rs` | 121 | `test_namespace_list_is_a_flush_technical_panel` | 1 | — |
| `src/http/overview/pages/tests.rs` | 174 | `test_rows_keep_their_namespace_hrefs` | 1 | — |
| `src/http/overview/pages/tests.rs` | 185 | `test_repo_hits_render_the_namespace_beneath_the_repository` | 1 | — |
| `src/http/overview/pages/tests.rs` | 218 | `test_repo_hits_respect_the_private_filter_and_missing_roots` | 1 | — |
| `src/http/repository/binaries.rs` | 179 | `parse_release_version` | 1 | — |
| `src/http/repository/binaries.rs` | 194 | `is_binary_size_allowed` | 1 | — |
| `src/http/repository/binaries.rs` | 198 | `binary_response` | 1 | — |
| `src/http/repository/binaries.rs` | 214 | `binary_versions_accept_optional_v_and_reject_latest_for_upload` | 1 | — |
| `src/http/repository/binaries.rs` | 224 | `binary_filename_and_size_validation_reject_unsafe_inputs` | 1 | — |
| `src/http/repository/editor/render.rs` | 108 | `render_conflict_actions` | 1 | — |
| `src/http/repository/editor/tests.rs` | 4 | `editor_page_includes_source_and_commit_precondition` | 1 | — |
| `src/http/repository/editor/tests.rs` | 37 | `source_page_does_not_offer_rich_controls` | 1 | — |
| `src/http/repository/editor/tests.rs` | 55 | `file_paths_are_encoded_without_losing_directory_separators` | 1 | — |
| `src/http/repository/editor/tests.rs` | 64 | `conflict_copy_names_include_a_safe_user_timestamp_and_collision_suffix` | 1 | — |
| `src/http/repository/editor/tests.rs` | 79 | `conflict_copy_filename_truncation_preserves_utf8_and_component_limit` | 1 | — |
| `src/http/repository/pages/content.rs` | 22 | `content_body` | 1 | — |
| `src/http/repository/pages/markdown.rs` | 18 | `markdown_body` | 1 | — |
| `src/http/repository/pages/markdown.rs` | 22 | `markdown_body_from` | 1 | — |
| `src/http/repository/pages/markdown.rs` | 178 | `markdown_options` | 1 | — |
| `src/http/repository/pages/paper.rs` | 67 | `paper_body` | 1 | — |
| `src/http/repository/pages/paper.rs` | 153 | `load_paper_pages` | 1 | — |
| `src/http/repository/pages/paper.rs` | 163 | `render_paper_page` | 1 | — |
| `src/http/repository/pages/paper.rs` | 206 | `paper_page_anchor` | 1 | — |
| `src/http/repository/pages/presentation.rs` | 41 | `present_body` | 1 | — |
| `src/http/repository/pages/presentation.rs` | 137 | `slide_counter` | 1 | — |
| `src/http/repository/pages/presentation.rs` | 402 | `render_pdf_slide_page` | 1 | — |
| `src/http/repository/pages/scripts.rs` | 47 | `scripts_group_handler` | 1 | — |
| `src/http/repository/pages/scripts.rs` | 206 | `script_command` | 1 | — |
| `src/http/repository/pages/scripts.rs` | 243 | `render_script_group_row` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 15 | `markdown_files` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 27 | `config_body` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 34 | `license_body` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 185 | `markdown_tab_handler` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 204 | `content_tab_handler` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 223 | `commits_tab_handler` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 242 | `config_tab_handler` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 261 | `present_tab_handler` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 280 | `paper_tab_handler` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 299 | `license_tab_handler` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 318 | `scripts_tab_handler` | 1 | — |
| `src/http/repository/pages/tabs.rs` | 418 | `render_commit` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 4 | `test_parent_path` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 12 | `test_breadcrumb_segments` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 27 | `test_content_rows_mark_directories_with_a_trailing_slash` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 55 | `test_content_dir_offers_a_parent_row_below_the_root` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 69 | `test_content_breadcrumbs_mark_the_final_segment_as_current` | 1 | — |
| `src/http/repository/pages/tests/content.rs` | 97 | `test_content_view_reports_missing_paths_and_binary_files` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 4 | `test_markdown_to_html` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 18 | `test_markdown_link_fixing` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 28 | `test_markdown_links_resolve_against_containing_directory` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 40 | `test_markdown_non_markdown_links_target_content_route` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 48 | `test_markdown_link_preserves_fragment_and_query` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 59 | `test_markdown_link_leaves_absolute_and_scheme_links_alone` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 68 | `test_slide_markdown_anchors_relative_links_to_the_slide_directory` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 80 | `test_slide_markdown_substitutes_template_variables` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 94 | `test_resolve_relative_path` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 104 | `test_has_url_scheme` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 114 | `test_get_default_markdown_file` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 133 | `test_markdown_tables_rendering` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 146 | `test_markdown_rail_appears_only_for_multiple_files` | 1 | — |
| `src/http/repository/pages/tests/markdown.rs` | 175 | `test_markdown_content_reports_a_missing_file` | 1 | — |
| `src/http/repository/pages/tests/mod.rs` | 13 | `base_frame` | 1 | — |
| `src/http/repository/pages/tests/mod.rs` | 27 | `frame_with` | 1 | — |
| `src/http/repository/pages/tests/mod.rs` | 34 | `slides` | 1 | — |
| `src/http/repository/pages/tests/mod.rs` | 42 | `entry` | 1 | — |
| `src/http/repository/pages/tests/mod.rs` | 50 | `count_of` | 1 | — |
| `src/http/repository/pages/tests/mod.rs` | 54 | `tab_labels` | 1 | — |
| `src/http/repository/pages/tests/mod.rs` | 67 | `representative_markup` | 1 | — |
| `src/http/repository/pages/tests/mod.rs` | 161 | `test_git_error_is_an_announced_danger_notice` | 1 | — |
| `src/http/repository/pages/tests/mod.rs` | 179 | `test_render_repo_auth_error` | 1 | — |
| `src/http/repository/pages/tests/navigation.rs` | 4 | `test_default_tab_prefers_configuration_then_markdown` | 1 | — |
| `src/http/repository/pages/tests/navigation.rs` | 13 | `test_tabs_are_an_out_of_band_labelled_nav` | 1 | — |
| `src/http/repository/pages/tests/navigation.rs` | 101 | `test_tabs_render_only_available_views_in_order` | 1 | — |
| `src/http/repository/pages/tests/navigation.rs` | 154 | `test_repo_page_leads_with_a_breadcrumb_head_then_the_tabs` | 1 | — |
| `src/http/repository/pages/tests/navigation.rs` | 175 | `test_repo_page_announces_its_tab_panel` | 1 | — |
| `src/http/repository/pages/tests/navigation.rs` | 184 | `test_tab_response_differs_for_htmx_and_direct_navigation` | 1 | — |
| `src/http/repository/pages/tests/navigation.rs` | 219 | `test_commit_row_is_a_dense_machine_record` | 1 | — |
| `src/http/repository/pages/tests/navigation.rs` | 258 | `test_commits_view_falls_back_to_an_empty_state` | 1 | — |
| `src/http/repository/pages/tests/navigation.rs` | 266 | `test_config_error_is_a_diagnostic_with_a_help_hint` | 1 | — |
| `src/http/repository/pages/tests/navigation.rs` | 319 | `test_config_and_license_views_render_code_and_empty_states` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 77 | `test_paper_headings_slug_anchor_and_dedupe` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 102 | `test_render_paper_markdown_puts_heading_ids_on_the_headings` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 119 | `test_render_paper_content_only_anchors_headings` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 138 | `test_paper_page_anchors_are_path_relative_and_unique` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 161 | `test_paper_view_without_pages_states_the_condition` | 1 | — |
| `src/http/repository/pages/tests/paper.rs` | 172 | `test_page_in_dir_accepts_only_paths_below_the_paper_directory` | 1 | — |
| `src/http/repository/pages/tests/presentation.rs` | 4 | `test_presentation_offers_the_pdf_export` | 1 | — |
| `src/http/repository/pages/tests/presentation.rs` | 233 | `test_slide_counter_pads_to_the_width_of_the_deck` | 1 | — |
| `src/http/repository/pages/tests/presentation.rs` | 240 | `test_present_view_without_slides_states_the_condition` | 1 | — |
| `src/http/repository/pages/tests/scripts.rs` | 4 | `test_base_url_schemes_follow_the_request` | 1 | — |
| `src/http/repository/pages/tests/scripts.rs` | 33 | `test_raw_content_type_is_text_for_scripts_and_neutral_otherwise` | 1 | — |
| `src/http/repository/pages.rs` | 66 | `text_size_persist` | 1 | — |
| `src/http/repository/pages.rs` | 70 | `text_size_list` | 1 | — |
| `src/http/repository/pages.rs` | 80 | `text_size_restore` | 1 | — |
| `src/http/repository/pages.rs` | 115 | `render_repo_auth_error` | 1 | — |
| `src/http/repository/pages.rs` | 166 | `frame` | 1 | — |
| `src/http/repository/pages.rs` | 238 | `tab_nav` | 1 | — |
| `src/http/repository/pages.rs` | 279 | `render_empty` | 1 | — |
| `src/http/repository/pages.rs` | 288 | `render_git_error` | 1 | — |
| `src/http/repository/pages.rs` | 320 | `render_repo_crumbs` | 1 | — |
| `src/http/repository/pages.rs` | 412 | `render_config_error` | 1 | — |
| `src/http/routes.rs` | 11 | `configure_routes` | 1 | — |
| `src/http/routes.rs` | 93 | `is_git` | 1 | — |
| `src/http/settings/pages/account.rs` | 12 | `render_settings` | 1 | — |
| `src/http/settings/pages/tests.rs` | 4 | `user_fixture` | 1 | — |
| `src/http/settings/pages/tests.rs` | 14 | `repo_fixture` | 1 | — |
| `src/http/settings/pages/tests.rs` | 22 | `populated_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 26 | `empty_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 30 | `repo_panel_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 34 | `empty_repo_panel_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 38 | `rename_panel_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 42 | `empty_rename_panel_html` | 1 | — |
| `src/http/settings/pages/tests.rs` | 70 | `test_account_page_renders_account_panel_only` | 1 | — |
| `src/http/settings/pages/tests.rs` | 111 | `test_settings_does_not_render_repository_moves` | 1 | — |
| `src/http/settings/pages/tests.rs` | 119 | `test_settings_deletion_panels_carry_danger_and_caution_affordances` | 1 | — |
| `src/http/settings/pages/tests.rs` | 228 | `test_settings_rename_panel_empty_state_offers_no_targets` | 1 | — |
| `src/http/settings/pages/tests.rs` | 241 | `test_settings_does_not_render_namespace_deletion_controls` | 1 | — |
| `src/http/settings/pages/tests.rs` | 279 | `test_settings_result_targets_announce_swaps` | 1 | — |
| `src/http/settings/pages/tests.rs` | 288 | `test_settings_keeps_identity_visible_and_email_break_safe` | 1 | — |
| `src/http/settings/pages/tests.rs` | 312 | `test_settings_empty_states_name_their_condition_and_offer_no_targets` | 1 | — |
| `src/http/settings/pages/tests.rs` | 360 | `test_namespace_form_deserialization` | 1 | — |
| `src/http/settings/pages/tests.rs` | 366 | `test_update_email_form_deserialization` | 1 | — |
| `src/http/settings/pages/tests.rs` | 372 | `test_delete_repo_form_deserialization` | 1 | — |
| `src/http/settings/pages/tests.rs` | 379 | `test_move_repo_form_deserialization` | 1 | — |
| `src/http/settings/pages/tests.rs` | 390 | `test_rename_repo_form_deserialization` | 1 | — |
| `src/http/settings/pages/tests.rs` | 399 | `test_rename_namespace_form_deserialization` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 3 | `core_system_endpoints` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 26 | `docs_endpoints` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 67 | `asset_endpoints` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 96 | `ui_endpoints` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 113 | `feature_endpoints` | 1 | — |
| `src/http/test_page/pages/catalog.rs` | 130 | `list_test_endpoints` | 1 | — |
| `src/http/test_page/pages/render.rs` | 3 | `render_pagehead` | 1 | — |
| `src/http/test_page/pages/render.rs` | 155 | `render_test_page` | 1 | — |
| `src/http/test_page/pages/render.rs` | 177 | `render_idle_content` | 1 | — |
| `src/http/test_page/pages/render.rs` | 194 | `render_stopped_content` | 1 | — |
| `src/http/test_page/pages/tests.rs` | 4 | `test_render_stopped_content` | 1 | — |
| `src/http/test_page/pages/tests.rs` | 11 | `test_render_test_page_is_idle_until_started` | 1 | — |
| `src/http/test_page/pages.rs` | 34 | `is_admin` | 1 | — |
| `src/http/test_page/pages.rs` | 56 | `generate_random_pin` | 1 | — |
| `src/http/test_page/pages.rs` | 182 | `is_hx_request` | 1 | — |
| `src/http/test_page/pages.rs` | 228 | `test_page_alias` | 1 | — |
| `src/http/tree/api.rs` | 77 | `test_config` | 1 | — |
| `src/http/tree/api.rs` | 89 | `test_tree_round_trips_as_named_messagepack` | 1 | — |
| `src/http/tree/api.rs` | 104 | `test_tree_endpoint_returns_namespaces_and_repositories` | 1 | — |
| `src/http/tree/api.rs` | 158 | `test_tree_endpoint_filters_private_repositories` | 1 | — |
| `src/http/tree/pages/data.rs` | 75 | `render_load_more` | 1 | — |
| `src/http/tree/pages/tests.rs` | 4 | `tree_hub_only_renders_enabled_sections` | 1 | — |
| `src/http/tree/pages/tests.rs` | 28 | `namespace_page_lists_repositories_with_confirmed_namespace_buttons` | 1 | — |
| `src/http/tree/pages/tests.rs` | 62 | `namespace_page_explains_when_no_namespace_can_be_deleted` | 1 | — |
| `src/http/tree/pages/tests.rs` | 75 | `namespace_page_lists_owned_namespaces_for_rename_with_the_htmx_contract` | 1 | — |
| `src/http/tree/pages/tests.rs` | 106 | `namespace_rename_panel_empty_state_offers_no_targets` | 1 | — |
| `src/http/tree/pages/tests.rs` | 117 | `data_page_uses_the_infinite_scroll_placeholder` | 1 | — |
| `src/http/tree/user_management/invitations.rs` | 68 | `is_expired` | 1 | — |
| `src/http/tree/user_management/invitations.rs` | 77 | `format_expiry` | 1 | — |
| `src/http/tree/user_management/invitations.rs` | 171 | `deliver_invitation` | 1 | — |
| `src/http/tree/user_management/tests.rs` | 3 | `scope` | 1 | — |
| `src/http/tree/user_management/tests.rs` | 22 | `invitation` | 1 | — |
| `src/http/tree/user_management/tests.rs` | 55 | `users_tab_is_reachable_with_htmx_and_renders_scoped_placeholder` | 1 | — |
| `src/http/tree/user_management/tests.rs` | 62 | `user_memberships_render_in_a_single_table_cell` | 1 | — |
| `src/http/tree/user_management/tests.rs` | 94 | `pending_invitation_lists_have_an_htmx_resend_action` | 1 | — |
| `src/http/tree/user_management.rs` | 35 | `default_invitation_role` | 1 | — |
| `src/http/view/test_util.rs` | 4 | `classes_in` | 1 | — |
| `src/http/view.rs` | 92 | `render_theme_toggle` | 1 | — |
| `src/http/view.rs` | 110 | `render_error` | 1 | — |
| `src/http/view.rs` | 119 | `render_error_with_action` | 1 | — |
| `src/http/view.rs` | 131 | `render_success` | 1 | — |
| `src/http/view.rs` | 145 | `layout_html` | 1 | — |
| `src/http/view.rs` | 173 | `test_layout_prefixes_named_page_title` | 1 | — |
| `src/http/view.rs` | 181 | `test_layout_escapes_named_page_title` | 1 | — |
| `src/http/view.rs` | 193 | `test_layout_skip_link_is_the_first_focusable_element` | 1 | — |
| `src/http/view.rs` | 241 | `test_layout_preserves_htmx_wiring` | 1 | — |
| `src/http/view.rs` | 279 | `test_layout_keeps_all_styling_in_stylesheets` | 1 | — |
| `src/http/view.rs` | 307 | `test_layout_optic_rule_is_decorative` | 1 | — |
| `src/http/view.rs` | 349 | `test_render_error_is_an_announced_danger_notice` | 1 | — |
| `src/http/view.rs` | 361 | `test_render_error_with_action_uses_a_real_recovery_link` | 1 | — |
| `src/http/view.rs` | 373 | `test_render_success_is_an_announced_status_notice` | 1 | — |
| `src/info/mod.rs` | 33 | `render_page_html` | 1 | — |
| `src/info/mod.rs` | 115 | `load_doc_page` | 1 | — |
| `src/info/mod.rs` | 127 | `test_about_page_has_content` | 1 | — |
| `src/info/mod.rs` | 133 | `test_list_doc_pages_sorted_by_order` | 1 | — |
| `src/info/mod.rs` | 143 | `test_environment_variables_page_is_grouped_under_self_hosting` | 1 | — |
| `src/info/mod.rs` | 170 | `test_load_doc_page_unknown_slug_is_none` | 1 | — |
| `src/info/mod.rs` | 175 | `test_render_page_html_produces_html` | 1 | — |
| `src/integration_tests/tests/admin.rs` | 4 | `test_user_management_renders_admin_page_and_htmx_tabs` | 1 | — |
| `src/integration_tests/tests/admin.rs` | 225 | `test_settings_page_without_login_is_an_htmx_recovery_fragment` | 1 | — |
| `src/integration_tests/tests/admin.rs` | 241 | `test_settings_page_with_expired_session_is_a_full_recovery_page` | 1 | — |
| `src/integration_tests/tests/admin.rs` | 259 | `test_settings_page_without_login` | 1 | — |
| `src/integration_tests/tests/admin.rs` | 273 | `test_update_email_invalid_format` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 4 | `test_health_endpoint` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 12 | `test_invite_page_loads` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 25 | `test_signup_page_loads` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 38 | `test_created_invitation_completes_account_and_membership_setup` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 169 | `namespace_owner_can_invite_only_to_owned_namespace` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 208 | `expired_namespace_invitation_link_returns_gone` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 239 | `test_login_page_loads` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 252 | `test_namespace_page_loads` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 264 | `test_generate_invite_with_invalid_api_key` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 278 | `test_generate_invite_with_valid_api_key` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 292 | `test_signup_with_invalid_invite` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 311 | `test_login_with_wrong_password` | 1 | — |
| `src/integration_tests/tests/auth.rs` | 325 | `test_create_namespace_without_session` | 1 | — |
| `src/integration_tests/tests/namespace.rs` | 4 | `contributor_cannot_create_namespaces_or_repositories_but_can_access_existing_repo` | 1 | — |
| `src/integration_tests/tests/namespace.rs` | 159 | `test_move_repo_between_owned_namespaces` | 1 | — |
| `src/integration_tests/tests/namespace.rs` | 189 | `test_move_repo_rejects_namespace_owned_by_another_user` | 1 | — |
| `src/integration_tests/tests/namespace.rs` | 223 | `test_rename_repo_within_owned_namespace` | 1 | — |
| `src/integration_tests/tests/namespace.rs` | 252 | `test_rename_repo_rejects_foreign_namespace_and_existing_name` | 1 | — |
| `src/integration_tests/tests/namespace.rs` | 298 | `test_rename_namespace_moves_directory_and_record` | 1 | — |
| `src/integration_tests/tests/namespace.rs` | 333 | `test_rename_namespace_rejects_foreign_owner_and_existing_name` | 1 | — |
| `src/integration_tests/tests/namespace.rs` | 366 | `test_cannot_create_namespace_with_reserved_prefix` | 1 | — |
| `src/integration_tests/tests/repository_auth.rs` | 4 | `test_private_repository_ui_visibility_gated_on_login` | 1 | — |
| `src/integration_tests/tests/repository_auth.rs` | 88 | `test_private_repository_git_read_gated_behind_auth` | 1 | — |
| `src/integration_tests/tests/repository_binaries.rs` | 4 | `test_http_binary_upload_and_download_work_for_public_and_private_repos` | 1 | — |
| `src/integration_tests/tests/repository_content.rs` | 4 | `test_content_tab_lists_repo_files` | 1 | — |
| `src/integration_tests/tests/repository_content.rs` | 19 | `test_content_file_view_serves_repo_files_only` | 1 | — |
| `src/integration_tests/tests/repository_content.rs` | 125 | `test_content_cannot_reach_sibling_namespace_via_dots` | 1 | — |
| `src/integration_tests/tests/repository_editor.rs` | 4 | `repository_editor_requires_access_and_commits_with_compare_and_swap` | 1 | — |
| `src/integration_tests/tests/repository_editor.rs` | 94 | `repository_editor_denies_authenticated_users_without_namespace_access` | 1 | — |
| `src/integration_tests/tests/repository_editor.rs` | 114 | `repository_editor_edits_non_markdown_files_as_exact_source_text` | 1 | — |
| `src/integration_tests/tests/repository_editor.rs` | 155 | `repository_editor_saves_conflicted_draft_as_prefixed_file_on_latest_head` | 1 | — |
| `src/integration_tests/tests/repository_tabs.rs` | 4 | `test_paper_tab_renders_lazily_loaded_pages` | 1 | — |
| `src/integration_tests/tests/repository_tabs.rs` | 52 | `test_scripts_tab_is_hidden_until_configured` | 1 | — |
| `src/integration_tests/tests/repository_tabs.rs` | 75 | `test_scripts_tab_shows_groups_with_curl_commands` | 1 | — |
| `src/integration_tests/tests/repository_tabs.rs` | 140 | `test_legacy_tab_url_redirects_to_the_tab_route` | 1 | — |
| `src/integration_tests/tests/repository_tabs.rs` | 178 | `test_invalid_twig_toml_shows_a_diagnostic` | 1 | — |
| `src/integration_tests/tests/support/app.rs` | 3 | `create_test_service` | 1 | — |
| `src/integration_tests/tests/support/app.rs` | 14 | `create_test_service_in` | 1 | — |
| `src/integration_tests/tests/support/app.rs` | 24 | `create_test_service_with_db_in` | 1 | — |
| `src/integration_tests/tests/support/app.rs` | 71 | `create_admin_test_service` | 1 | — |
| `src/integration_tests/tests/support/auth.rs` | 4 | `extract_invite` | 1 | — |
| `src/integration_tests/tests/support/auth.rs` | 12 | `signup_and_login` | 1 | — |
| `src/integration_tests/tests/support/auth.rs` | 62 | `create_namespace` | 1 | — |
| `src/integration_tests/tests/support/auth.rs` | 85 | `basic_auth` | 1 | — |
| `src/integration_tests/tests/support/repository.rs` | 3 | `init_text_repo` | 1 | — |
| `src/integration_tests/tests/support/repository.rs` | 60 | `drop` | 1 | — |
| `src/integration_tests/tests/support/repository.rs` | 69 | `setup_traversal_fixture` | 1 | — |
| `src/integration_tests/tests/support/repository.rs` | 96 | `create_traversal_service` | 1 | — |
| `src/integration_tests/tests/support/repository.rs` | 130 | `body_of` | 1 | — |
| `src/integration_tests/tests/support/repository.rs` | 145 | `init_private_repo` | 1 | — |
| `src/integration_tests/tests/support/repository.rs` | 218 | `init_repo_with_files` | 1 | — |
| `src/integration_tests/tests/support/repository.rs` | 295 | `get_body` | 1 | — |
| `src/integration_tests/tests/test_page.rs` | 4 | `test_test_page_404_when_test_user_unset` | 1 | — |
| `src/integration_tests/tests/test_page.rs` | 12 | `test_test_page_forbidden_when_unauthenticated` | 1 | — |
| `src/integration_tests/tests/test_page.rs` | 20 | `test_test_page_and_rapid_runner_when_authenticated_as_admin` | 1 | — |
| `src/integration_tests/tests/test_page.rs` | 92 | `test_test_page_pin_and_qr_code_multi_user_lifecycle` | 1 | — |
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
| `src/git/bare/mod.rs` | 156 | `test_init_branch_parameter` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 178 | `test_config_fallback_reads_fig_toml` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 194 | `test_config_fallback_reads_dot_fig_toml` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 209 | `test_config_prefers_current_name_over_fig` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 232 | `test_config_prefers_twig_short_name_over_fig` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 254 | `test_is_repo_private` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 278 | `test_list_dir_walks_head_tree` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 309 | `test_list_paper_pages_reads_sorted_markdown_under_dir` | `_cleanup` | 1 | Likely unused |
| `src/git/bare/mod.rs` | 346 | `test_open_rejects_parent_directory_components` | `_cleanup` | 1 | Likely unused |
| `src/http/auth/handlers.rs` | 74 | `create_invite_ui_handler` | `_req` | 1 | Likely unused |
| `src/http/auth/handlers.rs` | 114 | `signup_ui_handler` | `_req` | 1 | Likely unused |
| `src/http/auth/pages.rs` | 56 | `wrap_auth_content` | `_title` | 1 | Likely unused |
| `src/http/settings/pages/repositories.rs` | 237 | `move_repo` | `_namespace` | 1 | Likely unused |
| `src/http/settings/pages/repositories.rs` | 334 | `rename_repo` | `_namespace` | 1 | Likely unused |
| `src/http/tree/api.rs` | 172 | `test_tree_endpoint_filters_private_repositories` | `_p` | 1 | Likely unused |
| `src/integration_tests/tests/support/repository.rs` | 149 | `init_private_repo` | `_p` | 1 | Likely unused |
| `src/integration_tests/tests/support/repository.rs` | 178 | `init_repo_with_paper` | `_p` | 1 | Likely unused |
| `src/integration_tests/tests/support/repository.rs` | 259 | `init_repo_with_files` | `_p` | 1 | Likely unused |

> Underscore-prefixed names are lexical candidates, not compiler diagnostics. A name seen once in its function is marked likely unused; shadowing, macros, and complex Rust patterns can affect the result.
