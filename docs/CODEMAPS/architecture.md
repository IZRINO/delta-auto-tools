<!-- Generated: 2026-04-19 | Files scanned: 79 | Token estimate: ~780 -->

# 架构总览

## 项目类型
- 单体桌面应用
- 前端：React 19 + TypeScript + Vite + Bun
- 原生层：Tauri 2 + Rust
- 业务域：摩斯密码识别、区域框选、热键触发、自动输入

## 系统边界
```text
用户操作 / 全局热键
  -> React 工作台 (src/App.tsx, src/components/app/morse-page.tsx)
  -> Tauri invoke/event
  -> Rust morse 模块 (src-tauri/src/morse/*)
  -> 屏幕截图 / 图像识别 / 自动输入 / 本地设置文件
```

## 入口链路
```text
index.html
  -> src/main.tsx
  -> src/App.tsx
  -> MorsePage

src-tauri/src/main.rs
  -> src-tauri/src/lib.rs
  -> morse::initialize()
  -> Tauri commands / plugins / state
```

## 模式切换
- 正常桌面模式：`src/App.tsx`
- Overlay 模式：`?mode=overlay` -> `MorsePage overlayMode`
- 不使用前端路由；查询参数就是模式开关

## 核心数据流
```text
启动
  -> load_settings()
  -> register_hotkey()
  -> 注入 MorseState

主界面加载
  -> morse_get_bootstrap
  -> 设置 / 历史 / 最近结果

区域框选
  -> morse_begin_region_selection(slots)
  -> fullscreen transparent overlay
  -> morse_overlay_submit_selection(slot, rect)
  -> staged_regions
  -> 最后一步 save_settings()

识别
  -> morse_run_recognition(autoType?)
  -> xcap 截图
  -> image 二值化 + 连通域
  -> decoder 转数字
  -> 可选 enigo 自动输入
  -> emit morse://run-finished
```

## 服务边界
- 前端 UI：`src/components/app/morse-page.tsx`
- 原生状态协调：`src-tauri/src/morse/mod.rs`
- 框选状态机：`src-tauri/src/morse/overlay.rs`
- 图像识别：`src-tauri/src/morse/recognition.rs`
- 配置持久化：`src-tauri/src/morse/settings.rs`
- 自动输入：`src-tauri/src/morse/input.rs`

## 关键约束
- Overlay 必须保持透明，不能遮挡真实屏幕
- 3 个区域在一次 overlay 会话中连续完成
- 热键录制在前端，真正解绑/注册在 Rust
- 当前无数据库、无 HTTP API、无路由系统
