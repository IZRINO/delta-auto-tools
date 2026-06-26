# 趣闻

关于 Delta Auto Tools 代码库的一些边角料与冷知识。所有数字均基于 v0.17.5（235 次提交、63 个 tag）的实际仓库快照核实。

---

## 最古老的存活代码

项目里最「长寿」的代码，是 Morse 摩斯码解码器与热键系统——两者都从 2026-04-18 的首次提交 `feat: initial commit` 一路存活至今。

- **`src-tauri/src/morse/decoder.rs`（33 行）**：这是整个仓库里大概最古老、也最精简的存活文件。它只做一件事——把摩斯码点划串翻译成单个数字字符，靠一张 10 项的 `MORSE_DIGIT_MAP` 常量表完成全部工作：

  ```rust
  pub fn decode(morse: &str) -> Result<char, String> {
      MORSE_DIGIT_MAP
          .iter()
          .find_map(|(pattern, digit)| (*pattern == morse).then_some(*digit))
          .ok_or_else(|| format!("无法识别的摩斯密码: {morse}"))
  }
  ```

  注意它**只解码数字 0–9**，不解码字母——这呼应了《三角洲行动》里摩斯谜题的实际形态。历经多次重构，这份 33 行的文件几乎纹丝未动，堪称项目的「活化石」。

- **`src-tauri/src/hotkeys.rs`（约 1363 行）**：热键系统同样自首次提交存活至今。它从一开始就承担全局共享 willhook 键盘钩子、scope 注册、普通/hold 两种绑定、跨 scope 冲突检测（`ConflictPolicy`）等职责。与 `decoder.rs` 的「不动」不同，热键系统是「越长越大」，从最初的小模块膨胀成现在的千行级文件。

> 一句话：项目最古老的存活代码，要么小到不需要改（decoder.rs），要么大到一直在加（hotkeys.rs）。

---

## TODO / FIXME 计数：零

在整个 `src-tauri/src/`（Rust）与 `src/`（TypeScript/TSX）范围内检索 `TODO` / `FIXME` / `HACK` / `XXX` 注释，结果如下：

| 范围 | 命中数 |
|------|--------|
| Rust 源码（`*.rs`） | **0** |
| 前端源码（`*.ts` / `*.tsx` / `*.js`） | **0** |

也就是说，这是一个**完全没有 TODO/FIXME/HACK 注释**的代码库。这要么说明项目纪律异常严格，要么说明所有待办都被外化成了 GitHub Issues（项目确实用 `gh` CLI 管理五级分流标签：needs-triage / needs-info / ready-for-agent / ready-for-human / wontfix）。无论原因如何，对一个 235 次提交的项目而言，这个「零」相当罕见。

---

## 最长的文件：`rapidfire/mod.rs`

按行数排序，仓库最大的几个 Rust 文件如下（v0.17.5 实测）：

| 排名 | 文件 | 行数 |
|------|------|-----:|
| 1 | `rapidfire/mod.rs` | **2106** |
| 2 | `audio/watcher.rs` | 1674 |
| 3 | `hotkeys.rs` | 1363 |
| 4 | `timer/mod.rs` | 1300 |
| 5 | `audio/mod.rs` | 1155 |
| 6 | `counter/mod.rs` | 1107 |
| 7 | `profile/mod.rs` | 665 |
| 8 | `morse/recognition.rs` | 518 |

`rapidfire/mod.rs` 以 2106 行高居榜首。连发器的逻辑——按住触发键、每 session 独立 OS worker 线程、卡片级不追加/抖动/间距、hold scope 与普通 scope 共存的冲突策略——似乎都堆在这一个文件里。

> 温和的重构提示：当一个 `mod.rs` 超过 2000 行，往往意味着内部已经长出了多个可以独立成模块的子职责（worker 线程管理、键位序列生成、间距/抖动计算、状态机……）。考虑到 v0.17.5 刚刚把 timer/counter/rapidfire 的共享生命周期整合进 `sync_tool.rs`，连发器自身剩下的「私有复杂度」或许正是下一个值得拆分的候选。

---

## 命名起源：「Delta」从哪来

项目名 **Delta Auto Tools** 里的「Delta」并不是希腊字母 Δ 的抽象命名，而是直接指代游戏《**三角洲行动**》（Delta Force）。「三角洲」即 delta，工具是为这款游戏的玩家做的自动化辅助。

而项目最早的功能 **Morse 摩斯识别**，也对应着游戏内的一个具体谜题形态——《三角洲行动》中存在需要破译的摩斯密码，且谜底为数字。这就解释了为什么 `morse/decoder.rs` 只解码数字 0–9：它不是通用摩斯码库，而是针对游戏谜题的定向工具。从「为某个具体谜题写 33 行解码器」起步，最终长成涵盖计时、计数、连发、音频触发、主题引擎的桌面工具集——这是项目最朴素也最真实的起源故事。

---

## 依赖考古

翻看 `src-tauri/Cargo.toml` 与 `package.json`，能发现一些有意思的依赖选择。

### Rust 侧（`Cargo.toml`）

| 依赖 | 版本 | 角色 | 备注 |
|------|------|------|------|
| `enigo` | **0.6.1** | 输入模拟（自动按键/输入） | 跨平台输入库。0.6 系列已有些年头，0.x 的 API 与 0.5/0.7 都不同，算是一个「需要小心 pin」的老朋友。 |
| `willhook` | 0.6.3 | 全局键盘钩子 | 相对小众的 crate，承担热键系统的底层钩子。项目对它的封装见 `hotkeys.rs`。 |
| `xcap` | 0.9.6 | 屏幕截取 | Morse 识别「截屏 → 二值化」链路的截屏来源。 |
| `image` | 0.25.10 | 图像处理 | 二值化、轮廓检测、识色的图像运算底座。 |
| `rodio` | 0.20 | 音频播放 | v0.15.0 引入音频功能后加入。 |
| `windows-sys` | 0.61 | Windows API | 连发器 worker 线程、键位抑制等底层调用。 |
| `reqwest` | 0.12.28 | HTTP 客户端 | 早期为 Delta 鉴权/游戏数据引入；Delta 移除后大概仍服务于攻略站抓取。 |

最有「考古感」的是 `enigo 0.6.1`：在一个 2026 年的项目里 pin 在 0.6 系列，说明它的 API 正好满足自动输入需求，且升级成本可能不低，于是被长期保留。

### 前端侧（`package.json`）

| 依赖 | 版本 | 备注 |
|------|------|------|
| `react` / `react-dom` | **19.2.7** | React 19，相当靠前的版本。 |
| `tailwindcss` | **4.3.1** | Tailwind v4，CSS-first 方案，**不存在 `tailwind.config.js`**，主题 token 全部写在 `src/App.css` 的 `@theme inline`。 |
| `shadcn` | 4.11.0 | shadcn/ui CLI，组件源码直接落在 `src/components/ui/`。 |
| `@remixicon/react` | ^4.9.0 | 图标库；按约定 Button 内图标必须设置 `data-icon="inline-start"` / `"inline-end"`。 |
| `vite` | 7.3.5 | 构建器。 |
| `recharts` | 3.8.1 | 图表库，大概用于游戏数据/统计可视化（Delta 移除后用途可能已收窄）。 |
| `next-themes` | ^0.4.6 | 主题切换；与项目自己的主题引擎（v0.17.0 引入）配合。 |

前端栈整体偏「新」：React 19 + Tailwind v4 + Vite 7 + shadcn 4，都是 2025–2026 年的主力版本。这与 Rust 侧 `enigo 0.6.1` 的「老成」形成有趣对照——前端追新，后端在关键底层库上求稳。
