# 指纹密码

局内指纹锁辅助：名条框一次位置，识别时 OCR 读字认人；截候选九宫格定 A/B/C 档，用该人档案指纹 NCC 按 1→N 点格子中心。不用每个角色采集名条。OCR 失败则回退用档案指纹在图库里认人。

## 目录

```text
src-tauri/src/fingerprint/
├── mod.rs        # ToolLogic、command、热键、采集
├── matching.rs   # 占用方差、档位、贪心分配、点击顺序
├── pipeline.rs   # 截屏 → 认人 → 配图 → 点序
├── overlay.rs    # 名条 / 9 候选 / 8 档案槽 / 最多 7 个点击区域框选
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

1. 布局页：候选 9 格必框。名条只要框一次位置（识别 OCR 读字），不要每个角色采名条图。档案 8 槽在图库页，只给采 1–X 用。
2. 图库：安装包内置 9 人档案指纹（`src-tauri/resources/fingerprint/`）。启动时灌进空槽。不显示、不要求采名条。新角色只填名字添加，档案指纹在角色档案页采 1–X。
3. 热键（默认 F6，`AllowHold`）：九宫格一次截包围盒；名条另截小块 OCR。读中人名则只配该人档案，否则扫图库。裁 15% 边框后按方差缺口分成 5/7/9，同尺寸灰度 NCC + 最大权分配，档案图进程内缓存。缺枚报错带人名、档位、有纹格编号。可与摩斯/连发器等同键共存，按下后全部触发。
4. 自动点击链路与摩斯相同：识别成功且 `autoClickEnabled` 开启时，先按 1→N 点九宫格中心（间隔 `clickDelayMs`）；全部成功后再点配置的 `clickRegions`（最多 7 个，各有独立 `delayMs`）；这组也成功后若配置了 `afterClickHotkey` 再按一次该键。只识别入口不走这条链路。

设置不进 Profile。参考图只存路径，文件在配置目录 `fingerprint-images/`。

## Command

`fingerprint_get_bootstrap`、`fingerprint_save_settings`、`fingerprint_set_hotkey_recording`、`fingerprint_begin_region_selection`、`fingerprint_overlay_submit_selection`、`fingerprint_overlay_cancel_selection`、`fingerprint_run`、`fingerprint_capture_name`、`fingerprint_capture_archive`、`fingerprint_delete_person`、`fingerprint_read_image`。

事件：`fingerprint://run-finished`、`fingerprint://selection-progress`、`fingerprint://hotkey-error`。overlay mode：`fingerprint-overlay`。
