# Issues #81-#83 Recognition Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** 修复识色点击坐标、禁用草稿保存、常驻识别重复触发，同时保持配置格式与 Tauri 接口不变。

**Architecture:** 前端与 Rust 只对运行态卡片执行完整性校验。颜色匹配结果携带最接近目标色的实际命中像素。RegionWatch、ColorWatch 常驻循环共享小型边沿门控状态，每次 poll 完成匹配后再决定是否触发。

**Tech Stack:** React 19、TypeScript、Vitest、Tauri 2、Rust、Tokio、image、Bun

---

## 执行前置与文件职责

- 每个编码 worker 首次编辑前必须读取并调用项目 ponytail skill。
- 使用隔离 worktree 时，执行前调用 superpowers:using-git-worktrees。
- recognition-utils.ts 负责前端转换和保存校验；recognition/mod.rs 负责后端最终校验。
- watcher/matching.rs 负责命中坐标；watcher/manager.rs 负责坐标转换、poll、边沿状态。
- effects.rs 消费点击点；droid-wiki/features/recognition.md 记录用户可观察行为。
- 不改 command、capability、事件、查询参数、窗口 label、持久化字段。

### Task 1: 前端允许禁用卡片与禁用分组保存草稿

**Files:**
- Modify: src/components/app/recognition-utils.test.ts
- Modify: src/components/app/recognition-utils.ts:219-430

- [ ] **Step 1: 添加失败测试**

导入 RecognitionCardForm，增加完整 helper：

~~~ts
function draftCard(overrides: Partial<RecognitionCardForm> = {}): RecognitionCardForm {
    return {
        id: "draft", groupId: DEFAULT_RECOGNITION_GROUP_ID, order: 0,
        name: "草稿卡片", enabled: false, triggerMode: "hotkey", hotkey: "",
        watchRegion: null, watchReferenceImagePath: "", watchMatchThreshold: "0.75",
        watchPollIntervalMs: "500", activationMode: "always", activationHotkey: "",
        activationDurationMs: "10000", activationTriggerCount: "1",
        audioEffectEnabled: true, hotkeyEffectEnabled: false, clickEffectEnabled: false,
        effectHotkey: "", hotkeyEffectSteps: [], clickMode: "customRegion",
        clickCustomRegion: null, clickColorProbeIndex: "", audioFiles: [],
        playMode: "single", comboWindowMs: "60000", comboWindows: [], volume: "0.8",
        cooldownMs: "1000", allowSimultaneous: false, colorProbes: [],
        colorMatchMode: "all", colorMatchMethod: "average", ...overrides,
    };
}
~~~

增加四个用例：

~~~ts
it("允许禁用卡片保存未完成草稿", () => {
    const settings = parseSettingsForm({audioEnabled: true, cards: [draftCard()]});
    expect(settings.cards[0].hotkey).toBeNull();
    expect(settings.cards[0].effects?.audio?.audioFiles).toEqual([]);
});

it("允许禁用分组中的启用卡片保存未完成草稿", () => {
    const settings = parseSettingsForm({
        audioEnabled: true,
        cardGroups: [{id: "disabled", name: "禁用组", order: 0, collapsed: false, enabled: false}],
        cards: [draftCard({enabled: true, groupId: "disabled"})],
    });
    expect(settings.cards[0].hotkey).toBeNull();
});

it("启用未完成草稿时恢复严格校验", () => {
    expect(() => parseSettingsForm({audioEnabled: true, cards: [draftCard({enabled: true})]}))
        .toThrow("快捷键模式下必须设置触发快捷键");
});

it("禁用草稿仍校验数值范围", () => {
    expect(() => parseSettingsForm({audioEnabled: true, cards: [draftCard({cooldownMs: "-1"})]}))
        .toThrow("冷却时间必须在 0 到 60000 毫秒之间");
});
~~~

- [ ] **Step 2: 运行测试确认前两个用例失败**

Run: bunx vitest run src/components/app/recognition-utils.test.ts

Expected: 前两个新增用例 FAIL，错误包含“快捷键模式下必须设置触发快捷键”。

- [ ] **Step 3: 计算运行态并传给 card parser**

