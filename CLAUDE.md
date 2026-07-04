# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Delta Auto Tools — Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust 桌面工具，面向《三角洲行动》玩家。四个原生能力模块：Morse
摩斯识别、计时器、计数器、连发器。外加攻略网站工作台。

## Commands

```bash
bun install                    # 安装前端依赖
bun run dev                    # Vite 前端开发服务器（端口 1420，strictPort）
bun run tauri dev              # 完整桌面开发（Vite + Tauri）
bun run build                  # tsc && vite build
bun run test                   # Vitest 前端单元测试
bun run test:coverage          # 前端覆盖率（仅 morse-utils.ts）
cargo check --manifest-path src-tauri/Cargo.toml   # Rust 编译检查
cargo test --manifest-path src-tauri/Cargo.toml    # Rust 单元测试
```

运行单个前端测试：`bunx vitest run src/components/app/morse-utils.test.ts`
运行单个 Rust 测试：`cargo test --manifest-path src-tauri/Cargo.toml <test_name>`

## Source of Truth 优先级

1. `src-tauri/tauri.conf.json`
2. `package.json`
3. `src/` 和 `src-tauri/src/`

文档与代码不一致时以当前实现为准。

## Wiki 文档优先 (droid-wiki/)

`droid-wiki/` 是项目自维护的结构化文档（36 个页面），覆盖项目概览、系统架构、各功能模块、底层系统、开发流程、约定、配置参考和发布流程。**当不确定某模块如何工作、某约定是什么、某流程怎么走时，优先查阅 `droid-wiki/` 下的对应文档，不要凭记忆猜测。**

文档结构：

| 目录/文件 | 内容 |
|----------|------|
| `overview/` | 项目概览、系统架构、快速开始、术语表 |
| `features/` | 各功能模块详解（morse / timer / counter / rapidfire / recognition / strategy / about） |
| `systems/` | 底层系统（tool-base / sync-tool / hotkeys / key-suppressor / overlay-windows / global-state / logging / theme-engine / profile-system） |
| `how-to-contribute/` | 开发流程、测试、调试、模式与约定、工具链 |
| `reference/` | 配置项与依赖参考 |
| `deployment.md` | 部署与发布流程 |

入口：`droid-wiki/overview/index.md`。

<!-- CODEGRAPH_START -->
## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code questions in one call — the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep can't follow. Name a file or symbol in the query to read its current line-numbered source. If it's listed but deferred, load it by name via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely — indexing is the user's decision.

After modifying code, run `codegraph sync` to refresh the index — no need to sync after every small change, just before larger explorations.
<!-- CODEGRAPH_END -->

## AI 输出规范

### 事实标注（TAG）

事实声明必须标注来源标签，无标注的疾病/法规/引用/命名实体禁止出现：

- `[KNOWN]` 训练事实 · `[COMPUTED]` 计算 · `[INFERRED]` 推理 · `[COMMON]` 领域常识 · `[FRAME]` 符号框架（内部自洽 ≠ 现实映射） · `[GUESS]` 无依据

**FRAME→REALITY 禁令**：禁止将符号框架（占星、类型学等）翻译为现实世界断言（医学/法律/金融）而不标注翻译；结论留在源框架内。

**事后检验**：框架若不知结果就无法预测 → 标注 `[INFERRED, post-hoc]`，容纳性而非预测性。

### 置信度（CONFIDENCE）

- `HIGH` ≥80% · `MED` 50–80% · `LOW` 20–50% · `VERY LOW` <20% · `UNKNOWN`
- `[FRAME]` 现实映射和 `[GUESS]` 上限为 `LOW`。

### 不知原则

不确定时首行写「我不知道」，不掩盖、不捏造、不埋藏。

### 反谄媚（ANTI-SYCOPHANCY）

警惕信号：异常优雅、单一模式解释一切、无证据就同意、未授权权威给细节。应对：砍细节、加 `[GUESS]`、或「我不知道」。

### 引用与修正

禁止捏造引用。持立场因一致性时公开修正。末尾附 `[RULES I BROKE]: which, where, why.`。

### 豁免

