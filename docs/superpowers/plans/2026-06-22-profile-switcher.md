# 配置下拉切换 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在顶栏全局开关左侧添加配置下拉框，把 Profile 从“手动保存快照”改成“真实配置槽自动保存”。

**Architecture:** Profile 后端模块成为配置槽的唯一深模块，负责默认配置创建、默认配置新增、当前配置快照更新、切换和重命名。前端 `ProfileProvider` 只暴露小接口给顶栏 `ProfileSwitcher`，工具页继续通过现有 autosave 保存设置，Rust 工具命令保存成功后同步更新当前 active Profile snapshot。

**Tech Stack:** Tauri 2、Rust、React 19、TypeScript、Vite、Bun、Vitest、Cargo test。

## Global Constraints

- 所有 AI 输出、代码注释、错误信息和 UI 文案使用中文，技术术语保持英文原名。
- Commit message 使用中文。
- 使用 Bun，不切换到 npm、pnpm、yarn。
- 新增 Tauri command 必须注册到 `src-tauri/src/lib.rs` 的 `generate_handler![]`。
- `src-tauri/capabilities/default.json` 当前不逐条列 command 权限，本功能不需要新增 capability permission。
- Rust 对外序列化结构体使用 `#[serde(rename_all = "camelCase")]`。
- UI 保持 Swiss Industrial Print × Declassified Tactical Control Board：直角、硬边框、琥珀强调，不引入圆角卡片、柔和阴影、玻璃态或新 UI 依赖。
- 不保留设置页里的配置操作入口，设置 Dialog 只保留 `主题 / 关于`。
- 不提供删除配置入口。
- 无配置启动时创建真实 `配置1`，并继承现有本地工具设置。
- 点击 `新增配置` 创建全默认配置，命名按历史最大编号 + 1。

---

## File Structure

- Modify: `src-tauri/src/profile/types.rs`
  - 为 `ProfileSettings` 增加 `nextProfileNumber` 持久化字段。
  - 保持旧配置反序列化兼容，缺字段回退到 `1`。

- Modify: `src-tauri/src/profile/mod.rs`
  - 新增纯函数：`max_config_number`、`reserve_config_name`、`build_default_snapshot`、`append_profile`。
  - 新增命令：`profile_create_default`。
  - 修改 `profile_get_bootstrap`，无配置时创建真实 `配置1`，继承当前工具设置。
  - 新增内部接口：`ActiveProfileSnapshotPatch` 和 `update_active_profile_snapshot`，供工具模块保存后同步 active Profile snapshot。

- Modify: `src-tauri/src/lib.rs`
  - 注册 `profile::profile_create_default`。

- Modify: `src-tauri/src/morse/mod.rs`
  - 在 `morse_save_settings`、区域选择完成保存、提前结束保存后同步 active Profile snapshot。

- Modify: `src-tauri/src/timer/mod.rs`
  - 在 `timer_save_settings` 和位置提交保存后同步 active Profile snapshot。

- Modify: `src-tauri/src/counter/mod.rs`
  - 在 `counter_save_settings` 和位置提交保存后同步 active Profile snapshot。

- Modify: `src-tauri/src/rapidfire/mod.rs`
  - 在 `rapidfire_save_settings` 和位置提交保存后同步 active Profile snapshot。

- Modify: `src-tauri/src/audio/mod.rs`
  - 在 `audio_save_settings` 和 overlay 保存后同步 active Profile snapshot。

- Modify: `src/hooks/use-profile.tsx`
  - 删除“手动保存当前为配置”和删除配置对外能力。
  - 新增 `createDefaultProfile`、`activeProfile`、`activeProfileName`。

- Modify: `src/components/app/profile-types.ts`
  - 为 `ProfileSettings` 增加 `nextProfileNumber`。

- Modify: `src/components/app/profile-utils.ts`
  - 新增纯函数：`getActiveProfile`、`getProfileDisplayName`、`sortProfilesForSwitcher`。

- Modify: `src/components/app/profile-utils.test.ts`
  - 覆盖空配置显示、active 查找、下拉排序。

- Create: `src/components/app/profile-switcher.tsx`
  - 顶栏配置下拉框，负责切换、新增默认配置、重命名。

- Modify: `src/App.tsx`
  - 在 `GlobalSwitch` 左侧渲染 `ProfileSwitcher`。
  - 从设置 Dialog 中保留 `主题 / 关于`，移除配置 tab。

- Modify: `src/components/app/settings-page.tsx`
  - 移除 ProfilePanel import、`profile` tab 类型、配置 TabTrigger、ProfilePanel 内容。

---

### Task 1: Profile 后端配置槽语义

**Files:**
- Modify: `src-tauri/src/profile/types.rs`
- Modify: `src-tauri/src/profile/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/profile/types.rs`
- Test: `src-tauri/src/profile/mod.rs`

**Interfaces:**
- Consumes: existing `snapshot_current_settings(app: &AppHandle) -> Result<ToolSettingsSnapshot, String>` and `apply_snapshot_to_tools(app: &AppHandle, snapshot: &ToolSettingsSnapshot) -> Result<(), String>`.
- Produces:
  - `ProfileSettings.next_profile_number: u32`
  - `#[tauri::command] pub fn profile_create_default(app: AppHandle, state: State<'_, ProfileState>) -> Result<ProfileBootstrap, String>`
  - `pub(crate) enum ActiveProfileSnapshotPatch`
  - `pub(crate) fn update_active_profile_snapshot(app: &AppHandle, patch: ActiveProfileSnapshotPatch) -> Result<(), String>`

