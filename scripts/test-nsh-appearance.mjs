// Native NSH test input. Captures the gallery, both preview themes and grouped settings.
await app.setViewport(1100, 740);
await app.waitFor("latest-photo", {timeout: 30000});
const settle = () => new Promise(resolve => setTimeout(resolve, 500));
for (const scheme of ["light", "dark"]) {
  await app.setColorScheme(scheme);
  await settle();
  await app.screenshot("target/nsh-validation/" + scheme + "-library.png");
  await app.press("latest-photo");
  await app.waitFor("close-preview");
  await settle();
  await app.screenshot("target/nsh-validation/" + scheme + "-viewer.png");
  await app.drag("preview-zoom", {x:24,y:0});
  await app.waitUntil(() => !app.texts().includes("100%"));
  await app.press("close-preview");
  await app.waitFor("close-preview",{state:"gone"});
  await app.press("nav-settings");
  await app.waitFor("folder-path");
  await settle();
  await app.screenshot("target/nsh-validation/" + scheme + "-preferences.png");
  await app.press("nav-back");
  await app.waitFor("latest-photo");
}
await app.press("nav-settings");
await app.scroll("settings-content", 1800);
await app.press("diagnostics");
await app.waitFor("close-diagnostics");
await app.press("close-diagnostics");
await app.press("nav-home");
await app.press("navigation");
await app.waitFor("sidebar",{state:"gone"});
await app.waitFor("nav-settings");
await app.screenshot("target/nsh-validation/icon-rail.png");
