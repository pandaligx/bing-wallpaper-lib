import { expect, open, test } from "nsh/test";

test("opens a dated preview, archive month, favorites, downloads, local gallery and settings", async () => {
  const app = await open("./App.tsx", { width: 1100, height: 740 });
  await app.waitFor("latest-photo", { timeout: 30000 });
  await app.press("latest-photo");
  await app.waitFor("apply-photo");
  const preview = () => app.texts().find((line) => /^\d{4}-\d{2}-\d{2} ·/.test(line));
  const first = preview();
  expect(typeof first).toBe("string");
  await app.keyboard.press("right");
  await app.waitUntil(() => !!preview() && preview() !== first);
  await app.keyboard.press("left");
  await app.waitFor(first!);
  await app.keyboard.press("escape");
  await app.waitFor("apply-photo", { state: "gone" });
  await app.press("nav-favorites");
  await app.press("nav-downloads");
  await app.waitFor("batch-all");
  await app.press("nav-local");
  await app.waitFor("scan-local");
  await app.press("nav-settings");
  await app.waitFor("folder-path");
  await app.scroll("sidebar", 1500);
  await app.press("year-2018");
  await app.scroll("sidebar", 500);
  await app.press("month-2018-09");
  expect(app.texts()).toContain("2018-09");
  await app.press("nav-back");
  await app.waitFor("folder-path");
  await app.press("nav-forward");
  expect(app.texts()).toContain("2018-09");
  await app.press("nav-home");
  await app.waitFor("latest-photo", { timeout: 30000 });
});

test("centers the exit confirmation and restores the window on cancel", async () => {
  const app = await open("./App.tsx", {
    platform: "windows",
    width: 900,
    height: 660,
    modules: {
      "wallpaper-native": { "Library.trayAvailable": false, "Library.source": "" },
    },
  });
  await app.waitFor("latest-photo");
  await app.window("main").pressClose();
  await app.waitFor("cancel-exit");
  const box = app.layout("exit-prompt")!;
  expect(Math.abs(box.x + box.width / 2 - 450)).toBeLessThan(3);
  expect(Math.abs(box.y + box.height / 2 - 330)).toBeLessThan(3);
  await app.press("cancel-exit");
  await app.waitFor("cancel-exit", { state: "gone" });
  await app.waitFor("latest-photo");
});