- [ ] **Step 1: Write failing Rust tests for persisted next profile number**

Add these tests inside `src-tauri/src/profile/types.rs` existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn profile_settings_default_next_profile_number_is_one() {
    let settings = ProfileSettings::default();
    assert_eq!(settings.next_profile_number, 1);
}

#[test]
fn profile_settings_missing_next_profile_number_defaults_to_one() {
    let json = r#"{"profiles":[],"activeProfileId":""}"#;
    let loaded: ProfileSettings = serde_json::from_str(json).unwrap();
    assert_eq!(loaded.next_profile_number, 1);
}

#[test]
fn profile_settings_serializes_next_profile_number_camel_case() {
    let settings = ProfileSettings {
        profiles: Vec::new(),
        active_profile_id: String::new(),
        next_profile_number: 7,
    };
    let json = serde_json::to_string(&settings).unwrap();
    assert!(json.contains("\"nextProfileNumber\":7"));
    assert!(!json.contains("next_profile_number"));
}
```

- [ ] **Step 2: Run Rust tests and verify they fail**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml profile_settings_next -- --nocapture
```

Expected: fail with missing field `next_profile_number` or missing test symbol matches.

- [ ] **Step 3: Add `next_profile_number` to ProfileSettings**

Change `src-tauri/src/profile/types.rs`:

```rust
/// Profile 持久化设置，存到 `profile_settings.json`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSettings {
    /// 全部 Profile 列表。
    #[serde(default)]
    pub profiles: Vec<Profile>,
    /// 当前激活 Profile id。空串表示「默认」（未保存的现场）。
    #[serde(default)]
    pub active_profile_id: String,
    /// 下一次自动创建 `配置N` 时使用的编号。
    #[serde(default = "default_next_profile_number")]
    pub next_profile_number: u32,
}

fn default_next_profile_number() -> u32 {
    1
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            active_profile_id: String::new(),
            next_profile_number: default_next_profile_number(),
        }
    }
}
```

Update existing tests in the same file that construct `ProfileSettings` by adding `next_profile_number: 2` or `next_profile_number: 1` explicitly.

- [ ] **Step 4: Run Profile type tests and verify they pass**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml profile_settings -- --nocapture
```

Expected: all `profile_settings_*` tests pass.

- [ ] **Step 5: Write failing tests for automatic config names and default snapshot**

Add these tests inside `src-tauri/src/profile/mod.rs` existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn reserve_config_name_starts_at_config_one() {
    let mut settings = ProfileSettings::default();
    let name = reserve_config_name(&mut settings);
    assert_eq!(name, "配置1");
    assert_eq!(settings.next_profile_number, 2);
}

#[test]
fn reserve_config_name_uses_existing_max_number() {
    let mut settings = ProfileSettings {
        profiles: vec![
            Profile {
                id: "p1".to_string(),
                name: "配置1".to_string(),
                created_at: 1,
                updated_at: 1,
                snapshot: types::ToolSettingsSnapshot::empty(),
            },
            Profile {
                id: "p9".to_string(),
                name: "配置9".to_string(),
                created_at: 1,
                updated_at: 1,
                snapshot: types::ToolSettingsSnapshot::empty(),
            },
        ],
        active_profile_id: "p1".to_string(),
        next_profile_number: 2,
    };

    let name = reserve_config_name(&mut settings);

    assert_eq!(name, "配置10");
    assert_eq!(settings.next_profile_number, 11);
}

#[test]
fn reserve_config_name_skips_existing_manual_name() {
    let mut settings = ProfileSettings {
        profiles: vec![Profile {
            id: "manual".to_string(),
            name: "配置2".to_string(),
            created_at: 1,
            updated_at: 1,
            snapshot: types::ToolSettingsSnapshot::empty(),
        }],
        active_profile_id: "manual".to_string(),
        next_profile_number: 2,
    };

    let name = reserve_config_name(&mut settings);

    assert_eq!(name, "配置3");
    assert_eq!(settings.next_profile_number, 4);
}

#[test]
fn build_default_snapshot_includes_all_tools() {
    let snapshot = build_default_snapshot();
    assert!(snapshot.morse.is_some());
    assert!(snapshot.timer.is_some());
    assert!(snapshot.counter.is_some());
    assert!(snapshot.rapidfire.is_some());
    assert!(snapshot.audio.is_some());
}
```

