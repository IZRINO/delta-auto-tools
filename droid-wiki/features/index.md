# 功能

Delta Auto Tools 的原生桌面能力，每个功能由 `src-tauri/src/` 下的 Rust 模块和 `src/components/app/` 下的 React 页面支撑。所有功能共享 `ToolBase` 泛型状态层和 `HotkeyManager` 键盘钩子。

- **[摩斯密码识别](./morse.md)** — 截屏摩斯解码器：截取 3 个区域，二值化，轮廓检测，解码摩斯为数字 0-9，自动输入 3 位结果，可选自动点击链
- **[计时器](./timer.md)** — 多计时器面板，250ms tick 循环，倒计时/正计时，透明置顶叠加窗口，每卡片热键触发
- **[计数器](./counter.md)** — 多计数器面板，运行态独立持久化（`counter_state.json`），透明叠加窗口，每卡片递增/重置热键
- **[连发器](./rapidfire.md)** — 按住触发键自动化：每卡片抖动/间距/不追加策略，每 session 独立 OS worker 线程，透明叠加窗显示 ARMED/FIRING 状态
- **[音频触发器](./audio.md)** — 音频卡片，三种触发模式（快捷键、区域图像 NCC 模板匹配、识色 RGB 距离），rodio 播放 worker，overlay 区域选择
- **[攻略网站](./strategy.md)** — 主窗口内嵌 WebView2 攻略网站工作台：站点 Tab、自定义站点、自动刷新档位、兼容 HTTP 抓取器（JS 重定向跟随）
- **[关于与更新](./about.md)** — 关于面板（版本/许可证/依赖致谢）+ Tauri 官方更新器（检查/下载/安装 + 进度事件）