执行类任务（写/改/调试代码、跑命令、文件操作）豁免 TAG 与 CONFIDENCE 标注；仅在事实陈述、诊断结论、外部建议时使用。

所有 AI 输出必须使用中文，技术术语保持英文原名。代码中的字符串、错误信息、UI 文案使用中文。Commit message 使用中文。

英文输出风格：砍冠词/填充词/客套/模糊。Fragment OK。短同义词优先（big not extensive, fix not "implement a solution for"）。缩写常见术语（DB/auth/config/req/res/fn/impl）。箭头表因果（X -> Y）。一字够用一字。技术术语保持原文。代码块不变。错误原样引用。

模式：`[thing] [action] [reason]. [next step].`

中文输出风格：砍虚词（的/了/着/过/其实/ basically/ just）、客套（当然/没问题/很高兴/不难看出）、冗余修饰（非常/十分/极其/大幅度）。短词优先（改→非重构，修→非修复，删→非移除）。缩写常见术语（DB/鉴权/配置/请求/响应/函数/实现）。箭头表因果（X → Y）。技术术语保留英文原名，不硬译（mutex 不写"互斥锁"，render 不写"渲染"，callback 不写"回调"——除非是中文已广泛接受的如死锁、竞态条件）。一句够用一句。代码块不变。错误原样引用。

模式：`[对象] [动作] [原因]。[下一步]。`

**Auto-Clarity Exception**：安全警告、不可逆操作确认、多步骤序列（片段顺序易误解）、用户要求澄清或重复问题时，暂时切回完整表述，完成后再恢复精简风格。

**Auto-Clarity Exception**：安全警告、不可逆操作确认、多步骤序列（片段顺序易误解）、用户要求澄清或重复问题时，暂时切回完整表述，完成后再恢复精简风格。

## Architecture

### 前端入口链路

`index.html` → `src/main.tsx` → `src/App.tsx`

App.tsx 无路由库，通过 `useState<ToolId>` 切换工具页。Overlay/display/position 模式通过 `?mode=` 查询参数分支进入（
`overlay`、`timer-display`、`timer-position`、`counter-display`、`counter-position`、`rapidfire-display`、`rapidfire-position`
），不可用路由替代。

### 页面组件模式

每个工具页遵循同一模式：

- **Bootstrap/FormData 双状态**：`bootstrap`（Rust 返回的不可变规范态）+ `form`（本地可编辑草稿），脏检测通过 `JSON.stringify`
  比较
- **Autosave**：表单变更后 debounce 400ms（`AUTOSAVE_DELAY_MS`）调用 `xxx_save_settings`，`autosaveVersionRef` 防止陈旧保存覆盖
- **Form↔Settings 转层**：`settingsToForm()`（int→string 供 Input）和 `parseSettingsForm()`（验证+string→int 供 Rust）
- 容器页（`morse-page.tsx` 等）负责状态编排，子组件只接收 props

### Tauri IPC 模式

- **Command 调用**：`invoke<XxxBootstrap>("tool_action", { params })`
- **事件监听**：`listen<XxxPayload>("tool://event-name", callback)`，事件名格式 `{tool}://{event}`。后端在
  `morse/events.rs`、`timer/events.rs`、`counter/events.rs`、`rapidfire/events.rs` 定义常量，前端通过
  `src/lib/tauri-events.ts` 的 `MORSE_EVENTS` / `TIMER_EVENTS` / `COUNTER_EVENTS` / `RAPIDFIRE_EVENTS` / `GLOBAL_EVENTS`
  和显式泛型 `listen<PayloadType>(EVENTS.xxx, handler)` 订阅，避免硬编码事件名。
- **原生 shell 检测**：`useNativeShell()` 检查 `__TAURI_INTERNALS__`，浏览器预览模式下禁用所有原生命令

### Overlay 窗口系统

两类机制：

1. **同窗口 overlay**（Morse 区域框选）：`?mode=overlay` → `RegionSelectionOverlay`，全屏透明拖拽框选，坐标通过
   `morse_overlay_submit_selection` 提交
2. **独立窗口 overlay**（Timer/Rapidfire 透明显示/位置设置）：各自有 display 和 position 两种模式，position 模式拖拽定位坐标提交

