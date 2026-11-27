import { expect, open, test } from "nsh/test";

test("opens the offline library and switches collections", async () => {
  const app = await open("./App.tsx");
  expect(app.texts()).toContain("必应每日壁纸库");
  await app.waitFor("latest-photo");
  await app.press("latest-photo");
  await app.waitFor("设为桌面壁纸");
  await app.press("关闭");
  await app.press("我的收藏");
  expect(app.texts()).toContain("全部壁纸");
  await app.press("全部壁纸");
  expect(app.texts()).toContain("必应每日壁纸库");
});
