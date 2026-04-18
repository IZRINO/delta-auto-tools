<!-- Generated: 2026-04-19 | Files scanned: 79 | Token estimate: ~860 -->

# 后端 / 原生层 Codemap

## 运行时模型
- 这里的“后端”不是 HTTP 服务，而是 Tauri 原生命令层
- 入口：`src-tauri/src/main.rs` -> `src-tauri/src/lib.rs`
- 状态容器：`MorseState` / `MorseStateInner`

## Tauri Commands
- `morse_get_bootstrap`
  - `morse-page.tsx` 初始化加载
  - 返回：设置、历史、最近结果
- `morse_save_settings(settingsValue)`
  - 保存阈值、输入延迟、热键
  - 热键变更：先解绑旧热键，再注册新热键，失败则回滚
- `morse_begin_region_selection(slots)`
  - 创建透明 overlay 窗口
  - 建立多步骤框选会话
- `morse_overlay_submit_selection(slot, rect)`
  - 校验当前步骤
  - 更新 staged_regions
  - 最后一步才写入设置文件
- `morse_overlay_cancel_selection(slot)`
  - 取消当前会话并清理 pending 状态
- `morse_run_recognition(autoType?)`
  - 对 3 个区域截图、识别、聚合、可选自动输入

## 原生模块映射
- `src-tauri/src/lib.rs`
  - 注册 plugin、state、invoke_handler
- `src-tauri/src/morse/mod.rs`
  - 模块入口
  - 全局状态管理
  - 热键注册
  - 历史记录写入
  - 识别主流程调度
- `src-tauri/src/morse/overlay.rs`
  - `PendingSelection`
  - `PreparedSelection`
  - overlay 窗口创建/销毁
  - 多 slot 会话状态推进
- `src-tauri/src/morse/recognition.rs`
  - `run_recognition`
  - `capture_region`
  - `detect_morse`
  - Otsu / 手动阈值 / 连通域检测
- `src-tauri/src/morse/decoder.rs`
  - 摩斯序列转数字
- `src-tauri/src/morse/settings.rs`
  - `load_settings`
  - `save_settings`
  - 配置文件路径解析
- `src-tauri/src/morse/input.rs`
  - `type_result`
  - enigo 键盘注入
- `src-tauri/src/morse/types.rs`
  - 前后端共享 DTO

## 状态机
```text
idle
  -> begin_region_selection
  -> pending_selection(active)
  -> submit slot 1/2/3
  -> staged_regions 累积
  -> final submit
  -> save_settings + commit_selection
  -> idle
```

## 中间件 / 插件链
- `tauri_plugin_opener`
- `tauri_plugin_global_shortcut`
- 无 HTTP middleware、无 repository 层、无数据库访问层

## 事件流
- Emit: `morse://run-finished`
- Consumer: `src/components/app/morse-page.tsx`

## 关键文件
- `src-tauri/src/lib.rs`
- `src-tauri/src/morse/mod.rs`
- `src-tauri/src/morse/overlay.rs`
- `src-tauri/src/morse/recognition.rs`
- `src-tauri/src/morse/settings.rs`
- `src-tauri/src/morse/input.rs`