### Rust 后端模块

| 模块           | 路径                              | 职责                                                                                              |
|--------------|---------------------------------|-------------------------------------------------------------------------------------------------|
| tool_base    | `src-tauri/src/tool_base.rs`    | 工具模块共享泛型基座：ToolLogic trait、ToolState<T>、ToolStateInner<T>、get_bootstrap<T>                      |
| global_state | `src-tauri/src/global_state.rs` | 全局总开关（GlobalState）与 enabled-changed 事件                                                          |
| morse        | `src-tauri/src/morse/`          | 屏幕截取→二值化→轮廓检测→摩斯解码→自动输入；overlay 多步骤框选会话；MorseState = ToolState<MorseLogic>                      |
| timer        | `src-tauri/src/timer/`          | 多计时器，250ms tick 循环，透明窗口；TimerState 包装 ToolState<TimerLogic>                                     |
| counter      | `src-tauri/src/counter/`        | 多计数器，透明窗口，计数器运行态独立持久化；CounterState 包装 ToolState<CounterLogic>                                   |
| rapidfire    | `src-tauri/src/rapidfire/`      | 按住触发键连发，每 session 独立 OS worker 线程，卡片级不追加/抖动/间距；RapidfireState = ToolState<RapidfireLogic>       |
| hotkeys      | `src-tauri/src/hotkeys.rs`      | 全局共享 willhook 键盘钩子，scope 注册，普通/hold 两种绑定，跨 scope 冲突检测（ConflictPolicy）                           |
| about        | `src-tauri/src/about/`          | 关于面板（版本/协议/依赖致谢）+ Tauri 官方更新器（check/download_and_install），进度事件 `about://update-progress`        |
| strategy     | `src-tauri/src/strategy/`       | 兼容入口：`strategy_open_window` 创建子 WebView，`strategy_fetch_page` Chrome 头抓取+JS 重定向跟随               |

新增 Tauri command 必须同时注册到 `src-tauri/src/lib.rs` 的 `generate_handler![]` 和
`src-tauri/capabilities/default.json`。

### 更新器（Tauri Updater）

项目已接入 `tauri-plugin-updater`（Rust）与 `@tauri-apps/plugin-updater`（前端）+ `@tauri-apps/plugin-process`（relaunch）。

- `tauri.conf.json` 中 `plugins.updater` 配置了 GitHub Releases 端点与 `installMode: "passive"`；`pubkey` 字段需运行
  `scripts/setup-update-key.ps1` 生成密钥后填入
- 构建发布版前需设置 `$env:TAURI_SIGNING_PRIVATE_KEY`（私钥内容，非路径），可选 `$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- `about` 模块封装了 `about_check_for_update`（检查）和 `about_download_and_install`（下载+安装+进度推送），前端通过
  `ABOUT_EVENTS.updateProgress` 监听进度
- `pubkey` 为空时更新器返回错误，前端降级为「打开 GitHub Release 页面」模式

## UI & Styling

### 视觉方向

当前 UI 迁移到 daisyUI token 体系。保留 Radix headless 组件的焦点管理、键盘导航、Portal 与无障碍行为；视觉层使用 daisyUI class、Tailwind CSS 和 `src/App.css` token。内置主题保留 `olive-amber`、`valentine`、`arctic-blue`，默认 `valentine`。

### 组件层

- **共享工业组件**（`src/components/app/app-ui.tsx`）：`AppPage`（12 列 Work Grid）、`PageHero`、`SignalTile`、`TacticalCard`、
  `SectionHeader`、`ControlTile`、`InlineControl`、`CardBody` 等。三个以上页面需要同一种结构时应先扩展共享组件。
- **基础组件**（`src/components/ui/`）：保留 Radix headless 行为能力，class 优先映射 daisyUI 组件语义，图标库使用 remixicon。Button 内图标必须
  `data-icon="inline-start"` / `"inline-end"`。
- **禁止新增**旧桌面/战术风格自定义 CSS 类。仅 daisyUI + Tailwind 工具类 + `src/App.css` 主题 token。

### Tailwind v4

CSS-first 方案，**不存在** `tailwind.config.js`。主题 token 在 `src/App.css` 的 `@theme inline` 中。全局 `--radius: 0`。

### CSS 变量（设计 token）

Carbon `#0C0C0B`、Slate `#171715`、Iron `#232320`、Chalk `#D8D4CC`、Zinc `#9A968E`、Dust `#6E6B65`、Seam `#2A2926`、Amber
`#E8A000`、Rust `#C85400`、Moss `#3F8A30`、Void `#050504`。