- [ ] **Step 6: Run Rust tests and verify they fail**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml reserve_config_name -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml build_default_snapshot -- --nocapture
```

Expected: fail because `reserve_config_name` and `build_default_snapshot` are not defined.

- [ ] **Step 7: Implement Profile helper functions**

Add these helpers near `generate_profile_id()` in `src-tauri/src/profile/mod.rs`:

```rust
fn max_config_number(profiles: &[Profile]) -> u32 {
    profiles
        .iter()
        .filter_map(|profile| profile.name.strip_prefix("配置"))
        .filter_map(|suffix| suffix.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
}

fn reserve_config_name(settings: &mut ProfileSettings) -> String {
    let mut number = settings
        .next_profile_number
        .max(max_config_number(&settings.profiles).saturating_add(1))
        .max(1);

    loop {
        let name = format!("配置{number}");
        settings.next_profile_number = number.saturating_add(1);
        if !settings.profiles.iter().any(|profile| profile.name == name) {
            return name;
        }
        number = number.saturating_add(1);
    }
}

fn build_default_snapshot() -> types::ToolSettingsSnapshot {
    types::ToolSettingsSnapshot {
        morse: Some(morse::MorseSettings::default()),
        timer: Some(timer::TimerSettings::default()),
        counter: Some(counter::CounterSettings::default()),
        rapidfire: Some(rapidfire::RapidfireSettings::default()),
        audio: Some(audio::AudioSettings::default()),
    }
}

fn append_profile(
    settings: &mut ProfileSettings,
    name: String,
    snapshot: types::ToolSettingsSnapshot,
) -> Profile {
    let now = now_ms();
    let profile = Profile {
        id: generate_profile_id(),
        name,
        created_at: now,
        updated_at: now,
        snapshot,
    };
    settings.active_profile_id = profile.id.clone();
    settings.profiles.push(profile.clone());
    profile
}
```

- [ ] **Step 8: Refactor `profile_save_current` to use `append_profile`**

Replace the body that manually builds and pushes `Profile` in `profile_save_current` with:

```rust
let snapshot = snapshot_current_settings(&app)?;

let profile = {
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "Profile 状态锁已损坏")?;
    let profile = append_profile(&mut settings, name.trim().to_string(), snapshot);
    settings::save_settings(&app, &settings)?;
    profile
};

Ok(profile)
```

- [ ] **Step 9: Implement default profile creation and active snapshot patching**

Add this enum and function in `src-tauri/src/profile/mod.rs` after `profile_rename`:

```rust
pub(crate) enum ActiveProfileSnapshotPatch {
    Morse(morse::MorseSettings),
    Timer(timer::TimerSettings),
    Counter(counter::CounterSettings),
    Rapidfire(rapidfire::RapidfireSettings),
    Audio(audio::AudioSettings),
}

pub(crate) fn update_active_profile_snapshot(
    app: &AppHandle,
    patch: ActiveProfileSnapshotPatch,
) -> Result<(), String> {
    let Some(state) = app.try_state::<ProfileState>() else {
        return Ok(());
    };

    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "Profile 状态锁已损坏")?;

    if settings.profiles.is_empty() || settings.active_profile_id.is_empty() {
        return Ok(());
    }

    let active_id = settings.active_profile_id.clone();
    let Some(profile) = settings
        .profiles
        .iter_mut()
        .find(|profile| profile.id == active_id)
    else {
        return Ok(());
    };

    match patch {
        ActiveProfileSnapshotPatch::Morse(value) => profile.snapshot.morse = Some(value),
        ActiveProfileSnapshotPatch::Timer(value) => profile.snapshot.timer = Some(value),
        ActiveProfileSnapshotPatch::Counter(value) => profile.snapshot.counter = Some(value),
        ActiveProfileSnapshotPatch::Rapidfire(value) => profile.snapshot.rapidfire = Some(value),
        ActiveProfileSnapshotPatch::Audio(value) => profile.snapshot.audio = Some(value),
    }
    profile.updated_at = now_ms();
    settings::save_settings(app, &settings)
}
```

Add this command near the existing Profile commands:

```rust
#[tauri::command]
pub fn profile_create_default(
    app: AppHandle,
    state: State<'_, ProfileState>,
) -> Result<ProfileBootstrap, String> {
    let snapshot = build_default_snapshot();
    let profile_id = {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        let name = reserve_config_name(&mut settings);
        let profile = append_profile(&mut settings, name, snapshot.clone());
        settings::save_settings(&app, &settings)?;
        profile.id
    };

    apply_snapshot_to_tools(&app, &snapshot)?;

    {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        settings.active_profile_id = profile_id;
        settings::save_settings(&app, &settings)?;
    }

    Ok(build_bootstrap(&state))
}
```

- [ ] **Step 10: Ensure `profile_get_bootstrap` creates real `配置1`**

Replace `profile_get_bootstrap` in `src-tauri/src/profile/mod.rs` with:

```rust
#[tauri::command]
pub fn profile_get_bootstrap(
    app: AppHandle,
    state: State<'_, ProfileState>,
) -> Result<ProfileBootstrap, String> {
    let needs_default = {
        let settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        settings.profiles.is_empty()
    };

    if needs_default {
        let snapshot = snapshot_current_settings(&app)?;
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "Profile 状态锁已损坏")?;
        let name = reserve_config_name(&mut settings);
        append_profile(&mut settings, name, snapshot);
        settings::save_settings(&app, &settings)?;
    }

    Ok(build_bootstrap(&state))
}
```

- [ ] **Step 11: Register new command**

In `src-tauri/src/lib.rs`, add `profile::profile_create_default` after `profile::profile_save_current`:

```rust
            profile::profile_get_bootstrap,
            profile::profile_save_current,
            profile::profile_create_default,
            profile::profile_apply,
            profile::profile_delete,
            profile::profile_rename,
