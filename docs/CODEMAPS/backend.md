# 后端 / 原生层 Codemap

## 运行时模型
- 这里的“后端”不是 HTTP 服务，而是 Tauri 原生命令层
- 入口：`src-tauri/src/main.rs` -> `src-tauri/src/lib.rs`
- 状态容器：`MorseState` / `MorseStateInner`

## Tauri Commands
- `morse_get_bootstrap`
- `morse_save_settings(settingsValue)`
- `morse_set_hotkey_recording(recording)`
- `morse_begin_region_selection(slots)`
- `morse_overlay_submit_selection(slot, rect)`
- `morse_overlay_cancel_selection(slot)`
- `morse_run_recognition(autoType?)`

## 原生模块映射
- `src-tauri/src/lib.rs`
  - 注册 plugin、state、invoke_handler
- `src-tauri/src/morse/mod.rs`
  - 全局状态管理
  - 热键监听协调
  - 历史记录写入与裁剪
  - 识别主流程调度
- `src-tauri/src/morse/overlay.rs`
  - `PendingSelection`
  - `PreparedSelection`
  - overlay 窗口创建/销毁
  - 多 slot 会话状态推进
  - 单元测试覆盖 slot 校验与推进逻辑
- `src-tauri/src/morse/settings.rs`
  - 配置路径解析
  - 文件读写与序列化辅助逻辑
  - 单元测试覆盖缺省配置、round-trip、无效 JSON
- `src-tauri/src/morse/types.rs`
  - 前后端共享 DTO
  - 默认设置单元测试

## 测试关注点
- overlay 纯逻辑推进
- settings 读写/反序列化
- default settings
- 历史记录长度上限裁剪

## 约束
- 无 HTTP middleware、无数据库访问层
- 不改变 command 面和 DTO 对外语义
- 识别、截图、系统输入仍依赖真实桌面环境，当前单测不覆盖这部分