## Key Conventions

### Rust serde

所有对外序列化的 Rust 结构体**必须**使用 `#[serde(rename_all = "camelCase")]`。前端必须匹配。

### 热键冲突规则

- `ConflictPolicy` 枚举声明冲突策略：`Strict`（禁止跨 scope 复用）和 `AllowHold`（允许 hold scope 与普通 scope 共存）。
- `HotkeyRegistration` 和 `HoldRegistration` 均包含 `conflict_policy` 字段；`replace_scope` / `replace_hold_scope` 接收该参数。
- Timer 普通 scope 与 Counter 普通 scope 与 Rapidfire hold scope 允许同键共存（双方均使用 `ConflictPolicy::AllowHold`
  ）；运行时会先分发连发器 hold Down/Up，再分发计时器/计数器普通快捷键。Morse 与 Timer 普通快捷键冲突、Morse 与 Counter
  普通快捷键冲突、Morse 与 Rapidfire hold 冲突仍必须拒绝（Morse 使用 `ConflictPolicy::Strict`）。
- 其他跨 scope 冲突必须拒绝。录制热键时暂停对应 scope。

### Overlay 透明窗口约束

计时器/计数器/连发器透明窗口必须无边框、透明、置顶、点击穿透。位置设置窗口可保留校准靶风格。overlay 必须保持透明背景。

### 攻略网站约束

主窗口内嵌 `strategy-content` 子 WebView，不创建独立浏览器窗口，不使用 iframe/srcDoc。新增站点和刷新档位使用内联面板，不使用
Dialog/SelectContent 等浮层。不得隐藏 Left Index Rail。

## Version Release

### 版本号同步

版本号必须同步更新 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。如 `src-tauri/Cargo.lock` 中的本包版本随
Cargo 解析更新，也应一并提交。

### 签名密钥（首次必须）

接入自动更新前需要先执行 `scripts/setup-update-key.ps1` 生成密钥对：

- 私钥保存到 `$HOME/.tauri/delta-auto-tools.key`（**不入库**）
- 公钥自动写入 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`（**公开字段，必须随代码发布**）
- 私钥用 `bunx --offline tauri signer generate` 生成

### 构建

每次更新版本号后必须运行带签名的桌面构建。**构建时必须设置 `TAURI_SIGNING_PRIVATE_KEY` 环境变量**（私钥内容或
`TAURI_SIGNING_PRIVATE_KEY_PATH` 指向私钥文件路径），否则不会生成 `.sig` 签名文件。推荐用 `scripts/build-release.ps1`
一键签名构建。

打包成功后必须检查以下产物存在：

- `src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe`
- `src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe.sig`

### 生成 latest.json

构建成功后必须运行 `scripts/generate-latest-json.ps1`，从 `*-setup.exe.sig` 签名文件生成
`src-tauri/target/release/bundle/latest.json`。**这是 Tauri 官方更新器运行时拉取的清单文件**；不生成且不上传到 Release
会导致应用内「检查更新」失败（错误：Could not fetch a valid release JSON from the remote）。

### 发布 Commit

发布 commit 不能只写 `发布 v<version>`。Subject 使用 `发布 v<version>`，正文必须包含变更摘要，至少包含 `变更：` 段。变更项从本次实际 diff / Release notes 提炼，禁止泛泛"更新版本"。推荐格式：

```bash
git commit -m "发布 v<version>" -m "变更：
- ...
- ..."
```

### Tag

每次版本发布必须创建并推送对应 `v<version>` Tag：

```bash
git tag -a v<version> -m "发布 v<version>"
git push origin v<version>
```

### 网络与代理

`git push` 或 `gh release` 等操作访问 GitHub 时，如遇连接重置或超时（`Recv failure` / `Failed to connect`），可在命令前设置本地代理环境变量：

```bash
set HTTP_PROXY=http://127.0.0.1:7897&& set HTTPS_PROXY=http://127.0.0.1:7897&& git push origin master v<version>
set HTTP_PROXY=http://127.0.0.1:7897&& set HTTPS_PROXY=http://127.0.0.1:7897&& gh release create ...
```

> **注意**：`&&` 前不要有空格，否则 Windows `set` 会将尾部空格带入变量值导致 `Unsupported proxy syntax` 错误。

### GitHub Release + 3 个资产上传

每次版本发布必须创建 GitHub Release，并**同时上传 3 个资产**（缺一不可）：

| 资产                                             | 作用                                   |
|------------------------------------------------|--------------------------------------|
| `delta-auto-tools_<version>_x64-setup.exe`     | NSIS 一键安装包                           |
| `delta-auto-tools_<version>_x64-setup.exe.sig` | NSIS 签名                              |
| **`latest.json`**                              | Tauri updater 静态端点文件（不传则应用内「检查更新」失败） |

```bash
# 新建 Release 并上传 3 个资产
gh release create v<version> \
  src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe \
  src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe.sig \
  src-tauri/target/release/bundle/latest.json \
  --repo IZRINO/delta-auto-tools --target master \
  --title "delta-auto-tools <version>" --notes "<发布说明>"