```

- [ ] **Step 12: Run task validators**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml profile -- --nocapture
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: both commands pass.

- [ ] **Step 13: Commit Task 1**

```powershell
git add 'src-tauri/src/profile/types.rs' 'src-tauri/src/profile/mod.rs' 'src-tauri/src/lib.rs'
git commit -m "实现配置槽基础语义" -m "Co-authored-by: factory-droid[bot] <138933559+factory-droid[bot]@users.noreply.github.com>"
```

---

### Task 2: 工具保存同步 active Profile snapshot

**Files:**
- Modify: `src-tauri/src/morse/mod.rs`
- Modify: `src-tauri/src/timer/mod.rs`
- Modify: `src-tauri/src/counter/mod.rs`
- Modify: `src-tauri/src/rapidfire/mod.rs`
- Modify: `src-tauri/src/audio/mod.rs`

**Interfaces:**
- Consumes: `profile::update_active_profile_snapshot(app, ActiveProfileSnapshotPatch::<Tool>(settings)) -> Result<(), String>`.
- Produces: 所有工具设置保存成功后，当前 active Profile snapshot 与工具 settings 文件保持一致。

- [ ] **Step 1: Add Morse snapshot sync imports and calls**

At the top of `src-tauri/src/morse/mod.rs`, add:

```rust
use crate::profile::{self, ActiveProfileSnapshotPatch};
```

In `morse_save_settings`, after `inner.settings = settings_value;` and before returning, clone the bootstrap settings:

```rust
let bootstrap = crate::tool_base::ToolLogic::build_bootstrap(&inner);
drop(inner);
profile::update_active_profile_snapshot(
    &app,
    ActiveProfileSnapshotPatch::Morse(bootstrap.settings.clone()),
)?;
Ok(bootstrap)
```

In `morse_overlay_submit_selection`, inside `if is_complete`, after `settings::save_settings(&app, &settings_snapshot)?;`, add:

```rust
profile::update_active_profile_snapshot(
    &app,
    ActiveProfileSnapshotPatch::Morse(settings_snapshot),
)?;
```

In `morse_overlay_finish_early`, after `settings::save_settings(&app, &settings_snapshot)?;`, add:

```rust
profile::update_active_profile_snapshot(
    &app,
    ActiveProfileSnapshotPatch::Morse(settings_snapshot),
)?;
```

- [ ] **Step 2: Add Timer snapshot sync**

At the top of `src-tauri/src/timer/mod.rs`, add:

```rust
use crate::profile::{self, ActiveProfileSnapshotPatch};
```

In `timer_save_settings`, after `emit_state(&app, bootstrap.clone());` and before `Ok(bootstrap)`, add:

```rust
profile::update_active_profile_snapshot(
    &app,
    ActiveProfileSnapshotPatch::Timer(bootstrap.settings.clone()),
)?;
```

In `timer_position_commit`, find the point after the command saves the updated settings and builds `bootstrap`. Before `Ok(bootstrap)`, add the same call:

```rust
profile::update_active_profile_snapshot(
    &app,
    ActiveProfileSnapshotPatch::Timer(bootstrap.settings.clone()),
)?;
```

- [ ] **Step 3: Add Counter snapshot sync**

At the top of `src-tauri/src/counter/mod.rs`, add:

```rust
use crate::profile::{self, ActiveProfileSnapshotPatch};
```

In `counter_save_settings`, after `emit_state(&app, bootstrap.clone());` and before `Ok(bootstrap)`, add:

```rust
profile::update_active_profile_snapshot(
    &app,
    ActiveProfileSnapshotPatch::Counter(bootstrap.settings.clone()),
)?;
```

In `counter_position_commit`, before `Ok(bootstrap)`, add:

```rust
profile::update_active_profile_snapshot(
    &app,
    ActiveProfileSnapshotPatch::Counter(bootstrap.settings.clone()),
)?;
```

- [ ] **Step 4: Add Rapidfire snapshot sync**

At the top of `src-tauri/src/rapidfire/mod.rs`, add:

```rust
use crate::profile::{self, ActiveProfileSnapshotPatch};
```

In `rapidfire_save_settings`, after `emit_state(&app, bootstrap.clone());` and before `Ok(bootstrap)`, add:

```rust
profile::update_active_profile_snapshot(
    &app,
    ActiveProfileSnapshotPatch::Rapidfire(bootstrap.settings.clone()),
)?;
```

In `rapidfire_position_commit`, before `Ok(bootstrap)`, add:

```rust
profile::update_active_profile_snapshot(
    &app,
    ActiveProfileSnapshotPatch::Rapidfire(bootstrap.settings.clone()),
)?;
```

- [ ] **Step 5: Add Audio snapshot sync**

At the top of `src-tauri/src/audio/mod.rs`, add:

```rust
use crate::profile::{self, ActiveProfileSnapshotPatch};
```

In `audio_save_settings`, after the command saves settings and emits state, before returning the bootstrap, add:

```rust
profile::update_active_profile_snapshot(
    &app,
    ActiveProfileSnapshotPatch::Audio(bootstrap.settings.clone()),
)?;
```

In `audio_overlay_submit_selection`, after the command persists the updated audio settings, add:

```rust
profile::update_active_profile_snapshot(
    &app,
    ActiveProfileSnapshotPatch::Audio(settings_snapshot),
)?;
```

- [ ] **Step 6: Run Rust check**

Run:

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: pass. If the borrow checker reports a held lock around `update_active_profile_snapshot`, create `bootstrap` inside a scoped block, exit the block, then call the profile update with `bootstrap.settings.clone()`.

- [ ] **Step 7: Run focused Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml profile -- --nocapture
```

