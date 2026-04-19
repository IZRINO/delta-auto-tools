<!-- Generated: 2026-04-19 → 2026-04-19 (v0.1.0) | Files scanned: 79 | Token estimate: ~920 -->

# 前端 Codemap

## 页面树
```text
src/main.tsx
  -> TooltipProvider
  -> App

App
  -> overlayMode ? MorsePage(overlayMode)
  -> Sidebar shell + MorsePage
```

## 主要界面结构
- `src/App.tsx`
  - 负责桌面壳层
  - 负责 `?mode=overlay` 分支
- `src/components/app/morse-page.tsx`
  - 主工作台
  - Overlay 视图
  - 区域配置
  - 识别结果
  - 设置
  - 历史记录
- `src/components/app/tool-placeholder-page.tsx`
  - 预留/占位页面
- `src/components/ui/*`
  - shadcn/ui 基础组件

## 组件层级
```text
App
  -> SidebarProvider / Sidebar / SidebarInset
  -> MorsePage
      -> desktop-toolbar
      -> 采样区域 Card
      -> 操作按钮 Card
      -> 解析结果 Card
      -> 设置 Card
      -> 历史记录 Card

Overlay 模式
  -> RegionSelectionOverlay
      -> 当前拖拽框
      -> 已完成区域高亮
      -> 左上角步骤提示
      -> 右上角取消按钮
```

## 状态来源
- React 本地状态：加载、保存、运行中、录制热键、框选中、错误信息
- Tauri bootstrap：`settings`, `history`, `latestRun`, `hotkeyError`
- Tauri event：`morse://run-finished`, `morse://hotkey-error`
- 查询参数：`mode`, `slots`, `slot`

## 前端到原生调用
- `syncBootstrap()` -> `morse_get_bootstrap`
- `handleSaveSettings()` -> `morse_save_settings`
- `performSelectionSession()` -> `morse_begin_region_selection`
- Overlay mouse up -> `morse_overlay_submit_selection`
- Overlay cancel / Esc / 右键 -> `morse_overlay_cancel_selection`
- `handleRunRecognition()` -> `morse_run_recognition`

## 录制热键流
```text
点击热键按钮
  -> isRecordingHotkey = true
  -> onKeyDown 捕获组合键
  -> formatRecordedHotkey(event)
  -> 更新 form.hotkey
  -> 保存时交给 morse_save_settings
```

## 样式系统
- Tailwind v4：`src/App.css`
- shadcn config：`components.json`
- Vite plugin：`@tailwindcss/vite`
- 路径别名：`@ -> src`

## 关键文件
- `src/main.tsx`
- `src/App.tsx`
- `src/App.css`
- `src/components/app/morse-page.tsx`
- `src/components/ui/sidebar.tsx`
- `src/components/ui/tooltip.tsx`
- `src/lib/utils.ts`

## 约束
- 无 React Router
- Overlay 直接复用同一应用入口
- Tooltip 组件依赖根级 `TooltipProvider`
- 主业务逻辑目前高度集中在 `src/components/app/morse-page.tsx`
