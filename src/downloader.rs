//! 下载引擎：基于 aria2 的 JSON-RPC 接口。
//!
//! 启动时会在应用私有目录下释放内嵌的 `aria2c.exe`，以 `--enable-rpc` 模式
//! 作为子进程常驻运行，本进程仅通过 HTTP JSON-RPC（`aria2.addUri` 等）与其
//! 通信，从而做到"完全静态链接的单一可执行文件 + 通过标准 aria2 接口下载"。
//!
//! 为了达到最高下载速度，默认对每个下载任务使用：
//! - `split = 16`：单文件最多切分为 16 段并发下载；
//! - `max-connection-per-server = 16`：每个下载对同一服务器建立的最大连接数；
//! - `min-split-size = 1M`：允许对 1MB 以上的文件进行分段。

use anyhow::{bail, Context, Result};
use futures::AsyncReadExt;
use http_client::HttpClient;
use serde_json::{json, Value};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

/// Windows `CREATE_NO_WINDOW` 进程创建标志：启动子进程时不为其分配/显示
/// 控制台窗口。由于 `aria2c.exe` 本身是一个控制台程序，即使把它的
/// stdin/stdout/stderr 都重定向到 `Stdio::null()`，Windows 仍然会在没有
/// 这个标志的情况下为其短暂弹出一个黑色控制台窗口；加上该标志后系统根本
/// 不会为该进程创建控制台，从而彻底消除这个黑框闪烁问题。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// aria2 RPC 监听端口。
const RPC_PORT: u16 = 16800;
/// aria2 RPC 鉴权密钥（仅本机进程间通信使用，不对外暴露）。
const RPC_SECRET: &str = "bing-wallpaper-lib-local";

pub struct Aria2Manager {
    child: Child,
    http: Arc<dyn HttpClient>,
    rpc_url: String,
}

impl Aria2Manager {
    /// 启动 aria2c 常驻进程（RPC 模式）。
    pub async fn start(http: Arc<dyn HttpClient>) -> Result<Self> {
        let exe_path = crate::paths::ensure_aria2c()?;
        let download_dir = crate::paths::wallpapers_dir()?;

        let mut command = Command::new(&exe_path);
        command
            .arg("--enable-rpc")
            .arg(format!("--rpc-listen-port={RPC_PORT}"))
            .arg(format!("--rpc-secret={RPC_SECRET}"))
            .arg("--rpc-listen-all=false")
            .arg("--rpc-allow-origin-all=false")
            .arg(format!("--dir={}", download_dir.display()))
            .arg("--continue=true")
            .arg("--auto-file-renaming=false")
            .arg("--allow-overwrite=true")
            .arg("--max-connection-per-server=16")
            .arg("--split=16")
            .arg("--min-split-size=1M")
            .arg("--max-concurrent-downloads=8")
            .arg("--max-overall-download-limit=0")
            .arg("--disable-ipv6=true")
            .arg("--quiet=true")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let child = command.spawn().context("启动内置 aria2c.exe 失败")?;

        let rpc_url = format!("http://127.0.0.1:{RPC_PORT}/jsonrpc");

        let manager = Self {
            child,
            http,
            rpc_url,
        };
        manager.wait_until_ready().await?;
        Ok(manager)
    }

    /// 等待 aria2 RPC 服务就绪（进程刚启动时需要一点时间打开监听端口）。
    async fn wait_until_ready(&self) -> Result<()> {
        for _ in 0..30 {
            if self
                .rpc_call("aria2.getVersion", json!([self.secret_token()]))
                .await
                .is_ok()
            {
                return Ok(());
            }
            smol::Timer::after(Duration::from_millis(200)).await;
        }
        bail!("等待 aria2 RPC 服务启动超时")
    }

    fn secret_token(&self) -> String {
        format!("token:{RPC_SECRET}")
    }

    async fn rpc_call(&self, method: &str, params: Value) -> Result<Value> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": "bing-wallpaper-lib",
            "method": method,
            "params": params,
        });
        let body_bytes = serde_json::to_vec(&body).context("序列化 aria2 RPC 请求失败")?;

        let mut response = self
            .http
            .post_json(&self.rpc_url, body_bytes.into())
            .await
            .context("调用 aria2 RPC 失败")?;

        let mut buf = Vec::new();
        response
            .body_mut()
            .read_to_end(&mut buf)
            .await
            .context("读取 aria2 RPC 响应失败")?;
        let resp: Value = serde_json::from_slice(&buf).context("解析 aria2 RPC 响应失败")?;

        if let Some(err) = resp.get("error") {
            bail!("aria2 RPC 返回错误: {err}");
        }
        resp.get("result")
            .cloned()
            .context("aria2 RPC 响应缺少 result 字段")
    }

    /// 添加一个下载任务，返回 aria2 分配的任务 GID。
    pub async fn add_uri(&self, url: &str, out_filename: &str) -> Result<String> {
        let params = json!([
            self.secret_token(),
            [url],
            {
                "out": out_filename,
            }
        ]);
        let result = self.rpc_call("aria2.addUri", params).await?;
        result
            .as_str()
            .map(str::to_string)
            .context("aria2.addUri 未返回任务 GID")
    }

    /// 查询任务状态（`status`/`completedLength`/`totalLength` 等字段）。
    pub async fn tell_status(&self, gid: &str) -> Result<Value> {
        let params = json!([self.secret_token(), gid]);
        self.rpc_call("aria2.tellStatus", params).await
    }

    /// 在 aria2 已在运行时，实时修改其全局下载目录（用户在设置面板中修改下载路径后调用）。
    /// 对已经在进行中的任务不会生效，仅影响之后新提交的下载任务。
    pub async fn change_download_dir(&self, dir: &std::path::Path) -> Result<()> {
        let params = json!([
            self.secret_token(),
            { "dir": dir.display().to_string() }
        ]);
        self.rpc_call("aria2.changeGlobalOption", params).await?;
        Ok(())
    }

    /// 优雅地关闭 aria2 常驻进程：通过 RPC 通知其自行退出。
    ///
    /// 只需 `&self`（不消耗所有权），因此可以直接通过共享的 `Rc<Aria2Manager>` 调用，
    /// 无需在应用退出时尝试回收唯一所有权。aria2 收到 `aria2.shutdown` 后会自行退出，
    /// 无需我们再显式 `kill` 子进程；若 RPC 调用失败，[`Drop`] 仍会在进程退出时强制终止子进程典兼底。
    pub async fn shutdown(&self) {
        let _ = self
            .rpc_call("aria2.shutdown", json!([self.secret_token()]))
            .await;
    }
}

impl Drop for Aria2Manager {
    fn drop(&mut self) {
        // 保险丝机制：即使未显式调用 shutdown，也要在 Aria2Manager 被
        // 销毁时终止内置的 aria2c.exe 进程，避免它变成孤儿进程残留在后台。
        let _ = self.child.kill();
    }
}
