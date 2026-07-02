# 必应每日壁纸库

<p align="center">
  <img src="docs/screenshot-home.png" alt="必应每日壁纸库 - 首页网格视图" width="820" />
</p>

<p align="center">
  <a href="https://github.com/pandaligx/bing-wallpaper-lib/releases/latest"><img alt="release" src="https://img.shields.io/github/v/release/pandaligx/bing-wallpaper-lib"></a>
  <a href="https://github.com/pandaligx/bing-wallpaper-lib/releases/latest"><img alt="downloads" src="https://img.shields.io/github/downloads/pandaligx/bing-wallpaper-lib/total"></a>
  <a href="#下载使用"><img alt="platform" src="https://img.shields.io/badge/platform-Windows-0078D6?logo=windows&logoColor=white"></a>
  <a href="Cargo.toml"><img alt="language" src="https://img.shields.io/badge/language-Rust-DEA584?logo=rust&logoColor=white"></a>
  <a href="LICENSE"><img alt="license" src="https://img.shields.io/badge/license-GPL--3.0-blue"></a>
  <img alt="static linked" src="https://img.shields.io/badge/build-static%20linked%20exe-success">
</p>

一款基于 [Rust](https://www.rust-lang.org/) + [GPUI](https://gpui.rs)（Zed 编辑器同款 GPU 加速 UI 框架）与
[gpui-component](https://longbridge.github.io/gpui-component/zh-CN/) 组件库编写的 Windows 桌面应用，自动抓取开源项目
[niumoo/bing-wallpaper](https://github.com/niumoo/bing-wallpaper) 维护的**全部历史必应每日壁纸**（2021-02-01 至今），
按年 / 月分类展示，并支持一键下载 / 设为桌面壁纸、中文描述优先展示、自动检查更新。

## 目录

- [功能特性](#功能特性)
- [界面预览](#界面预览)
- [下载使用](#下载使用)
- [从源码构建](#从源码构建)
- [数据来源](#数据来源)
- [下载引擎](#下载引擎)
- [项目文档](#项目文档)
- [许可证](#许可证)

## 功能特性

| | |
|---|---|
| 📅 **全部历史壁纸** | 自动拉取 2021-02-01 至今的每日必应壁纸，中文数据源优先，中文缺失的历史日期用英文数据源补齐，左侧导航栏按年 / 月分类，可折叠收起。 |
| 🔄 **自动增量更新** | 每 30 分钟检查一次是否有新的一天壁纸发布，检测到后自动更新列表与本地缓存。 |
| 🖼️ **首页网格视图** | 默认展示最近壁纸网格，鼠标滚轮触底自动加载更多，右侧可拖动滚动条 + 右下角“回到顶部”按钮；点击图片可放大预览，悬停按钮可直接设为桌面壁纸并收藏。 |
| ⬇️ **高速下载引擎** | 基于 [aria2](https://github.com/aria2/aria2) 的 JSON-RPC 接口，多连接分片、不限速，下载时有实时进度条，并支持批量下载全部历史、当前月份或收藏壁纸。 |
| 🖥️ **一键设置桌面壁纸** | 通过 Win32 API 直接设置为当前桌面壁纸。 |
| ❤ **我的收藏** | 左侧导航栏新增“我的收藏”，首页和归档列表可用固定尺寸心形图标收藏/取消收藏壁纸。 |
| ⚙️ **自定义下载路径** | 左下角设置浮层可通过 Windows 原生文件夹选择窗口配置壁纸保存目录；下载文件名会包含日期和标题前半部分。 |
| 🆕 **一键检测更新** | 启动时自动检测 GitHub Releases 是否有新版本发布，设置面板也可手动检查；更新包下载优先使用国内可访问的直链镜像，完成后自动重启升级。 |
| ℹ️ **关于面板** | 设置浮层中的“关于软件”展示当前版本号、版权信息、数据源项目与本项目仓库入口。 |
| 🌗 **白天 / 夜间主题** | 默认跟随 Windows 系统深色 / 浅色模式，也可在设置浮层中手动固定为白天或夜间模式。 |
| 🪟 **沉浸式标题栏** | 自绘客户区标题栏，颜色与内容区背景保持一致，深浅两套主题下都不会有原生标题栏"跳色"的问题。 |
| 📦 **完全静态链接** | 发布的 exe 使用 `+crt-static` 静态链接 CRT，全新安装的 Windows 系统上无需安装任何 Visual C++ 运行库即可直接运行；同时内嵌 aria2c.exe，无需额外安装/联网下载任何依赖。 |
| 🔒 **单实例 + 自动管理员提权** | 重复启动会自动把已运行的窗口带到前台；启动时自动请求管理员权限。 |
| 🎨 **多分辨率图标** | 任务栏 / 标题栏 / 资源管理器均显示清晰无锯齿的自定义图标（16~256px）。 |
| 🚫 **无黑色控制台窗口** | 无论是软件本体还是内置的 aria2c.exe 下载引擎子进程，均不会弹出黑色控制台窗口。 |

## 界面预览

<p align="center">
  <img src="docs/screenshot-home.png" alt="首页网格视图" width="820" />
  <br/>
  <sub>首页网格视图：默认展示最近壁纸，支持无限滚动加载更多；左侧按年 / 月归档，左下角设置浮层提供下载目录、主题和更新等选项</sub>
</p>

## 下载使用

推荐从 **[Releases](https://github.com/pandaligx/bing-wallpaper-lib/releases/latest)** 页面下载最新的
`bing-wallpaper-lib-vX.Y.Z-x64.exe`。下载后双击即可运行，无需安装任何其他依赖，启动时会自动请求管理员权限。
软件内置检查更新功能（设置浮层中），可一键检测并升级到最新版本。

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

壁纸数据来自 [niumoo/bing-wallpaper](https://github.com/niumoo/bing-wallpaper) 维护的 Markdown 文件：
优先使用中文标题版本 [`zh-cn/bing-wallpaper.md`](https://github.com/niumoo/bing-wallpaper/blob/main/zh-cn/bing-wallpaper.md)，
并用英文版 [`bing-wallpaper.md`](https://github.com/niumoo/bing-wallpaper/blob/main/bing-wallpaper.md) 补齐中文源缺失的历史日期。
本项目仅负责抓取、解析、展示与下载，不拥有壁纸版权，图片版权归原摄影师 / 版权方所有。

## 下载引擎

下载功能基于 [aria2](https://github.com/aria2/aria2) 官方预编译二进制（[GPL-2.0](https://github.com/aria2/aria2/blob/master/COPYING)
许可证），以内嵌未修改二进制、仅通过其公开 JSON-RPC 接口调用的方式集成。如需商业分发，请自行核实 GPL-2.0
对该集成方式的合规要求。

## 项目文档

更详细的架构设计、关键实现决策与开发注意事项见 [`AGENTS.md`](AGENTS.md)。

## 许可证

本项目遵循仓库根目录下的 [`LICENSE`](LICENSE)（GPL-3.0）开源协议发布。

## 版权

© 2023-2026 小南瓜
