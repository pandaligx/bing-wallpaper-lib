//! Run one login/interval or fixed-time wallpaper job without opening a window.
fn main() -> anyhow::Result<()> {
    let arg = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Expected --daily-wallpaper or --scheduled-wallpaper"))?;
    if arg == "--install-wallpaper-update" {
        return wallpaper_native::install_update(
            &std::env::args()
                .nth(2)
                .ok_or_else(|| anyhow::anyhow!("Expected a version"))?,
        );
    }
    wallpaper_native::run_scheduled(&arg)
}
