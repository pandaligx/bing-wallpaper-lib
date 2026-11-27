import { expect, open, test } from "nsh/test";

test("batches the selected month and favorites, displays failures and retries only the chosen date", async () => {
  const dates = ["2025-12-02", "2025-12-01", "2025-11-30"];
  const archive = dates.map((date) => ({
    date,
    headline: date,
    description: "",
    thumbnail: "",
    preview: "",
  }));
  const calls: string[] = [];
  let fail = true;
  const app = await open("./App.tsx", {
    width: 1050,
    height: 720,
    modules: {
      "wallpaper-native": {
        "Library.catalog": JSON.stringify(archive),
        "Library.source": "",
        "Library.favorites": JSON.stringify([dates[2]]),
        "Library.downloadHistory": "[]",
        "Library.saveDownloadHistory": null,
        "Library.download": (date: string) => {
          calls.push(date);
          if (date === dates[0] && fail) {
            fail = false;
            throw Error("test connection closed");
          }
          return JSON.stringify({ reused: date === dates[1], path: "fixture/" + date + ".jpg" });
        },
      },
    },
  });
  await app.waitFor("latest-photo");
  await app.press("nav-downloads");
  await app.press("batch-month");
  await app.waitFor("retry-" + dates[0]);
  expect(calls.slice().sort()).toEqual(dates.slice(0, 2).sort());
  await app.press("retry-" + dates[0]);
  await app.waitFor("fixture/" + dates[0] + ".jpg");
  await app.press("batch-favorites");
  await app.waitFor("fixture/" + dates[2] + ".jpg");
  expect(calls.filter((d) => d === dates[0]).length).toBe(2);
  expect(calls.filter((d) => d === dates[2]).length).toBe(1);
  await app.press("batch-all");
  await app.waitUntil(() => calls.length === 7);
  expect(calls.slice(-3).sort()).toEqual(dates.slice().sort());
});
