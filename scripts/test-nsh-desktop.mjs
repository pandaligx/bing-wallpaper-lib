// Run against a disposable graphical CI session. This test changes its desktop background.
await app.waitFor("latest-photo", {timeout: 30000});
await app.setViewport(1100, 740);
await app.setColorScheme("light");
await app.screenshot("target/nsh-validation/light.png");
await app.press("latest-photo");
await app.waitFor("设为桌面壁纸");
await app.setColorScheme("dark");
await app.screenshot("target/nsh-validation/dark-preview.png");
await app.press("设为桌面壁纸");
await app.press("关闭");
await app.waitFor("已设置桌面壁纸", {timeout: 120000});
