# NSH 桌面版验收记录

## 2026-11-27 本地紧凑界面

Windows 11 Build 26300，170 Hz 显示器，系统“在 Windows 中显示动画”关闭。先前的 NSH 宿主在普通滚轮输入时直接跳到下一个位置，内置计数器仍可显示 170 FPS。当前宿主允许启用“平滑滚轮”的列表使用 NSH 原生滚动曲线。设置可随时关闭该选项。

使用相同的壁纸归档、窗口大小和往返滚动轨迹，分别启动修订前后的便携 exe。每轮 12 秒，用 DXGI 在可见窗口内取 96 × 240 像素，按秒统计画面实际变化次数，取中间 10 秒的中位数。正常启动测量时关闭框架帧率监视器。以下精细滚动来自高精度计时器投递的 Windows 滚轮消息。

| 画廊输入 | 修订前实际 FPS | 紧凑版实际 FPS |
| --- | ---: | ---: |
| 普通滚轮，每秒 9 格 | 9 | 169 |
| 普通滚轮，每秒 24 格 | 24 | 168.5 |
| 精细滚动，每秒 150 次 | 150 | 150 |
| 精细滚动，每秒 240 次 | 170 | 170 |

表内两版均使用普通启动。普通滚轮的变化来自两次输入之间的逐帧位移；高频精细输入的位移继续直接跟随输入。[逐秒采样记录](../docs/nsh-compact/windows-scroll.json)

NSH 套件共 62 项，默认运行通过 61 项；另行联网执行的一项从 Bing 下载、解码并复用了同一张壁纸。回归包括一万条归档滚动与缩放、月份切换、键盘预览、下载失败后重试，以及带数量的删除确认。旧版 Rust 套件的 54 项通过。窗口与扩展通过严格 Clippy。深浅色原生窗口都校验了平滑滚轮设置从默认开启到关闭并保存。

隔离配置连续迁移两次，通过了收藏变更、自定义目录、旧配置逐字节备份、原图摘要、跳过 aria2 半成品和确认删除测试。依次关闭 3 张 1080p 预览时，解码缓存分别从 22.35、16.50、16.50 MiB 回落到 12.00 MiB。

本轮 Windows 测试包位于 `target/nsh-local/windows`，48,408,064 字节，SHA-256 为 `EEE17A52D0B135EFC910A5C3D8AF08DB7D10424AA51CAD8B3099B0BC1EAC5C3C`。将该文件单独复制、阻断其网络后，原生窗口仍可读出多个壁纸日期，同目录只有一个 exe。已检查系统 DLL 导入和 Authenticode 状态 `NotSigned`；本地发布脚本遇到未签 exe 会在访问密钥或网站之前退出。

六种语言分别用原生窗口核验了侧栏的图库操作和月份名称。660 点宽的俄语设置页中，长标签可换行，右侧开关仍对齐。[中文首页](../docs/nsh-compact/windows-home-light.png) · [英文首页](../docs/nsh-compact/windows-home-en.png) · [浅色设置](../docs/nsh-compact/windows-settings-light.png) · [深色设置](../docs/nsh-compact/windows-settings-dark.png) · [俄语窄屏设置](../docs/nsh-compact/windows-settings-ru.png) · [深色预览](../docs/nsh-compact/windows-preview-dark.png)

下表含此前原型的跨平台原生结果。当前布局的 macOS 双架构、GNOME 和 KDE 原生复验、Windows 10 1809、实体混合 DPI 双屏及实际睡眠恢复列入下一轮验收。[按平台验收与签名顺序](README.md)

## 旧版功能对照

