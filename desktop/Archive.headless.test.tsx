import { expect, open, test } from "nsh/test";

test("acts directly on individual photos in the gallery, favorites and local folder", async () => {
  const photos = ["2026-11-26", "2026-11-25"].map((date) => ({
    date,
    headline: date,
    description: "Photo credit",
    thumbnail: "",
    preview: "",
  }));
  const transfers: [string, boolean][] = [];
  const applied: string[] = [];
  const file = "local.jpg";
  const longName = "A much longer wallpaper title ".repeat(6) + ".jpg";
  const app = await open("./App.tsx", {
    width: 1150,
    height: 840,
    modules: {
      "wallpaper-native": {
        "Library.catalog": JSON.stringify(photos),
        "Library.favorites": "[]",
        "Library.source": "",
        "Library.canApplyWallpaper": true,
        "Library.beginDownload": null,
        "Library.finishDownload": null,
        "Library.downloadHistory": "[]",
        "Library.saveDownloadHistory": null,
        "Library.favorite": JSON.stringify([photos[1].date]),
        "Library.download": (date: string, apply: boolean) => {
          transfers.push([date, apply]);
          return JSON.stringify({ path: date + ".jpg", reused: false });
        },
        "Library.localFiles": JSON.stringify([file, longName]),
        "Library.localImage": () => "",
        "Library.localDetails": () => JSON.stringify({ width: 1920, height: 1080, bytes: 1048576 }),
        "Library.applyLocal": (name: string) => {
          applied.push(name);
        },
      },
    },
  });
  const date = photos[1].date;
  await app.waitFor("card-download-" + date);
  await app.press("card-download-" + date);
  await app.waitFor(date + ".jpg");
  await app.press("card-apply-" + photos[0].date);
  await app.waitUntil(() => transfers.length === 2);
  expect(transfers).toEqual([
    [date, false],
    [photos[0].date, true],
  ]);
  await app.press("card-favorite-" + date);
  await app.press("nav-favorites");
  await app.waitFor("card-favorite-" + date);
  expect(app.accessibility("card-favorite-" + date)?.name).toBe("Cancel favorite");
  await app.press("nav-local");
  await app.waitFor("1920 × 1080 · 1.00 MiB");
  const small = app.layout("local-thumbnail-" + file)!;
  const large = app.layout("local-thumbnail-" + longName)!;
  expect(Math.abs(small.height - large.height)).toBeLessThan(1);
  expect(Math.abs(small.width - large.width)).toBeLessThan(1);
  await app.press("apply-local-" + file);
  await app.waitUntil(() => applied.length === 1);
  expect(applied).toEqual([file]);
  await app.press("delete-local-" + file);
  await app.press("cancel-delete");
  await app.waitFor("delete-local-" + file);
});

test("keeps a ten-thousand-item archive virtualized through scrolling and resizing", async () => {
  const items = Array.from({ length: 10000 }, (_, i) => ({
    date: new Date(Date.UTC(2026, 10, 26 - i)).toISOString().slice(0, 10),
    headline: "Fixture " + i,
    description: "",
    thumbnail: "",
    preview: "",
  }));
  const app = await open("./App.tsx", {
    width: 1120,
    height: 740,
    modules: {
      "wallpaper-native": {
        "Library.catalog": JSON.stringify(items),
        "Library.favorites": "[]",
        "Library.source": "",
      },
    },
  });
  await app.waitFor("latest-photo", { timeout: 30000 });
  expect(app.texts().some((text) => text.startsWith("10000"))).toBe(true);
  expect(app.texts().filter((t) => t.startsWith("Fixture ")).length).toBeLessThan(250);
  await app.waitFor("page-title", { state: "gone" });
  await app.waitFor("gallery-top", { state: "gone" });
  await app.scroll("wallpaper-grid", 3600);
  await app.waitFor("gallery-top");
  await app.press("gallery-top");
  await app.waitFor("gallery-top", { state: "gone" });
  await app.press("navigation");
  await app.waitFor("sidebar", { state: "gone" });
  await app.press("navigation");
  await app.waitFor("sidebar");
  await app.setViewport(620, 640);
  expect(app.texts().filter((t) => t.startsWith("Fixture ")).length).toBeLessThan(250);
  expect(app.texts().filter((t) => t.startsWith("Fixture ")).length).toBeGreaterThan(0);
});
