# Graph Report - .  (2026-04-22)

## Corpus Check
- Corpus is ~46,187 words - fits in a single context window. You may not need a graph.

## Summary
- 657 nodes · 1149 edges · 36 communities detected
- Extraction: 69% EXTRACTED · 31% INFERRED · 0% AMBIGUOUS · INFERRED: 360 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Get Game|Get Game]]
- [[_COMMUNITY_Cookie Status|Cookie Status]]
- [[_COMMUNITY_Get Uses|Get Uses]]
- [[_COMMUNITY_Frontend Tauri|Frontend Tauri]]
- [[_COMMUNITY_Parse Parses|Parse Parses]]
- [[_COMMUNITY_Threshold Capture|Threshold Capture]]
- [[_COMMUNITY_Wechat Status|Wechat Status]]
- [[_COMMUNITY_Run Set|Run Set]]
- [[_COMMUNITY_Selection Prepare|Selection Prepare]]
- [[_COMMUNITY_Accounts Account|Accounts Account]]
- [[_COMMUNITY_Handlekeydown Handlemousedown|Handlekeydown Handlemousedown]]
- [[_COMMUNITY_Settings Path|Settings Path]]
- [[_COMMUNITY_Handlekeydown Sidebarmenu|Handlekeydown Sidebarmenu]]
- [[_COMMUNITY_Circle Abstract|Circle Abstract]]
- [[_COMMUNITY_React Atom|React Atom]]
- [[_COMMUNITY_Auto Tools|Auto Tools]]
- [[_COMMUNITY_Accent Orb|Accent Orb]]
- [[_COMMUNITY_Application Interlocking|Application Interlocking]]
- [[_COMMUNITY_Circle Abstract|Circle Abstract]]
- [[_COMMUNITY_Auto Tools|Auto Tools]]
- [[_COMMUNITY_Circle Abstract|Circle Abstract]]
- [[_COMMUNITY_Auto Tools|Auto Tools]]
- [[_COMMUNITY_Circle Blue|Circle Blue]]
- [[_COMMUNITY_Auto Tools|Auto Tools]]
- [[_COMMUNITY_Auto Tools|Auto Tools]]
- [[_COMMUNITY_Auto Tools|Auto Tools]]
- [[_COMMUNITY_Ring Abstract|Ring Abstract]]
- [[_COMMUNITY_Handlers Layer|Handlers Layer]]
- [[_COMMUNITY_Apiresponse Response.rs|Apiresponse Response.rs]]
- [[_COMMUNITY_Idecall Ide.rs|Idecall Ide.rs]]
- [[_COMMUNITY_Vite Branding|Vite Branding]]
- [[_COMMUNITY_Settings.json Persistence|Settings.json Persistence]]
- [[_COMMUNITY_Rust Hotkey|Rust Hotkey]]
- [[_COMMUNITY_Http Api|Http Api]]
- [[_COMMUNITY_X30 Application|X30 Application]]
- [[_COMMUNITY_Overlay|Overlay]]

## God Nodes (most connected - your core abstractions)
1. `http_options()` - 36 edges
2. `GameService` - 18 edges
3. `make_service()` - 13 edges
4. `WegameAuthService` - 13 edges
5. `ide_form()` - 12 edges
6. `restore_cookie_json()` - 11 edges
7. `run_recognition_flow()` - 11 edges
8. `persist_account()` - 10 edges
9. `extract_jsonp_args()` - 10 edges
10. `build_client()` - 9 edges

## Surprising Connections (you probably didn't know these)
- `Delta Tool Interface Layer` --semantically_similar_to--> `DeltaForce API Rust Migration Plan`  [INFERRED] [semantically similar]
  AGENTS.md → Rust迁移实现文档.md
- `run()` --calls--> `initialize()`  [INFERRED]
  src-tauri\src\lib.rs → src-tauri\src\morse\mod.rs
- `main()` --calls--> `run()`  [INFERRED]
  src-tauri\src\main.rs → src-tauri\src\lib.rs
- `initialize()` --calls--> `load_settings()`  [INFERRED]
  src-tauri\src\morse\mod.rs → src-tauri\src\morse\settings.rs
- `decodes_qqsafe_token_payload()` --calls--> `decode_jwt_middle()`  [INFERRED]
  src-tauri\src\delta\services\qq_safe.rs → src-tauri\src\delta\utils\html.rs

## Hyperedges (group relationships)
- **Frontend Entry Chain** — index_src_main_tsx, claude_src_main_tsx, claude_src_app_tsx, claude_morse_page [EXTRACTED 1.00]
- **Overlay Selection Flow** — agents_overlay_mode, data_pending_selection, readme_overlay_mode, claude_morse_overlay [EXTRACTED 1.00]
- **Rust Migration Layering** — rust迁移_rust_migration_plan, rust迁移_axum_handlers, rust迁移_services_layer [EXTRACTED 1.00]

## Communities