Expected: pass.

- [ ] **Step 8: Commit Task 2**

```powershell
git add 'src-tauri/src/morse/mod.rs' 'src-tauri/src/timer/mod.rs' 'src-tauri/src/counter/mod.rs' 'src-tauri/src/rapidfire/mod.rs' 'src-tauri/src/audio/mod.rs'
git commit -m "同步工具保存到当前配置" -m "Co-authored-by: factory-droid[bot] <138933559+factory-droid[bot]@users.noreply.github.com>"
```

---

### Task 3: 前端 Profile Provider 与纯逻辑

**Files:**
- Modify: `src/components/app/profile-types.ts`
- Modify: `src/components/app/profile-utils.ts`
- Modify: `src/components/app/profile-utils.test.ts`
- Modify: `src/hooks/use-profile.tsx`

**Interfaces:**
- Consumes:
  - Tauri command `profile_get_bootstrap` now guarantees at least one real profile in native shell.
  - Tauri command `profile_create_default`.
  - Existing `profile_apply` and `profile_rename`.
- Produces:
  - `getActiveProfile(boot: ProfileBootstrap | null): Profile | null`
  - `getProfileDisplayName(boot: ProfileBootstrap | null): string`
  - `sortProfilesForSwitcher(profiles: readonly Profile[], activeProfileId: string): Profile[]`
  - `useProfile().activeProfile`
  - `useProfile().activeProfileName`
  - `useProfile().createDefaultProfile(): Promise<void>`

- [ ] **Step 1: Write failing frontend utility tests**

Add these tests to `src/components/app/profile-utils.test.ts`:

```ts
import {
    getActiveProfile,
    getProfileDisplayName,
    sortProfilesForSwitcher,
} from "@/components/app/profile-utils";

describe("getActiveProfile", () => {
    it("返回当前激活配置", () => {
        const boot: ProfileBootstrap = {
            profiles: [makeProfile("a", "配置1"), makeProfile("b", "配置2")],
            activeProfileId: "b",
        };
        expect(getActiveProfile(boot)?.name).toBe("配置2");
    });

    it("无 bootstrap 或未命中时返回 null", () => {
        expect(getActiveProfile(null)).toBeNull();
        expect(getActiveProfile({profiles: [makeProfile("a")], activeProfileId: "missing"})).toBeNull();
    });
});

describe("getProfileDisplayName", () => {
    it("无配置时显示配置1", () => {
        expect(getProfileDisplayName(null)).toBe("配置1");
        expect(getProfileDisplayName({profiles: [], activeProfileId: ""})).toBe("配置1");
    });

    it("有激活配置时显示配置名称", () => {
        const boot: ProfileBootstrap = {
            profiles: [makeProfile("a", "配置A")],
            activeProfileId: "a",
        };
        expect(getProfileDisplayName(boot)).toBe("配置A");
    });
});

describe("sortProfilesForSwitcher", () => {
    it("当前配置排在第一位，其余保持原顺序", () => {
        const profiles = [makeProfile("a"), makeProfile("b"), makeProfile("c")];
        expect(sortProfilesForSwitcher(profiles, "b").map((p) => p.id)).toEqual(["b", "a", "c"]);
    });
});
```

- [ ] **Step 2: Run utility tests and verify they fail**

Run:

```powershell
bunx vitest run src/components/app/profile-utils.test.ts
```

Expected: fail because the new functions are not exported.

- [ ] **Step 3: Implement frontend utility functions**

Append to `src/components/app/profile-utils.ts`:

```ts
export function getActiveProfile(boot: ProfileBootstrap | null): Profile | null {
    if (!boot) return null;
    return boot.profiles.find((profile) => profile.id === boot.activeProfileId) ?? null;
}

export function getProfileDisplayName(boot: ProfileBootstrap | null): string {
    return getActiveProfile(boot)?.name ?? "配置1";
}

export function sortProfilesForSwitcher(
    profiles: readonly Profile[],
    activeProfileId: string,
): Profile[] {
    const active = profiles.find((profile) => profile.id === activeProfileId);
    const rest = profiles.filter((profile) => profile.id !== activeProfileId);
    return active ? [active, ...rest] : [...profiles];
}
```

- [ ] **Step 4: Update frontend ProfileSettings type**

In `src/components/app/profile-types.ts`, change `ProfileSettings`:

```ts
/** Profile 持久化设置。 */
export interface ProfileSettings {
    profiles: Profile[];
    /** 当前激活 Profile id。 */
    activeProfileId: string;
    /** 下一次自动创建 `配置N` 时使用的编号。 */
    nextProfileNumber: number;
}
```

- [ ] **Step 5: Update ProfileProvider interface**

In `src/hooks/use-profile.tsx`, update imports:

```ts
import {
    getActiveProfile,
    getProfileDisplayName,
} from "@/components/app/profile-utils";
```

Replace `ProfileContextValue` with:

