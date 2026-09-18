# 连发器双主键触发键

日期：2026-09-19  
状态：设计已确认，待实施

## 1. 目标

连发器触发键与现有 `Shift+A` 同一套按住语义：绑定所需的键都在按下集合里就开火，松开导致缺任一所需键就停止。把辅助槽从 Ctrl/Alt/Shift/Win 扩到普通键和符号键，使 `A+B`、`F1+-` 可录入、可匹配。

**主例（必须过）：** 卡片 1 = `Shift+A`，卡片 2 = `A+B`。同时按住 Shift、A、B → 两张卡片都 Down。松 Shift（A、B 仍按住）→ 只停卡片 1；松 B（Shift、A 仍按住）→ 只停卡片 2；松 A → 两张都停。

## 2. 非目标

- 不支持三键及以上主键和弦（`A+B+C` 拒绝）。
- 不改目标键：仍为单键。
- 不改摩斯 / 计时器 / 计数器 / 识别的录入 UI。这些工具继续走单主键解析，`A+B` 仍报「多个主键」。
- 不改冲突策略矩阵。`A` 与 `A+B` 不是同一绑定，不冲突。
- 不加新 Tauri command、不加新持久化字段。仍写 `triggerKey` 字符串。

## 3. 语义（对齐 Shift+A）

匹配是**子集**：绑定要的键 ⊆ 当前按下集合，不是「按下集合恰好等于绑定」。多张卡片独立匹配，互不抢占。

| 操作 | `Shift+A`（现状） | `A+B`（本设计） |
|---|---|---|
| 先按辅助再按主键 | Shift 按住，A 按下 → Down | A 按住，B 按下 → Down |
| 先按主键再按辅助 | A 按住，再按 Shift → 补 Down | B 按住，再按 A → 补 Down |
| 松开导致缺所需键 | Up | Up |
| 顺序 | 无关 | 无关：`B+A` 即 `A+B` |
| 按下集合是超集 | `Shift+1` 也会打到裸 `1` | Shift+A+B 同时打到 `Shift+A` 和 `A+B` |

不存在「一张卡片独占 A」：A 同时是卡片 1 的主键和卡片 2 的成员。同一次按住集合可以激活任意多个满足子集的绑定。

修饰键可与双主键并存：卡片写成 `Shift+A+B` 时，三键都要按住才匹配该卡。这与主例不同——主例是两张独立卡片，不是一张三键卡。不单独做三键录入 UI。

## 4. 数据

`HotkeyBinding` 增加 `extra_primary: Option<PrimaryKey>`。

- 缺省 `None`：旧绑定，行为不变。
- `Some(k)`：两个主键。规范化时按 `primary_to_string` 排序，较小的进 `primary`，较大的进 `extra_primary`。`B+A` → `A+B`。
- `Eq`/`Hash` 跟排序后的字段走，冲突检测仍全等比较。

解析分两路，避免其它工具误存打不着的和弦：

- `HotkeyBinding::parse`：维持「最多一个主键」，摩斯/计时器/计数器/识别继续用。
- `HotkeyBinding::parse_allowing_chord`：允第二个非修饰段。连发器 `normalize_trigger_key` / `hotkey_to_string` 走这条。超过两个非修饰段 → 错误「组合键最多两个主键」。

`binding_to_string`：修饰键顺序不变（Ctrl > Alt > Shift > Super），随后两个主键按规范化顺序拼接。`+` 作为主键仍用现有 `Shift++` 规则；双主键含 `+` 时写成 `A++`（A 与 `+`）。

`hotkey_primary_label` 仍返回排序后的第一个主键，供旧调用。新增 `hotkey_primary_labels` → 1 或 2 个标签。连发器校验两个主键都能 `parse_target_key`。

## 5. Hold 匹配

每次 Down/Up 用当前按下集合重算活跃绑定，替换「按刚按下的那一个主键索引」：

```
pressed_primaries: HashSet<PrimaryKey>
pressed_modifiers: HashSet<ModifierKey>

desired = 所有 enabled hold 绑定，满足：
  binding 的每个主键 ∈ pressed_primaries
  binding.modifiers ⊆ effective_hold_modifiers(pressed_modifiers, primaries)
```

