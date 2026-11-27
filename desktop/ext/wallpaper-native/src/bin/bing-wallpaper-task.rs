//! Run one login/interval or fixed-time wallpaper job without opening a window.
fn main() -> anyhow::Result<()> {
    let arg = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Expected --daily-wallpaper or --scheduled-wallpaper"))?;
    wallpaper_native::run_scheduled(&arg)
}