| 0.2.37 用户操作 | NSH 验证方式 | 本机结果 |
| --- | --- | --- |
| 保留历史日期、每天的标题和勘误 | 旧版和 NSH 共用 `src/archive.rs`。回归覆盖同一天由 Bing 官方补强、同 OHR 跨日保留归档日期、缺失日期补回，以及纠错覆盖已缓存的旧 URL | 通过 |
| 下载名称、分辨率与现有文件 | 两个宿主共用 `src/model.rs` 的命名和地址转换。用例覆盖中英文多段地点、版权分隔符、Windows 非法字符、长标题、UHD、4K 与 1080p 预览 | 通过 |
| 本地缩略图与收藏随机规则 | 两个宿主共用 `src/local_thumbnails.rs` 与 `src/wallpaper_selection.rs`。用例覆盖原图变化时重建、坏缩略图清理、每日首次最新、仅从收藏选取和空收藏回退 | 通过 |
| 打开首页、切图、返回、选旧月份、切换收藏、本地图库、下载中心及设置 | `desktop/App.test.tsx` 使用实际 Rust 扩展和 Windows 原生窗口 | 通过 |
| 连续滚动和窗口缩放 | `desktop/Archive.headless.test.tsx` 注入 10,000 个独立日期并检查可见视图数 | 通过 |
| 卡片直接设为桌面、下载和收藏，宽屏侧栏折叠 | `desktop/Archive.headless.test.tsx` 检查每次操作交给 Rust 的日期和动作，检查本地图片尺寸、快捷删除和取消确认。独立旧配置另行验证了点击卡片取消收藏会写回磁盘 | 通过 |
| 关闭三张不同的大图 | `scripts/test-preview-memory.mjs` 在测试副本记录 NSH 解码缓存的实际字节数。Windows 原生窗口三次关闭前分别占 18.90、13.39、13.39 MiB，关闭后均回到 8.89 MiB | 通过 |
| 在桌面设置里分别选择国内和海外进行版本检查 | 两个独立用户目录通过实际 Rust 模块分别完成 GitCode 与 GitHub 的检查，两个归档缓存均更新到 2026-11-27，共 3,000 条 | 通过 |
| 读取旧收藏、自定义目录、法文和深色设置，跳过半成品，取消及确认删除 | `scripts/test-nsh-migration.mjs` 在独立用户目录副本上启动两次，校验原图摘要、原始配置字节和额外字段 | 通过 |
| 当前月份、收藏、全部历史和自选日期范围批量下载，展示失败、重试指定日期 | `desktop/Downloads.headless.test.tsx` 打开真实日历，检查禁用日期、起止日期和请求清单，再注入一次连接中断 | 通过 |
| 打开大图后使用键盘切图、关闭并重新打开原卡片 | Windows 前台原生窗口验证方向键 → 切至上一天、← 切回、Esc 返回、Enter 再次打开 | 通过 |
| 手动调整每日任务时间 | `registration::tests::manual_edit_skips_only_a_time_that_has_already_passed` 覆盖计划时刻之前、正好同一分钟和计划时刻之后 | 通过 |
| Windows 注册任务、移动便携版、实际触发执行 | `scripts/test-windows-task-registration.ps1` 用唯一测试名称注册每日计划和 1439 分钟的登录周期，读回触发器，迁移 exe 后通过任务计划程序分别触发两个无界面动作，最后删除测试任务 | 通过 |
| macOS 和 Linux 注册定时任务 | 托管桌面由 Rust 创建每日和周期任务，通过 launchd、用户级 systemd 分别触发，记录打包后的无界面入口实际收到的参数，最后移除注册 | 通过 |
| Windows 后台启动、单实例、关闭到托盘再唤回 | `scripts/test-windows-resident.ps1` 在私有测试实例上等待主窗口隐藏，两次重复启动分别唤回同一进程，中间对可见窗口发出正常关闭请求 | 通过 |
| 部分显示器断开时打开选屏菜单 | Windows 11 的 COM 枚举同时返回了可用屏幕和无有效矩形的设备。680 点宽的俄语深色设置页列出当前连接的 2560×1440 屏幕 | 通过 |
| 断网启动单文件 exe | `scripts/test-nsh-portable.ps1 -Offline` 将 exe 单独复制到空目录，以仅匹配该副本的防火墙规则阻止出站联网，再通过 Windows UI Automation 读取多张照片的日期 | Windows 11 Build 26300 通过 |
| 下载完整性、指定分辨率、已有原图、Range 返回 200 和远端长度变化 | 原生单元测试、隔离的真实 Bing 图片下载和再次复用 | 通过 |
| 拒绝坏签名、文件长度或摘要错误，窗口启动失败后还原旧文件 | 清单签名、文件校验和安装状态回归测试 | 通过，正式发行通道另验 |

Windows 便携更新另用临时 Ed25519 密钥和独立用户目录演练两次。真实的 0.3.0 安装器替换到 0.3.1，读取新窗口发出的版本确认。下一次改用签名和摘要均正确、但无法启动的测试文件，安装器恢复原 exe 并重新启动，恢复后的 SHA-256 与更新前相同。复测入口为 `node scripts/rehearse-windows-update.mjs`。

Windows 11 也验证了无法写入安装目录的情况。在独立的发布 exe 副本旁禁止当前用户创建子目录，安装器以退出码 1 返回“安装目录不可写”。目录内仍只有原 exe，摘要与上表相同。测试结束后恢复了该目录原有权限。

Windows 11 菜单也使用独立旧配置保存了一个当前不存在的设备 ID。菜单同时显示断开状态和能够选择的真实显示器。关闭后重新计算旧设置文件的 SHA-256，结果与打开前完全一致。

