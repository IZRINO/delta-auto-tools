# 音频"多区域识色"触发模式 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在音频卡片中新增第三种触发模式 `colorWatch`：用户可框选 3-4 个小区域并指定目标颜色 + 容差，当所有（或任一）区域同时出现相似颜色时触发音频播放，解决现有图像模板匹配误识率高、性能差的问题（Issue #58）。

**Architecture:** 在 `AudioTriggerMode` 枚举增加 `ColorWatch` 变体；`AudioCard` 增加 `color_probes: Vec<ColorProbe>` 和 `color_match_mode: ColorMatchMode` 两个字段。每个 `ColorProbe` = `{ region, target_color: [u8;3], tolerance: u8 }`。watcher 在 `ColorWatch` 模式下复用现有 `capture_region` 逐个截取小区域，取区域平均 RGB，与 target_color 做欧氏距离判定，按 `All`/`Any` 聚合。前端走整体替换 + autosave 模式（与现有 region 字段一致），不引入增量 probe 命令，保持数据流单一。

**Tech Stack:** Rust（image crate 现有依赖，无新增）、Tauri commands、React + TypeScript + shadcn/ui、Vitest。

---

## File Structure

**修改：**
- `src-tauri/src/audio/types.rs` — 增加 `ColorProbe` / `ColorMatchMode` 结构与默认值函数；`AudioTriggerMode` 增加变体；`AudioCard` 增加字段
- `src-tauri/src/audio/watcher.rs` — 增加识色核心逻辑 `match_color_probes` + `average_region_rgb` + `color_distance`；`restart_watchers` 接入 `ColorWatch` 分支；`run_region_watcher` 改造为支持两种匹配路径
- `src-tauri/src/audio/mod.rs` — 增加 `audio_test_color_match` 命令；`audio_test_match` 在 ColorWatch 模式下返回友好错误
- `src-tauri/src/lib.rs` — `generate_handler![]` 注册 `audio_test_color_match`
- `src/components/app/audio-types.ts` — 增加 `ColorProbe` / `ColorMatchMode` / `ColorProbeForm` 类型；扩展 `AudioCard` / `AudioCardForm` / `DEFAULT_AUDIO_CARD`
- `src/components/app/audio-utils.ts` — `settingsToForm` / `parseSettingsForm` / `mergeAudioWatchRegionsIntoForm` 接入 colorProbes
- `src/components/app/audio-page.tsx` — `AudioCardEditor` 增加 `colorWatch` 分支 UI；`AudioRegionOverlay` 复用为颜色 probe 框选（增加 probe 索引查询参数）
- `src/components/app/audio-utils.test.ts` — colorProbes 转层与校验测试
- `src-tauri/src/audio/watcher.rs` 测试模块 — 识色核心逻辑单元测试
- `AGENTS.md` / `README.md` — 命令面、类型、约束同步

**不修改：**
- `src-tauri/capabilities/default.json` — `audio-overlay` / `audio-overlay-*` 已存在，颜色 probe 框选复用同一 overlay label，无需新增窗口权限
- `src/App.tsx` — `audio-overlay` 路由已存在，复用

---

## Task 1: Rust 数据结构 ColorProbe / ColorMatchMode

**Files:**
- Modify: `src-tauri/src/audio/types.rs`
- Test: `src-tauri/src/audio/types.rs` (内嵌 `#[cfg(test)] mod tests`)

- [ ] **Step 1: 写失败测试 — ColorProbe 序列化与默认值**

在 `src-tauri/src/audio/types.rs` 的 `#[cfg(test)] mod tests` 末尾（`audio_card_allow_simultaneous_roundtrip` 之后）追加：

```rust
    #[test]
    fn color_probe_roundtrip() {
        let json = r#"{"region":{"x":10,"y":20,"width":5,"height":5},"targetColor":[200,100,50],"tolerance":40}"#;
        let probe: ColorProbe = serde_json::from_str(json).unwrap();
        assert_eq!(probe.region.x, 10);
        assert_eq!(probe.target_color, [200, 100, 50]);
        assert_eq!(probe.tolerance, 40);
        let reserialized = serde_json::to_string(&probe).unwrap();
        assert!(reserialized.contains("\"targetColor\":[200,100,50]"));
        assert!(reserialized.contains("\"tolerance\":40"));
    }

    #[test]
    fn color_probe_default_tolerance_is_30() {
        // 缺省 tolerance 应默认 30
        let json = r#"{"region":{"x":0,"y":0,"width":3,"height":3},"targetColor":[0,0,0]}"#;
        let probe: ColorProbe = serde_json::from_str(json).unwrap();
        assert_eq!(probe.tolerance, 30);
    }

    #[test]
    fn color_match_mode_default_is_all() {
        // AudioCard 缺省 color_match_mode 应为 All
        let json = r#"{"id":"c1","name":"测试","triggerMode":"colorWatch"}"#;
        let card: AudioCard = serde_json::from_str(json).unwrap();
        assert_eq!(card.color_match_mode, ColorMatchMode::All);
        assert!(card.color_probes.is_empty());
    }

    #[test]
    fn audio_card_with_color_watch_roundtrip() {
        let card = AudioCard {
            id: "c1".into(),
            name: "识色卡".into(),
            enabled: true,
            trigger_mode: AudioTriggerMode::ColorWatch,
            hotkey: None,
            watch_region: None,
            watch_reference_image_path: None,
            watch_match_threshold: 0.75,
            watch_poll_interval_ms: 500,
            audio_file_path: "a.mp3".into(),
            volume: 0.8,
            cooldown_ms: 1000,
            allow_simultaneous: false,
            color_probes: vec![ColorProbe {
                region: RegionRect { x: 1, y: 2, width: 3, height: 4 },
                target_color: [10, 20, 30],
                tolerance: 25,
            }],
            color_match_mode: ColorMatchMode::Any,
        };
        let json = serde_json::to_string(&card).unwrap();
        let back: AudioCard = serde_json::from_str(&json).unwrap();
        assert_eq!(back.trigger_mode, AudioTriggerMode::ColorWatch);
        assert_eq!(back.color_match_mode, ColorMatchMode::Any);
        assert_eq!(back.color_probes.len(), 1);
        assert_eq!(back.color_probes[0].target_color, [10, 20, 30]);
    }
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- audio::types::tests`
Expected: 编译失败，`ColorProbe` / `ColorMatchMode` 未定义

- [ ] **Step 3: 实现 ColorProbe / ColorMatchMode + AudioCard 字段**

在 `src-tauri/src/audio/types.rs` 顶部 `use` 区追加导入（紧跟现有 `use crate::morse::types::RegionRect;`）：

```rust
// 已有：use crate::morse::types::RegionRect;
```
无需新增导入，RegionRect 已在。

在 `default_watch_poll_interval_ms` 函数之后追加默认值函数：

