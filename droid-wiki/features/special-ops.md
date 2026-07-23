# 特勤处自动化（开发中）

`special_ops` 保存账号级制作台、子弹兑换和调度状态。每个账号包含 4 台制作台；同一账号的到期制作任务聚合处理。每日兑换时间按 `Asia/Shanghai` 的 `HH:mm` 解释。

## 区域校准

校准结果全局共享，不随账号或 Profile 复制。UI 不要求用户填写环境名称、显示器、分辨率、DPI 或窗口模式，只维护一套当前校准结果。旧版本存在多套环境时，加载后保留当时选中的一套。

框选行为沿用摩斯区域框选交互：先隐藏主窗口，再打开透明 overlay；按住左键拖拽，松开后立即提交并关闭。区域过小时要求重新框选，Esc 或右键取消。提交、取消或窗口异常关闭后恢复主窗口。点击动作执行时使用所选矩形中心。

窗口不使用 `fullscreen(true)` 或 `maximize()`。当前 WebView2 多屏环境中透明 fullscreen 可能退化为 `16x16` 命中区域。后端汇总全部显示器的物理边界，窗口先以可调整状态创建，再通过 `PhysicalPosition` 与 `PhysicalSize` 显式覆盖完整虚拟桌面，最后锁定尺寸；前端使用 Pointer Events 与 pointer capture 接收拖拽，并将局部坐标换算为虚拟桌面绝对坐标。

工作台通过 `special_ops_begin_calibration_selection` 打开框选窗口。提交调用 `special_ops_submit_calibration_selection`，取消调用 `special_ops_cancel_calibration_selection`。窗口 label 使用 `special-ops-calibration-*`，由 `overlays.json` 授权。

## 当前边界

已实现配置持久化、调度、暂停、账号/制作台配置和区域框选。WeGame 登录、OCR、键鼠执行、游戏崩溃恢复尚未实现，不能视为自动化完成。
