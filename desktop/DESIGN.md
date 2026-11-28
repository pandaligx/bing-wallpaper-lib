# 必应每日壁纸库桌面界面

照片占据画廊的主要面积。标题、日期和地点在照片下方独立排版，深浅主题都保持图片原色。工具栏延伸至标题栏，空白区域可拖动，窗口控制键由系统绘制。侧栏用于切换集合和月份，主区域用于浏览，预览与下载均能返回原位置。

| 角色 | 选择 |
| --- | --- |
| 标题、正文、日期 | 平台系统字体，正文与控件默认 12 个逻辑像素，页面标题 16，随系统文字缩放 |
| 间距、圆角 | 行内间距 4，分组间距 8–12，桌面图标 16，按键热区 28 |
| 导航与工具 | 侧栏使用图标与名称，当前页以背景标记；常用操作使用带悬停提示和读屏名称的图标，确认删除保留动词 |
| 色彩 | 使用 NSH background、card、foreground、muted、accent、primary 与 destructive 语义色 |
| 反馈 | 加载展示状态文字，失败保留标题及重试入口，操作状态同时通过文字和颜色表达 |

## 动效与输入

| 操作 | 响应 |
| --- | --- |
| 悬停、按下和键盘聚焦按钮 | 改变背景或描边，按下变暗，键盘保留可见焦点 |
| 查看图标含义 | 指针停留或键盘聚焦时由 NSH Tooltip 显示操作名 |
| 操作失败 | 在当前窗口显示原因，用户可关闭；列表内的单项下载可就地重试 |
| 调整窗口宽度 | 虚拟网格更改列数，保留首个可见项目 |
| 打开和关闭预览 | 背景高斯模糊，照片淡入，底部工具保持水平居中，关闭后焦点返回触发位置 |
| 下载进度 | 每 250 毫秒读取最新字节数，明确显示完成或错误 |

在 Windows、GNOME、Plasma 的明暗模式下分别检查。滚轮直接响应输入。Windows 可见窗口按显示器的 DXGI 同步节奏呈现，隐藏及最小化时停止连续提交。首页中心照片每四秒切换，悬停或开启预览时暂停，切图直接替换照片。侧图可点击切换。图库缩略图悬停时显示标题，预览标题与版权叠在照片底部。成功提示在四秒后消失，错误提示保留十二秒，诊断日志可回看。

图标工具栏参考 NSH Gallery 的 Toolbar 示例，设置分组参考 Tasks 的工具区，窗口和菜单参考 Desktop 的原生示例。交互和密度使用与运行时一致的 0.7.2 版本核对。[Gallery 工具栏](https://github.com/longbridge/nsh/blob/v0.7.2/examples/gallery/demos/ToolbarDemo.tsx) · [Tasks](https://github.com/longbridge/nsh/tree/v0.7.2/examples/tasks) · [Desktop](https://github.com/longbridge/nsh/tree/v0.7.2/examples/desktop) · [NSH 设计指南](https://nsh.dev/docs/design-guides/)

浮动控件的半透明材质参考 [Apple Liquid Glass](https://developer.apple.com/documentation/technologyoverviews/adopting-liquid-glass)。参考 [shadcn/ui Sidebar](https://ui.shadcn.com/docs/components/base/sidebar) 设计可折叠图标栏，按 [Ant Design Upload](https://ant.design/components/upload/) 将暂停、取消和重试放在对应任务行。确认框按内容限定宽度，参照 [Material UI Dialog](https://mui.com/material-ui/react-dialog/)。
