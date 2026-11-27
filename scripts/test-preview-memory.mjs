// Observe NSH's decoded-image budget on a private copy of the native extension.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

const root = resolve(".");
const results = join(root, "target/nsh-validation");
await mkdir(results, { recursive: true });
const dir = await mkdtemp(join(results, "preview-memory-"));
const archive = join(dir, "inputs.tar");
execFileSync("git", [
  "-c",
  "safe.directory=" + root.replaceAll("\\", "/"),
  "archive",
  "--format=tar",
  "--output",
  archive,
  "HEAD",
  "--",
  "desktop",
  "ico",
  "src",
  "assets/data",
]);
execFileSync("tar", ["-xf", archive, "-C", dir]);
const library = join(dir, "desktop/ext/wallpaper-native/src/lib.rs");
const original = await readFile(library, "utf8");
const measured = original.replace(
  "nsh_core::image::budget().clear();",
  `
        let memory = nsh_core::image::budget();
        let before = memory.used();
        memory.clear();
        let after = memory.used();
        use std::io::Write;
        let path = std::path::PathBuf::from(std::env::var_os("BING_WALLPAPER_TEST_PROFILE").unwrap()).join("budget.txt");
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
        writeln!(file, "{before} {after}").unwrap();
    `,
);
assert.notEqual(measured, original);
await writeFile(library, measured);
const config = join(dir, "profile/BingWallpaperLib");
await mkdir(config, { recursive: true });
await writeFile(join(config, "settings.json"), '{"language":"english","theme_preference":"light"}');
const target = { win32: "windows", linux: "linux", darwin: "macos" }[process.platform];
const args = ["test", "desktop/App.tsx", "--target", target, "--foreground"];
const dates = ["2026-11-25", "2026-11-24", "2026-11-23"];
const actions = `
  await app.setViewport(1120,800);
  await app.waitFor("latest-photo",{timeout:30000});
  await app.press("month-2026-11");
  for (const date of ${JSON.stringify(dates)}) {
    await app.press("photo-"+date);
    await app.waitFor("close-preview");
    await new Promise(resolve=>setTimeout(resolve,2400));
    await app.press("close-preview");
    await app.waitFor("close-preview",{state:"gone"});
    await new Promise(resolve=>setTimeout(resolve,650));
  }`;
const result = spawnSync(process.env.NSH_BIN || "nsh", args, {
  cwd: dir,
  input: actions,
  encoding: "utf8",
  timeout: 300000,
  env: {
    ...process.env,
    CARGO_TARGET_DIR: join(root, "target/nsh-cargo"),
    BING_WALLPAPER_TEST_PROFILE: join(dir, "profile"),
  },
});
const trace = result.stdout + "\n" + result.stderr;
await writeFile(join(dir, "native.log"), trace);
if (result.error) throw result.error;
assert.equal(result.status, 0, trace);
const audit = await readFile(join(dir, "profile/budget.txt"), "utf8");
const samples = [...audit.matchAll(/(\d+) (\d+)/g)].map(([, before, after]) => ({
  before: Number(before),
  after: Number(after),
}));
await writeFile(join(dir, "result.json"), JSON.stringify({ dates, samples }, null, 2));
assert.ok(samples.length >= dates.length, "Native cache samples missing. See " + join(dir, "native.log"));
assert.ok(
  samples.some((s) => s.before > s.after),
  "No released decoded bytes observed. See " + dir,
);
console.log(JSON.stringify({ dir, samples }));
