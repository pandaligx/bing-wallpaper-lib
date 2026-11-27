# 必应每日壁纸库

[English](README.md) · **简体中文**

按年月浏览必应每日摄影，收藏喜欢的日期，下载高清原图或设为桌面壁纸。本开发分支使用 NSH 0.7.2 编写界面，由 Rust 管理归档、下载和系统集成。

## 试用桌面版

| 平台 | 开发包 | 运行环境 |
| --- | --- | --- |
| Windows x64 | `BingWallpaperLib-v0.3.0-windows-x64.exe` | 单文件便携版。目标系统为 Windows 10 1809 及以上，已在 Windows 11 运行。 |
| macOS Intel 和 Apple Silicon | `BingWallpaperLib-v0.3.0-macos-universal.dmg` | macOS 14 及以上。将应用复制到「应用程序」。开发包使用临时签名。 |
| Linux x86_64 | `BingWallpaperLib-v0.3.0-linux-x86_64.AppImage` | 宿主需安装 GTK 4.14+ 和 WebKitGTK 6.0，首次启动前赋予 AppImage 执行权限。 |

开发包可从重写分支已完成的 CI 构建下载。较早的 Windows 系统可以继续使用 0.2.37。[开发构建](https://github.com/pandaligx/bing-wallpaper-lib/actions/workflows/nsh-desktop.yml) · [Windows 0.2.37](https://github.com/pandaligx/bing-wallpaper-lib/releases/tag/v0.2.37) · [验收记录](desktop/VALIDATION.md)

## 使用壁纸库

- 首页展示最新一张壁纸，向下滚动可连续浏览历史照片。侧栏展开年份后可选月份。程序先读离线归档，每 30 分钟刷新。大图预览使用 1080p，支持左右方向键和 Esc。
- 收藏以日期保存。下载和设置壁纸可选原图 UHD、4K、2K 或 1K。下载中心提供全部历史、当前月份、收藏和日期范围四种入口，显示速度、进度、已复用文件和失败原因。
- 本地图库扫描当前下载目录，可以预览、设置壁纸、单选或批量删除。确认弹窗会显示选中的数量。
- 设置支持中、英、日、韩、俄、法六种语言，提供跟随系统、浅色、深色外观，以及下载目录、登录启动、托盘和显示器选项。Windows 和 Plasma 可选单屏，GNOME 支持全桌面，其他 Linux 桌面调用可用的 Wallpaper Portal。
- 自动壁纸可设每日固定时间，或使用登录后每 1～1439 分钟执行的周期任务。周期任务支持当天首次成功时用最新图，此后可从历史或收藏中随机选择。

## 旧版迁移与更新

首次从 0.2.37 升级到 NSH Windows 版时，请手动下载新文件。新版沿用 `%LOCALAPPDATA%\BingWallpaperLib` 中的收藏、归档、下载目录和图片，首次修改设置会保存原始配置到 `settings.pre-nsh.json`。有 `.aria2` 控制文件的预分配图片会从本地图库中过滤。

中国大陆使用 GitCode，其他已知地区使用 GitHub。系统地区未知时由用户选择，也可随时在设置中修改。切换界面语言会保留原地区。GitCode 公开仓库及正式签名更新通道仍待发布验证。[GitCode 项目](https://gitcode.com/gcw_qWIjmve7/bing-wallpaper-lib) · [GitHub 发行页](https://github.com/pandaligx/bing-wallpaper-lib/releases)

仓库每四小时同步上游归档，合并已核实的修复。客户端使用 Bing 官方近期记录补强，壁纸版权归摄影师及相应权利人所有。[上游归档](https://github.com/zxyongyo/bing-daily-wallpaper) · [勘误记录](assets/data/zxyongyo-bing-wallpaper-corrections.json)

## 开发与维护

三端构建命令、原生测试、下载测速和发布校验见开发说明。仓库根目录保留旧版实现，便于逐项对照。[桌面开发说明](desktop/README.md) · [维护入口](AGENTS.md) · [旧版文档](https://github.com/pandaligx/bing-wallpaper-lib/blob/v0.2.37/README.zh-CN.md)

© 2023-2026 小南瓜 · [GPL-3.0](LICENSE)
