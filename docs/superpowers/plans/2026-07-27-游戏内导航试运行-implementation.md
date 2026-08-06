# 游戏内导航试运行 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 删除制作物品名称 UI，并提供从当前游戏窗口进入四制作台页面的独立导航试运行。

**Architecture:** 新建纯 Rust 导航状态机，通过小型 driver trait 隔离模板观察、窗口聚焦、点击和按键；生产 driver 复用现有截图模板匹配、桌面窗口与可取消输入能力。导航试运行复用现有单实例 runtime、operation window 和紧急停止资源，不新增并发执行器。

**Tech Stack:** React 19、TypeScript、Tauri 2、Rust、Tokio、Vitest、Rust tests。

---

### Task 1: 简化制作台 UI

**Files:**
- Modify: `src/components/app/special-ops-page.tsx`
- Test: `src/components/app/special-ops-page.test.tsx`

- [x] 添加失败测试：账号制作台不渲染“制作物品”输入框，小时与分钟仍存在。
- [x] 运行定向 Vitest，确认测试因现有输入框而失败。
- [x] 删除 `station.itemName` 对应 `DraftInput`，保留持久化字段和其余控件。
- [x] 重跑定向 Vitest，确认通过。

### Task 2: 可取消单键输入

**Files:**
- Modify: `src-tauri/src/input_simulation.rs`

- [x] 添加失败测试：单键操作必须发送一次 Press 和一次 Release；取消后不得继续输入。
- [x] 运行对应 Rust 定向测试，确认缺少 API 时失败。
- [x] 增加 `press_key_cancellable`，复用现有输入序列化、generation 和 injected key 跟踪。
- [x] 重跑定向测试，确认通过。

### Task 3: 游戏内导航纯状态机

**Files:**
- Create: `src-tauri/src/special_ops/game_navigation.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`

- [x] 添加失败测试：直接出现开始游戏时不按空格；活动弹窗出现时只按一次空格；成功动作顺序固定。
- [x] 运行定向 Rust 测试，确认模块尚未实现而失败。
- [x] 实现 `GameNavigationDriver`、步骤枚举、3 分钟独立步骤超时和取消分支。
- [x] 重跑定向测试，确认通过。

### Task 4: 生产 driver 与命令接线

**Files:**
- Modify: `src-tauri/src/special_ops/game_navigation.rs`
- Modify: `src-tauri/src/special_ops/login_runtime.rs`
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/components/app/special-ops-types.ts`
- Modify: `src/components/app/special-ops-page.tsx`
- Modify: `src/components/app/special-ops-page.test.tsx`

- [x] 添加失败测试：导航预检要求游戏路径、5 个模板与 1 个入口点击点；active run 时拒绝第二个 run。
- [x] 运行 Rust 与 Vitest 定向测试，确认失败。
- [x] 实现 `special_ops_start_navigation_trial`，复用 operation window、紧急停止热键和单实例 runtime。
- [x] 在试运行账号区增加“游戏内导航试运行”按钮，调用前先 flush settings。
- [x] 重跑 Rust 与 Vitest 定向测试，确认通过。

### Task 5: 文档与门禁

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `droid-wiki/features/special-ops.md`

- [x] 记录新 command、导航成功门槛和制作台 UI 变化。
- [x] 运行 `codegraph sync`。
- [x] 运行 `git diff --check`。
- [x] 使用独立 `CARGO_TARGET_DIR` 运行 `bun run check`。
- [ ] 现场试运行：当前游戏从模式选择页进入四制作台页面；验证活动弹窗有/无两条路径。

现场验证前不创建 commit。

### Task 6: 烽火地带入口改为点击点

**Files:**
- Modify: `src-tauri/src/special_ops/mod.rs`
- Modify: `src-tauri/src/special_ops/game_navigation.rs`
- Modify: `droid-wiki/features/special-ops.md`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `docs/superpowers/specs/2026-07-22-特勤处多账号自动化-设计草案.md`

- [x] **Step 1: 添加默认类型与旧配置迁移失败测试**

在 `src-tauri/src/special_ops/mod.rs` 增加测试，要求入口为点击点、守卫为 `game.modeReady`，并要求旧识别区域加载后清空：

```rust
#[test]
fn beacon_mode_is_click_point_guarded_by_mode_ready() {
    let target = default_calibration_targets()
        .into_iter()
        .find(|target| target.key == "game.beaconMode")
        .unwrap();
    assert_eq!(target.kind, CalibrationTargetKind::ClickPoint);
    assert_eq!(target.guard_any_of, vec!["game.modeReady".to_string()]);
}