### Community 0 - "Get Game"
Cohesion: 0.06
Nodes (68): AccountBoundAccess, AccountCookieRequest, CommandOptions, delta_delete_account(), delta_game_get_achievement(), delta_game_get_assets(), delta_game_get_bind(), delta_game_get_config() (+60 more)

### Community 1 - "Cookie Status"
Cohesion: 0.06
Nodes (35): dump_cookie_json(), finds_named_cookie_even_when_not_first_in_header(), insert_cookie(), inserts_cookie_into_jar(), must_cookie(), restore_cookie_json(), restores_and_dumps_cookie_json(), extract_jsonp_args() (+27 more)

### Community 2 - "Get Uses"
Cohesion: 0.14
Nodes (18): GameAuth, GameService, get_achievement_uses_documented_payload(), get_assets_maps_special_restriction_error(), get_guns_is_no_auth_and_enriches_details(), get_logs_type_three_returns_total_money_only(), get_manufacture_uses_documented_chart_and_source(), get_password_folds_secret_list_into_map() (+10 more)

### Community 3 - "Frontend Tauri"
Cohesion: 0.06
Nodes (33): Bun Toolchain Constraint, Delta Tool Interface Layer, Desktop Tool Repository, Morse Recognition Workbench, Overlay Query Mode, Tauri Commands Boundary, Core Recognition Data Flow, Query Parameter Mode Switch (+25 more)

### Community 4 - "Parse Parses"
Cohesion: 0.11
Nodes (24): AmmoItem, enrich_gun_detail(), leaves_missing_config_entries_empty(), normalize_caliber_code(), parse_accessory_config(), parse_ammo_config(), parse_bind_role_js(), parses_bind_role_fragment() (+16 more)

### Community 5 - "Threshold Capture"
Cohesion: 0.12
Nodes (22): DeltaError, get_gtk(), get_qr_token(), apply_threshold(), capture_region(), ComponentBounds, components_to_morse(), detect_components() (+14 more)

### Community 6 - "Wechat Status"
Cohesion: 0.09
Nodes (16): decode(), rejects_unknown_pattern(), decode_gbk(), decode_jwt_middle(), decodes_jwt_middle_payload(), extract_query_param(), extract_wx_errcode(), extracts_query_param() (+8 more)

### Community 7 - "Run Set"
Cohesion: 0.14
Nodes (18): PassiveHotkeyListener, begin_run(), finish_run(), morse_get_bootstrap(), morse_run_recognition(), morse_save_settings(), morse_set_hotkey_recording(), MorseState (+10 more)

### Community 8 - "Selection Prepare"
Cohesion: 0.17
Nodes (20): morse_begin_region_selection(), morse_overlay_cancel_selection(), morse_overlay_submit_selection(), begin_region_selection(), cancel_selection(), commit_selection(), completed_slots_reports_prefix(), destroy_overlay_window() (+12 more)

### Community 9 - "Accounts Account"
Cohesion: 0.13
Nodes (11): run(), main(), AccountKind, deletes_accounts(), DeltaAccountRecord, DeltaAccountUpsert, DeltaRepo, map_row() (+3 more)

### Community 10 - "Handlekeydown Handlemousedown"
Cohesion: 0.12
Nodes (7): handleMouseUp(), MorsePage(), formatRecordedHotkey(), getErrorMessage(), getSelectionRect(), normalizeHotkeyPrimaryKey(), normalizeRunDetails()

### Community 11 - "Settings Path"
Cohesion: 0.31
Nodes (12): deserialize_settings(), deserialize_settings_reports_invalid_json(), ensure_config_dir(), ensure_config_dir_creates_path(), load_settings(), read_settings_from_path(), read_settings_returns_default_when_missing(), sample_settings() (+4 more)

### Community 14 - "Handlekeydown Sidebarmenu"
Cohesion: 0.33
Nodes (2): SidebarMenuButton(), useSidebar()

### Community 37 - "Circle Abstract"
Cohesion: 1.0
Nodes (3): Abstract Logo Mark, Blue Circle, Yellow Circle

### Community 38 - "React Atom"
Cohesion: 1.0
Nodes (3): React Atom Logo, React Central Dot, Orbital Ellipse Set

### Community 39 - "Auto Tools"
Cohesion: 1.0
Nodes (3): Delta Auto Tools App Icon, Interlocking Circular Mark, Teal And Gold Palette

### Community 40 - "Accent Orb"
Cohesion: 1.0
Nodes (3): Abstract App Logo Mark, Cool Accent Orb, Warm Accent Orb

### Community 41 - "Application Interlocking"
Cohesion: 1.0
Nodes (3): Application Icon, Interlocking Dual Ring Mark, Teal and Gold Color Scheme

### Community 42 - "Circle Abstract"
Cohesion: 1.0
Nodes (3): Abstract Logo Mark, Lower Cyan Circle, Upper Gold Circle

