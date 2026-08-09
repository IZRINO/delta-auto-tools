# 特勤处配置纳入 Profile 备份设计

**状态：已确认，待实现。**

## 目标

将特勤处全部持久化配置纳入现有顶栏 Profile 的保存、切换、导入和导出。

- 保存一个 Profile 时，快照包含当前 `SpecialOpsSettings`。
- 新建默认 Profile 时，包含默认特勤处配置。
- 导入 Profile 后，特勤处配置暂不生效；用户切换至该 Profile 时才应用。
- 切换至含特勤处快照的 Profile 时，替换当前特勤处配置并保持暂停。

不改变参考图片文件，也不复制图片二进制。校准目标只保存已有本地图片路径。

## 方案选择

选择在 `ToolSettingsSnapshot` 增加可选 `specialOps` 字段。

未选择“将特勤处另存单独备份”或“只备份业务配置、不备份运行状态”。前者破坏统一 Profile 心智模型，后者会遗漏账号制作完成时间、当天子弹状态和人工状态，恢复后调度不可信。

## 快照内容

`specialOps` 完整保存规范化后的 `SpecialOpsSettings`，包括：

- 账号、顺序、账号状态、四制作台启用状态、时长、制作完成时间、人工状态和失败记录。
- 默认业务配置、账号独立配置、制作物品选择点击点、子弹目标、子弹当天成功/失败状态。
- 全局校准坐标、模板识别区域、模板图片路径、可执行文件路径、延迟、热键、全局开关。
- 利润筛选开关、规则、精确名称、审计、每日截止状态。

不保存以下进程内状态：运行中的轮次、登录运行态、操作提示窗口、scheduler 是否 armed、利润查询 generation、正在查询的 HTTP/WebView 任务、取消令牌、当前合格规则集合、active round targets。

## 切换与恢复

1. 在 Profile 保存或导出时，读取 `SpecialOpsState.settings` 的当前快照。
2. 在创建默认 Profile 时，写入 `SpecialOpsSettings::default()`。
3. 应用 Profile 前检查特勤处 `LoginRuntime`。
4. 存在任何特勤处试运行或自动轮次时，拒绝切换，返回明确错误；不发出停止、紧急停止或输入释放指令。
5. 无活动特勤处运行时，先 normalize 目标 `specialOps`，再在现有 Profile apply 串行锁和 `SettingsCoordinator` 临界区内写入 `special_ops_settings.json`、替换内存设置。
6. 应用后强制 `paused=true`，scheduler 不 armed，利润 runtime 失效并取消未完成查询；emit 最新特勤处 bootstrap。
7. 切换完成后保持暂停，用户手动点击“继续”后才允许计时调度和键鼠操作。

该顺序禁止“旧轮次继续执行但读取新 Profile 配置”的竞态。

## 兼容与失败处理

- 旧 Profile JSON 缺少 `specialOps`：导入成功；应用该 Profile 时保持当前特勤处配置不变，不触碰特勤处 scheduler。
- 导入 JSON 的 `specialOps` 无法反序列化：导入失败，不加入 Profile 列表。
- 目标 `specialOps` 无法 normalize：切换在开始既有工具写盘前失败；特勤处设置和 active Profile 不变。
- `special_ops_settings.json` 写入失败：内存不替换，active Profile 不更新，错误返回前端。
- 参考图片路径失效：Profile 仍可应用；对应校准项在下次运行前按既有校准验证规则报失效，禁止进入相关执行步骤。
- Profile 导入只加入列表，不自动应用，故不触发特勤处暂停或文件写入。

## Profile 同步

当前激活 Profile 存在时，特勤处每次成功保存配置后，使用 `ActiveProfileSnapshotPatch::SpecialOps` 更新该 Profile 快照与 `updatedAt`。这与既有五类工具自动同步行为一致。

若没有激活 Profile，特勤处仍写入 `special_ops_settings.json`；首次 `profile_get_bootstrap` 创建默认 Profile 时捕获当前特勤处配置。

## 测试

- 快照 JSON 的 `specialOps` 使用 camelCase；旧快照缺字段可反序列化。
- 默认 Profile、保存当前 Profile、导出和导入均保留特勤处完整字段与图片路径。
- 无活动运行时应用 Profile：写盘、替换设置、强制暂停、失效利润 runtime、emit bootstrap。
- 有试运行或自动轮次时：拒绝应用，旧 Profile 和特勤处设置不变。
- invalid `specialOps`：导入失败；normalize 失败：不启动 Profile apply；特勤处写盘失败：不替换特勤处内存状态、不更新 active Profile。
- 应用旧 Profile：特勤处设置与 scheduler 状态保持原值。
- Profile 自动同步特勤处更新后，重新导出得到新快照。

## 文档同步

实现时更新 `droid-wiki/systems/profile-system.md` 与 `droid-wiki/features/special-ops.md`：Profile 快照由五类工具扩展为六类，并注明参考图仅保存路径、切换强制暂停和运行中拒绝切换规则。
