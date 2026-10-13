# Margaret Project Complexity Report

Audit date (UTC): 2026-10-13

## Project summary

- Rust source files discovered: 63
- Files measured: 63
- Files skipped with read warnings: 0
- Total source size: 982627 bytes
- Total physical lines: 27520
- Functions analyzed: 1043
- Average estimated function complexity: 2.6
- Maximum estimated function complexity: 24
- Complexity review hotspots (>= 10): 48
- Large-file review hotspots (> 500 physical lines): 16
- Allowed/expected lint attributes: 8
- Likely unused underscore-prefixed bindings: 18

## Review thresholds

- Function hotspot: estimated complexity of 10 or higher.
- Large file: more than 500 physical lines.

## Rust source file inventory

| File | Bytes | Physical lines | Review status |
| --- | ---: | ---: | --- |
| `src/http/repository/pages.rs` | 143734 | 4033 | Large file |
| `src/integration_tests.rs` | 142475 | 3500 | Large file |
| `src/http/settings/pages.rs` | 44237 | 1239 | Large file |
| `src/git/bare/handle.rs` | 36300 | 1035 | Large file |
| `src/http/repo_edit/mod.rs` | 31818 | 992 | Large file |
| `src/http/tree/user_management.rs` | 31490 | 849 | Large file |
| `src/http/namespace/pages.rs` | 30739 | 844 | Large file |
| `src/http/tree/pages.rs` | 31328 | 795 | Large file |
| `src/http/test_page/pages.rs` | 26030 | 778 | Large file |
| `src/http/auth/pages.rs` | 25589 | 738 | Large file |
| `src/git/repo.rs` | 21406 | 615 | Large file |
| `src/db/namespaces.rs` | 20262 | 615 | Large file |
| `crates/twig-highlight/src/lib.rs` | 19737 | 586 | Large file |
| `src/git/bare/config.rs` | 19311 | 566 | Large file |
| `src/db/invitations.rs` | 18801 | 536 | Large file |
| `src/http/assets.rs` | 17428 | 522 | Large file |
| `src/http/overview/pages.rs` | 17804 | 488 | — |
| `src/auth/mod.rs` | 14295 | 485 | — |
| `src/config.rs` | 14760 | 470 | — |
| `src/git/mod.rs` | 16257 | 458 | — |
| `src/db/binaries.rs` | 14911 | 445 | — |
| `src/http/auth/handlers.rs` | 14265 | 443 | — |
| `src/http/info/pages.rs` | 15130 | 440 | — |
| `src/http/repo_edit/pages.rs` | 19215 | 436 | — |
| `src/http/view.rs` | 17426 | 396 | — |
| `src/git/backend.rs` | 12539 | 392 | — |
| `src/db/migration.rs` | 13618 | 390 | — |
| `src/git/bare/mod.rs` | 12851 | 359 | — |
| `src/email.rs` | 12342 | 350 | — |
| `src/db/users.rs` | 11332 | 323 | — |
| `src/git/bare/listing.rs` | 11226 | 291 | — |
| `src/http/repository/binaries.rs` | 8943 | 270 | — |
| `src/md/mod.rs` | 9145 | 266 | — |
| `src/http/auth/namespace_invitations.rs` | 10400 | 260 | — |
| `src/http/tree/api.rs` | 9485 | 247 | — |
| `src/auth/permissions.rs` | 6808 | 221 | — |
| `src/db/data.rs` | 6514 | 183 | — |
| `src/info/mod.rs` | 5453 | 180 | — |
| `src/http/routes.rs` | 7265 | 179 | — |
| `src/auth/session_store.rs` | 4878 | 157 | — |
| `src/main.rs` | 5142 | 148 | — |
| `src/db/tokens.rs` | 4281 | 140 | — |
| `src/db/invites.rs` | 4043 | 133 | — |
| `src/db/mod.rs` | 3282 | 116 | — |
| `src/git/reserved.rs` | 3116 | 107 | — |
| `src/db/test_pins.rs` | 3157 | 101 | — |
| `src/db/sessions.rs` | 2564 | 85 | — |
| `src/git/bare/history.rs` | 1666 | 67 | — |
| `src/git/bare/path.rs` | 1745 | 52 | — |
| `src/http/auth/session.rs` | 1469 | 43 | — |
| `crates/twig-highlight/build.rs` | 1339 | 38 | — |
| `src/http/api.rs` | 1012 | 35 | — |
| `src/http/health.rs` | 959 | 33 | — |
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
| `src/git/bare/handle.rs` | 302 | `apply_repo_edit` | 24 | Review hotspot |
| `src/db/invitations.rs` | 11 | `accept_invitation_in_transaction` | 23 | Review hotspot |
| `src/git/bare/handle.rs` | 205 | `commit_changes` | 23 | Review hotspot |
| `src/http/repo_edit/mod.rs` | 816 | `validate_draft` | 22 | Review hotspot |
| `src/db/migration.rs` | 156 | `create_tables` | 21 | Review hotspot |
| `src/git/bare/handle.rs` | 83 | `commit_file` | 20 | Review hotspot |
| `src/http/repository/pages.rs` | 1386 | `render_tabs` | 20 | Review hotspot |
| `src/git/bare/handle.rs` | 149 | `commit_new_file` | 19 | Review hotspot |
| `src/http/repo_edit/mod.rs` | 639 | `rename_workspace_file` | 19 | Review hotspot |
| `src/http/auth/namespace_invitations.rs` | 139 | `accept_invitation_handler` | 18 | Review hotspot |
| `src/git/repo.rs` | 234 | `install_pre_receive_hook` | 17 | Review hotspot |
| `src/http/settings/pages.rs` | 644 | `rename_namespace` | 17 | Review hotspot |
| `crates/twig-highlight/src/lib.rs` | 285 | `try_from` | 16 | Review hotspot |
| `src/db/invitations.rs` | 212 | `list_namespace_invitations_for` | 16 | Review hotspot |
| `src/db/binaries.rs` | 201 | `store_binary_in_transaction` | 15 | Review hotspot |
| `src/db/invitations.rs` | 129 | `get_namespace_invitation` | 15 | Review hotspot |
| `src/git/mod.rs` | 139 | `authenticate_git_request` | 15 | Review hotspot |
| `src/http/settings/pages.rs` | 444 | `move_repo` | 15 | Review hotspot |
| `src/db/data.rs` | 37 | `read_table_page` | 14 | Review hotspot |
| `src/http/settings/pages.rs` | 548 | `rename_repo` | 14 | Review hotspot |
| `src/http/auth/handlers.rs` | 113 | `signup_ui_handler` | 13 | Review hotspot |
| `src/db/users.rs` | 33 | `list_users_for_management_in` | 12 | Review hotspot |
| `src/http/repo_edit/mod.rs` | 753 | `validate_new_path` | 12 | Review hotspot |
| `src/email.rs` | 69 | `invitation_url` | 11 | Review hotspot |
| `src/git/bare/config.rs` | 243 | `should_ignore` | 11 | Review hotspot |
| `src/git/bare/handle.rs` | 588 | `collect_markdown_files` | 11 | Review hotspot |
| `src/git/bare/listing.rs` | 19 | `extract_license_from_cargo_toml` | 11 | Review hotspot |
| `src/http/auth/handlers.rs` | 267 | `create_namespace_ui_handler` | 11 | Review hotspot |
| `src/http/namespace/pages.rs` | 96 | `format_date` | 11 | Review hotspot |
| `src/http/overview/pages.rs` | 18 | `render_index` | 11 | Review hotspot |
| `src/http/repo_edit/mod.rs` | 169 | `editor_file_fragment` | 11 | Review hotspot |
| `src/http/repo_edit/mod.rs` | 695 | `delete_workspace_file` | 11 | Review hotspot |
| `src/http/repository/binaries.rs` | 96 | `download_binary` | 11 | Review hotspot |
| `src/http/repository/pages.rs` | 1051 | `slide_handler` | 11 | Review hotspot |
| `crates/twig-highlight/src/lib.rs` | 216 | `token_kind` | 10 | Review hotspot |
| `src/db/binaries.rs` | 89 | `list_binary_releases` | 10 | Review hotspot |
| `src/db/binaries.rs` | 165 | `get_latest_binary` | 10 | Review hotspot |
| `src/db/invites.rs` | 22 | `get_invite` | 10 | Review hotspot |
| `src/db/mod.rs` | 33 | `conn` | 10 | Review hotspot |
| `src/db/namespaces.rs` | 86 | `namespaces_with_owners` | 10 | Review hotspot |
| `src/db/users.rs` | 89 | `get_user_by_username` | 10 | Review hotspot |
| `src/db/users.rs` | 112 | `get_user_by_id` | 10 | Review hotspot |
| `src/git/bare/handle.rs` | 495 | `list_dir` | 10 | Review hotspot |
| `src/git/repo.rs` | 103 | `create_repo` | 10 | Review hotspot |
| `src/http/namespace/pages.rs` | 124 | `render_namespace` | 10 | Review hotspot |
| `src/http/repository/pages.rs` | 1742 | `render_scripts_view` | 10 | Review hotspot |
| `src/http/tree/user_management.rs` | 592 | `resend_namespace_invitation` | 10 | Review hotspot |
| `src/md/mod.rs` | 95 | `render_mermaid_blocks` | 10 | Review hotspot |
| `crates/twig-highlight/src/lib.rs` | 33 | `class` | 9 | — |
| `crates/twig-highlight/src/lib.rs` | 341 | `match_at` | 9 | — |
| `src/auth/mod.rs` | 280 | `seed_dev_admin` | 9 | — |
| `src/db/binaries.rs` | 45 | `put_binary` | 9 | — |
| `src/db/namespaces.rs` | 52 | `get_namespace_by_name` | 9 | — |
| `src/db/namespaces.rs` | 179 | `get_owned_namespaces_for_user` | 9 | — |
| `src/db/namespaces.rs` | 256 | `get_namespaces_for_user` | 9 | — |
| `src/git/backend.rs` | 54 | `kind` | 9 | — |
| `src/git/repo.rs` | 144 | `bare_init` | 9 | — |
| `src/http/repo_edit/mod.rs` | 87 | `open_workspace_context` | 9 | — |
| `src/http/repository/binaries.rs` | 29 | `upload_binary` | 9 | — |
| `src/http/repository/pages.rs` | 816 | `raw_handler` | 9 | — |
| `src/http/tree/pages.rs` | 484 | `repositories_page` | 9 | — |
| `src/http/tree/user_management.rs` | 113 | `render_user_management_content` | 9 | — |
| `src/db/namespaces.rs` | 6 | `create_namespace` | 8 | — |
| `src/db/namespaces.rs` | 296 | `delete_namespace` | 8 | — |
| `src/git/bare/handle.rs` | 272 | `create_edit_tree` | 8 | — |
| `src/git/mod.rs` | 17 | `git_handler` | 8 | — |
| `src/git/mod.rs` | 239 | `is_authenticated` | 8 | — |
| `src/git/repo.rs` | 30 | `init` | 8 | — |
| `src/http/auth/handlers.rs` | 201 | `login_ui_handler` | 8 | — |
| `src/http/auth/namespace_invitations.rs` | 77 | `load_active_invitation` | 8 | — |
| `src/http/repo_edit/mod.rs` | 447 | `prepare_workspace_commit` | 8 | — |
| `src/http/repo_edit/mod.rs` | 785 | `validate_existing_text_path` | 8 | — |
| `src/http/repository/pages.rs` | 468 | `tab_body` | 8 | — |
| `src/http/repository/pages.rs` | 2154 | `present_print_handler` | 8 | — |
| `src/http/repository/pages.rs` | 2434 | `paper_headings` | 8 | — |
| `src/http/tree/pages.rs` | 58 | `render_namespaces_page` | 8 | — |
| `src/http/tree/pages.rs` | 234 | `render_data_page` | 8 | — |
| `src/http/tree/user_management.rs` | 273 | `render_invitation_list` | 8 | — |
| `src/http/tree/user_management.rs` | 509 | `create_namespace_invitation` | 8 | — |
| `src/main.rs` | 77 | `main` | 8 | — |
| `src/md/mod.rs` | 42 | `highlight_rust_code_blocks` | 8 | — |
| `crates/twig-highlight/src/lib.rs` | 378 | `delimited_end` | 7 | — |
| `crates/twig-highlight/src/lib.rs` | 407 | `hash_raw_string_end` | 7 | — |
| `crates/twig-highlight/src/lib.rs` | 437 | `escape_into` | 7 | — |
| `src/auth/permissions.rs` | 29 | `display_name` | 7 | — |
| `src/auth/permissions.rs` | 104 | `namespaces_for_user` | 7 | — |
| `src/db/namespaces.rs` | 209 | `user_can_create_namespace` | 7 | — |
| `src/db/users.rs` | 6 | `list_users_for_management` | 7 | — |
| `src/git/backend.rs` | 89 | `run_with_config` | 7 | — |
| `src/git/mod.rs` | 391 | `ensure_repo_exists` | 7 | — |
| `src/http/namespace/pages.rs` | 345 | `create_repo_handler` | 7 | — |
| `src/http/repo_edit/mod.rs` | 311 | `apply_operation` | 7 | — |
| `src/http/repo_edit/mod.rs` | 800 | `read_text_file` | 7 | — |
| `src/http/repository/pages.rs` | 288 | `open_repo` | 7 | — |
| `src/http/repository/pages.rs` | 988 | `content_handler` | 7 | — |
| `src/http/settings/pages.rs` | 759 | `delete_namespace` | 7 | — |
| `src/http/tree/user_management.rs` | 206 | `render_user_access_tree` | 7 | — |
| `src/http/tree/user_management.rs` | 444 | `user_management_page` | 7 | — |
| `src/auth/mod.rs` | 158 | `extract_basic_auth` | 6 | — |
| `src/auth/mod.rs` | 223 | `user_id_from_request` | 6 | — |
| `src/db/binaries.rs` | 134 | `get_binary` | 6 | — |
| `src/db/data.rs` | 16 | `list_table_names` | 6 | — |
| `src/db/namespaces.rs` | 141 | `get_namespace_role` | 6 | — |
| `src/db/test_pins.rs` | 19 | `get_test_pin` | 6 | — |
| `src/db/tokens.rs` | 17 | `get_token_user` | 6 | — |
| `src/email.rs` | 200 | `read_http_request` | 6 | — |
| `src/git/backend.rs` | 176 | `prepare_cgi_env` | 6 | — |
| `src/git/bare/config.rs` | 95 | `collect_script_groups` | 6 | — |
| `src/git/bare/handle.rs` | 66 | `file_mode` | 6 | — |
| `src/git/bare/handle.rs` | 404 | `read_blob_bytes` | 6 | — |
| `src/git/bare/handle.rs` | 541 | `list_paper_pages` | 6 | — |
| `src/git/bare/listing.rs` | 174 | `is_repo_private` | 6 | — |
| `src/git/bare/path.rs` | 5 | `is_safe_component` | 6 | — |
| `src/http/overview/pages.rs` | 136 | `index` | 6 | — |
| `src/http/repo_edit/mod.rs` | 595 | `apply_file_operation` | 6 | — |
| `src/http/repo_edit/pages.rs` | 275 | `file_editor` | 6 | — |
| `src/http/repository/binaries.rs` | 173 | `require_namespace_writer` | 6 | — |
| `src/http/repository/binaries.rs` | 222 | `is_safe_filename` | 6 | — |
| `src/http/repository/pages.rs` | 588 | `handler` | 6 | — |
| `src/http/repository/pages.rs` | 927 | `open_paper_page` | 6 | — |
| `src/http/repository/pages.rs` | 1186 | `rewrite_markdown_link` | 6 | — |
| `src/http/repository/pages.rs` | 1938 | `script_base_url` | 6 | — |
| `src/http/repository/pages.rs` | 2400 | `heading_level` | 6 | — |
| `src/http/repository/pages.rs` | 2412 | `heading_slug` | 6 | — |
| `src/http/settings/pages.rs` | 375 | `delete_repo` | 6 | — |
| `src/http/test_page/pages.rs` | 465 | `authenticate_caller` | 6 | — |
| `src/http/tree/pages.rs` | 414 | `load_namespace_page_data` | 6 | — |
| `src/http/tree/pages.rs` | 583 | `data_page` | 6 | — |
| `src/http/tree/user_management.rs` | 40 | `user_management_scope` | 6 | — |
| `src/auth/permissions.rs` | 76 | `for_user` | 5 | — |
| `src/db/invitations.rs` | 100 | `create_namespace_invitation` | 5 | — |
| `src/db/invitations.rs` | 183 | `accept_namespace_invitation` | 5 | — |
| `src/db/migration.rs` | 249 | `test_create_tables_runs_all_migrations` | 5 | — |
| `src/db/test_pins.rs` | 46 | `validate_test_pin` | 5 | — |
| `src/email.rs` | 124 | `send_invitation_to` | 5 | — |
| `src/git/bare/handle.rs` | 445 | `get_license_content` | 5 | — |
| `src/git/bare/listing.rs` | 90 | `last_commit_date` | 5 | — |
| `src/git/bare/mod.rs` | 87 | `build_tree` | 5 | — |
| `src/git/mod.rs` | 320 | `authenticate_git_user` | 5 | — |
| `src/git/repo.rs` | 313 | `set_hook_executable` | 5 | — |
| `src/git/reserved.rs` | 33 | `validate_repo_name` | 5 | — |
| `src/http/auth/handlers.rs` | 39 | `logout_ui_handler` | 5 | — |
| `src/http/auth/pages.rs` | 488 | `test_auth_pages_label_and_describe_every_input` | 5 | — |
| `src/http/overview/pages.rs` | 271 | `test_search_cluster_preserves_the_get_form_contract` | 5 | — |
| `src/http/repo_edit/mod.rs` | 540 | `render_workspace_commit_result` | 5 | — |
| `src/http/repo_edit/pages.rs` | 205 | `file_tree` | 5 | — |
| `src/http/repository/pages.rs` | 490 | `respond_tab` | 5 | — |
| `src/http/repository/pages.rs` | 1164 | `resolve_relative_path` | 5 | — |
| `src/http/repository/pages.rs` | 1666 | `render_content_dir` | 5 | — |
| `src/http/repository/pages.rs` | 1699 | `render_content_file` | 5 | — |
| `src/http/repository/pages.rs` | 3295 | `test_print_export_is_a_chrome_free_document` | 5 | — |
| `src/http/repository/pages.rs` | 3543 | `test_paper_view_lazy_loads_pages_with_zoom_and_font_controls` | 5 | — |
| `src/http/settings/pages.rs` | 46 | `require_namespace_permission` | 5 | — |
| `src/http/settings/pages.rs` | 270 | `settings_page` | 5 | — |
| `src/http/test_page/pages.rs` | 246 | `render_pin_panel` | 5 | — |
| `src/http/test_page/pages.rs` | 556 | `handle_test_page` | 5 | — |
| `src/http/tree/pages.rs` | 634 | `data_rows` | 5 | — |
| `src/http/tree/user_management.rs` | 356 | `parse_expiry` | 5 | — |
| `src/integration_tests.rs` | 1189 | `init_repo_files` | 5 | — |
| `src/integration_tests.rs` | 2398 | `tree` | 5 | — |
| `src/main.rs` | 43 | `spawn_database_init` | 5 | — |
| `crates/twig-highlight/src/lib.rs` | 126 | `highlight` | 4 | — |
| `src/auth/session_store.rs` | 60 | `save` | 4 | — |
| `src/auth/session_store.rs` | 77 | `update` | 4 | — |
| `src/config.rs` | 135 | `maybe_reset_database` | 4 | — |
| `src/config.rs` | 186 | `from_env` | 4 | — |
| `src/db/invitations.rs` | 163 | `list_namespace_invitations` | 4 | — |
| `src/db/mod.rs` | 66 | `init_tables` | 4 | — |
| `src/db/namespaces.rs` | 120 | `user_has_namespace_access` | 4 | — |
| `src/db/namespaces.rs` | 239 | `user_has_any_namespaces` | 4 | — |
| `src/db/sessions.rs` | 4 | `load_actix_session` | 4 | — |
| `src/db/test_pins.rs` | 4 | `set_test_pin` | 4 | — |
| `src/db/users.rs` | 251 | `user_management_lists_global_users_and_scopes_members_by_namespace` | 4 | — |
| `src/git/bare/handle.rs` | 35 | `open` | 4 | — |
| `src/git/bare/handle.rs` | 376 | `get_commits` | 4 | — |
| `src/git/bare/handle.rs` | 476 | `list_files` | 4 | — |
| `src/git/bare/listing.rs` | 98 | `list_repos_with_info` | 4 | — |
| `src/git/bare/path.rs` | 18 | `is_safe_repo_path` | 4 | — |
| `src/git/mod.rs` | 114 | `build_response` | 4 | — |
| `src/git/repo.rs` | 299 | `next_preserved_hook_path` | 4 | — |
| `src/http/auth/namespace_invitations.rs` | 16 | `render_invitation_setup` | 4 | — |
| `src/http/auth/pages.rs` | 20 | `render_field` | 4 | — |
| `src/http/info/pages.rs` | 73 | `render_docs_menu` | 4 | — |
| `src/http/info/pages.rs` | 420 | `test_page_carries_no_outgoing_classes_or_inline_styles` | 4 | — |
| `src/http/namespace/pages.rs` | 46 | `user_has_namespace_permission` | 4 | — |
| `src/http/namespace/pages.rs` | 225 | `handler` | 4 | — |
| `src/http/repo_edit/mod.rs` | 152 | `encode_file_path` | 4 | — |
| `src/http/repo_edit/mod.rs` | 221 | `workspace_page` | 4 | — |
| `src/http/repo_edit/mod.rs` | 269 | `tree_fragment` | 4 | — |
| `src/http/repo_edit/mod.rs` | 372 | `commit_workspace` | 4 | — |
| `src/http/repo_edit/mod.rs` | 910 | `legacy_file_entry` | 4 | — |
| `src/http/repo_edit/pages.rs` | 133 | `workspace_changes_panel` | 4 | — |
| `src/http/repo_edit/pages.rs` | 400 | `commit_result` | 4 | — |
| `src/http/repository/pages.rs` | 873 | `markdown_handler` | 4 | — |
| `src/http/repository/pages.rs` | 1139 | `has_url_scheme` | 4 | — |
| `src/http/repository/pages.rs` | 1266 | `render_paper_markdown` | 4 | — |
| `src/http/repository/pages.rs` | 1576 | `breadcrumb_segments` | 4 | — |
| `src/http/repository/pages.rs` | 1593 | `render_content_breadcrumbs` | 4 | — |
| `src/http/repository/pages.rs` | 1630 | `render_content_view` | 4 | — |
| `src/http/repository/pages.rs` | 2038 | `render_slide_content` | 4 | — |
| `src/http/repository/pages.rs` | 2223 | `render_pdf_document` | 4 | — |
| `src/http/repository/pages.rs` | 4013 | `test_repo_markup_carries_no_inline_presentation` | 4 | — |
| `src/http/settings/pages.rs` | 157 | `render_repo_deletion_panel` | 4 | — |
| `src/http/settings/pages.rs` | 209 | `render_repo_rename_panel` | 4 | — |
| `src/http/settings/pages.rs` | 339 | `update_email` | 4 | — |
| `src/http/tree/pages.rs` | 34 | `render_tree_hub` | 4 | — |
| `src/http/tree/pages.rs` | 352 | `namespaces_page` | 4 | — |
| `src/http/tree/user_management.rs` | 367 | `render_invitation_created` | 4 | — |
| `crates/twig-highlight/src/lib.rs` | 78 | `highlight` | 3 | — |
| `crates/twig-highlight/src/lib.rs` | 92 | `highlight_path` | 3 | — |
| `src/auth/mod.rs` | 61 | `parse` | 3 | — |
| `src/auth/mod.rs` | 203 | `create_session` | 3 | — |
| `src/auth/mod.rs` | 262 | `validate_test_pin` | 3 | — |
| `src/auth/session_store.rs` | 100 | `update_ttl` | 3 | — |
| `src/db/invites.rs` | 5 | `create_invite` | 3 | — |
| `src/db/invites.rs` | 46 | `mark_invite_used` | 3 | — |
| `src/db/namespaces.rs` | 283 | `rename_namespace` | 3 | — |
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
| `src/git/bare/config.rs` | 433 | `test_config_parse_errors_are_reported_not_swallowed` | 3 | — |
| `src/git/bare/handle.rs` | 46 | `head_commit` | 3 | — |
| `src/git/bare/handle.rs` | 422 | `read_file` | 3 | — |
| `src/git/bare/handle.rs` | 467 | `load_config_with_raw` | 3 | — |
| `src/git/repo.rs` | 345 | `run_pre_receive` | 3 | — |
| `src/http/assets.rs` | 113 | `find` | 3 | — |
| `src/http/assets.rs` | 135 | `assets` | 3 | — |
| `src/http/assets.rs` | 281 | `test_twig_css_confines_raw_colour_values_to_the_token_block` | 3 | — |
| `src/http/assets.rs` | 303 | `test_twig_css_honours_the_depth_and_motion_bans` | 3 | — |
| `src/http/assets.rs` | 327 | `test_twig_css_uses_only_the_two_authoritative_breakpoints` | 3 | — |
| `src/http/assets.rs` | 413 | `test_assets_handler_known_files` | 3 | — |
| `src/http/auth/handlers.rs` | 73 | `create_invite_ui_handler` | 3 | — |
| `src/http/auth/namespace_invitations.rs` | 118 | `accept_invitation_page` | 3 | — |
| `src/http/auth/pages.rs` | 443 | `attr_value` | 3 | — |
| `src/http/auth/pages.rs` | 458 | `test_auth_markup_uses_only_twig_design_system_classes` | 3 | — |
| `src/http/auth/pages.rs` | 472 | `test_auth_markup_carries_no_inline_styles_or_glyph_decoration` | 3 | — |
| `src/http/namespace/pages.rs` | 272 | `render_create_repo_form` | 3 | — |
| `src/http/namespace/pages.rs` | 515 | `test_namespace_head_action_is_gated_on_namespace_access` | 3 | — |
| `src/http/namespace/pages.rs` | 630 | `test_namespace_uses_only_twig_design_system_classes` | 3 | — |
| `src/http/namespace/pages.rs` | 687 | `test_create_repo_form_is_a_panel_that_keeps_its_field_contract` | 3 | — |
| `src/http/overview/pages.rs` | 467 | `test_index_uses_only_design_system_classes_and_no_inline_style` | 3 | — |
| `src/http/repo_edit/mod.rs` | 621 | `create_workspace_file` | 3 | — |
| `src/http/repo_edit/mod.rs` | 734 | `discard_workspace_file` | 3 | — |
| `src/http/repo_edit/pages.rs` | 388 | `operation_result` | 3 | — |
| `src/http/repository/pages.rs` | 445 | `scripts_body` | 3 | — |
| `src/http/repository/pages.rs` | 576 | `resolved_default_tab` | 3 | — |
| `src/http/repository/pages.rs` | 957 | `paper_handler` | 3 | — |
| `src/http/repository/pages.rs` | 1212 | `fix_link_tag` | 3 | — |
| `src/http/repository/pages.rs` | 1249 | `markdown_to_html` | 3 | — |
| `src/http/repository/pages.rs` | 1452 | `default_tab` | 3 | — |
| `src/http/repository/pages.rs` | 1463 | `get_default_markdown_file` | 3 | — |
| `src/http/repository/pages.rs` | 1482 | `render_commits_view` | 3 | — |
| `src/http/repository/pages.rs` | 1649 | `render_content_row` | 3 | — |
| `src/http/repository/pages.rs` | 1969 | `load_present_slides` | 3 | — |
| `src/http/repository/pages.rs` | 2354 | `paper_path_slug` | 3 | — |
| `src/http/repository/pages.rs` | 2374 | `paper_page_anchors` | 3 | — |
| `src/http/repository/pages.rs` | 2480 | `render_paper_toolbar` | 3 | — |
| `src/http/repository/pages.rs` | 2526 | `render_paper_view` | 3 | — |
| `src/http/repository/pages.rs` | 2581 | `render_markdown_view` | 3 | — |
| `src/http/repository/pages.rs` | 3271 | `test_print_export_opens_the_dialog_once_loaded` | 3 | — |
| `src/http/repository/pages.rs` | 3456 | `test_presentation_controls_are_labelled_and_bounded` | 3 | — |
| `src/http/repository/pages.rs` | 3996 | `test_repo_markup_uses_only_twig_design_system_classes` | 3 | — |
| `src/http/settings/pages.rs` | 104 | `render_profile_panel` | 3 | — |
| `src/http/settings/pages.rs` | 985 | `test_settings_delete_repo_rows_preserve_the_htmx_contract` | 3 | — |
| `src/http/settings/pages.rs` | 1015 | `test_settings_rename_repo_rows_preserve_the_htmx_contract` | 3 | — |
| `src/http/settings/pages.rs` | 1168 | `test_settings_uses_only_twig_design_system_classes` | 3 | — |
| `src/http/test_page/pages.rs` | 319 | `render_pin_entry_page` | 3 | — |
| `src/http/test_page/pages.rs` | 454 | `get_base_url` | 3 | — |
| `src/http/test_page/pages.rs` | 523 | `verify_admin_only` | 3 | — |
| `src/http/test_page/pages.rs` | 608 | `test_pin_create` | 3 | — |
| `src/http/test_page/pages.rs` | 629 | `test_pin_remove` | 3 | — |
| `src/http/test_page/pages.rs` | 649 | `test_pin_qr` | 3 | — |
| `src/http/tree/api.rs` | 31 | `tree_endpoint` | 3 | — |
| `src/http/tree/pages.rs` | 133 | `render_namespace_rename_panel` | 3 | — |
| `src/http/tree/pages.rs` | 190 | `render_namespace_deletion_panel` | 3 | — |
| `src/http/tree/pages.rs` | 306 | `render_more_rows` | 3 | — |
| `src/http/tree/user_management.rs` | 413 | `display_invite_url` | 3 | — |
| `src/http/view.rs` | 6 | `render_layout` | 3 | — |
| `src/http/view.rs` | 292 | `test_layout_uses_only_twig_design_system_classes` | 3 | — |
| `src/integration_tests.rs` | 1798 | `repo_edit_refuses_ignored_binary_invalid_and_non_regular_files` | 3 | — |
| `src/integration_tests.rs` | 2258 | `test_content_rejects_nul_byte_paths` | 3 | — |
| `src/integration_tests.rs` | 2344 | `init_repo_with_paper` | 3 | — |
| `src/integration_tests.rs` | 2843 | `test_binary_uploads_retain_semver_releases_and_latest_resolves_by_asset` | 3 | — |
| `src/md/mod.rs` | 86 | `is_rust_block_start` | 3 | — |
| `crates/twig-highlight/build.rs` | 3 | `main` | 2 | — |
| `crates/twig-highlight/src/lib.rs` | 112 | `grammars` | 2 | — |
| `crates/twig-highlight/src/lib.rs` | 185 | `try_from` | 2 | — |
| `crates/twig-highlight/src/lib.rs` | 572 | `custom_grammars_use_the_shared_safe_writer` | 2 | — |
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
| `src/config.rs` | 169 | `effective_api_key` | 2 | — |
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
| `src/git/bare/handle.rs` | 61 | `head_oid` | 2 | — |
| `src/git/bare/mod.rs` | 26 | `has_any_repository` | 2 | — |
| `src/git/mod.rs` | 370 | `ensure_namespace_exists` | 2 | — |
| `src/git/reserved.rs` | 66 | `test_namespace_level_routes_are_reserved` | 2 | — |
| `src/git/reserved.rs` | 76 | `test_ordinary_names_are_allowed` | 2 | — |
| `src/git/reserved.rs` | 93 | `test_path_traversal_is_refused` | 2 | — |
| `src/http/assets.rs` | 127 | `url` | 2 | — |
| `src/http/assets.rs` | 161 | `body_of` | 2 | — |
| `src/http/assets.rs` | 168 | `css_without_comments` | 2 | — |
| `src/http/assets.rs` | 181 | `css_outside_root_blocks` | 2 | — |
| `src/http/assets.rs` | 201 | `test_asset_constants_are_non_empty` | 2 | — |
| `src/http/assets.rs` | 245 | `test_twig_schema_lists_every_config_key` | 2 | — |
| `src/http/assets.rs` | 348 | `test_twig_css_drops_the_outgoing_system` | 2 | — |
| `src/http/auth/namespace_invitations.rs` | 237 | `invitation_setup_prefills_email_and_shows_namespace_and_role` | 2 | — |
| `src/http/auth/pages.rs` | 117 | `invite_page` | 2 | — |
| `src/http/auth/pages.rs` | 211 | `signup_page` | 2 | — |
| `src/http/auth/pages.rs` | 279 | `login_page` | 2 | — |
| `src/http/auth/pages.rs` | 344 | `namespace_page` | 2 | — |
| `src/http/auth/pages.rs` | 450 | `field_tag` | 2 | — |
| `src/http/auth/pages.rs` | 519 | `test_auth_pages_announce_their_result_targets` | 2 | — |
| `src/http/auth/pages.rs` | 529 | `test_invite_page_preserves_its_form_contract` | 2 | — |
| `src/http/auth/pages.rs` | 552 | `test_signup_page_preserves_its_form_contract` | 2 | — |
| `src/http/auth/pages.rs` | 590 | `test_login_page_preserves_its_form_contract` | 2 | — |
| `src/http/auth/pages.rs` | 620 | `test_namespace_page_preserves_its_form_contract` | 2 | — |
| `src/http/auth/pages.rs` | 643 | `test_auth_submits_are_block_primary_buttons` | 2 | — |
| `src/http/auth/pages.rs` | 655 | `test_auth_copy_says_an_invite` | 2 | — |
| `src/http/auth/pages.rs` | 665 | `test_success_notices_are_announced_status_notices` | 2 | — |
| `src/http/auth/pages.rs` | 701 | `test_render_signup_success_offers_login_and_namespace` | 2 | — |
| `src/http/auth/pages.rs` | 728 | `test_success_notices_escape_untrusted_values` | 2 | — |
| `src/http/auth/session.rs` | 5 | `get_username_from_request` | 2 | — |
| `src/http/info/pages.rs` | 36 | `from_query` | 2 | — |
| `src/http/info/pages.rs` | 43 | `as_str` | 2 | — |
| `src/http/info/pages.rs` | 51 | `render_tabs` | 2 | — |
| `src/http/info/pages.rs` | 203 | `index` | 2 | — |
| `src/http/info/pages.rs` | 228 | `anchor_containing` | 2 | — |
| `src/http/info/pages.rs` | 328 | `test_render_docs_tab_pairs_the_rail_with_measured_prose` | 2 | — |
| `src/http/info/pages.rs` | 352 | `test_every_doc_page_offers_a_copy_button_carrying_its_markdown` | 2 | — |
| `src/http/info/pages.rs` | 401 | `test_copy_script_uses_the_clipboard_with_a_selection_fallback` | 2 | — |
| `src/http/namespace/pages.rs` | 329 | `create_repo_form_handler` | 2 | — |
| `src/http/namespace/pages.rs` | 539 | `test_namespace_search_cluster_uses_the_field_primitives` | 2 | — |
| `src/http/namespace/pages.rs` | 596 | `test_namespace_empty_states_preserve_their_messages` | 2 | — |
| `src/http/namespace/pages.rs` | 653 | `test_create_repo_form_preserves_the_htmx_contract` | 2 | — |
| `src/http/namespace/pages.rs` | 769 | `test_format_date_singular_units` | 2 | — |
| `src/http/overview/pages.rs` | 114 | `search_repo_hits` | 2 | — |
| `src/http/overview/pages.rs` | 342 | `test_rows_keep_owner_metadata_visible_and_labelled_at_every_width` | 2 | — |
| `src/http/overview/pages.rs` | 427 | `test_empty_states_name_their_condition_and_keep_their_copy` | 2 | — |
| `src/http/repo_edit/mod.rs` | 880 | `file_fragment` | 2 | — |
| `src/http/repo_edit/pages.rs` | 7 | `workspace_shell` | 2 | — |
| `src/http/repo_edit/pages.rs` | 70 | `workspace_files_panel` | 2 | — |
| `src/http/repo_edit/pages.rs` | 111 | `workspace_document_panel` | 2 | — |
| `src/http/repository/pages.rs` | 56 | `text_size_step` | 2 | — |
| `src/http/repository/pages.rs` | 202 | `render_for_request` | 2 | — |
| `src/http/repository/pages.rs` | 356 | `can_view_namespace` | 2 | — |
| `src/http/repository/pages.rs` | 405 | `commits_body` | 2 | — |
| `src/http/repository/pages.rs` | 534 | `render_tab_shell` | 2 | — |
| `src/http/repository/pages.rs` | 551 | `render_tab_response` | 2 | — |
| `src/http/repository/pages.rs` | 861 | `tab_handler` | 2 | — |
| `src/http/repository/pages.rs` | 917 | `page_in_dir` | 2 | — |
| `src/http/repository/pages.rs` | 1154 | `split_link_suffix` | 2 | — |
| `src/http/repository/pages.rs` | 1307 | `render_slide_markdown` | 2 | — |
| `src/http/repository/pages.rs` | 1355 | `render_not_found_for_request` | 2 | — |
| `src/http/repository/pages.rs` | 1498 | `render_config_view` | 2 | — |
| `src/http/repository/pages.rs` | 1544 | `render_license_view` | 2 | — |
| `src/http/repository/pages.rs` | 1559 | `content_urls` | 2 | — |
| `src/http/repository/pages.rs` | 1568 | `parent_path` | 2 | — |
| `src/http/repository/pages.rs` | 1822 | `render_script_group_tabs` | 2 | — |
| `src/http/repository/pages.rs` | 1853 | `render_script_row` | 2 | — |
| `src/http/repository/pages.rs` | 1959 | `raw_content_type` | 2 | — |
| `src/http/repository/pages.rs` | 2016 | `render_slide_nav_button` | 2 | — |
| `src/http/repository/pages.rs` | 2121 | `render_present_view` | 2 | — |
| `src/http/repository/pages.rs` | 2273 | `render_pdf_pagehead` | 2 | — |
| `src/http/repository/pages.rs` | 2307 | `render_pdf_license_page` | 2 | — |
| `src/http/repository/pages.rs` | 2342 | `paper_relative_path` | 2 | — |
| `src/http/repository/pages.rs` | 2559 | `render_paper_content_only` | 2 | — |
| `src/http/repository/pages.rs` | 2620 | `render_markdown_content_only` | 2 | — |
| `src/http/repository/pages.rs` | 2962 | `test_tabs_preserve_every_htmx_navigation_attribute` | 2 | — |
| `src/http/repository/pages.rs` | 2987 | `test_tabs_mark_exactly_one_current_tab` | 2 | — |
| `src/http/repository/pages.rs` | 3229 | `test_print_export_renders_one_page_per_slide_with_the_license_last` | 2 | — |
| `src/http/repository/pages.rs` | 3406 | `test_presentation_targets_its_container_and_scopes_the_keyboard` | 2 | — |
| `src/http/repository/pages.rs` | 3753 | `test_tab_content_shows_a_config_error_on_every_tab` | 2 | — |
| `src/http/routes.rs` | 122 | `test_is_git_guard_matches_git_user_agent` | 2 | — |
| `src/http/routes.rs` | 142 | `test_is_git_guard_rejects_non_git_user_agent` | 2 | — |
| `src/http/routes.rs` | 162 | `test_is_git_guard_rejects_missing_user_agent` | 2 | — |
| `src/http/settings/pages.rs` | 329 | `render_settings_auth_error` | 2 | — |
| `src/http/settings/pages.rs` | 876 | `test_settings_leads_with_crumbs_and_page_head` | 2 | — |
| `src/http/settings/pages.rs` | 1081 | `test_settings_email_form_preserves_the_htmx_contract` | 2 | — |
| `src/http/test_page/pages.rs` | 31 | `display_name` | 2 | — |
| `src/http/test_page/pages.rs` | 39 | `username` | 2 | — |
| `src/http/test_page/pages.rs` | 57 | `generate_qr_svg` | 2 | — |
| `src/http/test_page/pages.rs` | 419 | `render_runner_content` | 2 | — |
| `src/http/test_page/pages.rs` | 676 | `test_runner` | 2 | — |
| `src/http/test_page/pages.rs` | 690 | `test_stopped` | 2 | — |
| `src/http/test_page/pages.rs` | 704 | `test_ping` | 2 | — |
| `src/http/test_page/pages.rs` | 718 | `test_feature_check` | 2 | — |
| `src/http/test_page/pages.rs` | 762 | `test_endpoint_catalog_uses_current_api_and_info_routes` | 2 | — |
| `src/http/tree/api.rs` | 17 | `msgpack_responder` | 2 | — |
| `src/http/tree/pages.rs` | 284 | `render_data_row` | 2 | — |
| `src/http/tree/pages.rs` | 317 | `require_admin` | 2 | — |
| `src/http/tree/pages.rs` | 336 | `tree_page` | 2 | — |
| `src/http/tree/user_management.rs` | 196 | `render_permission_list` | 2 | — |
| `src/http/tree/user_management.rs` | 346 | `invitations_for_scope` | 2 | — |
| `src/http/tree/user_management.rs` | 705 | `htmx_user_management_response_contains_content_without_tabs` | 2 | — |
| `src/http/tree/user_management.rs` | 716 | `invite_tab_renders_htmx_form_with_contributor_default_and_expiry` | 2 | — |
| `src/http/tree/user_management.rs` | 741 | `user_memberships_render_as_namespace_role_permission_trees` | 2 | — |
| `src/http/tree/user_management.rs` | 769 | `site_admin_tree_shows_all_namespace_permissions` | 2 | — |
| `src/http/tree/user_management.rs` | 795 | `expiry_values_map_to_expected_expiration` | 2 | — |
| `src/http/tree/user_management.rs` | 820 | `delivery_notices_preserve_link_and_distinguish_sent_skipped_and_failed` | 2 | — |
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
| `src/integration_tests.rs` | 876 | `test_tree_hub_and_admin_data_view_are_gated_and_paginated` | 2 | — |
| `src/integration_tests.rs` | 1224 | `repo_edit_workspace_navigates_tree_and_reads_files_through_htmx` | 2 | — |
| `src/integration_tests.rs` | 1579 | `repo_edit_commit_rejects_invalid_message_noop_and_oversized_file` | 2 | — |
| `src/integration_tests.rs` | 2208 | `test_content_rejects_dotdot_segments` | 2 | — |
| `src/integration_tests.rs` | 2236 | `test_content_rejects_namespace_and_repo_traversal` | 2 | — |
| `src/integration_tests.rs` | 2277 | `test_markdown_view_rejects_traversal` | 2 | — |
| `src/integration_tests.rs` | 2948 | `test_raw_route_serves_repo_files_and_refuses_ignored_paths` | 2 | — |
| `src/integration_tests.rs` | 3259 | `create_test_service_with_test_user` | 2 | — |
| `src/md/mod.rs` | 5 | `replace_mustache` | 2 | — |
| `src/md/mod.rs` | 16 | `is_markdown` | 2 | — |
| `src/md/mod.rs` | 136 | `is_mermaid_block_start` | 2 | — |
| `crates/twig-highlight/src/lib.rs` | 55 | `text` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 60 | `token` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 99 | `highlight_with` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 464 | `unknown_languages_are_not_highlighted` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 469 | `extension_lookup_is_case_insensitive` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 475 | `source_is_escaped_even_when_it_is_not_a_token` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 480 | `rust_grammar_is_loaded_from_the_toml_registry` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 489 | `javascript_grammar_is_loaded_from_toml_and_maps_its_extensions` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 500 | `shell_grammar_highlights_script_tokens_and_maps_shell_extensions` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 515 | `css_grammar_highlights_styles_and_maps_css_files` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 532 | `grammar_rules_highlight_tokens_and_escape_source` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 548 | `handles_nested_comments_raw_strings_raw_identifiers_and_lifetimes` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 565 | `unclosed_constructs_are_still_safely_rendered` | 1 | — |
| `crates/twig-highlight/src/lib.rs` | 576 | `highlight` | 1 | — |
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
| `src/config.rs` | 25 | `new` | 1 | — |
| `src/config.rs` | 50 | `with_cache_control` | 1 | — |
| `src/config.rs` | 55 | `with_test_user` | 1 | — |
| `src/config.rs` | 60 | `with_admin_user` | 1 | — |
| `src/config.rs` | 65 | `with_resend_settings` | 1 | — |
| `src/config.rs` | 77 | `resend_api_key` | 1 | — |
| `src/config.rs` | 81 | `resend_from` | 1 | — |
| `src/config.rs` | 85 | `public_base_url` | 1 | — |
| `src/config.rs` | 89 | `admin_user` | 1 | — |
| `src/config.rs` | 93 | `is_configured_admin` | 1 | — |
| `src/config.rs` | 97 | `address` | 1 | — |
| `src/config.rs` | 101 | `log_level` | 1 | — |
| `src/config.rs` | 105 | `project_root` | 1 | — |
| `src/config.rs` | 109 | `db_path` | 1 | — |
| `src/config.rs` | 114 | `reset_db` | 1 | — |
| `src/config.rs` | 119 | `test_user` | 1 | — |
| `src/config.rs` | 124 | `is_test_user_enabled` | 1 | — |
| `src/config.rs` | 129 | `is_test_user` | 1 | — |
| `src/config.rs` | 154 | `cache_control` | 1 | — |
| `src/config.rs` | 158 | `session_key` | 1 | — |
| `src/config.rs` | 162 | `with_session_key` | 1 | — |
| `src/config.rs` | 181 | `fmt` | 1 | — |
| `src/config.rs` | 251 | `ascii` | 1 | — |
| `src/config.rs` | 279 | `test_server_new` | 1 | — |
| `src/config.rs` | 299 | `test_server_cache_control_round_trip` | 1 | — |
| `src/config.rs` | 317 | `test_server_clone` | 1 | — |
| `src/config.rs` | 332 | `test_effective_api_key_with_set_key` | 1 | — |
| `src/config.rs` | 345 | `test_effective_api_key_generates_when_empty` | 1 | — |
| `src/config.rs` | 360 | `test_maybe_reset_database_no_flag` | 1 | — |
| `src/config.rs` | 373 | `test_address_returns_clone` | 1 | — |
| `src/config.rs` | 387 | `test_server_test_user` | 1 | — |
| `src/config.rs` | 408 | `test_server_admin_user_is_independent_from_test_user` | 1 | — |
| `src/config.rs` | 426 | `test_blank_admin_user_is_not_configured` | 1 | — |
| `src/config.rs` | 441 | `test_resend_settings_are_optional_and_keep_sender_configuration` | 1 | — |
| `src/db/binaries.rs` | 300 | `test_db_path` | 1 | — |
| `src/db/binaries.rs` | 304 | `version` | 1 | — |
| `src/db/binaries.rs` | 309 | `binary_database_round_trips_arbitrary_blob_bytes` | 1 | — |
| `src/db/binaries.rs` | 333 | `binary_database_replaces_and_lists_asset_metadata` | 1 | — |
| `src/db/binaries.rs` | 425 | `latest_binary_resolves_highest_release_containing_filename` | 1 | — |
| `src/db/data.rs` | 91 | `table_reader_lists_tables_and_pages_values` | 1 | — |
| `src/db/data.rs` | 134 | `table_reader_rejects_an_oversized_page_loudly` | 1 | — |
| `src/db/data.rs` | 148 | `table_reader_returns_an_empty_page_past_the_end_of_the_table` | 1 | — |
| `src/db/invitations.rs` | 256 | `setup_db` | 1 | — |
| `src/db/invitations.rs` | 279 | `create_get_and_scope_list_namespace_invitations` | 1 | — |
| `src/db/invitations.rs` | 331 | `pending_invitation` | 1 | — |
| `src/db/invitations.rs` | 348 | `invited_user` | 1 | — |
| `src/db/invitations.rs` | 354 | `acceptance_creates_account_membership_and_consumes_invite_atomically` | 1 | — |
| `src/db/invitations.rs` | 412 | `failed_acceptance_rolls_back_account_membership_and_invite_state` | 1 | — |
| `src/db/invitations.rs` | 458 | `expired_or_mismatched_invitation_cannot_be_accepted` | 1 | — |
| `src/db/invitations.rs` | 506 | `concurrent_acceptance_only_creates_one_account_and_membership` | 1 | — |
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
| `src/db/namespaces.rs` | 75 | `get_all_namespaces_with_owners` | 1 | — |
| `src/db/namespaces.rs` | 79 | `search_namespaces_with_owners` | 1 | — |
| `src/db/namespaces.rs` | 342 | `setup_db_with_user` | 1 | — |
| `src/db/namespaces.rs` | 360 | `test_namespace` | 1 | — |
| `src/db/namespaces.rs` | 370 | `test_create_and_get_namespace_by_name` | 1 | — |
| `src/db/namespaces.rs` | 390 | `test_get_all_namespaces_with_owners` | 1 | — |
| `src/db/namespaces.rs` | 411 | `test_user_has_namespace_access_owner` | 1 | — |
| `src/db/namespaces.rs` | 430 | `test_user_has_namespace_access_denied` | 1 | — |
| `src/db/namespaces.rs` | 450 | `namespace_roles_control_owner_scopes_and_creation_permissions` | 1 | — |
| `src/db/namespaces.rs` | 541 | `test_search_namespaces_with_owners` | 1 | — |
| `src/db/namespaces.rs` | 566 | `test_rename_namespace` | 1 | — |
| `src/db/namespaces.rs` | 595 | `test_delete_namespace` | 1 | — |
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
| `src/git/bare/config.rs` | 36 | `default_script_shell` | 1 | — |
| `src/git/bare/config.rs` | 157 | `is_configured` | 1 | — |
| `src/git/bare/config.rs` | 162 | `default_paper_dir` | 1 | — |
| `src/git/bare/config.rs` | 229 | `load` | 1 | — |
| `src/git/bare/config.rs` | 238 | `parse` | 1 | — |
| `src/git/bare/config.rs` | 280 | `script_groups` | 1 | — |
| `src/git/bare/config.rs` | 294 | `test_twig_config_parse` | 1 | — |
| `src/git/bare/config.rs` | 306 | `test_present_config_parse` | 1 | — |
| `src/git/bare/config.rs` | 326 | `test_paper_config_parse` | 1 | — |
| `src/git/bare/config.rs` | 346 | `test_paper_config_empty_dir_is_not_configured` | 1 | — |
| `src/git/bare/config.rs` | 352 | `test_scripts_config_parses_groups_and_entries` | 1 | — |
| `src/git/bare/config.rs` | 400 | `test_scripts_config_absent_or_empty_yields_no_groups` | 1 | — |
| `src/git/bare/config.rs` | 452 | `test_twig_config_should_ignore_folder` | 1 | — |
| `src/git/bare/config.rs` | 469 | `test_twig_config_should_ignore_with_slash_pattern` | 1 | — |
| `src/git/bare/config.rs` | 486 | `test_twig_config_should_ignore_multiple_patterns` | 1 | — |
| `src/git/bare/config.rs` | 504 | `test_twig_config_empty` | 1 | — |
| `src/git/bare/config.rs` | 511 | `test_twig_config_should_ignore_folder_with_trailing_slash` | 1 | — |
| `src/git/bare/config.rs` | 528 | `test_twig_config_should_ignore_exact_file` | 1 | — |
| `src/git/bare/config.rs` | 545 | `test_twig_config_toml_format_parsing` | 1 | — |
| `src/git/bare/config.rs` | 556 | `test_twig_config_parse_private` | 1 | — |
| `src/git/bare/handle.rs` | 31 | `from_repository` | 1 | — |
| `src/git/bare/handle.rs` | 430 | `path_exists` | 1 | — |
| `src/git/bare/handle.rs` | 633 | `new` | 1 | — |
| `src/git/bare/handle.rs` | 640 | `handle` | 1 | — |
| `src/git/bare/handle.rs` | 669 | `empty_handle` | 1 | — |
| `src/git/bare/handle.rs` | 678 | `drop` | 1 | — |
| `src/git/bare/handle.rs` | 684 | `commit_file_writes_nested_content_and_preserves_other_entries_and_mode` | 1 | — |
| `src/git/bare/handle.rs` | 727 | `commit_file_refuses_to_overwrite_a_newer_branch_tip` | 1 | — |
| `src/git/bare/handle.rs` | 764 | `commit_new_file_adds_a_nested_path_and_refuses_overwrites_and_stale_heads` | 1 | — |
| `src/git/bare/handle.rs` | 845 | `commit_changes_applies_multiple_file_operations_in_one_commit` | 1 | — |
| `src/git/bare/handle.rs` | 917 | `commit_changes_rejects_stale_head_and_invalid_or_no_op_batches` | 1 | — |
| `src/git/bare/handle.rs` | 1005 | `commit_changes_can_create_the_first_commit_in_an_empty_repository` | 1 | — |
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
| `src/git/repo.rs` | 25 | `default_branch` | 1 | — |
| `src/git/repo.rs` | 336 | `sh` | 1 | — |
| `src/git/repo.rs` | 379 | `test_default_branch_is_main` | 1 | — |
| `src/git/repo.rs` | 384 | `test_create_repo_creates_directory_structure` | 1 | — |
| `src/git/repo.rs` | 401 | `test_bare_init_creates_valid_repository` | 1 | — |
| `src/git/repo.rs` | 437 | `contributor_hook_allows_fast_forwards_and_rejects_main_rewrites_and_deletions` | 1 | — |
| `src/git/repo.rs` | 518 | `receive_hook_installation_preserves_chains_and_is_idempotent` | 1 | — |
| `src/git/reserved.rs` | 25 | `is_reserved_repo_name` | 1 | — |
| `src/git/reserved.rs` | 59 | `test_reserved_check_is_case_insensitive` | 1 | — |
| `src/git/reserved.rs` | 87 | `test_empty_name_is_refused` | 1 | — |
| `src/git/reserved.rs` | 103 | `test_dotfile_names_are_refused` | 1 | — |
| `src/http/api.rs` | 11 | `version_endpoint` | 1 | — |
| `src/http/api.rs` | 23 | `test_version_endpoint_returns_package_version` | 1 | — |
| `src/http/assets.rs` | 23 | `name` | 1 | — |
| `src/http/assets.rs` | 27 | `versioned_name` | 1 | — |
| `src/http/assets.rs` | 196 | `starts_a_hex_colour` | 1 | — |
| `src/http/assets.rs` | 225 | `test_urls_carry_the_version` | 1 | — |
| `src/http/assets.rs` | 265 | `test_twig_schema_describes_nested_script_groups` | 1 | — |
| `src/http/assets.rs` | 357 | `test_versioned_assets_handler_uses_configured_cache_control` | 1 | — |
| `src/http/assets.rs` | 384 | `test_unversioned_assets_must_revalidate` | 1 | — |
| `src/http/assets.rs` | 474 | `test_assets_handler_unknown_file_returns_404` | 1 | — |
| `src/http/assets.rs` | 497 | `test_assets_handler_rejects_another_version` | 1 | — |
| `src/http/auth/handlers.rs` | 365 | `test_generate_token_format` | 1 | — |
| `src/http/auth/handlers.rs` | 374 | `test_invite_struct_with_none_user_id` | 1 | — |
| `src/http/auth/handlers.rs` | 388 | `test_invite_struct_with_some_user_id` | 1 | — |
| `src/http/auth/handlers.rs` | 402 | `test_create_invite_with_null_user_id` | 1 | — |
| `src/http/auth/namespace_invitations.rs` | 63 | `render_account_created` | 1 | — |
| `src/http/auth/namespace_invitations.rs` | 222 | `invitation` | 1 | — |
| `src/http/auth/namespace_invitations.rs` | 253 | `account_created_notice_offers_login_and_names_the_namespace` | 1 | — |
| `src/http/auth/pages.rs` | 47 | `render_submit` | 1 | — |
| `src/http/auth/pages.rs` | 56 | `wrap_auth_content` | 1 | — |
| `src/http/auth/pages.rs` | 67 | `render_invite_page` | 1 | — |
| `src/http/auth/pages.rs` | 135 | `render_signup_page` | 1 | — |
| `src/http/auth/pages.rs` | 229 | `render_login_page` | 1 | — |
| `src/http/auth/pages.rs` | 293 | `render_namespace_page` | 1 | — |
| `src/http/auth/pages.rs` | 363 | `render_invite_success` | 1 | — |
| `src/http/auth/pages.rs` | 379 | `render_signup_success` | 1 | — |
| `src/http/auth/pages.rs` | 395 | `render_login_success` | 1 | — |
| `src/http/auth/pages.rs` | 414 | `auth_pages` | 1 | — |
| `src/http/auth/pages.rs` | 423 | `success_notices` | 1 | — |
| `src/http/auth/pages.rs` | 434 | `input_tags` | 1 | — |
| `src/http/auth/pages.rs` | 687 | `test_render_invite_success_shows_the_invite_as_code` | 1 | — |
| `src/http/auth/pages.rs` | 714 | `test_render_login_success_offers_namespace_creation` | 1 | — |
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
| `src/http/namespace/pages.rs` | 17 | `user_can_create_repositories` | 1 | — |
| `src/http/namespace/pages.rs` | 33 | `user_has_namespace_permission_for_request` | 1 | — |
| `src/http/namespace/pages.rs` | 434 | `missing_email_response` | 1 | — |
| `src/http/namespace/pages.rs` | 451 | `repo` | 1 | — |
| `src/http/namespace/pages.rs` | 461 | `test_namespace_breadcrumb_trail_is_semantic` | 1 | — |
| `src/http/namespace/pages.rs` | 488 | `test_namespace_is_headed_by_its_identifier_inside_the_trail` | 1 | — |
| `src/http/namespace/pages.rs` | 507 | `test_namespace_does_not_repeat_the_repository_section_heading` | 1 | — |
| `src/http/namespace/pages.rs` | 564 | `test_namespace_rows_keep_last_commit_labelled_when_stacked` | 1 | — |
| `src/http/namespace/pages.rs` | 672 | `test_create_repo_form_cancel_is_plain_text_with_its_dismiss_behaviour` | 1 | — |
| `src/http/namespace/pages.rs` | 713 | `test_format_date_years` | 1 | — |
| `src/http/namespace/pages.rs` | 723 | `test_format_date_months` | 1 | — |
| `src/http/namespace/pages.rs` | 733 | `test_format_date_days` | 1 | — |
| `src/http/namespace/pages.rs` | 743 | `test_format_date_hours` | 1 | — |
| `src/http/namespace/pages.rs` | 753 | `test_format_date_minutes` | 1 | — |
| `src/http/namespace/pages.rs` | 763 | `test_format_date_just_now` | 1 | — |
| `src/http/namespace/pages.rs` | 794 | `test_missing_email_response_is_a_bad_request_fragment_with_account_link` | 1 | — |
| `src/http/namespace/pages.rs` | 813 | `test_params_deserialization` | 1 | — |
| `src/http/namespace/pages.rs` | 819 | `test_create_repo_form_deserialization` | 1 | — |
| `src/http/namespace/pages.rs` | 827 | `test_create_repo_form_default_branch` | 1 | — |
| `src/http/namespace/pages.rs` | 834 | `test_search_query_deserialization` | 1 | — |
| `src/http/namespace/pages.rs` | 840 | `test_search_query_empty` | 1 | — |
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
| `src/http/repo_edit/mod.rs` | 146 | `html_response` | 1 | — |
| `src/http/repo_edit/mod.rs` | 165 | `workspace_file_url` | 1 | — |
| `src/http/repo_edit/mod.rs` | 435 | `workspace_commit_preparation_error` | 1 | — |
| `src/http/repo_edit/mod.rs` | 525 | `render_commit_result` | 1 | — |
| `src/http/repo_edit/mod.rs` | 875 | `parent_path` | 1 | — |
| `src/http/repo_edit/mod.rs` | 946 | `workspace_shell_loads_the_file_tree_through_htmx` | 1 | — |
| `src/http/repo_edit/mod.rs` | 972 | `workspace_paths_are_encoded_for_query_and_route_components` | 1 | — |
| `src/http/repo_edit/mod.rs` | 981 | `editor_operation_forms_override_implicit_get_inheritance` | 1 | — |
| `src/http/repo_edit/pages.rs` | 49 | `workspace_document_url` | 1 | — |
| `src/http/repo_edit/pages.rs` | 56 | `workspace_breadcrumbs` | 1 | — |
| `src/http/repo_edit/pages.rs` | 186 | `workspace_restore_form` | 1 | — |
| `src/http/repo_edit/pages.rs` | 378 | `file_unavailable` | 1 | — |
| `src/http/repo_edit/pages.rs` | 418 | `file_placeholder` | 1 | — |
| `src/http/repo_edit/pages.rs` | 427 | `editor_assets` | 1 | — |
| `src/http/repository/binaries.rs` | 217 | `parse_release_version` | 1 | — |
| `src/http/repository/binaries.rs` | 232 | `is_binary_size_allowed` | 1 | — |
| `src/http/repository/binaries.rs` | 236 | `binary_response` | 1 | — |
| `src/http/repository/binaries.rs` | 252 | `binary_versions_accept_optional_v_and_reject_latest_for_upload` | 1 | — |
| `src/http/repository/binaries.rs` | 262 | `binary_filename_and_size_validation_reject_unsafe_inputs` | 1 | — |
| `src/http/repository/pages.rs` | 35 | `text_size_persist` | 1 | — |
| `src/http/repository/pages.rs` | 39 | `text_size_list` | 1 | — |
| `src/http/repository/pages.rs` | 49 | `text_size_restore` | 1 | — |
| `src/http/repository/pages.rs` | 215 | `render_repo_auth_error` | 1 | — |
| `src/http/repository/pages.rs` | 268 | `frame` | 1 | — |
| `src/http/repository/pages.rs` | 378 | `markdown_files` | 1 | — |
| `src/http/repository/pages.rs` | 386 | `markdown_body` | 1 | — |
| `src/http/repository/pages.rs` | 390 | `markdown_body_from` | 1 | — |
| `src/http/repository/pages.rs` | 397 | `content_body` | 1 | — |
| `src/http/repository/pages.rs` | 410 | `config_body` | 1 | — |
| `src/http/repository/pages.rs` | 417 | `license_body` | 1 | — |
| `src/http/repository/pages.rs` | 422 | `present_body` | 1 | — |
| `src/http/repository/pages.rs` | 432 | `paper_body` | 1 | — |
| `src/http/repository/pages.rs` | 529 | `tab_nav` | 1 | — |
| `src/http/repository/pages.rs` | 642 | `markdown_tab_handler` | 1 | — |
| `src/http/repository/pages.rs` | 661 | `content_tab_handler` | 1 | — |
| `src/http/repository/pages.rs` | 680 | `commits_tab_handler` | 1 | — |
| `src/http/repository/pages.rs` | 699 | `config_tab_handler` | 1 | — |
| `src/http/repository/pages.rs` | 718 | `present_tab_handler` | 1 | — |
| `src/http/repository/pages.rs` | 737 | `paper_tab_handler` | 1 | — |
| `src/http/repository/pages.rs` | 756 | `license_tab_handler` | 1 | — |
| `src/http/repository/pages.rs` | 775 | `scripts_tab_handler` | 1 | — |
| `src/http/repository/pages.rs` | 794 | `scripts_group_handler` | 1 | — |
| `src/http/repository/pages.rs` | 1241 | `markdown_options` | 1 | — |
| `src/http/repository/pages.rs` | 1329 | `render_empty` | 1 | — |
| `src/http/repository/pages.rs` | 1338 | `render_git_error` | 1 | — |
| `src/http/repository/pages.rs` | 1370 | `render_repo_crumbs` | 1 | — |
| `src/http/repository/pages.rs` | 1525 | `render_config_error` | 1 | — |
| `src/http/repository/pages.rs` | 1844 | `script_command` | 1 | — |
| `src/http/repository/pages.rs` | 1881 | `render_script_group_row` | 1 | — |
| `src/http/repository/pages.rs` | 1994 | `load_paper_pages` | 1 | — |
| `src/http/repository/pages.rs` | 2003 | `slide_counter` | 1 | — |
| `src/http/repository/pages.rs` | 2287 | `render_pdf_slide_page` | 1 | — |
| `src/http/repository/pages.rs` | 2325 | `render_paper_page` | 1 | — |
| `src/http/repository/pages.rs` | 2368 | `paper_page_anchor` | 1 | — |
| `src/http/repository/pages.rs` | 2640 | `render_commit` | 1 | — |
| `src/http/repository/pages.rs` | 2668 | `base_frame` | 1 | — |
| `src/http/repository/pages.rs` | 2686 | `frame_with` | 1 | — |
| `src/http/repository/pages.rs` | 2696 | `slides` | 1 | — |
| `src/http/repository/pages.rs` | 2704 | `entry` | 1 | — |
| `src/http/repository/pages.rs` | 2712 | `count_of` | 1 | — |
| `src/http/repository/pages.rs` | 2716 | `tab_labels` | 1 | — |
| `src/http/repository/pages.rs` | 2728 | `test_base_url_schemes_follow_the_request` | 1 | — |
| `src/http/repository/pages.rs` | 2757 | `test_raw_content_type_is_text_for_scripts_and_neutral_otherwise` | 1 | — |
| `src/http/repository/pages.rs` | 2770 | `test_parent_path` | 1 | — |
| `src/http/repository/pages.rs` | 2778 | `test_breadcrumb_segments` | 1 | — |
| `src/http/repository/pages.rs` | 2793 | `test_markdown_to_html` | 1 | — |
| `src/http/repository/pages.rs` | 2807 | `test_markdown_link_fixing` | 1 | — |
| `src/http/repository/pages.rs` | 2817 | `test_markdown_links_resolve_against_containing_directory` | 1 | — |
| `src/http/repository/pages.rs` | 2829 | `test_markdown_non_markdown_links_target_content_route` | 1 | — |
| `src/http/repository/pages.rs` | 2837 | `test_markdown_link_preserves_fragment_and_query` | 1 | — |
| `src/http/repository/pages.rs` | 2848 | `test_markdown_link_leaves_absolute_and_scheme_links_alone` | 1 | — |
| `src/http/repository/pages.rs` | 2857 | `test_slide_markdown_anchors_relative_links_to_the_slide_directory` | 1 | — |
| `src/http/repository/pages.rs` | 2869 | `test_slide_markdown_substitutes_template_variables` | 1 | — |
| `src/http/repository/pages.rs` | 2883 | `test_resolve_relative_path` | 1 | — |
| `src/http/repository/pages.rs` | 2893 | `test_has_url_scheme` | 1 | — |
| `src/http/repository/pages.rs` | 2903 | `test_get_default_markdown_file` | 1 | — |
| `src/http/repository/pages.rs` | 2922 | `test_markdown_tables_rendering` | 1 | — |
| `src/http/repository/pages.rs` | 2935 | `test_default_tab_prefers_configuration_then_markdown` | 1 | — |
| `src/http/repository/pages.rs` | 2944 | `test_tabs_are_an_out_of_band_labelled_nav` | 1 | — |
| `src/http/repository/pages.rs` | 3005 | `test_tabs_render_only_available_views_in_order` | 1 | — |
| `src/http/repository/pages.rs` | 3058 | `test_repo_page_leads_with_a_breadcrumb_head_then_the_tabs` | 1 | — |
| `src/http/repository/pages.rs` | 3079 | `test_repo_page_announces_its_tab_panel` | 1 | — |
| `src/http/repository/pages.rs` | 3088 | `test_tab_response_differs_for_htmx_and_direct_navigation` | 1 | — |
| `src/http/repository/pages.rs` | 3123 | `test_commit_row_is_a_dense_machine_record` | 1 | — |
| `src/http/repository/pages.rs` | 3162 | `test_commits_view_falls_back_to_an_empty_state` | 1 | — |
| `src/http/repository/pages.rs` | 3170 | `test_content_rows_mark_directories_with_a_trailing_slash` | 1 | — |
| `src/http/repository/pages.rs` | 3198 | `test_content_dir_offers_a_parent_row_below_the_root` | 1 | — |
| `src/http/repository/pages.rs` | 3212 | `test_presentation_offers_the_pdf_export` | 1 | — |
| `src/http/repository/pages.rs` | 3330 | `test_content_breadcrumbs_mark_the_final_segment_as_current` | 1 | — |
| `src/http/repository/pages.rs` | 3360 | `test_markdown_rail_appears_only_for_multiple_files` | 1 | — |
| `src/http/repository/pages.rs` | 3391 | `test_markdown_content_reports_a_missing_file` | 1 | — |
| `src/http/repository/pages.rs` | 3522 | `test_slide_counter_pads_to_the_width_of_the_deck` | 1 | — |
| `src/http/repository/pages.rs` | 3529 | `test_present_view_without_slides_states_the_condition` | 1 | — |
| `src/http/repository/pages.rs` | 3616 | `test_paper_headings_slug_anchor_and_dedupe` | 1 | — |
| `src/http/repository/pages.rs` | 3641 | `test_render_paper_markdown_puts_heading_ids_on_the_headings` | 1 | — |
| `src/http/repository/pages.rs` | 3658 | `test_render_paper_content_only_anchors_headings` | 1 | — |
| `src/http/repository/pages.rs` | 3677 | `test_paper_page_anchors_are_path_relative_and_unique` | 1 | — |
| `src/http/repository/pages.rs` | 3700 | `test_paper_view_without_pages_states_the_condition` | 1 | — |
| `src/http/repository/pages.rs` | 3711 | `test_page_in_dir_accepts_only_paths_below_the_paper_directory` | 1 | — |
| `src/http/repository/pages.rs` | 3721 | `test_config_error_is_a_diagnostic_with_a_help_hint` | 1 | — |
| `src/http/repository/pages.rs` | 3775 | `test_config_and_license_views_render_code_and_empty_states` | 1 | — |
| `src/http/repository/pages.rs` | 3801 | `test_content_view_reports_missing_paths_and_binary_files` | 1 | — |
| `src/http/repository/pages.rs` | 3866 | `test_git_error_is_an_announced_danger_notice` | 1 | — |
| `src/http/repository/pages.rs` | 3885 | `representative_markup` | 1 | — |
| `src/http/repository/pages.rs` | 3980 | `test_render_repo_auth_error` | 1 | — |
| `src/http/routes.rs` | 12 | `configure_routes` | 1 | — |
| `src/http/routes.rs` | 106 | `is_git` | 1 | — |
| `src/http/settings/pages.rs` | 86 | `render_settings` | 1 | — |
| `src/http/settings/pages.rs` | 832 | `user_fixture` | 1 | — |
| `src/http/settings/pages.rs` | 842 | `repo_fixture` | 1 | — |
| `src/http/settings/pages.rs` | 850 | `populated_html` | 1 | — |
| `src/http/settings/pages.rs` | 854 | `empty_html` | 1 | — |
| `src/http/settings/pages.rs` | 858 | `repo_panel_html` | 1 | — |
| `src/http/settings/pages.rs` | 863 | `empty_repo_panel_html` | 1 | — |
| `src/http/settings/pages.rs` | 867 | `rename_panel_html` | 1 | — |
| `src/http/settings/pages.rs` | 871 | `empty_rename_panel_html` | 1 | — |
| `src/http/settings/pages.rs` | 901 | `test_account_page_renders_account_panel_only` | 1 | — |
| `src/http/settings/pages.rs` | 942 | `test_settings_does_not_render_repository_moves` | 1 | — |
| `src/http/settings/pages.rs` | 950 | `test_settings_deletion_panels_carry_danger_and_caution_affordances` | 1 | — |
| `src/http/settings/pages.rs` | 1061 | `test_settings_rename_panel_empty_state_offers_no_targets` | 1 | — |
| `src/http/settings/pages.rs` | 1074 | `test_settings_does_not_render_namespace_deletion_controls` | 1 | — |
| `src/http/settings/pages.rs` | 1112 | `test_settings_result_targets_announce_swaps` | 1 | — |
| `src/http/settings/pages.rs` | 1121 | `test_settings_keeps_identity_visible_and_email_break_safe` | 1 | — |
| `src/http/settings/pages.rs` | 1145 | `test_settings_empty_states_name_their_condition_and_offer_no_targets` | 1 | — |
| `src/http/settings/pages.rs` | 1193 | `test_namespace_form_deserialization` | 1 | — |
| `src/http/settings/pages.rs` | 1199 | `test_update_email_form_deserialization` | 1 | — |
| `src/http/settings/pages.rs` | 1205 | `test_delete_repo_form_deserialization` | 1 | — |
| `src/http/settings/pages.rs` | 1213 | `test_move_repo_form_deserialization` | 1 | — |
| `src/http/settings/pages.rs` | 1224 | `test_rename_repo_form_deserialization` | 1 | — |
| `src/http/settings/pages.rs` | 1233 | `test_rename_namespace_form_deserialization` | 1 | — |
| `src/http/test_page/pages.rs` | 26 | `is_admin` | 1 | — |
| `src/http/test_page/pages.rs` | 48 | `generate_random_pin` | 1 | — |
| `src/http/test_page/pages.rs` | 76 | `core_system_endpoints` | 1 | — |
| `src/http/test_page/pages.rs` | 99 | `docs_endpoints` | 1 | — |
| `src/http/test_page/pages.rs` | 140 | `asset_endpoints` | 1 | — |
| `src/http/test_page/pages.rs` | 169 | `ui_endpoints` | 1 | — |
| `src/http/test_page/pages.rs` | 186 | `feature_endpoints` | 1 | — |
| `src/http/test_page/pages.rs` | 203 | `list_test_endpoints` | 1 | — |
| `src/http/test_page/pages.rs` | 212 | `render_pagehead` | 1 | — |
| `src/http/test_page/pages.rs` | 364 | `render_test_page` | 1 | — |
| `src/http/test_page/pages.rs` | 386 | `render_idle_content` | 1 | — |
| `src/http/test_page/pages.rs` | 403 | `render_stopped_content` | 1 | — |
| `src/http/test_page/pages.rs` | 552 | `is_hx_request` | 1 | — |
| `src/http/test_page/pages.rs` | 598 | `test_page_alias` | 1 | — |
| `src/http/test_page/pages.rs` | 739 | `test_render_stopped_content` | 1 | — |
| `src/http/test_page/pages.rs` | 746 | `test_render_test_page_is_idle_until_started` | 1 | — |
| `src/http/tree/api.rs` | 77 | `test_config` | 1 | — |
| `src/http/tree/api.rs` | 89 | `test_tree_round_trips_as_named_messagepack` | 1 | — |
| `src/http/tree/api.rs` | 104 | `test_tree_endpoint_returns_namespaces_and_repositories` | 1 | — |
| `src/http/tree/api.rs` | 158 | `test_tree_endpoint_filters_private_repositories` | 1 | — |
| `src/http/tree/pages.rs` | 294 | `render_load_more` | 1 | — |
| `src/http/tree/pages.rs` | 668 | `tree_hub_only_renders_enabled_sections` | 1 | — |
| `src/http/tree/pages.rs` | 694 | `namespace_page_lists_repositories_with_confirmed_namespace_buttons` | 1 | — |
| `src/http/tree/pages.rs` | 729 | `namespace_page_explains_when_no_namespace_can_be_deleted` | 1 | — |
| `src/http/tree/pages.rs` | 742 | `namespace_page_lists_owned_namespaces_for_rename_with_the_htmx_contract` | 1 | — |
| `src/http/tree/pages.rs` | 771 | `namespace_rename_panel_empty_state_offers_no_targets` | 1 | — |
| `src/http/tree/pages.rs` | 782 | `data_page_uses_the_infinite_scroll_placeholder` | 1 | — |
| `src/http/tree/user_management.rs` | 30 | `default_invitation_role` | 1 | — |
| `src/http/tree/user_management.rs` | 88 | `render_user_management_tabs` | 1 | — |
| `src/http/tree/user_management.rs` | 259 | `render_user_management` | 1 | — |
| `src/http/tree/user_management.rs` | 333 | `is_expired` | 1 | — |
| `src/http/tree/user_management.rs` | 339 | `format_expiry` | 1 | — |
| `src/http/tree/user_management.rs` | 427 | `deliver_invitation` | 1 | — |
| `src/http/tree/user_management.rs` | 660 | `scope` | 1 | — |
| `src/http/tree/user_management.rs` | 679 | `invitation` | 1 | — |
| `src/http/tree/user_management.rs` | 694 | `full_user_management_render_has_one_link_per_tab` | 1 | — |
| `src/http/tree/user_management.rs` | 734 | `users_tab_is_reachable_with_htmx_and_renders_scoped_placeholder` | 1 | — |
| `src/http/tree/user_management.rs` | 810 | `pending_invitation_lists_have_an_htmx_resend_action` | 1 | — |
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
| `src/integration_tests.rs` | 7 | `create_test_service` | 1 | — |
| `src/integration_tests.rs` | 17 | `create_test_service_in` | 1 | — |
| `src/integration_tests.rs` | 27 | `create_test_service_with_db_in` | 1 | — |
| `src/integration_tests.rs` | 74 | `create_admin_test_service` | 1 | — |
| `src/integration_tests.rs` | 185 | `test_health_endpoint` | 1 | — |
| `src/integration_tests.rs` | 193 | `test_invite_page_loads` | 1 | — |
| `src/integration_tests.rs` | 206 | `test_signup_page_loads` | 1 | — |
| `src/integration_tests.rs` | 219 | `test_user_management_renders_admin_page_and_htmx_tabs` | 1 | — |
| `src/integration_tests.rs` | 296 | `test_created_invitation_completes_account_and_membership_setup` | 1 | — |
| `src/integration_tests.rs` | 427 | `namespace_owner_can_invite_only_to_owned_namespace` | 1 | — |
| `src/integration_tests.rs` | 466 | `contributor_cannot_create_namespaces_or_repositories_but_can_access_existing_repo` | 1 | — |
| `src/integration_tests.rs` | 622 | `configured_admin_has_all_git_permissions_without_namespace_membership` | 1 | — |
| `src/integration_tests.rs` | 745 | `expired_namespace_invitation_link_returns_gone` | 1 | — |
| `src/integration_tests.rs` | 776 | `test_login_page_loads` | 1 | — |
| `src/integration_tests.rs` | 789 | `test_namespace_page_loads` | 1 | — |
| `src/integration_tests.rs` | 801 | `test_generate_invite_with_invalid_api_key` | 1 | — |
| `src/integration_tests.rs` | 815 | `test_generate_invite_with_valid_api_key` | 1 | — |
| `src/integration_tests.rs` | 829 | `test_signup_with_invalid_invite` | 1 | — |
| `src/integration_tests.rs` | 848 | `test_login_with_wrong_password` | 1 | — |
| `src/integration_tests.rs` | 862 | `test_create_namespace_without_session` | 1 | — |
| `src/integration_tests.rs` | 1046 | `test_settings_page_without_login_is_an_htmx_recovery_fragment` | 1 | — |
| `src/integration_tests.rs` | 1062 | `test_settings_page_with_expired_session_is_a_full_recovery_page` | 1 | — |
| `src/integration_tests.rs` | 1080 | `test_settings_page_without_login` | 1 | — |
| `src/integration_tests.rs` | 1094 | `test_update_email_invalid_format` | 1 | — |
| `src/integration_tests.rs` | 1110 | `extract_invite` | 1 | — |
| `src/integration_tests.rs` | 1118 | `signup_and_login` | 1 | — |
| `src/integration_tests.rs` | 1165 | `create_namespace` | 1 | — |
| `src/integration_tests.rs` | 1185 | `init_text_repo` | 1 | — |
| `src/integration_tests.rs` | 1332 | `repo_edit_operations_stage_multiple_file_changes_without_writing_git` | 1 | — |
| `src/integration_tests.rs` | 1447 | `repo_edit_commits_the_complete_draft_as_one_commit_and_keeps_the_workspace` | 1 | — |
| `src/integration_tests.rs` | 1525 | `repo_edit_commit_conflict_preserves_draft_and_concurrent_head` | 1 | — |
| `src/integration_tests.rs` | 1681 | `repo_edit_commit_accepts_drafts_larger_than_the_default_form_limit` | 1 | — |
| `src/integration_tests.rs` | 1730 | `repo_edit_commit_requires_an_author_email` | 1 | — |
| `src/integration_tests.rs` | 1778 | `repo_edit_legacy_file_entry_denies_authenticated_users_without_namespace_access` | 1 | — |
| `src/integration_tests.rs` | 1849 | `test_move_repo_between_owned_namespaces` | 1 | — |
| `src/integration_tests.rs` | 1879 | `test_move_repo_rejects_namespace_owned_by_another_user` | 1 | — |
| `src/integration_tests.rs` | 1913 | `test_rename_repo_within_owned_namespace` | 1 | — |
| `src/integration_tests.rs` | 1942 | `test_rename_repo_rejects_foreign_namespace_and_existing_name` | 1 | — |
| `src/integration_tests.rs` | 1988 | `test_rename_namespace_moves_directory_and_record` | 1 | — |
| `src/integration_tests.rs` | 2023 | `test_rename_namespace_rejects_foreign_owner_and_existing_name` | 1 | — |
| `src/integration_tests.rs` | 2056 | `test_cannot_create_namespace_with_reserved_prefix` | 1 | — |
| `src/integration_tests.rs` | 2088 | `drop` | 1 | — |
| `src/integration_tests.rs` | 2097 | `setup_traversal_fixture` | 1 | — |
| `src/integration_tests.rs` | 2124 | `create_traversal_service` | 1 | — |
| `src/integration_tests.rs` | 2158 | `body_of` | 1 | — |
| `src/integration_tests.rs` | 2174 | `test_content_tab_lists_repo_files` | 1 | — |
| `src/integration_tests.rs` | 2189 | `test_content_file_view_serves_repo_files_only` | 1 | — |
| `src/integration_tests.rs` | 2295 | `test_content_cannot_reach_sibling_namespace_via_dots` | 1 | — |
| `src/integration_tests.rs` | 2321 | `init_private_repo` | 1 | — |
| `src/integration_tests.rs` | 2390 | `init_repo_with_files` | 1 | — |
| `src/integration_tests.rs` | 2463 | `get_body` | 1 | — |
| `src/integration_tests.rs` | 2477 | `test_paper_tab_renders_lazily_loaded_pages` | 1 | — |
| `src/integration_tests.rs` | 2525 | `test_scripts_tab_is_hidden_until_configured` | 1 | — |
| `src/integration_tests.rs` | 2548 | `test_scripts_tab_shows_groups_with_curl_commands` | 1 | — |
| `src/integration_tests.rs` | 2615 | `test_http_binary_upload_and_download_work_for_public_and_private_repos` | 1 | — |
| `src/integration_tests.rs` | 2999 | `test_legacy_tab_url_redirects_to_the_tab_route` | 1 | — |
| `src/integration_tests.rs` | 3037 | `test_invalid_twig_toml_shows_a_diagnostic` | 1 | — |
| `src/integration_tests.rs` | 3061 | `basic_auth` | 1 | — |
| `src/integration_tests.rs` | 3071 | `test_private_repository_ui_visibility_gated_on_login` | 1 | — |
| `src/integration_tests.rs` | 3155 | `test_private_repository_git_read_gated_behind_auth` | 1 | — |
| `src/integration_tests.rs` | 3305 | `test_test_page_404_when_test_user_unset` | 1 | — |
| `src/integration_tests.rs` | 3313 | `test_test_page_forbidden_when_unauthenticated` | 1 | — |
| `src/integration_tests.rs` | 3321 | `test_test_page_and_rapid_runner_when_authenticated_as_admin` | 1 | — |
| `src/integration_tests.rs` | 3393 | `test_test_page_pin_and_qr_code_multi_user_lifecycle` | 1 | — |
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

