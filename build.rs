//! 构建脚本：在 Windows 目标上为最终 exe 嵌入应用图标（`ico/icon.ico`）。
//!
//! 注意：这里只嵌入图标资源，**不**嵌入 Windows Manifest —— 管理员权限提权改为在
//! `src/elevate.rs` 中运行时处理（详见 AGENTS.md §5），以避免与 `gpui` 自身通过
//! `windows-manifest` feature 内嵌的清单资源在链接期发生资源冲突。

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        println!("cargo:rerun-if-changed=ico/icon.rc");
        println!("cargo:rerun-if-changed=ico/icon.ico");
        embed_resource::compile("ico/icon.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