# Release 已存在时覆盖上传（同样 3 个资产）
gh release upload v<version> \
  src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe \
  src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe.sig \
  src-tauri/target/release/bundle/latest.json \
  --repo IZRINO/delta-auto-tools --clobber
```

### 验证

Release 发布后必须验证：

```bash
gh release view v<version> --repo IZRINO/delta-auto-tools \
  --json tagName,url,isDraft,isPrerelease,assets
```

确认非 draft、非 prerelease，且**全部 3 个资产**状态均为 `uploaded`。

### Beta / 预发布版本

Beta 版本用于快速向测试用户推送未正式发布的功能。**流程轻量，可随时发布**。

#### 版本号格式

Beta 版本号必须使用 **SemVer pre-release** 格式：`<major>.<minor>.<patch>-beta.<N>`

```
0.17.0-beta.1    ← 第 1 个 beta
0.17.0-beta.2    ← 第 2 个 beta（修复）
0.17.0           ← 正式发布（stable 通道自动检测到）
```

> **重要**：`0.17.0-beta.1 < 0.17.0`（SemVer 规则，pre-release 版本优先级低于对应正式版）。
> 本项目更新逻辑遵循 SemVer 全序比较（数值部分 + pre-release），因此：
> - `0.17.0-beta.5` → `0.17.0`：**提供更新**（同数值正式版 > beta，beta 升级到正式版）
> - `0.17.0-beta.5` → `0.17.1`：**提供更新**（数值 `0.17.1 > 0.17.0`）
> - `0.17.0` → `0.17.0-beta.5`：**不更新**（正式版不降级到同数值 beta）

#### 与正式版的关键差异

| 项目         | 正式版（stable）                | Beta 版                             |
|------------|---------------------------|-------------------------------------|
| 构建签名      | ✅ 必须（`TAURI_SIGNING_PRIVATE_KEY`） | ❌ 不需要（不生成 `.sig`）                  |
| 产物         | `.exe` + `.sig` + `latest.json` | 仅 `.exe`                            |
| GitHub Release | 默认                       | `--prerelease` 标记                    |
| 自动更新（beta→beta） | —                        | ❌ 不支持（无 beta 通道、无 `latest-beta.json`） |
| 自动更新（beta→stable） | —                        | ✅ 同数值正式版即可更新（如 `0.17.0-beta.5`→`0.17.0`）；数值更高也更新（如 `0.17.0-beta.5`→`0.17.1`） |

#### 自动更新机制

Beta 版本**不建立独立的 beta 更新通道**，不需要 `latest-beta.json`。Beta 应用内的「检查更新」走 stable 端点
（`/releases/latest/download/latest.json`），与正式版完全一致。因为 GitHub `/releases/latest` 只解析**非 prerelease**
的 Release，所以：

- 当前无更新时 → 显示"已是最新"
- 正式版 `0.17.0` 发布后 → beta `0.17.0-beta.5` 检测到 `0.17.0` 并提示更新（同数值正式版 > beta）→ 下载签名安装包 → 自动更新到正式版
- 正式版 `0.17.1` 发布后 → beta `0.17.0-beta.5` 检测到 `0.17.1` 并提示更新 → 下载签名安装包 → 自动更新到正式版

> **实现**：`src-tauri/src/about/mod.rs` 中的 `should_offer_update()` 函数按 SemVer 全序比较
> （`version_rank`：数值部分 + 是否正式版 + pre-release 字符串），同数值正式版严格高于对应 beta。
> 这覆盖了 `about_check_for_update` 和 `about_download_and_install` 两个命令。

#### Beta 发布完整流程

1. 同步更新版本号（三处：`package.json` / `Cargo.toml` / `tauri.conf.json`），版本号带 `-beta.N` 后缀
2. **无签名构建**：`bun run tauri build`（不需要 `TAURI_SIGNING_PRIVATE_KEY`）
3. 检查产物：`src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe`
4. 提交 + Tag：`git commit` → `git tag -a v<version>` → `git push origin master v<version>`
5. 创建 **prerelease** Release 并上传 1 个资产：

```bash
gh release create v<version> \
  src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe \
  --repo IZRINO/delta-auto-tools --target master \
  --prerelease \
  --title "delta-auto-tools <version>" --notes "<beta 发布说明>"

