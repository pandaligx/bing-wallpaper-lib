# 必应每日壁纸库

[English](README.md) | **简体中文**

<p align="center">
  <img src="docs/screenshot-home.png" alt="必应每日壁纸库首页" width="820" />
</p>

<p align="center">
  <a href="https://github.com/pandaligx/bing-wallpaper-lib/releases/latest"><img alt="release" src="https://img.shields.io/github/v/release/pandaligx/bing-wallpaper-lib"></a>
  <a href="https://github.com/pandaligx/bing-wallpaper-lib/releases/latest"><img alt="downloads" src="https://img.shields.io/github/downloads/pandaligx/bing-wallpaper-lib/total"></a>
  <img alt="platform" src="https://img.shields.io/badge/platform-Windows-0078D6?logo=windows&logoColor=white">
  <img alt="language" src="https://img.shields.io/badge/language-Rust-DEA584?logo=rust&logoColor=white">
  <img alt="license" src="https://img.shields.io/badge/license-GPL--3.0-blue">
</p>

一款使用 Rust、GPUI 与 gpui-component 编写的 Windows 桌面应用。软件合并 [zxyongyo/bing-daily-wallpaper](https://github.com/zxyongyo/bing-daily-wallpaper) 的完整历史归档和 Bing 近期官方数据，可浏览、收藏、下载并一键设置每日壁纸。

## 功能特性

- 按年 / 月展示全部历史壁纸，内置离线快照，并通过独立纠错表修复上游缺失或损坏的数据。
- 支持跟随系统语言，以及简体中文、English、日本語、한국어、Русский、Français。
- 全局分辨率支持原图 UHD、4K、2K、1K。
- 首页虚拟网格、大图预览、我的收藏和已下载壁纸画廊。
- 内置 aria2 JSON-RPC 多连接高速下载引擎。
- 支持全部历史、当前月份、收藏和指定日期范围批量下载。
- 支持同步全部显示器或单独设置指定显示器。
- 支持每日自动壁纸、开机自启、后台常驻和系统托盘菜单。
- 支持 Windows 计划任务：用户登录时执行，并可设置每隔 1 分钟至 23 小时 59 分钟自动更换；每天首次成功执行可使用 Bing 最新壁纸，后续可随机历史或随机收藏，每次执行完成后自动退出。
- 支持浅色、深色、跟随系统主题，以及 Gitee / GitHub 自动更新。
- 单文件静态链接 exe，无需额外安装 Visual C++ 运行库或 aria2。

点击左侧栏左下角“设置”按钮右侧的国旗按钮即可切换界面语言，选择会自动保存并立即生效。

## 自动壁纸计划

原有“每天固定时间执行一次”的自动壁纸功能继续保留。v0.2.34 在“设置 → 自动壁纸”中新增独立的 Windows 计划任务模式：

- 当前用户登录 Windows 时立即执行，之后按选定的小时 / 分钟间隔重复执行。
- 不会唤醒电脑；关机或睡眠期间错过任务时，会在 Windows 恢复可用后补执行一次。
- 每天首次成功执行可优先使用 Bing 最新壁纸，后续可随机全部历史或随机我的收藏；收藏为空时自动回退到随机历史。
- 每次任务更换壁纸后都会退出。启用计划任务会关闭旧的注册表开机自启，避免两种启动机制发生冲突。

## 下载使用

中国大陆用户可优先从 [Gitee Releases](https://gitee.com/pandaligx/bing-wallpaper-lib/releases) 下载最新版 `bing-wallpaper-lib-vX.Y.Z-x64.exe`；也可从 [GitHub Releases](https://github.com/pandaligx/bing-wallpaper-lib/releases/latest) 下载同名文件。

下载后直接双击运行，无需安装。程序启动时会请求管理员权限。

## 从源码构建

需要稳定版 Rust、`x86_64-pc-windows-msvc` target，以及包含“使用 C++ 的桌面开发”和 Windows 10/11 SDK 的 Visual Studio Build Tools。

```powershell
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

发布产物位于 `target/x86_64-pc-windows-msvc/release/bing-wallpaper-lib.exe`。

## 壁纸数据

历史数据来自上游 `map.json`，每四小时同步到 [`assets/data/zxyongyo-bing-wallpaper.json`](assets/data/zxyongyo-bing-wallpaper.json)。软件运行时优先读取 Gitee 镜像，失败后回退 jsDelivr / GitHub，并通过 Bing `HPImageArchive.aspx` API 补强近期数据。

经过核验的修复记录独立保存在 [`assets/data/zxyongyo-bing-wallpaper-corrections.json`](assets/data/zxyongyo-bing-wallpaper-corrections.json)，避免后续同步再次覆盖修复内容，其中包括已经修正图片地址的 2024-01-26 北鹰鸮壁纸。

壁纸版权归 Bing、摄影师及相应版权方所有。

## 许可证

本项目遵循仓库根目录下的 [GPL-3.0](LICENSE) 开源协议。内置的未修改 aria2 可执行文件遵循 [GPL-2.0](https://github.com/aria2/aria2/blob/master/COPYING)，软件仅通过其公开 JSON-RPC 接口调用。

© 2023-2026 小南瓜