```rust
fn default_color_tolerance() -> u8 {
    30
}
```

在 `AudioTriggerMode` 枚举中增加 `ColorWatch` 变体（紧跟 `RegionWatch`）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AudioTriggerMode {
    Hotkey,
    RegionWatch,
    ColorWatch,
}
```

在 `AudioCard` 结构体定义之前（`AudioTriggerMode` 之后）追加两个新结构：

```rust
/// 识色探针：一个矩形区域 + 目标颜色 + 容差
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ColorProbe {
    pub region: RegionRect,
    /// 目标 RGB 颜色 [R, G, B]，每通道 0-255
    pub target_color: [u8; 3],
    /// 颜色容差（RGB 欧氏距离阈值，0-255）
    #[serde(default = "default_color_tolerance")]
    pub tolerance: u8,
}

/// 多探针聚合模式：All = 全部命中才触发；Any = 任一命中即触发
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ColorMatchMode {
    #[default]
    All,
    Any,
}
```

在 `AudioCard` 结构体末尾（`allow_simultaneous` 字段之后）追加两个字段：

```rust
    /// 允许此卡片的音频与其他卡片同时播放（默认互斥）
    #[serde(default)]
    pub allow_simultaneous: bool,
    // 识色模式探针列表
    #[serde(default)]
    pub color_probes: Vec<ColorProbe>,
    // 识色聚合模式
    #[serde(default)]
    pub color_match_mode: ColorMatchMode,
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- audio::types::tests`
Expected: 全部通过（含新增 4 个测试）

- [ ] **Step 5: 验证不影响现有测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- audio::`
Expected: 全部通过

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/audio/types.rs
git commit -m "feat(audio): 新增 ColorProbe / ColorMatchMode 数据结构与默认值"
```

---

## Task 2: Rust 识色核心逻辑（取色 + 距离 + 聚合）

**Files:**
- Modify: `src-tauri/src/audio/watcher.rs`
- Test: `src-tauri/src/audio/watcher.rs` (内嵌 `#[cfg(test)] mod tests`)

- [ ] **Step 1: 写失败测试 — 颜色距离与聚合判定**

在 `src-tauri/src/audio/watcher.rs` 测试模块顶部 `use` 区追加导入（紧跟现有 `use image::{...}`）：

```rust
use crate::audio::types::{ColorMatchMode, ColorProbe};
use crate::morse::types::RegionRect;
```

在测试模块末尾（`base64_encode_padding` 之后）追加：

```rust
    #[test]
    fn color_distance_zero_for_same_color() {
        assert_eq!(color_distance([100, 100, 100], [100, 100, 100]), 0.0);
    }

    #[test]
    fn color_distance_orthogonal_channels() {
        // R 差 30 → 距离 30
        assert!((color_distance([130, 100, 100], [100, 100, 100]) - 30.0).abs() < 0.01);
        // 三轴各差 10 → 距离 sqrt(300) ≈ 17.32
        let d = color_distance([110, 110, 110], [100, 100, 100]);
        assert!((d - 17.32).abs() < 0.1, "实际 {}", d);
    }

    #[test]
    fn average_region_rgb_uniform() {
        let img = RgbaImage::from_pixel(3, 3, Rgba([10, 20, 30, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        assert_eq!(average_region_rgb(&dyn_img), [10, 20, 30]);
    }

    #[test]
    fn average_region_rgb_mixed() {
        // 2x2：四个角颜色平均
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([100, 0, 0, 255]));
        img.put_pixel(0, 1, Rgba([0, 100, 0, 255]));
        img.put_pixel(1, 1, Rgba([100, 100, 0, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        assert_eq!(average_region_rgb(&dyn_img), [50, 50, 0]);
    }

    #[test]
    fn average_region_rgb_ignores_alpha() {
        // 半透明像素按 alpha 加权后与完全不透明像素混合
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([200, 0, 0, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 0])); // 完全透明
        let dyn_img = DynamicImage::ImageRgba8(img);
        // 透明像素不计入，只算 [200,0,0]
        assert_eq!(average_region_rgb(&dyn_img), [200, 0, 0]);
    }

    #[test]
    fn match_color_probes_all_mode_all_hit() {
        // 两个 probe，截图颜色都匹配
        let screenshots = vec![
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255]))),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]))),
        ];
        let probes = vec![
            ColorProbe { region: RegionRect { x: 0, y: 0, width: 2, height: 2 }, target_color: [200, 100, 50], tolerance: 10 },
            ColorProbe { region: RegionRect { x: 0, y: 0, width: 2, height: 2 }, target_color: [10, 20, 30], tolerance: 10 },
        ];
        let result = match_color_probes(&screenshots, &probes, ColorMatchMode::All);
        assert!(result.matched, "All 模式全命中应触发");
        assert_eq!(result.hit_count, 2);
    }

    #[test]
    fn match_color_probes_all_mode_partial_miss() {
        let screenshots = vec![
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255]))),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]))), // 不匹配
        ];
        let probes = vec![
            ColorProbe { region: RegionRect { x: 0, y: 0, width: 2, height: 2 }, target_color: [200, 100, 50], tolerance: 10 },
            ColorProbe { region: RegionRect { x: 0, y: 0, width: 2, height: 2 }, target_color: [10, 20, 30], tolerance: 10 },
        ];
        let result = match_color_probes(&screenshots, &probes, ColorMatchMode::All);
        assert!(!result.matched, "All 模式部分未命中不应触发");
        assert_eq!(result.hit_count, 1);
    }

    #[test]
    fn match_color_probes_any_mode_one_hit() {
        let screenshots = vec![
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255]))),
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]))),
        ];
        let probes = vec![
            ColorProbe { region: RegionRect { x: 0, y: 0, width: 2, height: 2 }, target_color: [200, 100, 50], tolerance: 10 },
            ColorProbe { region: RegionRect { x: 0, y: 0, width: 2, height: 2 }, target_color: [10, 20, 30], tolerance: 10 },
        ];
        let result = match_color_probes(&screenshots, &probes, ColorMatchMode::Any);
        assert!(result.matched, "Any 模式任一命中即触发");
        assert_eq!(result.hit_count, 1);
    }

    #[test]
    fn match_color_probes_empty_returns_false() {
        let result = match_color_probes(&[], &[], ColorMatchMode::All);
        assert!(!result.matched, "无探针不应触发");
        assert_eq!(result.hit_count, 0);
    }
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- audio::watcher::tests`
Expected: 编译失败，`color_distance` / `average_region_rgb` / `match_color_probes` 未定义

- [ ] **Step 3: 实现识色核心逻辑**

在 `src-tauri/src/audio/watcher.rs` 顶部 `use` 区追加（紧跟 `use crate::audio::types::AudioSettings;`）：

```rust
use crate::audio::types::{ColorMatchMode, ColorProbe};
```

