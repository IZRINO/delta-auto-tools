# 特勤处限时商品颜色选择与账号级交易行配置设计

## 目标

本轮设计处理两组需求：

1. 限时商品颜色 1、颜色 2 改为 Recognition 识别触发同款原生颜色控件，支持系统吸管取色和颜色面板选择。
2. 交易行购买业务配置保留在默认账号配置与账号独立配置中。

## 限时商品颜色配置

### 数据边界

`LimitedSupplySettings` 继续保存全局共享内容：

- 限时商品总开关。
- 研发部门等待时间。
- 研发页面就绪超时。
- 颜色 1、颜色 2 的 RGB 与容差。
- 每日 12:00、20:00 固定检查规则。

删除 `colorSampleRegions`。旧配置中的该字段由 serde 作为未知字段忽略，下一次保存后自然消失，不需要迁移成其他字段。

9 个 `limited.color.1` 至 `limited.color.9` 仍只保存运行时识别区域。它们不再承担颜色吸取来源，也不与颜色 1、颜色 2 建立绑定。

### 颜色控件

颜色 1、颜色 2 各显示一行：

- 原生颜色色块 `input[type=color]`。点击后打开与 Recognition 识别触发一致的系统颜色面板，可使用吸管或手动选色。
- `#RRGGBB` 文本输入框。失焦或按 Enter 时解析并保存 RGB；格式无效时保留原值并显示错误。
- 容差整数输入框，范围继续为 0–255。

删除以下旧 UI：

- R、G、B 三个独立数字输入框。
- “取色区域”下拉框。
- 独立“吸取”按钮与“正在取色”状态。
- “限时商品识色区域校准”说明折叠框。

9 个识色区域继续在下方统一“点击区域校准”表格中框选和测试。删除说明折叠框不删除任何实际校准能力。

### 后端清理

删除已无调用方的区域平均色吸取链路：

- `special_ops_sample_limited_supply_color` Tauri command。
- `LimitedSupplyColorSampleResult` 前端类型。
- command 注册与 async command contract 断言。
- 特勤处页面的采样状态、反馈与 invoke 调用。

Recognition 的共享颜色匹配算法继续保留。正式限时商品检查仍对 9 个区域执行 `AnyPixel` 匹配：任一区域存在像素命中颜色 1 或颜色 2，即记录高价值提醒。颜色选择方式变化不修改运行时识别规则。

## 交易行账号业务配置

`BusinessConfig.market` 保存：

- `enabled`：是否执行交易行购买。
- `purchaseCount`：每日购买次数。
- `itemNote`：显示备注。
- `maxPrice`：允许购买的最高价格。
- `productPoint`：商品入口点击点。

默认账号配置保存默认值。账号关闭独立设置时继承默认值；开启独立设置时复制当时默认值，之后使用账号独立配置。

交易行入口识别区域、价格 OCR 区域、返回点击点、达标购买点击点仍属于显示环境校准，不按账号复制。交易行固定 02:00–04:00 时间窗及入口等待参数保持不变。

## 数据流

### 颜色编辑

1. 用户通过系统颜色面板、吸管或 `#RRGGBB` 文本选择颜色。
2. 前端把 hex 转为 `[R, G, B]`。
3. 现有特勤处 settings autosave 保存 `limitedSupply.colors`。
4. 正式检查与试运行读取保存后的两种颜色及容差。

颜色编辑不聚焦游戏窗口、不截图、不占用键鼠、不显示操作倒计时。

### 限时商品检查

1. runtime 进入研发部门并等待页面就绪。
2. 截取 9 个 `limited.color.N` 区域。
3. 每个区域按 `AnyPixel` 与颜色 1、颜色 2 比较。
4. 任一区域命中则记录高价值；全部未命中则记录无高价值。

## 错误处理

- hex 文本格式无效：不覆盖已保存 RGB，页面显示“颜色必须使用 #RRGGBB 格式”。
- 原生颜色面板取消：配置不变。
- 识色区域未校准、截图失败或游戏窗口不可用：继续使用现有试运行/账号失败处理，不伪造识别结果。
- 识色未命中：属于有效结果，不当作 command 失败。

## 验证

### 前端

- hex 与 RGB 双向转换覆盖大小写和边界值。
- 无效 hex 不修改保存值。
- 页面存在两个 `input[type=color]` 与两个 hex 输入框。
- 页面不再包含“取色区域”“吸取”“限时商品识色区域校准”。
- 颜色改变后 `limitedSupply.colors` 正确更新。
- 交易行默认与账号独立业务配置回归通过。

### Rust

- `LimitedSupplySettings` 不再序列化 `colorSampleRegions`。
- 含旧 `colorSampleRegions` 的 JSON 仍可加载并在重新序列化后移除旧字段。
- `special_ops_sample_limited_supply_color` 不再注册。
- 9 区 `AnyPixel` 匹配与双采样测试保持通过。

### 完整门禁

- `bun run build`
- `bun run test`
- `bun run check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## 不做的事

- 不把限时商品配置移入账号业务配置。
- 不删除 9 个正式识色区域。
- 不修改 12:00、20:00 固定检查时间。
- 不修改正式识色的 `AnyPixel` 规则。
- 不保存运行期截图。
