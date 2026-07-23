# 特勤处自动化（开发中）

`special_ops` 保存账号级制作台、子弹兑换和调度状态。每个账号包含 4 台制作台；同一账号的到期制作任务聚合处理。每日兑换时间按 `Asia/Shanghai` 的 `HH:mm` 解释。

## 校准

校准按显示环境全局共享，不随账号或 Profile 复制。显示环境记录显示器、分辨率、DPI、游戏窗口模式及标准步骤坐标。每个目标分为点击点、输入区域、识别区域；点击点执行时使用所选矩形中心。

工作台通过 `special_ops_begin_calibration_selection` 打开全屏透明框选窗口。确认调用 `special_ops_submit_calibration_selection`，取消调用 `special_ops_cancel_calibration_selection`。校准窗口 label 使用 `special-ops-calibration-*`，由 `overlays.json` 授权。

分辨率、DPI 或窗口模式变化时，不按比例缩放旧坐标。用户必须切回已校准环境、新建环境，或明确覆盖现有环境。

## 当前边界

已实现配置持久化、调度、暂停、账号/制作台配置、显示环境与区域框选。WeGame 登录、OCR、键鼠执行、游戏崩溃恢复尚未实现，不能视为自动化完成。