# Release 已存在时覆盖上传
gh release upload v<version> \
  src-tauri/target/release/bundle/nsis/delta-auto-tools_<version>_x64-setup.exe \
  --repo IZRINO/delta-auto-tools --clobber
```

6. 验证：

```bash
gh release view v<version> --repo IZRINO/delta-auto-tools \
  --json tagName,url,isDraft,isPrerelease,assets
```

确认 `isPrerelease: true`，且 `.exe` 资产状态为 `uploaded`。

## Agent skills

### Issue tracker

Issues 使用 GitHub Issues，通过 `gh` CLI 读写。详见 `docs/agents/issue-tracker.md`。

> **Windows cmd 多行字符串截断**：`gh issue comment` / `gh release create` 等命令的 `--body` / `--notes` 参数在 Windows cmd.exe 中传递多行内容时会被截断为只发送第一行。**必须使用 `--body-file` / `--notes-file` 从文件读取**：先将内容写入临时文件（如 `temp/release-notes.md`），然后 `gh issue comment <number> --body-file temp/issue-reply.md` 或 `gh release create v<version> --notes-file temp/release-notes.md ...`。禁止在 cmd.exe 中直接用 `--body "多行内容"` 或 `--notes "多行内容"` 传多行字符串。

### Triage labels

使用五级分流标签：needs-triage / needs-info / ready-for-agent / ready-for-human / wontfix。详见
`docs/agents/triage-labels.md`。

### Domain docs

Single-context 布局：根目录 `CONTEXT.md` + `docs/adr/`。详见 `docs/agents/domain.md`。

## Repo-Specific Notes

- 使用 **Bun**，不要切换到 npm/pnpm/yarn
- 不存在 `tailwind.config.js`
- 前端测试覆盖已扩展至 `morse-utils.ts` + `timer-utils.ts` + `favorites-utils.ts` +
  `use-bootstrap-form-logic.ts` +
  `use-hotkey-recorder.ts` + `use-autosave.ts` + `about-deps.ts`（Vitest coverage 配置仍只包含 `morse-utils.ts`）
- `.agents/skills/` 和 `.omp/extensions/` 是项目级扩展目录，不要误删
- `README.md`、`AGENTS.md` 和 `CLAUDE.md` 需随重大功能变更一起更新
- 修改代码时，如果改动涉及 `droid-wiki/` 已记录的内容，必须同步更新对应 wiki 页面（改动工具模块行为→`features/<tool>.md`/`systems/<system>.md`；改动架构/约定→`overview/architecture.md`/`how-to-contribute/patterns-and-conventions.md`；改动配置/依赖→`reference/`；改动发布流程→`deployment.md`）；纯文案或纯重构无需更新