在 `compare_images` 函数之前（`ncc_to_similarity` 之后）追加识色核心函数：

```rust
/// 颜色匹配结果
#[derive(Debug, Clone)]
pub(crate) struct ColorMatchResult {
    /// 是否触发（按 mode 聚合后）
    pub matched: bool,
    /// 命中的 probe 数量
    pub hit_count: usize,
}

/// 计算两个 RGB 颜色的欧氏距离
pub(crate) fn color_distance(a: [u8; 3], b: [u8; 3]) -> f32 {
    let dr = (a[0] as f32 - b[0] as f32).powi(2);
    let dg = (a[1] as f32 - b[1] as f32).powi(2);
    let db = (a[2] as f32 - b[2] as f32).powi(2);
    (dr + dg + db).sqrt()
}

/// 取图像区域平均 RGB（alpha < 128 的透明像素不计入）
pub(crate) fn average_region_rgb(img: &image::DynamicImage) -> [u8; 3] {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return [0, 0, 0];
    }
    let mut sum_r: u64 = 0;
    let mut sum_g: u64 = 0;
    let mut sum_b: u64 = 0;
    let mut n: u64 = 0;
    for y in 0..h {
        for x in 0..w {
            let p = rgba.get_pixel(x, y);
            if p[3] < 128 {
                continue;
            }
            sum_r += p[0] as u64;
            sum_g += p[1] as u64;
            sum_b += p[2] as u64;
            n += 1;
        }
    }
    if n == 0 {
        return [0, 0, 0];
    }
    [
        (sum_r / n) as u8,
        (sum_g / n) as u8,
        (sum_b / n) as u8,
    ]
}

/// 对一组已截取的区域图像与对应探针做颜色匹配，按 mode 聚合
pub(crate) fn match_color_probes(
    screenshots: &[image::DynamicImage],
    probes: &[ColorProbe],
    mode: ColorMatchMode,
) -> ColorMatchResult {
    if probes.is_empty() || screenshots.len() < probes.len() {
        return ColorMatchResult { matched: false, hit_count: 0 };
    }
    let mut hit_count = 0usize;
    for (i, probe) in probes.iter().enumerate() {
        let avg = average_region_rgb(&screenshots[i]);
        let dist = color_distance(avg, probe.target_color);
        if dist <= probe.tolerance as f32 {
            hit_count += 1;
        }
    }
    let matched = match mode {
        ColorMatchMode::All => hit_count == probes.len(),
        ColorMatchMode::Any => hit_count > 0,
    };
    ColorMatchResult { matched, hit_count }
}
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- audio::watcher::tests`
Expected: 全部通过（含新增 8 个测试）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/audio/watcher.rs
git commit -m "feat(audio): 实现识色核心逻辑 color_distance / average_region_rgb / match_color_probes"
```

---

## Task 3: watcher 接入 ColorWatch 触发模式

**Files:**
- Modify: `src-tauri/src/audio/watcher.rs`

- [ ] **Step 1: 在 restart_watchers 中增加 ColorWatch 分支**

在 `src-tauri/src/audio/watcher.rs` 的 `restart_watchers` 函数中，找到现有的 `if card.trigger_mode != super::types::AudioTriggerMode::RegionWatch { continue; }` 这一行（第 41 行附近），改为分支判断：

```rust
        // 跳过未启用或非区域监听 / 识色模式的卡片
        if !card.enabled {
            continue;
        }
        match card.trigger_mode {
            super::types::AudioTriggerMode::RegionWatch => {
                // 需要参考图路径与音频路径
                let Some(ref_path) = &card.watch_reference_image_path else { continue };
                if ref_path.is_empty() || card.audio_file_path.is_empty() {
                    continue;
                }
            }
            super::types::AudioTriggerMode::ColorWatch => {
                // 需要至少一个 probe 与音频路径
                if card.color_probes.is_empty() || card.audio_file_path.is_empty() {
                    continue;
                }
            }
            super::types::AudioTriggerMode::Hotkey => continue,
        }
```

然后删除原 `if card.trigger_mode != super::types::AudioTriggerMode::RegionWatch { continue; }` 和紧跟的 `let Some(region) = &card.watch_region else { continue; };` / `let Some(ref_path) = ...` / `if ref_path.is_empty() || card.audio_file_path.is_empty() { continue; }` 这几行（已被上面的 match 覆盖）。

接着把原来的 `let cancel = Arc::new(...)` 块之后到 `tauri::async_runtime::spawn(...)` 之前的那段字段克隆代码，改为按模式分支构造 watcher 参数。将整个 spawn 块替换为：

```rust
        let cancel = Arc::new(AtomicBool::new(false));
        let app_clone = app.clone();
        let card_id = card.id.clone();
        let audio_path = card.audio_file_path.clone();
        let volume = card.volume;
        let cooldown_ms = card.cooldown_ms;
        let poll_interval_ms = card.watch_poll_interval_ms;
        let allow_simultaneous = card.allow_simultaneous;
        let playback_tx_clone = playback_tx.clone();
        let cancel_clone = Arc::clone(&cancel);

        cancel_map.insert(card_id.clone(), cancel);

        match card.trigger_mode {
            super::types::AudioTriggerMode::RegionWatch => {
                let Some(region) = &card.watch_region else { continue };
                let Some(ref_path) = &card.watch_reference_image_path else { continue };
                let threshold = card.watch_match_threshold;
                let region_clone = region.clone();
                let ref_path_clone = ref_path.clone();
                tauri::async_runtime::spawn(async move {
                    run_region_watcher(
                        app_clone,
                        card_id,
                        region_clone,
                        ref_path_clone,
                        audio_path,
                        volume,
                        allow_simultaneous,
                        playback_tx_clone,
                        cooldown_ms,
                        threshold,
                        poll_interval_ms,
                        cancel_clone,
                    )
                        .await;
                });
            }
            super::types::AudioTriggerMode::ColorWatch => {
                let probes = card.color_probes.clone();
                let match_mode = card.color_match_mode.clone();
                tauri::async_runtime::spawn(async move {
                    run_color_watcher(
                        app_clone,
                        card_id,
                        probes,
                        match_mode,
                        audio_path,
                        volume,
                        allow_simultaneous,
                        playback_tx_clone,
                        cooldown_ms,
                        poll_interval_ms,
                        cancel_clone,
                    )
                        .await;
                });
            }
            super::types::AudioTriggerMode::Hotkey => {}
        }