Down = `desired - active`；Up = `active - desired`。Alt 作主键时仍从修饰集合去掉 Alt（现逻辑）。

主例在按下 Shift+A+B 时 `desired` 同时含 `Shift+A` 与 `A+B`，两张卡片各自 Down。缺 Shift 时前者离开 `desired`，后者留下。

注入事件继续忽略。全局开关关闭仍更新按下集合，只跳过回调。

## 6. 录入（仅连发器触发键）

对齐 Shift+A：辅助键按下不提交，主键按下才提交。普通键/符号键现在也能当辅助键，所以**无修饰键时**第一颗主键不能立刻提交，否则录不成 `A+B`。

状态机（只用于 `field === "triggerKey"`）：

1. 修饰键单独按下：不提交（现状「请按下组合键的主键」）。
2. 第一颗主键按下且 **Ctrl/Alt/Shift/Win 已按住**：立即提交（`Shift+A` 现状，keydown 即存）。
3. 第一颗主键按下且 **无修饰键**：记为辅助，不提交；文案提示可再按第二键，或松手保存单键。
4. 第二颗主键按下且第一颗仍按住：立即提交和弦；此时若修饰键也按着，一并写入（`A` 按住 → Shift → `B` 得到 `Shift+A+B`）。
5. 第一颗主键抬起且没有第二键：提交单键 `A`。
6. 三颗主键同时按住：拒绝，继续录制。
7. 失焦：取消，恢复草稿。

有修饰键时 keydown 即提交 → UI 录不出「先按 Shift 再按 A 再按 B」。`Shift+A+B` 只作为按住集合的顺带产物，不作为录入主路径。

目标键仍按下一键即存。其它页面的 `useHotkeyRecorder` 默认行为不变。连发器触发键走 hook 的可选抬起/二次按下路径，不复制一套录制器。

## 7. 忽略触发键

`ignore_trigger_key` 必须抑制绑定内每一个主键的 VK，不能只吞 `hotkey_primary_to_vk` 的第一个。`suppress_key` / `unsuppress_key` 对和弦字符串抑制/解除全部主键；`hotkey_primary_to_vk` 可保留给单主键路径，和弦走 `hotkey_primary_labels` + `primary_key_to_vk`。

## 8. 前端校验

`validateRapidfireHotkeyPrimary` 允第二个非修饰段。`normalizeRapidfireHotkey` 排序两个主键，修饰键顺序仍 Ctrl > Alt > Shift > Super。超过两个主键抛「组合键最多两个主键」。目标键校验不改。

## 9. 测试

Rust：

- `parse_allowing_chord("B+A")` → `A+B`；`parse("A+B")` 仍失败。
- `parse_allowing_chord("Shift+A")` 与旧 `parse` 结果一致（`extra_primary = None`）。
- `parse_allowing_chord("A+B+C")` 失败。
- Hold：A 再 B → `A+B` Down；松 A 或 B → Up；B 再 A 同样 Down。
- Hold：`Shift+A` 回归（先 Shift 再 A、先 A 再 Shift、松 Shift 只停组合不停裸 A）。
- Hold 主例：同时注册 `Shift+A` 与 `A+B`；按下 Shift+A+B → 两个 Down；松 Shift → 只 `Shift+A` Up；松 B → 只 `A+B` Up；松 A → 两个 Up。
- 抑制：`A+B` 解析出两个 VK。

前端：

- 触发键录制：A 按下不提交；A 抬起提交 `A`；A 按住再按 B 提交 `A+B`；Shift 按住再按 A 仍 keydown 提交 `Shift+A`。
- `parseRapidfireSettingsForm` 接受 `b+a` → `A+B`，拒绝 `A+B+C`。

## 10. 文档

- `droid-wiki/features/rapidfire.md` 触发键：单键或双主键，语义同 `Shift+A`。
- `droid-wiki/systems/hotkeys.md`：`extra_primary`、hold 按按下集合重算、`parse` vs `parse_allowing_chord`。

不改 README。
