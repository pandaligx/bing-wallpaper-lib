## 必应每日壁纸库 v0.2.28

本版本把 v0.2.27 的国内数据源优化正式整理进发布流程，并补上 Gitee / GitHub 双 Release 更新通道。

### 数据源

- 历史壁纸归档改用 `zxyongyo/bing-daily-wallpaper` 的 `map.json`。
- 删除旧的 Markdown 快照文件，内置快照改为 `assets/data/zxyongyo-bing-wallpaper.json`。
- 软件运行时优先访问 Gitee 国内镜像，失败后回退 jsDelivr / GitHub。
- 启动时会先展示本地缓存或内置快照，避免无 VPN 时首屏长时间空白。

### 自动同步

- GitHub Actions 每 6 小时同步上游 `map.json`，并同步到 Gitee。
- GitHub `main` 分支 push 后会自动同步代码与 tag 到 Gitee。
- Gitee Release 附件发布改为本机上传优先，避免 GitHub Runner 到 Gitee 上传大文件时变慢或卡住。

### 更新与下载

- 软件检查更新时优先读取 Gitee Releases，失败后回退 GitHub Releases。
- 更新包下载优先使用 Gitee Release 附件，失败后回退 GitHub Release 附件。
- 卡片标题改为 `日期 + Bing 短标题`，例如 `2026-07-04 紫色花海`。
- 下载文件名保留日期、短标题与地点信息。
- 同一张图片即使在不同数据源中日期相差一天，也会按 OHR 图片标识去重。
