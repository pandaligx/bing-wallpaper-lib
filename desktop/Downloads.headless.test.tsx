import { expect, open, test } from "nsh/test";

test("batches by month, favorite, history and inclusive calendar range and retries a failed date", async () => {
  const now = new Date();
  const currentMonth = now.getFullYear() + "-" + String(now.getMonth() + 1).padStart(2, "0");
  const previous = new Date(Date.UTC(now.getFullYear(), now.getMonth(), 0)).toISOString().slice(0, 10);
  const dates = [currentMonth + "-13", currentMonth + "-12", previous];
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
        "Library.beginDownload": null,
        "Library.finishDownload": null,
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
  expect(app.texts().some((line) => line.includes("test connection closed"))).toBe(true);
  await app.press("dismiss-status");
  await app.waitFor("dismiss-status", { state: "gone" });
  expect(app.accessibility("retry-" + dates[0])?.name).toBe("Retry");
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
  await app.press("range-picker");
  expect(app.accessibility("label:" + currentMonth + "-14")?.disabled).toBe(true);
  await app.press("label:" + dates[1]);
  await app.press("label:" + dates[0]);
  await app.press("batch-range");
  await app.waitUntil(() => calls.length === 9);
  expect(calls.slice(-2).sort()).toEqual(dates.slice(0, 2).sort());
});

test("restores interrupted transfers, cancels a paused task and retries it", async () => {
  const date = "2024-01-26";
  const stopped: string[] = [];
  const app = await open("./App.tsx", {
    width: 1000,
    height: 700,
    modules: {
      "wallpaper-native": {
        "Library.source": "",
        "Library.catalog": JSON.stringify([
          { date, headline: "Hawk owl", description: "", thumbnail: "", preview: "" },
        ]),
        "Library.downloadHistory": JSON.stringify([{ date, state: "Active", apply: false }]),
        "Library.saveDownloadHistory": null,
        "Library.stopDownload": (id: string, cancel: boolean) => {
          if (cancel) stopped.push(id);
        },
        "Library.beginDownload": null,
        "Library.finishDownload": null,
        "Library.download": JSON.stringify({ path: "fixture/completed.jpg", reused: false }),
      },
    },
  });
  await app.waitFor("latest-photo");
  await app.press("nav-downloads");
  await app.waitFor("cancel-" + date);
  expect(app.accessibility("retry-" + date)?.name).toBe("Resume");
  await app.press("cancel-" + date);
  await app.waitFor("cancel-" + date, { state: "gone" });
  expect(stopped).toEqual([date]);
  expect(app.accessibility("retry-" + date)?.name).toBe("Retry");
  await app.press("retry-" + date);
  await app.waitFor("fixture/completed.jpg");
});
