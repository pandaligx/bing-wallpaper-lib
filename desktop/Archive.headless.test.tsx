import { expect, open, test } from "nsh/test";

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
  await app.scroll("wallpaper-grid", 3600);
  await app.setViewport(620, 640);
  expect(app.texts().filter((t) => t.startsWith("Fixture ")).length).toBeLessThan(250);
  expect(app.texts().filter((t) => t.startsWith("Fixture ")).length).toBeGreaterThan(0);
});
