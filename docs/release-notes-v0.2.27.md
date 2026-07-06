## 必应每日壁纸库 v0.2.27

本版本重点优化国内网络环境下的壁纸列表加载可靠性，并完成 Gitee 自动镜像链路。

### 数据源

- 历史壁纸归档改用 `zxyongyo/bing-daily-wallpaper` 的 `map.json`。
- 删除旧的 Markdown 快照文件，内置快照改为 `assets/data/zxyongyo-bing-wallpaper.json`。
- 软件运行时优先访问 Gitee 国内镜像，失败后回退 jsDelivr / GitHub。
- 启动时会先展示本地缓存或内置快照，避免无 VPN 时首屏长时间空白。

### 自动同步

- 新增 GitHub Actions 定时任务，每 6 小时同步上游 `map.json`。
- 新增 GitHub 到 Gitee 的自动镜像 workflow，推送 main 分支和 tag。

### 展示与下载

- 卡片标题改为 `日期 + Bing 短标题`，例如 `2026-07-04 紫色花海`。
- 下载文件名保留日期、短标题与地点信息。
- 同一张图片即使在不同数据源中日期相差一天，也会按 OHR 图片标识去重。