2026 年 2 月整月的 28 张 4K 请求共返回 25.31 MiB。相同样本按三种模式各运行五次，420 个输出逐张哈希一致。单连接、新引擎多连接和 aria2 的中位总耗时分别为 4.83、2.80、1.49 秒，峰值工作集的中位数分别为 21.0、20.8、17.1 MiB。此次整月样本中新引擎的吞吐约为 aria2 的 53%。下表的大文件数字属于另一组独立测试。可用 `scripts/bench-month.ps1` 复测。

## Windows 与下载

2026-11-27 在 Windows 11 专业版 x64（Build 26300）上，把发布 exe 单独放入空目录运行。通过系统辅助功能接口读到了壁纸库、收藏按钮和 3,000 条记录。随构建嵌入的离线快照含 2,999 个日期；本机联网刷新后新增 2026-11-27。Windows 10 1809 干净虚拟机保留待验。

`dumpbin /dependents` 在首个单文件原型中只找到系统 DLL。资源信息页显示版本 0.3.0.0 和 © 2023-2026 小南瓜。Windows 原生 NSH 测试已覆盖打开预览、关闭预览和切换收藏，并取得浅色首页、深色预览截图。

在临时用户下载目录和独立 SQLite 数据库中，Rust 下载服务实际下载了 2026-11-26 的必应图片，解码为 1920×1080。重复操作返回同一文件且修改时间相同。旧版下载目录的回归用例另行验证了保留原图与 `.aria2` 控制文件。

| 样本 | 单流中位数 | 新引擎中位数 | aria2 中位数 | 新引擎吞吐 / aria2 |
| --- | ---: | ---: | ---: | ---: |
| 本地每连接限速，8 MiB | 4.054 s | 0.525 s | 0.526 s | 100.2% |
| 实际 Bing，2026-11-26 的 4K 壁纸 | 0.881 s | 0.800 s | 1.004 s | 125.5% |
| 实际 GitHub，0.2.37 Windows 附件 | 2.426 s | 2.451 s | 2.501 s | 102.0% |
| 实际 GitCode，同一个 0.2.37 Windows 附件 | 0.668 s | 0.702 s | 0.732 s | 104.2% |

每项用同一台机器、同一 URL 连跑五轮，计入进程启动和退出。受控服务器实测单流同时建立 1 条传输连接，新引擎和 aria2 均为 8 条。每个样本的 15 个输出哈希完全一致。Bing 第一次对比中 aria2 返回退出码 1，单次诊断下载和随后完整五轮均成功。以上统计取完整五轮。GitCode 的五轮结果均与 GitHub 原附件的 SHA-256 一致。

增加进程工作集采样后另跑了五轮。每个样本使用同一 URL，比较同一轮中的三个模式。表内依次给出单连接、新引擎多连接、aria2 的中位数。四个样本的 60 次下载均通过内容哈希核对。

| 样本 | 耗时（秒） | 工作集峰值（MiB） | 新引擎吞吐 / aria2 |
| --- | --- | --- | ---: |
| 每连接限速，8 MiB | 4.023 / 0.526 / 0.518 | 13.3 / 14.3 / 19.9 | 98.5% |
| 实际 Bing，1,807,120 字节 | 0.403 / 0.400 / 0.347 | 18.3 / 18.4 / 14.7 | 86.8% |
| GitHub，23,737,856 字节 | 2.121 / 1.648 / 2.321 | 19.8 / 24.5 / 16.2 | 140.8% |
| GitCode，同一个 exe | 0.682 / 0.656 / 0.905 | 20.2 / 19.5 / 15.9 | 138.0% |

90% 验收线用于同机同源的大文件。小壁纸的结果受当时网络、连接和服务端缓存影响，整月下载仍有性能差距。

## 平台和发布边界