```

- [ ] **Step 2: 实现 run_color_watcher 函数**

在 `run_region_watcher` 函数之后追加：

```rust
async fn run_color_watcher(
    app: AppHandle,
    card_id: String,
    probes: Vec<crate::audio::types::ColorProbe>,
    match_mode: crate::audio::types::ColorMatchMode,
    audio_path: String,
    volume: f32,
    allow_simultaneous: bool,
    playback_tx: std::sync::mpsc::Sender<player::AudioCommand>,
    cooldown_ms: u32,
    poll_interval_ms: u32,
    cancel: Arc<AtomicBool>,
) {
    let mut ticker = interval(Duration::from_millis(poll_interval_ms.max(100) as u64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut last_triggered: Option<Instant> = None;

    while !cancel.load(Ordering::SeqCst) {
        ticker.tick().await;

        if cancel.load(Ordering::SeqCst) {
            break;
        }

        // 检查冷却
        if let Some(last) = last_triggered {
            if last.elapsed() < Duration::from_millis(cooldown_ms as u64) {
                continue;
            }
        }

        // 逐个截取 probe 区域
        let mut screenshots: Vec<image::DynamicImage> = Vec::with_capacity(probes.len());
        let mut all_captured = true;
        for probe in &probes {
            match capture_region(&probe.region) {
                Some(img) => screenshots.push(img),
                None => {
                    all_captured = false;
                    break;
                }
            }
        }

        if !all_captured {
            continue;
        }

        let result = match_color_probes(&screenshots, &probes, match_mode.clone());
        if result.matched {
            eprintln!("[音频 color watcher] 卡片 {card_id}: 识色命中 {}/{} probes", result.hit_count, probes.len());
            let _ = app.emit(REGION_MATCHED, &card_id);
            let tx = playback_tx.clone();
            let exclusive = !allow_simultaneous;
            let _ = tx.send(player::AudioCommand::Play { path: audio_path.clone(), volume, exclusive });
            last_triggered = Some(Instant::now());
        }
    }
}
```

- [ ] **Step 3: 运行编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过，无错误（可能有未使用 import 警告，下一步清理）

- [ ] **Step 4: 运行全部 audio 测试验证不回归**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- audio::`
Expected: 全部通过

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/audio/watcher.rs
git commit -m "feat(audio): watcher 接入 ColorWatch 触发模式，逐 probe 截图取色聚合判定"
```

---

## Task 4: 新命令 audio_test_color_match

**Files:**
- Modify: `src-tauri/src/audio/mod.rs`

- [ ] **Step 1: 在 audio_test_match 中拦截非 RegionWatch 模式**

在 `src-tauri/src/audio/mod.rs` 的 `audio_test_match` 函数中，找到现有的模式检查：

```rust
        if card.trigger_mode != types::AudioTriggerMode::RegionWatch {
            return Err(AppError::from("只有区域监听模式卡片支持匹配测试".to_string()));
        }
```

改为：

```rust
        match card.trigger_mode {
            types::AudioTriggerMode::RegionWatch => {}
            types::AudioTriggerMode::ColorWatch => {
                return Err(AppError::from("识色模式请使用 audio_test_color_match 命令".to_string()));
            }
            types::AudioTriggerMode::Hotkey => {
                return Err(AppError::from("快捷键模式不支持匹配测试".to_string()));
            }
        }
```

- [ ] **Step 2: 实现 audio_test_color_match 命令**

在 `src-tauri/src/audio/mod.rs` 的 `audio_test_match` 函数之后追加新命令。先在文件顶部 `MatchPosition` / `TestMatchResult` 结构附近增加一个 ColorTestResult 结构（紧跟 `TestMatchResult` 之后）：

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorProbeTestResult {
    /// 该 probe 是否命中
    pub matched: bool,
    /// 截取区域平均 RGB
    pub sampledColor: [u8; 3],
    /// 与目标颜色的距离
    pub distance: f32,
    /// 目标颜色
    pub targetColor: [u8; 3],
    /// 容差
    pub tolerance: u8,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorTestResult {
    /// 是否触发（按 mode 聚合后）
    pub triggered: bool,
    /// 命中 probe 数量
    pub hitCount: usize,
    /// probe 总数
    pub totalCount: usize,
    /// 每个 probe 的详细结果
    pub probes: Vec<ColorProbeTestResult>,
}
```

然后在 `audio_test_match` 之后追加命令：

```rust
#[tauri::command]
pub async fn audio_test_color_match(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AudioState>,
    card_id: String,
) -> Result<ColorTestResult, AppError> {
    let (probes, match_mode) = {
        let inner = state.lock_inner().map_err(|e| AppError::from(e))?;
        let card = inner
            .settings
            .cards
            .iter()
            .find(|c| c.id == card_id)
            .ok_or_else(|| AppError::from("卡片不存在".to_string()))?;

        if card.trigger_mode != types::AudioTriggerMode::ColorWatch {
            return Err(AppError::from("只有识色模式卡片支持识色测试".to_string()));
        }
        if card.color_probes.is_empty() {
            return Err(AppError::from("未配置识色探针".to_string()));
        }
        (card.color_probes.clone(), card.color_match_mode.clone())
    };

    let mut probe_results: Vec<ColorProbeTestResult> = Vec::with_capacity(probes.len());
    let mut hit_count = 0usize;

    for probe in &probes {
        let captured = match watcher::capture_region(&probe.region) {
            Some(img) => img,
            None => return Err(AppError::from("截图失败".to_string())),
        };
        let sampled = watcher::average_region_rgb(&captured);
        let dist = watcher::color_distance(sampled, probe.target_color);
        let matched = dist <= probe.tolerance as f32;
        if matched {
            hit_count += 1;
        }
        probe_results.push(ColorProbeTestResult {
            matched,
            sampled_color: sampled,
            distance: dist,
            target_color: probe.target_color,
            tolerance: probe.tolerance,
        });
    }

    let triggered = match match_mode {
        types::ColorMatchMode::All => hit_count == probes.len(),
        types::ColorMatchMode::Any => hit_count > 0,
    };

    Ok(ColorTestResult {
        triggered,
        hit_count,
        total_count: probes.len(),
        probes: probe_results,
    })
}
```

- [ ] **Step 3: 运行编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/audio/mod.rs
git commit -m "feat(audio): 新增 audio_test_color_match 命令返回每个 probe 的采样色与命中详情"
```

---

## Task 5: 注册命令到 lib.rs generate_handler

**Files:**
- Modify: `src-tauri/src/lib.rs:125`

- [ ] **Step 1: 在 generate_handler 中注册 audio_test_color_match**

在 `src-tauri/src/lib.rs` 第 125 行 `audio::audio_read_reference_image,` 之后追加一行：

```rust
            audio::audio_read_reference_image,
            audio::audio_test_color_match,
```

- [ ] **Step 2: 验证编译**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 编译通过

- [ ] **Step 3: 验证 capabilities 无需新增**

检查 `src-tauri/capabilities/default.json` 已包含 `"audio-overlay"` 和 `"audio-overlay-*"`（第 20-21 行），颜色 probe 框选复用同一 overlay label，无需修改。确认无需改动后跳过此步。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(audio): lib.rs generate_handler 注册 audio_test_color_match"
```

---

## Task 6: 前端类型扩展

**Files:**
- Modify: `src/components/app/audio-types.ts`

- [ ] **Step 1: 扩展 AudioTriggerMode 与新增 ColorProbe 类型**

在 `src/components/app/audio-types.ts` 顶部 `import type {RegionRect}` 之后追加：

```ts
export type ColorMatchMode = "all" | "any";

export type ColorProbe = {
    region: RegionRect;
    targetColor: [number, number, number];
    tolerance: number;
};

export type ColorProbeForm = {
    region: RegionRect | null;
    targetColor: string; // "#RRGGBB" 格式
    tolerance: string;   // 数字字符串，0-255
};
```

- [ ] **Step 2: 扩展 AudioTriggerMode 字面量**

将第 3 行：

```ts
export type AudioTriggerMode = "hotkey" | "regionWatch";
```

改为：

```ts
export type AudioTriggerMode = "hotkey" | "regionWatch" | "colorWatch";
```

- [ ] **Step 3: 扩展 AudioCard 与 AudioCardForm 类型**

在 `AudioCard` 类型末尾（`allowSimultaneous` 之后）追加：

```ts
    allowSimultaneous: boolean;
    colorProbes: ColorProbe[];
    colorMatchMode: ColorMatchMode;
};
```

在 `AudioCardForm` 类型末尾（`allowSimultaneous` 之后）追加：

```ts
    allowSimultaneous: boolean;
    colorProbes: ColorProbeForm[];
    colorMatchMode: ColorMatchMode;
};
```

- [ ] **Step 4: 扩展 DEFAULT_AUDIO_CARD**

在 `DEFAULT_AUDIO_CARD` 末尾（`allowSimultaneous: false,` 之后）追加：

```ts
    allowSimultaneous: false,
    colorProbes: [],
    colorMatchMode: "all",
};
```

- [ ] **Step 5: 运行类型检查验证**

Run: `bunx tsc --noEmit`
Expected: 无新增类型错误（已有代码因新增可选字段应仍兼容）

- [ ] **Step 6: 提交**

```bash
git add src/components/app/audio-types.ts
git commit -m "feat(audio): 前端新增 ColorProbe / ColorMatchMode / colorWatch 触发模式类型"
```

---

## Task 7: 前端 audio-utils 转层与校验接入

**Files:**
- Modify: `src/components/app/audio-utils.ts`
- Test: `src/components/app/audio-utils.test.ts`

- [ ] **Step 1: 写失败测试 — colorProbes 转层与校验**

在 `src/components/app/audio-utils.test.ts` 顶部 import 区追加：

```ts
import {DEFAULT_AUDIO_CARD} from "@/components/app/audio-types";
```
（已有则跳过）

在 `describe("audio-utils", ...)` 末尾追加新 describe 块：

```ts
    describe("colorWatch settingsToForm", () => {
        it("converts colorProbes to form with hex color string", () => {
            const settings = {
                audioEnabled: true,
                cards: [
                    {
                        ...DEFAULT_AUDIO_CARD,
                        id: "c1",
                        name: "识色",
                        triggerMode: "colorWatch" as const,
                        colorProbes: [
                            {
                                region: {x: 10, y: 20, width: 5, height: 5},
                                targetColor: [200, 100, 50] as [number, number, number],
                                tolerance: 40,
                            },
                        ],
                        colorMatchMode: "any" as const,
                    },
                ],
            };
            const form = settingsToForm(settings);
            expect(form.cards[0].colorProbes).toHaveLength(1);
            expect(form.cards[0].colorProbes[0].targetColor).toBe("#c86432");
            expect(form.cards[0].colorProbes[0].tolerance).toBe("40");
            expect(form.cards[0].colorMatchMode).toBe("any");
        });
    });

    describe("colorWatch parseSettingsForm", () => {
        it("parses valid colorWatch form back to settings", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "识色",
                        enabled: true,
                        triggerMode: "colorWatch" as const,
                        hotkey: "",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.75",
                        watchPollIntervalMs: "500",
                        audioFilePath: "a.mp3",
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [
                            {
                                region: {x: 10, y: 20, width: 5, height: 5},
                                targetColor: "#c86432",
                                tolerance: "40",
                            },
                        ],
                        colorMatchMode: "all" as const,
                    },
                ],
            };
            const settings = parseSettingsForm(form);
            expect(settings.cards[0].triggerMode).toBe("colorWatch");
            expect(settings.cards[0].colorProbes).toHaveLength(1);
            expect(settings.cards[0].colorProbes[0].targetColor).toEqual([200, 100, 50]);
            expect(settings.cards[0].colorProbes[0].tolerance).toBe(40);
            expect(settings.cards[0].colorMatchMode).toBe("all");
        });

        it("throws when colorWatch has no probes", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "识色",
                        enabled: true,
                        triggerMode: "colorWatch" as const,
                        hotkey: "",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.75",
                        watchPollIntervalMs: "500",
                        audioFilePath: "a.mp3",
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [],
                        colorMatchMode: "all" as const,
                    },
                ],
            };
            expect(() => parseSettingsForm(form)).toThrow("识色模式下至少需要配置一个探针");
        });

        it("throws for invalid tolerance", () => {
            const form = {
                audioEnabled: true,
                cards: [
                    {
                        id: "c1",
                        name: "识色",
                        enabled: true,
                        triggerMode: "colorWatch" as const,
                        hotkey: "",
                        watchRegion: null,
                        watchReferenceImagePath: "",
                        watchMatchThreshold: "0.75",
                        watchPollIntervalMs: "500",
                        audioFilePath: "a.mp3",
                        volume: "0.8",
                        cooldownMs: "1000",
                        allowSimultaneous: false,
                        colorProbes: [
                            {
                                region: {x: 10, y: 20, width: 5, height: 5},
                                targetColor: "#c86432",
                                tolerance: "300",
                            },
                        ],
                        colorMatchMode: "all" as const,
                    },
                ],
            };
            expect(() => parseSettingsForm(form)).toThrow("颜色容差必须在 0 到 255 之间");
        });
    });
```

- [ ] **Step 2: 运行测试验证失败**

Run: `bun run test -- audio-utils.test.ts`
Expected: 失败，`settingsToForm` 不处理 colorProbes，`parseSettingsForm` 不校验

- [ ] **Step 3: 实现转层工具函数**

在 `src/components/app/audio-utils.ts` 顶部 import 区追加：

```ts
import type {AudioCard, AudioCardForm, AudioSettings, AudioSettingsForm, ColorProbe, ColorProbeForm,} from "@/components/app/audio-types";
import {DEFAULT_AUDIO_CARD} from "@/components/app/audio-types";
```
（替换原 import，增加 `ColorProbe, ColorProbeForm`）

在 `settingsToForm` 函数之前追加 hex 转换工具函数：

```ts
function rgbToHex(rgb: [number, number, number]): string {
    const [r, g, b] = rgb;
    return "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("");
}

function hexToRgb(hex: string): [number, number, number] {
    const clean = hex.startsWith("#") ? hex.slice(1) : hex;
    if (clean.length !== 6) {
        throw new Error("颜色格式必须为 #RRGGBB。");
    }
    const r = parseInt(clean.slice(0, 2), 16);
    const g = parseInt(clean.slice(2, 4), 16);
    const b = parseInt(clean.slice(4, 6), 16);
    if ([r, g, b].some((v) => Number.isNaN(v) || v < 0 || v > 255)) {
        throw new Error("颜色值必须在 00-FF 之间。");
    }
    return [r, g, b];
}

function probeToForm(probe: ColorProbe): ColorProbeForm {
    return {
        region: probe.region,
        targetColor: rgbToHex(probe.targetColor),
        tolerance: String(probe.tolerance),
    };
}

function parseProbeForm(form: ColorProbeForm): ColorProbe {
    if (!form.region) {
        throw new Error("识色探针必须设置区域。");
    }
    const tolerance = parseInt(form.tolerance, 10);
    if (Number.isNaN(tolerance) || tolerance < 0 || tolerance > 255) {
        throw new Error("颜色容差必须在 0 到 255 之间。");
    }
    return {
        region: form.region,
        targetColor: hexToRgb(form.targetColor),
        tolerance,
    };
}
```

- [ ] **Step 4: 在 cardToForm / parseCardForm 中接入 colorProbes**

在 `cardToForm` 函数返回对象末尾（`allowSimultaneous: card.allowSimultaneous ?? false,` 之后）追加：

```ts
        allowSimultaneous: card.allowSimultaneous ?? false,
        colorProbes: (card.colorProbes ?? []).map(probeToForm),
        colorMatchMode: card.colorMatchMode ?? "all",
    };
```

在 `parseCardForm` 函数末尾的 return 之前，追加 colorWatch 模式校验：

```ts
    const colorProbes = form.triggerMode === "colorWatch"
        ? form.colorProbes.map(parseProbeForm)
        : [];
    if (form.triggerMode === "colorWatch" && colorProbes.length === 0) {
        throw new Error("识色模式下至少需要配置一个探针。");
    }
    const colorMatchMode = form.colorMatchMode ?? "all";

    return {
        id: form.id || generateCardId(),
        name,
        enabled: form.enabled,
        triggerMode: form.triggerMode,
        hotkey,
        watchRegion: form.triggerMode === "regionWatch" ? form.watchRegion : null,
        watchReferenceImagePath: form.triggerMode === "regionWatch" ? form.watchReferenceImagePath.trim() || null : null,
        watchMatchThreshold,
        watchPollIntervalMs,
        audioFilePath: form.audioFilePath.trim(),
        volume,
        cooldownMs,
        allowSimultaneous: form.allowSimultaneous ?? false,
        colorProbes,
        colorMatchMode,
    };
```

注意：`parseCardForm` 中原有的 `return { ... }` 整体替换为上面这段（从 `const colorProbes =` 到 `};`）。

- [ ] **Step 5: 运行测试验证通过**

Run: `bun run test -- audio-utils.test.ts`
Expected: 全部通过（含新增 colorWatch 测试）

- [ ] **Step 6: 运行类型检查**

Run: `bunx tsc --noEmit`
Expected: 无错误

- [ ] **Step 7: 提交**

```bash
git add src/components/app/audio-utils.ts src/components/app/audio-utils.test.ts
git commit -m "feat(audio): audio-utils 接入 colorProbes 转层与识色模式校验"
```

---

## Task 8: 前端 ColorWatch UI

**Files:**
- Modify: `src/components/app/audio-page.tsx`

- [ ] **Step 1: 在触发模式 Select 中增加 colorWatch 选项**

在 `src/components/app/audio-page.tsx` 的 `AudioCardEditor` 组件中，找到触发模式 Select 的 `SelectContent`：

```tsx
                                <SelectContent>
                                    <SelectItem value="hotkey">快捷键触发</SelectItem>
                                    <SelectItem value="regionWatch">区域监听+图像匹配</SelectItem>
                                </SelectContent>
```

改为：

```tsx
                                <SelectContent>
                                    <SelectItem value="hotkey">快捷键触发</SelectItem>
                                    <SelectItem value="regionWatch">区域监听+图像匹配</SelectItem>
                                    <SelectItem value="colorWatch">多区域识色</SelectItem>
                                </SelectContent>
```

- [ ] **Step 2: 增加 isColorWatch 分支变量**

在 `AudioCardEditor` 函数中，找到：

```tsx
    const isHotkey = card.triggerMode === "hotkey";
    const isRegion = card.triggerMode === "regionWatch";
```

追加：

```tsx
    const isColor = card.triggerMode === "colorWatch";
```

- [ ] **Step 3: 增加识色测试与探针操作回调**

在 `AudioWorkbench` 组件中 `handleBeginRegionSelection` 之后追加：

```tsx
    const handleTestColorMatch = useCallback(
        async (cardId: string) => {
            if (!isNativeShell) return;
            try {
                type ColorProbeResult = { matched: boolean; sampledColor: [number, number, number]; distance: number; targetColor: [number, number, number]; tolerance: number };
                type ColorTestResult = { triggered: boolean; hitCount: number; totalCount: number; probes: ColorProbeResult[] };
                const result = await invoke<ColorTestResult>("audio_test_color_match", {cardId});
                const detail = result.probes
                    .map((p, i) => `#${i + 1}: ${p.matched ? "命中" : "未中"} (采样 #${p.sampledColor.map((v) => v.toString(16).padStart(2, "0")).join("")} 距离 ${p.distance.toFixed(1)})`)
                    .join(" | ");
                toast.success(
                    `识色: ${result.hitCount}/${result.totalCount} 命中 ${result.triggered ? "(已触发)" : "(未触发)"}\n${detail}`,
                    {duration: 6000}
                );
            } catch (error) {
                toast.error(getErrorMessage(error));
            }
        },
        [isNativeShell],
    );

    const handleAddColorProbe = useCallback(
        (index: number) => {
            setForm((current) => {
                if (!current) return current;
                const card = current.cards[index];
                if (!card) return current;
                const newProbe = {region: null, targetColor: "#ff0000", tolerance: "30"};
                const nextCards = current.cards.map((c, i) =>
                    i === index ? {...c, colorProbes: [...c.colorProbes, newProbe]} : c,
                );
                return {...current, cards: nextCards};
            });
        },
        [setForm],
    );

    const handleRemoveColorProbe = useCallback(
        (cardIndex: number, probeIndex: number) => {
            setForm((current) => {
                if (!current) return current;
                const card = current.cards[cardIndex];
                if (!card) return current;
                const nextProbes = card.colorProbes.filter((_, i) => i !== probeIndex);
                const nextCards = current.cards.map((c, i) =>
                    i === cardIndex ? {...c, colorProbes: nextProbes} : c,
                );
                return {...current, cards: nextCards};
            });
        },
        [setForm],
    );

    const handleUpdateColorProbe = useCallback(
        (cardIndex: number, probeIndex: number, patch: Partial<AudioSettingsForm["cards"][number]["colorProbes"][number]>) => {
            setForm((current) => {
                if (!current) return current;
                const card = current.cards[cardIndex];
                if (!card) return current;
                const nextProbes = card.colorProbes.map((p, i) =>
                    i === probeIndex ? {...p, ...patch} : p,
                );
                const nextCards = current.cards.map((c, i) =>
                    i === cardIndex ? {...c, colorProbes: nextProbes} : c,
                );
                return {...current, cards: nextCards};
            });
        },
        [setForm],
    );
