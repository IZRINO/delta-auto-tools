# 指纹密码

局内指纹锁辅助：同一帧截屏，名条模板匹配定人，候选九宫格按纹理计数定 A/B/C 档，再把格子对应该人档案顺序的指纹图做 NCC，按 1→N 点格子中心。无 OCR，不内置游戏截图。

## 目录

```text
src-tauri/src/fingerprint/
├── mod.rs        # ToolLogic、command、热键、采集
├── matching.rs   # 占用方差、档位、贪心分配、点击顺序
├── pipeline.rs   # 截屏 → 认人 → 配图 → 点序
├── overlay.rs    # 名条 / 9 候选 / 8 档案槽框选
├── types.rs
├── settings.rs   # fingerprint_settings.json + fingerprint-images/
├── events.rs
src/components/app/
├── fingerprint-page.tsx
├── fingerprint-overlay.tsx
├── fingerprint-types.ts
└── fingerprint-utils.ts
```

## 流程

1. 全局校准一次：名条 1 块、候选 9 格、档案 8 槽（A 用前 4，B 前 6，C 全用）。跟人无关。
2. 图库：安装包内置 9 人档案指纹（`src-tauri/resources/fingerprint/`）。启动时灌进空槽。遇到新角色只采名条。档案指纹也可按 1–4 / 1–6 / 1–8 自己重采。
3. 热键（默认 F6，`Strict`）：同一帧认人 → 有纹格数必须是 5/7/9 → 用前 4/6/8 枚模板配对应格子 → 按模板序号点击。缺枚则报错不点。

设置不进 Profile。参考图只存路径，文件在配置目录 `fingerprint-images/`。

## Command

`fingerprint_get_bootstrap`、`fingerprint_save_settings`、`fingerprint_set_hotkey_recording`、`fingerprint_begin_region_selection`、`fingerprint_overlay_submit_selection`、`fingerprint_overlay_cancel_selection`、`fingerprint_run`、`fingerprint_capture_name`、`fingerprint_capture_archive`、`fingerprint_delete_person`、`fingerprint_read_image`。

事件：`fingerprint://run-finished`、`fingerprint://selection-progress`、`fingerprint://hotkey-error`。overlay mode：`fingerprint-overlay`。