> Complexity is a lexical estimate: start at 1 per function, add 1 for each `if`, loop, `&&`, `||`, and `?`, and add one for each `match` arm after the first. Macros are not expanded; decision-like tokens inside invocations may be counted lexically. Syntax edge cases can make these values differ from AST-based tools.

## Allowed or expected lint attributes

| File | Line | Attribute | Lint names |
| --- | ---: | --- | --- |
| `src/config.rs` | 24 | `allow` | `clippy::too_many_arguments` |
| `src/db/namespaces.rs` | 255 | `allow` | `dead_code` |
| `src/git/bare/handle.rs` | 82 | `allow` | `dead_code` |
| `src/git/bare/handle.rs` | 148 | `allow` | `dead_code` |
| `src/git/bare/handle.rs` | 580 | `allow` | `dead_code` |
| `src/git/bare/mod.rs` | 14 | `allow` | `unused_imports` |
| `src/http/repository/pages.rs` | 252 | `allow` | `clippy::struct_excessive_bools` |
| `src/http/repository/pages.rs` | 2685 | `allow` | `clippy::fn_params_excessive_bools` |

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
| `src/http/auth/handlers.rs` | 74 | `create_invite_ui_handler` | `_req` | 1 | Likely unused |
| `src/http/auth/handlers.rs` | 114 | `signup_ui_handler` | `_req` | 1 | Likely unused |
| `src/http/auth/pages.rs` | 56 | `wrap_auth_content` | `_title` | 1 | Likely unused |
| `src/http/settings/pages.rs` | 478 | `move_repo` | `_namespace` | 1 | Likely unused |
| `src/http/settings/pages.rs` | 583 | `rename_repo` | `_namespace` | 1 | Likely unused |
| `src/http/tree/api.rs` | 172 | `test_tree_endpoint_filters_private_repositories` | `_p` | 1 | Likely unused |
| `src/integration_tests.rs` | 2325 | `init_private_repo` | `_p` | 1 | Likely unused |
| `src/integration_tests.rs` | 2350 | `init_repo_with_paper` | `_p` | 1 | Likely unused |
| `src/integration_tests.rs` | 2427 | `init_repo_with_files` | `_p` | 1 | Likely unused |

> Underscore-prefixed names are lexical candidates, not compiler diagnostics. A name seen once in its function is marked likely unused; shadowing, macros, and complex Rust patterns can affect the result.
