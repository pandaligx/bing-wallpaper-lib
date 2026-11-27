import { expect, open, test } from "nsh/test";

test("opens a dated preview, archive month, favorites, downloads, local gallery and settings", async () => {
  const app = await open("./App.tsx",{width:1100,height:740});
  await app.waitFor("latest-photo");
  await app.press("latest-photo");
  await app.waitFor("apply-photo");
  await app.press("next-photo");
  await app.press("close-preview");
  await app.press("nav-favorites");
  await app.press("nav-downloads");
  await app.waitFor("batch-all");
  await app.press("nav-local");
  await app.waitFor("scan-local");
  await app.press("nav-settings");
  await app.waitFor("folder-path");
  await app.scroll("sidebar",1500);
  await app.press("year-2018");
  await app.scroll("sidebar",500);
  await app.press("month-2018-09");
  expect(app.texts()).toContain("2018-09");
  await app.scroll("sidebar",-2000);
  await app.press("nav-home");
  await app.waitFor("latest-photo");
});