```ts
type ProfileContextValue = {
    /** 当前 bootstrap（含全部 Profile 列表与激活 id）。 */
    bootstrap: ProfileBootstrap | null;
    /** 当前激活配置对象。 */
    activeProfile: Profile | null;
    /** 顶栏显示名；无 bootstrap 时回退为“配置1”。 */
    activeProfileName: string;
    /** 是否正在加载初始 bootstrap。 */
    loading: boolean;
    /** 错误信息。 */
    error: string | null;
    /** 切换 Profile 后自增的 nonce。 */
    reloadNonce: number;
    /** 新建一个全默认配置并立即切换过去。 */
    createDefaultProfile: () => Promise<void>;
    /** 切换到指定 Profile：写盘 + reload 各工具 + 重置计数器运行值。 */
    switchProfile: (id: string) => Promise<void>;
    /** 重命名 Profile。 */
    renameProfile: (id: string, name: string) => Promise<void>;
};
```

Remove `saveCurrentAs` and `deleteProfile` from the context value. Keep the old functions only if another file still imports them; if TypeScript reports unused local declarations, delete those declarations too.

- [ ] **Step 6: Add create default action**

In `src/hooks/use-profile.tsx`, add:

```ts
const createDefaultProfile = useCallback(async () => {
    try {
        const boot = await invoke<ProfileBootstrap>("profile_create_default");
        setBootstrap(boot);
        setReloadNonce((n) => n + 1);
    } catch (err: unknown) {
        setError(String(err));
        throw err;
    }
}, []);
```

Add derived values before `useMemo`:

```ts
const activeProfile = useMemo(() => getActiveProfile(bootstrap), [bootstrap]);
const activeProfileName = useMemo(() => getProfileDisplayName(bootstrap), [bootstrap]);
```

Update the `value` object:

```ts
const value = useMemo<ProfileContextValue>(
    () => ({
        bootstrap,
        activeProfile,
        activeProfileName,
        loading,
        error,
        reloadNonce,
        createDefaultProfile,
        switchProfile,
        renameProfile,
    }),
    [
        bootstrap,
        activeProfile,
        activeProfileName,
        loading,
        error,
        reloadNonce,
        createDefaultProfile,
        switchProfile,
        renameProfile,
    ],
);
```

- [ ] **Step 7: Run frontend focused tests**

Run:

```powershell
bunx vitest run src/components/app/profile-utils.test.ts
```

Expected: pass.

- [ ] **Step 8: Run TypeScript build check**

Run:

```powershell
bun run build
```

Expected: pass. If `profile-panel.tsx` fails because it expects removed provider methods, leave the provider methods as deprecated compatibility methods for now, or update `profile-panel.tsx` to stop compiling those destructures. Since Task 4 removes the settings entry but TypeScript still compiles exported files, the fastest safe fix is to keep compatibility methods in the context until Task 4 decides whether to delete or rewrite `profile-panel.tsx`.

- [ ] **Step 9: Commit Task 3**

```powershell
git add 'src/components/app/profile-types.ts' 'src/components/app/profile-utils.ts' 'src/components/app/profile-utils.test.ts' 'src/hooks/use-profile.tsx'
git commit -m "更新前端配置状态接口" -m "Co-authored-by: factory-droid[bot] <138933559+factory-droid[bot]@users.noreply.github.com>"
```

---

### Task 4: 顶栏 ProfileSwitcher UI 与设置页移除配置入口

**Files:**
- Create: `src/components/app/profile-switcher.tsx`
- Modify: `src/App.tsx`
- Modify: `src/components/app/settings-page.tsx`

**Interfaces:**
- Consumes:
  - `useProfile().bootstrap`
  - `useProfile().activeProfile`
  - `useProfile().activeProfileName`
  - `useProfile().loading`
  - `useProfile().error`
  - `useProfile().createDefaultProfile()`
  - `useProfile().switchProfile(id: string)`
  - `useProfile().renameProfile(id: string, name: string)`
  - `sortProfilesForSwitcher(profiles, activeProfileId)`
  - `validateProfileName(name)`
- Produces: `export function ProfileSwitcher(): JSX.Element`

- [ ] **Step 1: Create ProfileSwitcher component**

Create `src/components/app/profile-switcher.tsx`:

