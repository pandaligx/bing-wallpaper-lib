// Run against a disposable graphical CI session. This test changes its desktop background.
await app.waitFor("latest-photo", {timeout: 30000});
await app.setViewport(1100, 740);
await app.setColorScheme("light");
await new Promise(resolve => setTimeout(resolve, 1000));
await app.screenshot("target/nsh-validation/light.png");
await app.press("latest-photo");
await app.waitFor("apply-photo");
await app.setColorScheme("dark");
await new Promise(resolve => setTimeout(resolve, 750));
await app.screenshot("target/nsh-validation/dark-preview.png");
await app.press("apply-photo");
await app.press("close-preview");
await app.waitFor("applied", {timeout: 120000});