#[test]
fn normalize_clears_legacy_beacon_recognition_region() {
    let mut settings = SpecialOpsSettings::default();
    let target = calibration_target_mut(&mut settings, "game.beaconMode");
    target.kind = CalibrationTargetKind::RecognitionRegion;
    target.rect = Some(CalibrationRect { x: 1, y: 2, width: 30, height: 40 });
    target.reference_image_path = Some("legacy.png".to_string());
    target.verified_signature = Some("legacy".to_string());
    target.verified_at_ms = Some(1);

    let normalized = normalize_settings(settings).unwrap();
    let target = normalized.calibration_environments[0]
        .targets.iter().find(|target| target.key == "game.beaconMode").unwrap();
    assert_eq!(target.kind, CalibrationTargetKind::ClickPoint);
    assert!(target.rect.is_none());
    assert!(target.reference_image_path.is_none());
    assert!(target.verified_signature.is_none());
    assert!(target.verified_at_ms.is_none());
}
```

- [x] **Step 2: 运行测试并确认 RED**

```powershell
$env:CARGO_TARGET_DIR='C:\delta-auto-tools\temp\game-navigation-target'
cargo test --manifest-path src-tauri/Cargo.toml beacon_mode -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml normalize_clears_legacy_beacon_recognition_region -- --nocapture
```

预期：默认类型仍为 `RecognitionRegion`，两项测试失败。

- [x] **Step 3: 最小实现默认类型和迁移**

在 `default_guard_any_of` 增加：

```rust
"game.beaconMode" => &["game.modeReady"],
```

将默认目标改为：

```rust
("game.beaconMode", "烽火地带入口点击点", ClickPoint),
```

在 `normalize_settings` 合并 required target 时先记录类型变化，并清空旧校准：

```rust
let kind_changed = target.kind != required.kind;
target.label = required.label.clone();
target.kind = required.kind.clone();
target.recognition_method = required.recognition_method.clone();
target.guard_any_of = required.guard_any_of.clone();
if kind_changed {
    target.rect = None;
    target.reference_image_path = None;
    target.verified_signature = None;
    target.verified_at_ms = None;
}
```

- [x] **Step 4: 重跑迁移测试并确认 GREEN**

运行 Step 2 两条命令。预期：两项测试通过。

- [x] **Step 5: 添加导航预检失败测试**

将 `navigation_trial_preflight_requires_only_game_path_account_and_six_templates` 改名为 `navigation_trial_preflight_requires_five_templates_and_beacon_click_point`。测试只给 5 个识别目标写参考图与验证签名，给 `game.beaconMode` 只写矩形，并断言：

```rust
let beacon = calibration_target_mut(&mut fixture.settings, "game.beaconMode");
beacon.rect = Some(CalibrationRect { x: 10, y: 20, width: 1, height: 1 });