```tsx
import {useMemo, useState} from "react";
import {
    RiAddLine,
    RiArrowDownSLine,
    RiCheckLine,
    RiEditLine,
} from "@remixicon/react";

import {Button} from "@/components/ui/button";
import {Input} from "@/components/ui/input";
import {
    Popover,
    PopoverContent,
    PopoverTrigger,
} from "@/components/ui/popover";
import {useProfile} from "@/hooks/use-profile";
import type {Profile} from "@/components/app/profile-types";
import {
    sortProfilesForSwitcher,
    validateProfileName,
} from "@/components/app/profile-utils";
import {cn} from "@/lib/utils";

export function ProfileSwitcher() {
    const {
        bootstrap,
        activeProfile,
        activeProfileName,
        loading,
        error,
        createDefaultProfile,
        switchProfile,
        renameProfile,
    } = useProfile();
    const [open, setOpen] = useState(false);
    const [renamingId, setRenamingId] = useState<string | null>(null);
    const [renameValue, setRenameValue] = useState("");
    const [message, setMessage] = useState<string | null>(null);
    const [busy, setBusy] = useState(false);

    const profiles = useMemo(
        () => sortProfilesForSwitcher(bootstrap?.profiles ?? [], bootstrap?.activeProfileId ?? ""),
        [bootstrap?.activeProfileId, bootstrap?.profiles],
    );

    const beginRename = (profile: Profile) => {
        setRenamingId(profile.id);
        setRenameValue(profile.name);
        setMessage(null);
    };

    const commitRename = async () => {
        if (!renamingId) return;
        const validation = validateProfileName(renameValue);
        if (validation) {
            setMessage(validation);
            return;
        }
        setBusy(true);
        try {
            await renameProfile(renamingId, renameValue.trim());
            setRenamingId(null);
            setRenameValue("");
            setMessage(null);
        } catch (err) {
            setMessage(String(err));
        } finally {
            setBusy(false);
        }
    };

    const handleSwitch = async (profile: Profile) => {
        if (profile.id === activeProfile?.id) return;
        setBusy(true);
        try {
            await switchProfile(profile.id);
            setOpen(false);
            setMessage(null);
        } catch (err) {
            setMessage(String(err));
        } finally {
            setBusy(false);
        }
    };

    const handleCreate = async () => {
        setBusy(true);
        try {
            await createDefaultProfile();
            setOpen(false);
            setMessage(null);
        } catch (err) {
            setMessage(String(err));
        } finally {
            setBusy(false);
        }
    };

    return (
        <Popover open={open} onOpenChange={setOpen}>
            <PopoverTrigger asChild>
                <Button
                    type="button"
                    variant="outline"
                    size="lg"
                    className="h-[34px] min-w-[132px] justify-between border-2 px-2 text-[0.58rem]"
                    aria-label="切换配置"
                    disabled={loading}
                >
                    <span className="min-w-0 truncate">
                        {loading ? "配置加载中" : error ? "配置异常" : activeProfileName}
                    </span>
                    <RiArrowDownSLine className="size-3.5" data-icon="inline-end" aria-hidden="true"/>
                </Button>
            </PopoverTrigger>
            <PopoverContent align="end" className="w-72 gap-2 p-2">
                <div className="border-b border-[var(--chalk)] pb-2">
                    <p className="font-mono text-[0.58rem] font-black tracking-[0.18em] text-[var(--zinc)] uppercase">
                        PROFILE / CONFIG SLOT
                    </p>
                    <p className="mt-1 truncate text-sm font-black">{activeProfileName}</p>
                </div>

                {message ? (
                    <div className="border border-[var(--rust)] bg-[var(--rust)]/10 px-2 py-1 font-mono text-[0.58rem] text-[var(--rust)]">
                        {message}
                    </div>
                ) : null}

                <div className="flex max-h-64 flex-col overflow-y-auto border border-[var(--seam)]">
                    {profiles.map((profile) => {
                        const active = profile.id === activeProfile?.id;
                        const renaming = renamingId === profile.id;
                        return (
                            <div
                                key={profile.id}
                                className={cn(
                                    "grid grid-cols-[minmax(0,1fr)_auto] items-center border-b border-[var(--seam)] last:border-b-0",
                                    active ? "bg-[var(--amber)]/10" : "bg-[var(--carbon)]",
                                )}
                            >
                                {renaming ? (
                                    <div className="col-span-2 flex items-center gap-1 p-1.5">
                                        <Input
                                            value={renameValue}
                                            onChange={(event) => setRenameValue(event.target.value)}
                                            onKeyDown={(event) => {
                                                if (event.key === "Enter") void commitRename();
                                                if (event.key === "Escape") setRenamingId(null);
                                            }}
                                            className="h-7 flex-1"
                                            autoFocus
                                            spellCheck={false}
                                        />
                                        <Button
                                            type="button"
                                            size="icon-sm"
                                            variant="outline"
                                            onClick={() => void commitRename()}
                                            disabled={busy}
                                            aria-label="确认重命名"
                                        >
                                            <RiCheckLine className="size-3.5" aria-hidden="true"/>
                                        </Button>
                                    </div>
                                ) : (
                                    <>
                                        <button
                                            type="button"
                                            className="min-w-0 px-2 py-2 text-left hover:bg-[var(--slate)] focus:outline-none focus-visible:outline-2 focus-visible:outline-[var(--amber)]"
                                            onClick={() => void handleSwitch(profile)}
                                            disabled={busy}
                                        >
                                            <span className="block truncate text-xs font-black">
                                                {profile.name}
                                            </span>
                                            <span className="mt-0.5 block font-mono text-[0.56rem] text-[var(--zinc)]">
                                                {active ? "ACTIVE" : "READY"}
                                            </span>
                                        </button>
                                        <Button
                                            type="button"
                                            size="icon-sm"
                                            variant="ghost"
                                            className="mr-1"
                                            onClick={() => beginRename(profile)}
                                            disabled={busy}
                                            aria-label={`重命名 ${profile.name}`}
                                        >
                                            <RiEditLine className="size-3.5" aria-hidden="true"/>
                                        </Button>
                                    </>
                                )}
                            </div>
                        );
                    })}
                </div>

                <Button
                    type="button"
                    variant="default"
                    className="w-full justify-start"
                    onClick={() => void handleCreate()}
                    disabled={busy || loading}
                >
                    <RiAddLine className="size-4" data-icon="inline-start" aria-hidden="true"/>
                    新增配置
                </Button>
            </PopoverContent>
        </Popover>
    );
}
```

- [ ] **Step 2: Render ProfileSwitcher left of GlobalSwitch**

