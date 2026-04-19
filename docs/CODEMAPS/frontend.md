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
  - 主工作台容器
  - bootstrap 同步
  - autosave、热键录制、识别执行、事件订阅
- `src/components/app/morse-overlay.tsx`
  - overlay 拖拽与步骤提示
- `src/components/app/morse-panels.tsx`
  - 控制台面板（设置 + 测试验证）
  - 结果面板
  - 采样区域面板
  - 历史面板
- `src/components/app/morse-utils.ts`
  - 表单解析、热键格式化、时间/区域格式化、overlay 参数解析
- `src/components/app/morse-types.ts`
  - 页面共享类型与常量

## 状态来源
- React 本地状态：加载、保存、运行中、录制热键、框选中、错误信息、测试验证状态
- Tauri bootstrap：`settings`, `history`, `latestRun`, `hotkeyError`
- Tauri event：`morse://run-finished`, `morse://selection-progress`, `morse://hotkey-error`
- 查询参数：`mode`, `slots`, `slot`

## 前端到原生调用
- `syncBootstrap()` -> `morse_get_bootstrap`
- autosave -> `morse_save_settings`
- `performSelectionSession()` -> `morse_begin_region_selection`
- `handleVerificationRun()` -> `morse_run_recognition(autoType: false)`
- Overlay mouse up -> `morse_overlay_submit_selection`
- Overlay cancel / Esc / 右键 -> `morse_overlay_cancel_selection`

## 测试入口
- `src/components/app/morse-utils.test.ts`
  - 覆盖表单解析、热键格式化、overlay 参数解析、矩形标准化、结果标准化、时间/区域格式化
