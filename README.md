# 必应每日壁纸库

一款基于 [Rust](https://www.rust-lang.org/) + [GPUI](https://gpui.rs)（Zed 编辑器同款 GPU 加速 UI 框架）与
[gpui-component](https://longbridge.github.io/gpui-component/zh-CN/) 组件库编写的 Windows 桌面应用，自动抓取开源项目
[niumoo/bing-wallpaper](https://github.com/niumoo/bing-wallpaper) 维护的**全部历史必应每日壁纸**（2021-02-01 至今），
按年 / 月分类展示，并支持一键下载 / 设为桌面壁纸。

## 功能特性

- 📅 **全部历史壁纸**：自动拉取 2021-02-01 至今的每日必应壁纸，左侧导航栏按年 / 月分类，可折叠收起。
- 🔄 **自动增量更新**：每 30 分钟检查一次是否有新的一天壁纸发布，检测到后自动更新列表与本地缓存。
- 🖼️ **首页网格视图**：默认展示最近壁纸的瀑布网格，鼠标滚轮触底自动加载更多（无限滚动），悬停显示"预览图片"按钮，
  点击放大预览并可直接下载 / 设为桌面壁纸。
- ⬇️ **高速下载引擎**：基于 [aria2](https://github.com/aria2/aria2) 的 JSON-RPC 接口，多连接分片、不限速，下载时
  有实时进度条。
- 🖥️ **一键设置桌面壁纸**：通过 Win32 API 直接设置为当前桌面壁纸。
- ⚙️ **自定义下载路径**：左下角设置面板可手动配置壁纸保存目录。
- 🌗 **白天 / 夜间主题**：默认浅色主题，并自动跟随 Windows 系统深色 / 浅色模式实时切换。
- 🪟 **沉浸式标题栏**：自绘客户区标题栏，颜色与内容区背景保持一致，深浅两套主题下都不会有原生标题栏"跳色"的问题。
- 📦 **完全静态链接**：发布的 exe 使用 `+crt-static` 静态链接 CRT，全新安装的 Windows 系统上无需安装任何
  Visual C++ 运行库即可直接运行；同时内嵌 aria2c.exe，无需额外安装/联网下载任何依赖。
- 🔒 **单实例 + 自动管理员提权**：重复启动会自动把已运行的窗口带到前台；启动时自动请求管理员权限。
- 🎨 **多分辨率图标**：任务栏 / 标题栏 / 资源管理器均显示清晰无锯齿的自定义图标（16~256px）。
- 🚫 **无黑色控制台窗口**：无论是软件本体还是内置的 aria2c.exe 下载引擎子进程，均不会弹出黑色控制台窗口。

## 下载使用

直接从 [`dist/必应每日壁纸库.exe`](dist/必应每日壁纸库.exe) 下载即可运行，无需安装任何其他依赖，双击后会自动请求
管理员权限。

## 从源码构建

### 环境要求

- Rust 稳定版工具链，`x86_64-pc-windows-msvc` target（仓库 `.cargo/config.toml` 已将其设为默认 `build.target`）。
- Visual Studio Build Tools（或完整 VS），需包含"使用 C++ 的桌面开发"工作负载及对应的 Windows 10/11 SDK。

### 构建与检查

```powershell
cargo check                                  # 快速类型检查
cargo test                                   # 运行单元测试
cargo clippy --all-targets -- -D warnings    # 严格 Clippy 检查
cargo build --release                        # 发布构建
```

首次构建会克隆 `zed-industries/zed` 与 `longbridge/gpui-component`（GPUI 目前只发布 Git 依赖），耗时可能较长，
后续增量构建会快很多。

发布产物位于 `target/x86_64-pc-windows-msvc/release/bing-wallpaper-lib.exe`。

## 数据来源

壁纸数据来自 [niumoo/bing-wallpaper](https://github.com/niumoo/bing-wallpaper) 维护的
[`bing-wallpaper.md`](https://raw.githubusercontent.com/niumoo/bing-wallpaper/main/bing-wallpaper.md)，
本项目仅负责抓取、解析、展示与下载，不拥有壁纸版权，图片版权归原摄影师 / 版权方所有。

## 下载引擎

下载功能基于 [aria2](https://github.com/aria2/aria2) 官方预编译二进制（[GPL-2.0](https://github.com/aria2/aria2/blob/master/COPYING)
许可证），以内嵌未修改二进制、仅通过其公开 JSON-RPC 接口调用的方式集成。如需商业分发，请自行核实 GPL-2.0
对该集成方式的合规要求。

## 项目文档

更详细的架构设计、关键实现决策与开发注意事项见 [`AGENTS.md`](AGENTS.md)。

## 版权

© 2023-2026 小南瓜