In `src/App.tsx`, add import:

```tsx
import {ProfileSwitcher} from "@/components/app/profile-switcher";
```

Replace:

```tsx
<div className="flex items-center gap-3">
    <GlobalSwitch/>
```

with:

```tsx
<div className="flex items-center gap-3">
    <ProfileSwitcher/>
    <GlobalSwitch/>
```

- [ ] **Step 3: Remove settings Profile tab**

In `src/components/app/settings-page.tsx`, change imports:

```tsx
import {useState} from "react";
import {RiPaletteLine, RiInformationLine} from "@remixicon/react";
```

Remove:

```tsx
import {ProfilePanel} from "@/components/app/profile-panel";
```

Change prop type:

```tsx
initialTab?: "theme" | "about";
```

Change state:

```tsx
const [tab, setTab] = useState<"theme" | "about">(initialTab);
```

Change description:

```tsx
<DialogDescription>
    主题外观与软件信息
</DialogDescription>
```

Remove the `TabsTrigger value="profile"` block and the `TabsContent value="profile"` block. Keep only theme and about triggers:

```tsx
<TabsList className="w-full">
    <TabsTrigger value="theme" className="flex-1">
        <RiPaletteLine className="size-3.5" data-icon="inline-start" aria-hidden="true"/>
        主题
    </TabsTrigger>
    <TabsTrigger value="about" className="flex-1">
        <RiInformationLine className="size-3.5" data-icon="inline-start" aria-hidden="true"/>
        关于
    </TabsTrigger>
</TabsList>
```

- [ ] **Step 4: Run frontend build**

Run:

```powershell
bun run build
```

Expected: pass.

- [ ] **Step 5: Run frontend focused tests**

Run:

```powershell
bunx vitest run src/components/app/profile-utils.test.ts
```

Expected: pass.

- [ ] **Step 6: Commit Task 4**

```powershell
git add 'src/components/app/profile-switcher.tsx' 'src/App.tsx' 'src/components/app/settings-page.tsx'
git commit -m "添加顶栏配置下拉框" -m "Co-authored-by: factory-droid[bot] <138933559+factory-droid[bot]@users.noreply.github.com>"
```

---

### Task 5: Final validation and regression sweep

**Files:**
- No code files expected.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: verified implementation ready for review.

- [ ] **Step 1: Inspect git status**

Run:

```powershell
git status --short
```

Expected: no unexpected untracked files. Only intended source changes may remain if earlier commit steps were skipped.

- [ ] **Step 2: Run frontend unit tests**

Run:

```powershell
bun run test
```

Expected: all Vitest tests pass.

- [ ] **Step 3: Run frontend type and build check**

Run:

```powershell
bun run build
```

Expected: TypeScript and Vite build pass.

- [ ] **Step 4: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all Rust tests pass.

- [ ] **Step 5: Run Rust compile check**

Run:

```powershell
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: pass.

- [ ] **Step 6: Manual desktop smoke test**

Run:

```powershell
bun run tauri dev
```

Expected:

1. 顶栏全局开关左边显示 `配置1`。
2. 首次启动如果没有 profile，`profile_settings.json` 中存在真实 `配置1`。
3. 修改计时器设置后切到其他配置再切回，修改仍存在。
4. 点击 `新增配置` 后出现 `配置2`，工具设置恢复默认值。
5. 重命名当前配置后顶栏立即显示新名称。
6. 设置 Dialog 只显示 `主题 / 关于`，没有配置管理入口。
7. 顶栏没有删除配置入口。

- [ ] **Step 7: Review diff for sensitive data**

Run:

```powershell
git diff --cached
git diff
```

Expected: no secrets、token、cookie、access token、个人路径或构建产物被加入。

- [ ] **Step 8: Commit validation-only leftovers if any**

If validators changed lockfiles or generated source files, inspect them first, then run:

```powershell
git add <明确需要提交的文件>
git commit -m "验证配置下拉变更" -m "Co-authored-by: factory-droid[bot] <138933559+factory-droid[bot]@users.noreply.github.com>"
```

Expected: commit succeeds only when there are real, reviewed changes.

---

## Self-Review

**Spec coverage:**  
- 顶栏全局开关左侧下拉框：Task 4。  
- 无配置显示并创建真实 `配置1`：Task 1、Task 3。  
- 新增配置常驻按钮：Task 4。  
- 新增后配置为默认值：Task 1。  
- 后续变更保存到当前配置：Task 2。  
- 更改配置名：Task 3、Task 4。  
- 设置页配置操作移除：Task 4。  
- 不提供删除配置：Task 4。  
- 旧设置迁移到 `配置1`：Task 1。

**Placeholder scan:**  
本文没有 `TBD`、`TODO`、`implement later`、未定义函数引用或“照前面做”的步骤。每个新增接口在任务中定义，后续任务按定义消费。

**Type consistency:**  
- Rust 使用 `next_profile_number`，serde 输出为 `nextProfileNumber`。  
- Frontend 使用 `nextProfileNumber`。  
- Tauri command 名称为 `profile_create_default`。  
- Frontend provider 方法名为 `createDefaultProfile`。  
- Snapshot patch enum 名称为 `ActiveProfileSnapshotPatch`，变体为 `Morse`、`Timer`、`Counter`、`Rapidfire`、`Audio`。
