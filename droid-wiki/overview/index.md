# Delta Auto Tools 项目概览

Delta Auto Tools 是一款面向《三角洲行动》玩家的桌面工具，基于 Tauri 2 + React 19 + TypeScript + Vite + Bun + Rust 构建。它提供摩斯密码识别、计时器、计数器、连发器、音频触发器等原生自动化能力，以及攻略网站工作台、主题引擎和多配置系统。

## 项目定位

这款工具解决的核心问题是：在游戏中需要快速、可靠地执行重复操作。摩斯密码识别通过截屏和图像分析自动破译游戏内的摩斯信号并输入答案；计时器和计数器提供透明叠加窗口，在不遮挡游戏画面的前提下显示倒计时和计数；连发器按住触发键时以可配置间隔持续触发目标键；音频触发器支持快捷键、区域图像匹配和屏幕识色三种触发方式播放音频文件。

所有自动化功能受全局总开管控，一键暂停所有热键和自动化行为。

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri 2.11 |
| 前端 | React 19 + TypeScript 5.8 + Vite 7 |
| 后端 | Rust (edition 2021) |
| 包管理 | Bun |
| 样式 | Tailwind CSS v4 (CSS-first) + daisyUI + Radix/shadcn API |
| 图标 | @remixicon/react |
| 截屏 | xcap |
| 输入模拟 | enigo |
| 键盘钩子 | willhook |
| 音频播放 | rodio |
| 自动更新 | tauri-plugin-updater |

## 快速导航

- 新手上手见 [快速开始](./getting-started.md)
- 整体架构见 [系统架构](./architecture.md)
- 项目术语见 [术语表](./glossary.md)
- 各功能模块见 [功能](../features/index.md)
- 底层系统见 [系统](../systems/index.md)
- 发布流程见 [部署与发布](../deployment.md)

## 仓库信息

- GitHub: [IZRINO/delta-auto-tools](https://github.com/IZRINO/delta-auto-tools)
- 当前版本: 0.17.5
- 许可证: 见仓库根目录 LICENSE 文件
- 平台: Windows (x64)