let config = freeze_navigation_run_config(&fixture.settings, "selected").unwrap();
let beacon = config.targets.get("game.beaconMode").unwrap();
assert!(beacon.template.is_none());
assert_eq!(beacon.guard_any_of, vec!["game.modeReady".to_string()]);
```

- [x] **Step 6: 运行导航预检测试并确认 RED**

```powershell
$env:CARGO_TARGET_DIR='C:\delta-auto-tools\temp\game-navigation-target'
cargo test --manifest-path src-tauri/Cargo.toml navigation_trial_preflight_requires_five_templates_and_beacon_click_point -- --nocapture
```

预期：现有预检仍要求入口参考图，测试失败。

- [x] **Step 7: 允许运行配置冻结点击点**

在 `freeze_navigation_run_config` 中始终要求矩形；仅 `RecognitionRegion` 要求参考图与当前验证签名。`ClickPoint` 构造 `template: None`，并保留 `guard_any_of`：

```rust
let template = match target.kind {
    CalibrationTargetKind::RecognitionRegion => {
        let reference = target.reference_image_path.as_deref()
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| format!("导航试运行校准未完成：{} 尚未上传参考图", target.label))?;
        if !verification_is_current(target) {
            return Err(format!("导航试运行校准未完成：{} 尚未测试或验证失效", target.label));
        }
        Some(template_observer::RuntimeTemplate {
            key: key.to_string(),
            region: region.clone(),
            reference_image_path: std::fs::canonicalize(reference)
                .map_err(|_| format!("{} 的参考图文件不存在", target.label))?,
            threshold: target.match_threshold,
        })
    }
    CalibrationTargetKind::ClickPoint => None,
    CalibrationTargetKind::InputRegion => {
        return Err(format!("导航校准目标 {} 类型无效", target.label));
    }
};
targets.insert(key.to_string(), RuntimeTarget {
    key: key.to_string(),
    region,
    template,
    guard_any_of: target.guard_any_of.clone(),
});
```

复用当前 canonicalize、签名校验和错误文案；不新增持久化字段。

- [x] **Step 8: 点击动作按 `guard_any_of` 复验**

修改 `ProductionGameNavigationDriver::verify_guard`：目标自身有 template 时复验自身；没有 template 时解析 `guard_any_of` 对应模板并调用 `wait_for_any_consistent_match`。空守卫返回配置错误。`game.beaconMode` 因此在点击前再次双采样 `game.modeReady`。

```rust
if target.template.is_some() {
    return super::template_observer::wait_for_target_match(
        &RuntimeSimilaritySampler,
        target,
        cancelled,
    ).await.map(|_| ());
}
if target.guard_any_of.is_empty() {
    return Err(format!("导航点击目标 {target_key} 缺少识别守卫"));
}
let templates = target.guard_any_of.iter()
    .map(|guard_key| {
        self.config.targets.get(guard_key)
            .and_then(|guard| guard.template.as_ref())
            .ok_or_else(|| format!("导航识别守卫 {guard_key} 未配置已验证模板"))
    })
    .collect::<Result<Vec<_>, _>>()?;
super::template_observer::wait_for_any_consistent_match(
    &RuntimeSimilaritySampler,
    &templates,
    cancelled,
).await.map(|_| ())
```

- [x] **Step 9: 重跑 Rust 定向测试**

```powershell
$env:CARGO_TARGET_DIR='C:\delta-auto-tools\temp\game-navigation-target'
cargo test --manifest-path src-tauri/Cargo.toml special_ops::game_navigation -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml beacon_mode -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml navigation_trial_preflight_requires_five_templates_and_beacon_click_point -- --nocapture
```

预期：全部通过。

- [x] **Step 10: 同步用户文档**

更新 `README.md`、`AGENTS.md`、`droid-wiki/features/special-ops.md` 与总设计草案：入口为点击点；点击前守卫为 `game.modeReady`；预检为 5 个模板加 1 个点击点；旧识别区域必须重新校准。

- [x] **Step 11: 同步索引并运行完整门禁**

```powershell
codegraph sync
git diff --check
$env:CARGO_TARGET_DIR='C:\delta-auto-tools\temp\game-navigation-target'
bun run check
```

预期：TypeScript、Vitest、coverage、Rust fmt、Clippy 和 Rust tests 全部退出 0。现场验证项继续保留未完成，不创建 commit。
