## 必应每日壁纸库 v0.2.32

本版本补齐 2025 年 4 月和 5 月缺失的中国区必应每日壁纸，并修复旧缓存与不完整远程归档会覆盖补全数据的问题。

### 历史壁纸补全

- 补充 `2025-04-09`、`2025-04-16` 至 `2025-04-30`、`2025-05-01` 至 `2025-05-05`，共 21 张壁纸。
- 补全后 2025 年 4 月完整显示 30 张，5 月完整显示 31 张。
- 日期、中文标题、版权信息与 OHR 图片标识均经过两份独立中国区归档交叉核验，21 个 Bing 图片地址全部验证可访问。
- 新增独立修正表，自动同步上游 `map.json` 时会持续合并，不会在下一次定时同步后再次丢失。

### 缓存与远程刷新

- 启动时不再直接展示非空旧缓存，而是先与内置完整快照合并，旧版本留下的 4 月 14 张、5 月 26 张缓存会立即补齐。
- 后台远程刷新始终保留本地和内置快照已有日期，即使 Gitee、jsDelivr 或 GitHub 返回不完整归档也不会删除补全记录。
- 刷新后的完整列表会重新写入 `%LOCALAPPDATA%\BingWallpaperLib\wallpapers_cache.json`。

### 构建输出

- 新增项目级稳定 Rust 工具链配置，避免 Rustup 向父目录探测时产生路径规范化警告。
- Windows 下仅允许 MSVC 正常的导入库创建信息，不屏蔽真正的编译、Clippy 或链接诊断。

### 验证

- `cargo check`
- `cargo test`：39 项全部通过
- `cargo clippy --all-targets -- -D warnings`
- `cargo build --release`

### 发布与下载

- GitHub Release 与 Gitee Release 均提供 `bing-wallpaper-lib-v0.2.32-x64.exe`。
- 国内更新优先使用 Gitee Release 附件，失败后自动回退 GitHub Release 附件。
