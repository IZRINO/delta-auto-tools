<!-- Generated: 2026-04-19 | Files scanned: 79 | Token estimate: ~620 -->

# 数据 Codemap

## 持久化形态
- 无数据库
- 无 migration 体系
- 无 ORM / repository / table schema
- 当前唯一业务持久化文件：`morse_settings.json`

## 配置文件
- 路径来源：`app.path().app_config_dir()`
- 文件名：`morse_settings.json`
- 读写模块：`src-tauri/src/morse/settings.rs`

## 主要数据结构
- `MorseSettings`
  - `hotkey: String`
  - `regions: [Option<RegionRect>; 3]`
  - `binary_threshold: u8`
  - `auto_input_delay: u64`
- `RegionRect`
  - `x`
  - `y`
  - `width`
  - `height`
- `MorseRunResult`
  - `value`
  - `details`
  - `triggered_by`
  - `auto_typed`
  - `occurred_at_ms`
  - `error`
- `HistoryEntry`
  - 保存在内存 `VecDeque`
  - 当前不会落盘

## 内存态 vs 持久化
```text
磁盘
  -> morse_settings.json
  -> 启动时 load_settings()
  -> 注入 MorseState.settings

运行时
  -> latest_run (内存)
  -> history (内存，最多 1000 条)
  -> pending_selection.staged_regions (会话内暂存)
```

## 生命周期
- `settings`：持久化
- `history`：仅内存
- `latest_run`：仅内存
- `pending_selection`：仅当前 overlay 会话

## 数据关系
- 1 份设置
  - 包含 3 个区域槽位
- 1 次识别结果
  - 包含 3 个区域 detail
- N 条历史记录
  - 由识别结果派生

## 缺失项
- 无数据库表
- 无迁移历史
- 无远程同步
- 无用户账户或多租户模型