以下文件取自同一提交 `d40afabd4488512d1bbd3d3667e48d99925682d9`。Windows、macOS、Linux 三个原生任务，以及在 Intel 和 Apple Silicon 上分别挂载同一 DMG、打开应用窗口的两项检查均通过。Windows 原生核心通过 57 项默认测试，macOS 和 Linux 各通过 55 项。每端另有原生窗口、迁移、严格 Clippy 和 NSH 交互测试。从构建服务取回产物后逐个核对了归档摘要，再对解包后的最终文件计算 SHA-256。[对应构建](https://github.com/pandaligx/bing-wallpaper-lib/actions/runs/45978901111)

| 安装包 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `BingWallpaperLib-v0.3.0-windows-x64.exe` | 45,545,984 | `2dc013158d7a6874359480ef52bf4fa3c2ac3e5d8d0b101a0b4241bbbd867493` |
| `BingWallpaperLib-v0.3.0-macos-universal.dmg` | 57,494,829 | `322ca14c5436a2adb6cc11ba88d839566375c393b07bb35f3d65877cfa032380` |
| `BingWallpaperLib-v0.3.0-linux-x86_64.AppImage` | 42,383,864 | `8060c5319aeff98515fa66e015d21729885253d45d1c733006c423afe0c8c9db` |

上一轮在 Linux 英文桌面的截图中发现中文元数据显示成方框。此 AppImage 带有 Noto Sans CJK 和字体许可。Ubuntu 从已生成的 AppImage 提取字体配置，经 Fontconfig 定位到包内字体，再由 Pango 实际排版中日韩文字，缺失字形计数为零。此版 Windows exe 在 Build 26300 上重新通过断网单文件测试，导入表只有系统 DLL。

| 原生窗口 | 首页（浅色） | 大图（深色） |
| --- | --- | --- |
| Windows | [首页](../docs/nsh-v0.3.0-rc.1/windows-light.png) · [简体中文首页](../docs/nsh-v0.3.0-rc.1/windows-zh.png) | [预览](../docs/nsh-v0.3.0-rc.1/windows-dark-preview.png) |
| macOS | [首页](../docs/nsh-v0.3.0-rc.1/macos-light.png) | [预览](../docs/nsh-v0.3.0-rc.1/macos-dark-preview.png) |
| GNOME / KDE Plasma | [首页](../docs/nsh-v0.3.0-rc.1/linux-light.png) | [预览](../docs/nsh-v0.3.0-rc.1/linux-dark-preview.png) |

两个 Linux 桌面各自生成了原始截图，同一界面的 PNG 摘要相同。macOS 托管屏幕的截图右边缘受显示区域限制。Windows 中文首页使用同一提交，在本地原生窗口连接 GitCode 刷新后截图。

另外从 GitHub 公共发行页下载同一个 AppImage，在 Ubuntu 24.04 的 GNOME Shell / Mutter 嵌套 Wayland 会话里执行无界面每日任务。实际下载并解码了 1920×1080 壁纸，浅色、深色背景地址均指向该文件，当日成功标记写入。两次抓取合成器画面，核对活动输出区域有 42.6% 的采样像素发生明显变化。[更换前](../docs/nsh-v0.3.0-rc.1/gnome-wallpaper-before.png) · [更换后](../docs/nsh-v0.3.0-rc.1/gnome-wallpaper-after.png) · [GNOME 实际桌面记录](https://github.com/pandaligx/bing-wallpaper-lib/actions/runs/45991630333)

Windows 11 验证了已签测试清单驱动的便携升级和回滚。Ubuntu 的独立 XDG 用户目录中，真实 AppImage 完成了一次签名升级并打开新版窗口。再用签名正确但无法启动的包触发失败，原路径上的文件恢复到旧版 SHA-256。GNOME 同时读回浅色与深色桌面的图片地址，KDE Plasma 从当前活动的桌面配置读回实际设置的图片。[原生验收与回滚日志](https://github.com/pandaligx/bing-wallpaper-lib/actions/runs/45974101767)

macOS 15 在「应用程序」下的独立测试目录中，还演练了系统签名拒绝。用有效的 Ed25519 测试清单提交这次已发布的真实开发 DMG，安装器在替换前因系统签名检查拒绝该包。原应用的资源签名和可执行文件摘要保持一致，原窗口再次成功打开。[macOS 更新拒绝演练](https://github.com/pandaligx/bing-wallpaper-lib/actions/runs/45987806774)

实体混合 DPI 双屏、实际睡眠唤醒、干净 Windows 10 1809 及使用 Developer ID 公证的 macOS 安装与升级仍待验证。Windows 11 实测了当前显示器和断开设备的区分。定时逻辑另有跨休眠时长、遗漏多个周期后仅补一次的时钟用例。

国内公开仓库 `gcw_qWIjmve7/bing-wallpaper-lib` 已建立并启用自动拉取。两端 main 的归档都有 3,000 个日期，截止 2026-11-27，字节数均为 1,785,998，SHA-256 均为 `919eb7e779019aca2f11030c994e2fd7e37e3b490eb521685756e33a121b6fec`。从 GitCode 的匿名 Release API 返回的附件中，按完整文件名筛出旧版 exe，再经 file-cdn.gitcode.com 读回，23,737,856 字节与 GitHub 原件一致。[国内旧版下载](https://gitcode.com/gcw_qWIjmve7/bing-wallpaper-lib/releases/v0.2.37)

RC1 历史演练中，两站的三个安装包均已匿名下载，六份文件与上表逐项相符。测试前驱客户端从 GitCode 和 GitHub 各下载、验签并暂存了 45,545,984 字节的 Windows exe。维护者要求撤下测试发行页，两站匿名发行接口随后均返回 404，稳定版本均为 0.2.37。上述原生记录与历史截图对应旧 RC1，新界面的本地验收另记。