~~~ts
export function parseSettingsForm(form: RecognitionSettingsForm): RecognitionSettings {
    const cardGroups = form.cardGroups ?? [defaultRecognitionGroup()];
    const groupEnabledById = new Map(cardGroups.map((group) => [group.id, group.enabled ?? true]));
    const cards = form.cards.map((card) => {
        const groupId = card.groupId?.trim() || DEFAULT_RECOGNITION_GROUP_ID;
        const strict = card.enabled && (groupEnabledById.get(groupId) ?? true);
        return parseCardForm(card, strict);
    });
    return {
        recognitionEnabled: form.recognitionEnabled ?? form.audioEnabled ?? true,
        cardGroups,
        cards,
    };
}

function parseCardForm(form: RecognitionCardForm, strict = true): RecognitionCard {
~~~

给现有四类运行必填分支增加 strict 门控：触发快捷键、activation 快捷键、ColorWatch 至少一个 probe、至少一个效果。名称及所有数值范围不加门控。

- [ ] **Step 4: 放宽禁用草稿的效果完整性**

parseAudioEffect 增加 strict 参数；保留音量和 combo window 数值校验，改动必填分支：

~~~ts
const audioFiles = (form.audioFiles ?? []).map((file) => file.trim()).filter(Boolean);
if (strict && audioFiles.length === 0) {
    throw new Error("请至少添加一个音频文件。");
}
if (strict && playMode !== "single" && audioFiles.length < 2) {
    throw new Error("连杀或随机播放至少需要添加 2 个音频文件。");
}
~~~

调用改为 parseAudioEffect(form, strict)。按键效果仅在 strict && steps.length === 0 时抛错，禁用草稿保存 {hotkey: "", steps: []}。parseClickEffect 增加 strict 参数，仅在 strict 时拒绝 Hotkey + recognitionRegion 或无效 Color probe index；草稿中的无效 index 保存 null。

- [ ] **Step 5: 运行测试和 build**

Run: bunx vitest run src/components/app/recognition-utils.test.ts

Expected: 全部 PASS。

Run: bun run build

Expected: tsc 和 Vite build exit code 0。

- [ ] **Step 6: 提交**

~~~powershell
git add -- src/components/app/recognition-utils.ts src/components/app/recognition-utils.test.ts
git commit -m "fix(recognition): 允许禁用卡片保存草稿"
~~~

### Task 2: Rust 只校验运行态卡片

**Files:**
- Modify: src-tauri/src/recognition/mod.rs:1067-1135

- [ ] **Step 1: 添加失败测试**

~~~rust
#[test]
fn validate_accepts_incomplete_disabled_card() {
    let mut card = base_card();
    card.enabled = false;
    card.hotkey = None;
    card.effects.audio = Some(types::RecognitionAudioEffect {
        audio_files: vec![], play_mode: types::PlayMode::Combo,
        combo_window_ms: 60000, combo_windows: vec![], volume: 0.8,
        allow_simultaneous: false,
    });
    let settings = RecognitionSettings {
        recognition_enabled: true, card_groups: vec![], cards: vec![card],
    };
    validate_settings(&settings).unwrap();
}

#[test]
fn validate_accepts_incomplete_card_in_disabled_group() {
    let mut card = base_card();
    card.group_id = Some("disabled".into());
    card.hotkey = None;
    card.effects = types::RecognitionEffects::default();
    let settings = RecognitionSettings {
        recognition_enabled: true,
        card_groups: vec![types::RecognitionGroup {
            id: "disabled".into(), name: "禁用组".into(), order: 0,
            collapsed: false, enabled: false,
        }],
        cards: vec![card],
    };
    validate_settings(&settings).unwrap();
}

#[test]
fn validate_still_rejects_incomplete_enabled_card() {
    let mut card = base_card();
    card.hotkey = None;
    let settings = RecognitionSettings {
        recognition_enabled: true, card_groups: vec![], cards: vec![card],
    };
    assert!(validate_settings(&settings).unwrap_err().contains("必须设置触发快捷键"));
}
~~~

- [ ] **Step 2: 运行测试确认禁用用例失败**

Run: cargo test --manifest-path src-tauri/Cargo.toml validate_accepts_incomplete

Expected: 两个 validate_accepts_incomplete_* 用例 FAIL。

- [ ] **Step 3: 使用统一运行态 iterator**

把 validate_settings 中的循环头从：

~~~rust
for card in &settings.cards {
~~~

改为：

~~~rust
for card in runtime_cards(settings) {
~~~

保留 validate_hotkey_duplicates 及循环内全部既有校验，不复制 enabled/group 判断。

- [ ] **Step 4: 运行 validation tests**

Run: cargo test --manifest-path src-tauri/Cargo.toml recognition::tests::validate_

Expected: 新增与既有 validation tests 全部 PASS。

- [ ] **Step 5: 提交**

~~~powershell
git add -- src-tauri/src/recognition/mod.rs
git commit -m "fix(recognition): 忽略禁用草稿运行校验"
~~~

### Task 3: AnyPixel 点击实际命中像素

**Files:**
- Modify: src-tauri/src/recognition/watcher/matching.rs:397-707
- Modify: src-tauri/src/recognition/watcher/mod.rs:20-680
- Modify: src-tauri/src/recognition/watcher/manager.rs:330-370,647-680
- Modify: src-tauri/src/recognition/effects.rs:9-24,140-225

- [ ] **Step 1: 添加坐标失败测试**

~~~rust
#[test]
fn any_pixel_returns_closest_matching_position() {
    let mut img = RgbaImage::from_pixel(3, 3, Rgba([0, 0, 0, 255]));
    img.put_pixel(0, 2, Rgba([105, 100, 100, 255]));
    img.put_pixel(2, 0, Rgba([101, 100, 100, 255]));
    let result = scan_region_for_color(
        &DynamicImage::ImageRgba8(img), [100, 100, 100], 10.0, true,
    );
    assert_eq!(result.match_position, Some((2, 0)));
}

#[test]
fn match_color_probes_keeps_match_position() {
    let mut img = RgbaImage::from_pixel(3, 3, Rgba([0, 0, 0, 255]));
    img.put_pixel(1, 2, Rgba([100, 100, 100, 255]));
    let probes = vec![ColorProbe {
        region: Some(RegionRect {x: 10, y: 20, width: 3, height: 3}),
        targets: vec![ColorTarget {color: [100, 100, 100], tolerance: 0}],
        probe_match_mode: ColorMatchMode::Any,
        legacy_target_color: None, legacy_tolerance: None,
    }];
    let result = match_color_probes(
        &[DynamicImage::ImageRgba8(img)], &probes,
        ColorMatchMode::All, ColorMatchMethod::AnyPixel,
    );
    assert_eq!(result.matched_probes[0].match_position, Some((1, 2)));
}
~~~

在 effects.rs 增加：

~~~rust
#[test]
fn color_click_uses_actual_match_point() {
    let effect = RecognitionClickEffect {
        mode: RecognitionClickMode::RecognitionRegion,
        custom_region: None, color_probe_index: Some(0),
    };
    let context = TriggerContext::Color {
        matched_probes: vec![ColorProbeMatch {index: 0, point_x: 31, point_y: 42}],
    };
    assert_eq!(click_point_for_effect(&effect, &context), Some((31, 42)));
}
~~~

在 manager.rs tests 增加坐标转换覆盖：

~~~rust
#[test]
fn any_pixel_context_adds_probe_origin() {
    let probes = vec![crate::recognition::types::ColorProbe {
        region: Some(crate::morse::types::RegionRect {x: 100, y: 200, width: 20, height: 10}),
        targets: vec![],
        probe_match_mode: crate::recognition::types::ColorMatchMode::Any,
        legacy_target_color: None,
        legacy_tolerance: None,
    }];
    let result = matching::ColorMatchResult {
        matched: true,
        hit_count: 1,
        matched_probes: vec![matching::MatchedColorProbe {index: 0, match_position: Some((3, 4))}],
    };
    let TriggerContext::Color {matched_probes} = color_trigger_context(
        &result, &probes, &crate::recognition::types::ColorMatchMethod::AnyPixel,
    ) else { panic!("应生成识色上下文") };
    assert_eq!((matched_probes[0].point_x, matched_probes[0].point_y), (103, 204));
}

#[test]
fn average_context_uses_probe_center() {
    let probes = vec![crate::recognition::types::ColorProbe {
        region: Some(crate::morse::types::RegionRect {x: 100, y: 200, width: 20, height: 10}),
        targets: vec![],
        probe_match_mode: crate::recognition::types::ColorMatchMode::Any,
        legacy_target_color: None,
        legacy_tolerance: None,
    }];
    let result = matching::ColorMatchResult {
        matched: true,
        hit_count: 1,
        matched_probes: vec![matching::MatchedColorProbe {index: 0, match_position: None}],
    };
    let TriggerContext::Color {matched_probes} = color_trigger_context(
        &result, &probes, &crate::recognition::types::ColorMatchMethod::Average,
    ) else { panic!("应生成识色上下文") };
    assert_eq!((matched_probes[0].point_x, matched_probes[0].point_y), (110, 205));
}
~~~

- [ ] **Step 2: 运行并确认字段缺失**

Run: cargo test --manifest-path src-tauri/Cargo.toml any_pixel_returns_closest_matching_position

Expected: compile FAIL，PixelScanResult 没有 match_position。

- [ ] **Step 3: 扩展结果类型**

~~~rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchedColorProbe {
    pub index: usize,
    pub match_position: Option<(u32, u32)>,
}

pub(crate) struct ColorMatchResult {
    pub matched: bool,
    pub hit_count: usize,
    pub matched_probes: Vec<MatchedColorProbe>,
}
~~~

为 PixelScanResult、SingleTargetHit、TargetHit、ProbeHit 增加：

~~~rust
pub match_position: Option<(u32, u32)>,
~~~

- [ ] **Step 4: 选择全区颜色距离最小的命中像素**

scan_region_for_color 初始化并在像素循环更新：

~~~rust
let mut best_match_distance = f32::INFINITY;
let mut match_position = None;

if dist <= tolerance {
    matching_count += 1;
    if dist < best_match_distance {
        best_match_distance = dist;
        match_position = Some((x, y));
    }
    if count_only && dist == 0.0 {
        return PixelScanResult {matching_count, nearest_color, nearest_distance, match_position};
    }
}
~~~

删除“任意命中即早退”；最终返回加入 match_position。Average 的 SingleTargetHit 写 None；AnyPixel 写 scan.match_position。probe_hit_targets 透传。

aggregate_probe_hits 单独从已命中目标选择最小 distance 坐标：

~~~rust
let match_position = hits.iter()
    .filter(|hit| hit.matched)
    .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal))
    .and_then(|hit| hit.match_position);
~~~

构造 ProbeHit 时加入该字段。

- [ ] **Step 5: 结构化总匹配结果**

~~~rust
let mut matched_probes = Vec::new();
for (index, probe) in probes.iter().enumerate() {
    let hit = probe_hit(&screenshots[index], probe, method.clone(), true);
    if hit.matched {
        hit_count += 1;
        matched_probes.push(MatchedColorProbe {index, match_position: hit.match_position});
    }
}
let matched = match mode {
    ColorMatchMode::All => hit_count == probes.len(),
    ColorMatchMode::Any => hit_count > 0,
};
ColorMatchResult {matched, hit_count, matched_probes}
~~~

空输入返回 matched_probes: Vec::new()；删除所有 matched_indices 读取。

既有测试若断言索引数组，统一改为：

~~~rust
let matched_indices = result.matched_probes.iter().map(|probe| probe.index).collect::<Vec<_>>();
assert_eq!(matched_indices, vec![0]);
~~~

- [ ] **Step 6: 转换为屏幕绝对坐标**

在 manager.rs 增加，供 run_color_once、run_color_watcher 共用：

~~~rust
fn color_trigger_context(
    result: &matching::ColorMatchResult,
    probes: &[crate::recognition::types::ColorProbe],
    method: &crate::recognition::types::ColorMatchMethod,
) -> TriggerContext {
    let matched_probes = result.matched_probes.iter().filter_map(|matched| {
        let region = probes.get(matched.index)?.region.as_ref()?;
        let (point_x, point_y) = match (method, matched.match_position) {
            (crate::recognition::types::ColorMatchMethod::AnyPixel, Some((x, y))) =>
                (region.x + x as i32, region.y + y as i32),
            _ => (region.x + region.width / 2, region.y + region.height / 2),
        };
        Some(ColorProbeMatch {index: matched.index, point_x, point_y})
    }).collect();
    TriggerContext::Color {matched_probes}
}
~~~

两个颜色入口均调用 effects::execute(..., color_trigger_context(...))。

- [ ] **Step 7: 更新 effect context**

~~~rust
pub(crate) struct ColorProbeMatch {
    pub index: usize,
    pub point_x: i32,
    pub point_y: i32,
}
~~~

click_point_for_effect 映射改为 .map(|probe| (probe.point_x, probe.point_y))；同步更新既有测试构造字段。

- [ ] **Step 8: 运行测试**

Run: cargo test --manifest-path src-tauri/Cargo.toml any_pixel_

Expected: PASS。

Run: cargo test --manifest-path src-tauri/Cargo.toml color_click_

Expected: PASS。

Run: cargo test --manifest-path src-tauri/Cargo.toml

Expected: 全部 PASS，无 matched_indices、center_x、center_y 遗留错误。

- [ ] **Step 9: 提交**

~~~powershell
git add -- src-tauri/src/recognition/watcher/matching.rs src-tauri/src/recognition/watcher/mod.rs src-tauri/src/recognition/watcher/manager.rs src-tauri/src/recognition/effects.rs
git commit -m "fix(recognition): 点击识色实际命中像素"
~~~

### Task 4: 常驻识别改为目标重新出现时触发

**Files:**
- Modify: src-tauri/src/recognition/watcher/manager.rs:488-694

- [ ] **Step 1: 添加状态机失败测试**

~~~rust
#[test]
fn match_gate_triggers_once_until_explicit_miss() {
    let start = Instant::now();
    let mut gate = MatchGate::default();
    assert!(gate.observe(MatchObservation::Matched, 1000, start));
    assert!(!gate.observe(MatchObservation::Matched, 1000, start + Duration::from_secs(2)));
    assert!(!gate.observe(MatchObservation::NotMatched, 1000, start + Duration::from_secs(3)));
    assert!(gate.observe(MatchObservation::Matched, 1000, start + Duration::from_secs(4)));
}

#[test]
fn match_gate_capture_failure_does_not_rearm() {
    let start = Instant::now();
    let mut gate = MatchGate::default();
    assert!(gate.observe(MatchObservation::Matched, 0, start));
    assert!(!gate.observe(MatchObservation::CaptureFailed, 0, start + Duration::from_secs(1)));
    assert!(!gate.observe(MatchObservation::Matched, 0, start + Duration::from_secs(2)));
}

#[test]
fn match_gate_consumes_rising_edge_during_cooldown() {
    let start = Instant::now();
    let mut gate = MatchGate::default();
    assert!(gate.observe(MatchObservation::Matched, 5000, start));
    gate.observe(MatchObservation::NotMatched, 5000, start + Duration::from_secs(1));
    assert!(!gate.observe(MatchObservation::Matched, 5000, start + Duration::from_secs(2)));
    assert!(!gate.observe(MatchObservation::Matched, 5000, start + Duration::from_secs(6)));
}
~~~

- [ ] **Step 2: 运行并确认类型缺失**

Run: cargo test --manifest-path src-tauri/Cargo.toml match_gate_

Expected: compile FAIL，MatchGate/MatchObservation 未定义。

- [ ] **Step 3: 实现边沿门控**

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchObservation { CaptureFailed, Matched, NotMatched }

#[derive(Debug, Default)]
struct MatchGate {
    was_matched: bool,
    last_triggered: Option<Instant>,
}

impl MatchGate {
    fn observe(&mut self, observation: MatchObservation, cooldown_ms: u32, now: Instant) -> bool {
        match observation {
            MatchObservation::CaptureFailed => false,
            MatchObservation::NotMatched => { self.was_matched = false; false }
            MatchObservation::Matched if self.was_matched => false,
            MatchObservation::Matched => {
                self.was_matched = true;
                let ready = self.last_triggered
                    .map(|last| now.duration_since(last) >= Duration::from_millis(cooldown_ms as u64))
                    .unwrap_or(true);
                if ready { self.last_triggered = Some(now); }
                ready
            }
        }
    }
}
~~~

- [ ] **Step 4: 调整 RegionWatch poll 顺序**

用 let mut match_gate = MatchGate::default() 替代 last_triggered，删除截图前 cooldown continue。截图失败调用 CaptureFailed；相似度不足调用 NotMatched；匹配后先执行：

~~~rust
if !match_gate.observe(MatchObservation::Matched, cooldown_ms, Instant::now()) {
    continue;
}
~~~

只有 true 才 emit 并执行 effects。effects 失败不重置 gate。

- [ ] **Step 5: 调整 ColorWatch poll 顺序**

删除截图前 cooldown。任一 probe 截图失败时调用 CaptureFailed 并 continue；!result.matched 时调用 NotMatched；命中时先调用 Matched，返回 false 则跳过事件与 effects。每个 ticker tick 仍执行截图和匹配。

- [ ] **Step 6: 运行 watcher 与完整 Rust 测试**

Run: cargo test --manifest-path src-tauri/Cargo.toml match_gate_

Expected: 三个用例 PASS。

Run: cargo test --manifest-path src-tauri/Cargo.toml recognition::watcher

Expected: watcher tests PASS。

Run: cargo test --manifest-path src-tauri/Cargo.toml

Expected: 全部 PASS。

- [ ] **Step 7: 提交**

~~~powershell
git add -- src-tauri/src/recognition/watcher/manager.rs
git commit -m "fix(recognition): 目标重新出现时再触发"
~~~

### Task 5: 同步 wiki 并完成全量验证

**Files:**
- Modify: droid-wiki/features/recognition.md

- [ ] **Step 1: 更新点击规则和常驻识别规则**

将 ColorWatch 点击规则写为：

~~~markdown
- ColorWatch recognitionRegion：只在显式选择的 probe 命中时点击；anyPixel 点击该 probe 内与目标色距离最小的实际命中像素，average 点击 probe 区域中心。指定 probe 未命中时跳过点击，其他效果照常执行。
~~~

在“当前行为补充”增加：

~~~markdown
- always 常驻 RegionWatch / ColorWatch 按 watchPollIntervalMs 持续检查，但只在“未命中 → 命中”上升沿执行效果；目标持续命中不会重复触发，明确未命中后重新武装。截图失败不视为目标消失，cooldownMs 继续限制不同上升沿的最短触发间隔。
- 禁用卡片或禁用分组中的卡片可保存未完成草稿；重新启用时恢复快捷键、激活方式和效果完整性校验。
~~~

- [ ] **Step 2: 运行全量验证**

Run: bun run test

Expected: Vitest 全部 PASS。

Run: bun run build

Expected: tsc 与 Vite build exit code 0。

Run: cargo test --manifest-path src-tauri/Cargo.toml

Expected: Rust tests 全部 PASS。

Run: cargo check --manifest-path src-tauri/Cargo.toml

Expected: exit code 0。

- [ ] **Step 3: 刷新 CodeGraph 并检查 diff**

Run: codegraph sync

Expected: sync 成功。

Run: git diff --check

Expected: 无输出。

Run: git status --short

Expected: 只显示预期 wiki 或被跟踪 CodeGraph 索引变更。

- [ ] **Step 4: 提交文档**

~~~powershell
git add -- droid-wiki/features/recognition.md
git commit -m "docs(recognition): 补充点击与重新武装规则"
~~~

- [ ] **Step 5: 汇总证据**

Run: git log -5 --oneline

Expected: Task 1-5 中文 commits 可见。

Run: git status --short

Expected: worktree 无未提交业务改动。

实现完成后分别向 #81、#82、#83 回复实际修复结论与验证命令，不关闭 Issue，等待报告人确认。