### Community 43 - "Auto Tools"
Cohesion: 1.0
Nodes (3): Delta Auto Tools App Icon, Interlocking Circular Mark, Teal And Gold Palette

### Community 44 - "Circle Abstract"
Cohesion: 1.0
Nodes (3): Abstract Logo Mark, Blue Circle, Yellow Circle

### Community 45 - "Auto Tools"
Cohesion: 1.0
Nodes (3): Delta Auto Tools App Icon, Interlocking Circular Mark, Teal And Gold Palette

### Community 46 - "Circle Blue"
Cohesion: 1.0
Nodes (3): Blue Circle, Square 310x310 Logo Mark, Yellow Circle

### Community 47 - "Auto Tools"
Cohesion: 1.0
Nodes (3): Delta Auto Tools App Icon, Interlocking Circular Mark, Teal And Gold Palette

### Community 48 - "Auto Tools"
Cohesion: 1.0
Nodes (3): Delta Auto Tools App Icon, Interlocking Circular Mark, Teal And Gold Palette

### Community 49 - "Auto Tools"
Cohesion: 1.0
Nodes (3): Delta Auto Tools App Icon, Interlocking Circular Mark, Teal And Gold Palette

### Community 50 - "Ring Abstract"
Cohesion: 1.0
Nodes (3): Abstract Logo Mark, Cyan Ring, Orange Ring

### Community 51 - "Handlers Layer"
Cohesion: 0.67
Nodes (3): Axum Handlers Layer, Keep Handlers Thin And Business Logic In Services, Services Business Logic Layer

### Community 81 - "Apiresponse Response.rs"
Cohesion: 1.0
Nodes (1): ApiResponse

### Community 82 - "Idecall Ide.rs"
Cohesion: 1.0
Nodes (1): IdeCall

### Community 83 - "Vite Branding"
Cohesion: 1.0
Nodes (2): Vite Branding Gradient Icon, Vite Logo SVG

### Community 84 - "Settings.json Persistence"
Cohesion: 1.0
Nodes (2): morse_settings.json Persistence File, morse_settings.json Stored Configuration

### Community 85 - "Rust Hotkey"
Cohesion: 1.0
Nodes (2): Rust Hotkey Registration Responsibility, Frontend Records While Rust Saves Hotkeys

### Community 86 - "Http Api"
Cohesion: 1.0
Nodes (2): No HTTP API Or Router, Native Command Layer Instead Of HTTP Backend

### Community 107 - "X30 Application"
Cohesion: 1.0
Nodes (1): 30x30 Application Icon

### Community 108 - "Overlay"
Cohesion: 1.0
Nodes (1): Morse Overlay UI

## Knowledge Gaps
- **74 isolated node(s):** `CommandOptions`, `AccountCookieRequest`, `QqUpdateRequest`, `WechatAccessRequest`, `WechatUpdateRequest` (+69 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Handlekeydown Sidebarmenu`** (7 nodes): `cn()`, `handleKeyDown()`, `SidebarMenu()`, `SidebarMenuButton()`, `SidebarMenuItem()`, `useSidebar()`, `sidebar.tsx`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Apiresponse Response.rs`** (2 nodes): `ApiResponse`, `response.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Idecall Ide.rs`** (2 nodes): `IdeCall`, `ide.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Vite Branding`** (2 nodes): `Vite Branding Gradient Icon`, `Vite Logo SVG`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Settings.json Persistence`** (2 nodes): `morse_settings.json Persistence File`, `morse_settings.json Stored Configuration`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Rust Hotkey`** (2 nodes): `Rust Hotkey Registration Responsibility`, `Frontend Records While Rust Saves Hotkeys`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Http Api`** (2 nodes): `No HTTP API Or Router`, `Native Command Layer Instead Of HTTP Backend`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `X30 Application`** (1 nodes): `30x30 Application Icon`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Overlay`** (1 nodes): `Morse Overlay UI`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `prepare_selection_from_pending()` connect `Selection Prepare` to `Get Game`?**
  _High betweenness centrality (0.013) - this node is a cross-community bridge._
- **Why does `run_recognition_flow()` connect `Run Set` to `Get Game`, `Parse Parses`, `Threshold Capture`?**
  _High betweenness centrality (0.012) - this node is a cross-community bridge._
- **Why does `read_settings_from_path()` connect `Settings Path` to `Get Game`, `Cookie Status`?**
  _High betweenness centrality (0.010) - this node is a cross-community bridge._
- **What connects `CommandOptions`, `AccountCookieRequest`, `QqUpdateRequest` to the rest of the system?**
  _74 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Get Game` be split into smaller, more focused modules?**
  _Cohesion score 0.06 - nodes in this community are weakly interconnected._
- **Should `Cookie Status` be split into smaller, more focused modules?**
  _Cohesion score 0.06 - nodes in this community are weakly interconnected._
- **Should `Get Uses` be split into smaller, more focused modules?**
  _Cohesion score 0.14 - nodes in this community are weakly interconnected._