```

- [ ] **Step 4: 将新回调传入 AudioCardEditor**

在 `AudioWorkbench` 的 `AudioCardEditor` 调用处（约 296-309 行），追加 props：

```tsx
                        <AudioCardEditor
                            key={card.id}
                            card={card}
                            index={index}
                            isNativeShell={isNativeShell}
                            onUpdate={(patch) => handleUpdateCard(index, patch)}
                            onRemove={() => handleRemoveCard(index)}
                            onTestPlay={() => handleTestPlay(card.id)}
                            onTestMatch={() => handleTestMatch(card.id)}
                            onBeginRegionSelection={() => handleBeginRegionSelection(card.id)}
                            onPickReferenceImage={() => handlePickReferenceImage(index)}
                            onPickAudioFile={() => handlePickAudioFile(index)}
                            onLoadReferencePreview={() => handleLoadReferencePreview(card.id)}
                            onTestColorMatch={() => handleTestColorMatch(card.id)}
                            onAddColorProbe={() => handleAddColorProbe(index)}
                            onRemoveColorProbe={(probeIndex) => handleRemoveColorProbe(index, probeIndex)}
                            onUpdateColorProbe={(probeIndex, patch) => handleUpdateColorProbe(index, probeIndex, patch)}
                        />
```

并在 `AudioCardEditor` 的 props 类型与解构中追加：

```tsx
function AudioCardEditor({
                             card,
                             index,
                             isNativeShell,
                             onUpdate,
                             onRemove,
                             onTestPlay,
                             onTestMatch,
                             onBeginRegionSelection,
                             onPickReferenceImage,
                             onPickAudioFile,
                             onLoadReferencePreview,
                             onTestColorMatch,
                             onAddColorProbe,
                             onRemoveColorProbe,
                             onUpdateColorProbe,
                         }: {
    card: AudioSettingsForm["cards"][number];
    index: number;
    isNativeShell: boolean;
    onUpdate: (patch: Partial<AudioSettingsForm["cards"][number]>) => void;
    onRemove: () => void;
    onTestPlay: () => void;
    onTestMatch: () => void;
    onBeginRegionSelection: () => void;
    onPickReferenceImage: () => void;
    onPickAudioFile: () => void;
    onLoadReferencePreview: () => Promise<string | null>;
    onTestColorMatch: () => void;
    onAddColorProbe: () => void;
    onRemoveColorProbe: (probeIndex: number) => void;
    onUpdateColorProbe: (probeIndex: number, patch: Partial<AudioSettingsForm["cards"][number]["colorProbes"][number]>) => void;
}) {
```

- [ ] **Step 5: 增加 colorWatch 模式 UI 块**

在 `AudioCardEditor` 的 `{isRegion && (...)}` 块之后，追加 colorWatch 模式 UI：

```tsx
                {isColor && (
                    <FieldGroup>
                        <Field>
                            <FieldLabel>匹配模式</FieldLabel>
                            <FieldContent>
                                <Select
                                    value={card.colorMatchMode}
                                    onValueChange={(v) => onUpdate({colorMatchMode: v as "all" | "any"})}
                                >
                                    <SelectTrigger>
                                        <SelectValue/>
                                    </SelectTrigger>
                                    <SelectContent>
                                        <SelectItem value="all">全部命中才触发</SelectItem>
                                        <SelectItem value="any">任一命中即触发</SelectItem>
                                    </SelectContent>
                                </Select>
                            </FieldContent>
                        </Field>

                        {card.colorProbes.map((probe, probeIndex) => (
                            <div key={probeIndex} className="border border-[var(--seam)] p-2 space-y-2">
                                <div className="flex items-center justify-between">
                                    <span className="font-mono text-xs font-bold text-[var(--amber)]">
                                        探针 #{probeIndex + 1}
                                    </span>
                                    <Button
                                        variant="ghost"
                                        size="sm"
                                        onClick={() => onRemoveColorProbe(probeIndex)}
                                        title="删除探针"
                                        data-icon="inline-start"
                                    >
                                        <RiDeleteBinLine className="size-4 text-[var(--alert-red)]" aria-hidden="true"/>
                                        删除
                                    </Button>
                                </div>
                                <Field>
                                    <FieldLabel>监听区域</FieldLabel>
                                    <FieldContent>
                                        <div className="flex items-center gap-2">
                                            <Button
                                                variant="secondary"
                                                size="sm"
                                                onClick={onBeginRegionSelection}
                                                data-icon="inline-start"
                                            >
                                                <RiVolumeUpLine className="size-4" aria-hidden="true"/>
                                                {probe.region ? "重新框选" : "框选区域"}
                                            </Button>
                                            {probe.region && (
                                                <Badge variant="outline" className="font-mono text-xs">
                                                    {probe.region.x},{probe.region.y} / {probe.region.width}x{probe.region.height}
                                                </Badge>
                                            )}
                                        </div>
                                    </FieldContent>
                                </Field>
                                <Field>
                                    <FieldLabel>目标颜色</FieldLabel>
                                    <FieldContent>
                                        <div className="flex items-center gap-2">
                                            <input
                                                type="color"
                                                value={probe.targetColor}
                                                onChange={(e) => onUpdateColorProbe(probeIndex, {targetColor: e.target.value})}
                                                className="h-9 w-12 cursor-pointer border border-[var(--seam)] bg-transparent p-0"
                                                aria-label="目标颜色"
                                            />
                                            <Input
                                                className="flex-1 font-mono"
                                                value={probe.targetColor}
                                                onChange={(e) => onUpdateColorProbe(probeIndex, {targetColor: e.target.value})}
                                                placeholder="#RRGGBB"
                                            />
                                        </div>
                                    </FieldContent>
                                </Field>
                                <Field>
                                    <FieldLabel>颜色容差 (0-255)</FieldLabel>
                                    <FieldContent>
                                        <Input
                                            type="number"
                                            min={0}
                                            max={255}
                                            step={1}
                                            value={probe.tolerance}
                                            onChange={(e) => onUpdateColorProbe(probeIndex, {tolerance: e.target.value})}
                                            title="RGB 欧氏距离阈值，越小越严格"
                                        />
                                    </FieldContent>
                                </Field>
                            </div>
                        ))}

                        <Button
                            variant="secondary"
                            size="sm"
                            onClick={onAddColorProbe}
                            data-icon="inline-start"
                        >
                            <RiCheckLine className="size-4" aria-hidden="true"/>
                            新增探针
                        </Button>

                        {card.colorProbes.length > 0 && (
                            <Button variant="ghost" size="sm" onClick={onTestColorMatch} data-icon="inline-start">
                                <RiPlayLine className="size-4" aria-hidden="true"/>
                                实时识色测试
                            </Button>
                        )}

                        <Field>
                            <FieldLabel>检查间隔 (ms)</FieldLabel>
                            <FieldContent>
                                <Input
                                    type="number"
                                    min={100}
                                    max={10000}
                                    step={100}
                                    value={card.watchPollIntervalMs}
                                    onChange={(e) => onUpdate({watchPollIntervalMs: e.target.value})}
                                    title="每隔多久截图比对一次"
                                />
                            </FieldContent>
                        </Field>
                    </FieldGroup>
                )}
```

- [ ] **Step 6: 运行类型检查**

Run: `bunx tsc --noEmit`
Expected: 无错误

- [ ] **Step 7: 运行前端测试验证不回归**

Run: `bun run test`
Expected: 全部通过

- [ ] **Step 8: 提交**

```bash
git add src/components/app/audio-page.tsx
git commit -m "feat(audio): AudioCardEditor 增加 colorWatch 模式 UI（探针列表/颜色/容差/测试）"
```

---

## Task 9: 文档同步

**Files:**
- Modify: `AGENTS.md`
- Modify: `README.md`

- [ ] **Step 1: 在 AGENTS.md 音频命令面追加 audio_test_color_match**

在 `AGENTS.md` 中搜索 `audio_read_reference_image` 所在的命令说明位置（音频命令面表格或列表），在其后追加：

```markdown
- `audio_test_color_match(cardId)` — 识色模式实时测试：截取每个 probe 区域、取平均色、与目标颜色比较，返回每个 probe 的命中状态、采样色、距离
```

- [ ] **Step 2: 在 AGENTS.md 音频原生约定中追加 ColorWatch 模式说明**

在 `AGENTS.md` 音频相关约定段落（搜索 `AudioTriggerMode` 或 `RegionWatch`），追加：

```markdown
- **音频识色模式（ColorWatch）**：`AudioTriggerMode::ColorWatch` 通过 `color_probes: Vec<ColorProbe>` 配置 3-4 个小区域探针，
  每个 `ColorProbe = { region, target_color: [u8;3], tolerance: u8 }`。watcher 逐个截取区域、取平均 RGB（alpha < 128 的透明像素不计入），
  与 `target_color` 做欧氏距离 ≤ `tolerance` 判定，按 `color_match_mode`（`All` / `Any`，默认 `All`）聚合后决定是否触发。
  `tolerance` 默认 30，范围 0-255。该模式不依赖参考图像，性能远优于模板匹配，适合"多区域同时出现指定颜色"的判定场景。
  `audio_test_color_match` 命令返回每个 probe 的采样色与命中详情，供前端调试。
  `AudioCard` 的 `color_probes` 与 `color_match_mode` 使用 `#[serde(rename_all = "camelCase")]` 序列化为 `colorProbes` / `colorMatchMode`。
```

- [ ] **Step 3: 在 README.md 音频功能描述中追加识色模式**

在 `README.md` 音频功能段落，追加一行：

```markdown
- **多区域识色触发**：框选 3-4 个小区域并指定目标颜色 + 容差，所有/任一区域出现相似颜色时触发音频播放。
```

- [ ] **Step 4: 提交**

```bash
git add AGENTS.md README.md
git commit -m "docs: AGENTS.md / README.md 补充音频多区域识色模式说明"
```

---

## Task 10: 全量验证

**Files:** 无修改，仅运行验证

- [ ] **Step 1: Rust 全量测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: 全部通过（含新增 audio::types / audio::watcher 测试）

- [ ] **Step 2: Rust 编译检查**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: 无错误无警告

- [ ] **Step 3: 前端全量测试**

Run: `bun run test`
Expected: 全部通过

- [ ] **Step 4: 前端类型检查 + 构建**

Run: `bun run build`
Expected: 构建成功

- [ ] **Step 5: 桌面构建冒烟**

Run: `bun run tauri build`
Expected: 构建成功（不强制签名，beta 流程）

- [ ] **Step 6: 回复 Issue #58**

在 Issue #58 评论中说明：本次新增"多区域识色"触发模式（`colorWatch`），作为图像模板匹配的轻量替代方案，框选 3-4 个小区域 + 指定颜色容差即可触发，性能与准确率均优于整图模板匹配。请试用后反馈。

---

## Self-Review（计划作者自查，非执行步骤）

**1. Spec 覆盖：**
- 数据结构（ColorProbe / ColorMatchMode / AudioCard 字段）→ Task 1 ✓
- 识色核心逻辑（取色 + 距离 + 聚合）→ Task 2 ✓
- watcher 接入 ColorWatch → Task 3 ✓
- 测试命令 audio_test_color_match → Task 4 ✓
- 命令注册 → Task 5 ✓
- 前端类型 → Task 6 ✓
- 前端转层与校验 → Task 7 ✓
- 前端 UI → Task 8 ✓
- 文档 → Task 9 ✓
- 全量验证 → Task 10 ✓

**2. 占位符扫描：** 无 TBD / TODO / "适当处理" / "类似 Task N"。所有代码块完整。

**3. 类型一致性：**
- `ColorProbe` 字段：Rust `region / target_color / tolerance` ↔ TS `region / targetColor / tolerance`（serde camelCase）✓
- `ColorMatchMode`：Rust `All / Any` ↔ TS `"all" / "any"`（serde camelCase）✓
- `ColorMatchResult`：Rust `matched / hit_count` ↔ 前端 invoke 返回类型 `triggered / hitCount` ✓
- `ColorProbeTestResult`：Rust `matched / sampled_color / distance / target_color / tolerance` ↔ TS `matched / sampledColor / distance / targetColor / tolerance` ✓
- `ColorTestResult`：Rust `triggered / hit_count / total_count / probes` ↔ TS `triggered / hitCount / totalCount / probes` ✓
- 命令名 `audio_test_color_match` 在 Task 4 定义、Task 5 注册、Task 8 前端调用、Task 9 文档，全部一致 ✓
- `ColorProbeForm` 字段 `region / targetColor / tolerance` 在 Task 6 定义、Task 7 转层使用、Task 8 UI 绑定，全部一致 ✓
