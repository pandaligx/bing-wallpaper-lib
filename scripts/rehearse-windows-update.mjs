// Build two private rehearsal versions and exercise the same signed installer as a portable release.
import assert from "node:assert/strict";
import { createHash, generateKeyPairSync } from "node:crypto";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { createReadStream } from "node:fs";
import { cp, copyFile, mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { signManifest } from "./sign-desktop-release.mjs";

if (process.platform !== "win32") throw Error("Run on a Windows x64 build machine");
const root = resolve("."),
  artifacts = join(root, "target", "nsh-validation");
await mkdir(artifacts, { recursive: true });
const run = await mkdtemp(join(artifacts, "signed-upgrade-"));
const source = join(run, "source"),
  downloadName = "BingWallpaperLib-v0.3.1-windows-x64.exe";
const executable = join(run, "BingWallpaperLib.exe"),
  candidate = join(run, "new.exe"),
  original = join(run, "old.exe");
const profile = join(run, "profile", "BingWallpaperLib");
const staging = join(profile, "updates", "GitHub", "0.3.1");
const id = "cn.lgxng.wallpaper.rehearsal." + run.split("-").pop().toLowerCase();
const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const env = {
  ...process.env,
  BING_WALLPAPER_UPDATE_PUBLIC_KEY: publicKey.export({ format: "jwk" }).x,
  BING_WALLPAPER_TEST_PROFILE: join(run, "profile"),
};
async function command(program, args, options = {}) {
  const child = spawn(program, args, { env, stdio: "inherit", windowsHide: true, ...options });
  const [code] = await once(child, "exit");
  assert.equal(code, 0, program + " failed");
}
async function digest(path) {
  const hash = createHash("sha256");
  let bytes = 0;
  for await (const chunk of createReadStream(path)) {
    hash.update(chunk);
    bytes += chunk.length;
  }
  return { bytes, sha256: hash.digest("hex") };
}
async function replace(file, before, after) {
  const path = join(source, file),
    text = await readFile(path, "utf8");
  const result = text.replaceAll(before, after);
  assert.notEqual(result, text, file);
  await writeFile(path, result);
}
for (const path of [
  "ico",
  "src",
  "assets/data",
  "vendor/nsh-platform-windows",
  "desktop/windows/Cargo.toml",
  "desktop/windows/Cargo.lock",
  "desktop/windows/build.rs",
  "desktop/windows/src",
  "desktop/ext/wallpaper-native/Cargo.toml",
  "desktop/ext/wallpaper-native/src",
  "desktop/.nsh/out/bundle.nshc",
  "desktop/.nsh/out/capabilities.json",
]) {
  await cp(join(root, path), join(source, path), { recursive: true });
}
await replace("desktop/windows/src/main.rs", "cn.lgxng.bingwallpaperlib", id);
await replace("src/startup.rs", 'const VALUE_NAME: &str = "BingWallpaperLib"', 'const VALUE_NAME: &str = "' + id + '"');
await replace("src/task_scheduler.rs", "BingWallpaperLibPeriodicWallpaper", id + ".periodic");
await replace("desktop/ext/wallpaper-native/src/registration.rs", "BingWallpaperLibDailyWallpaper", id + ".daily");
await mkdir(staging, { recursive: true });
await writeFile(join(profile, "update-region"), "international");
const target = join(root, "target", "nsh-cargo"),
  arch = "x86_64-pc-windows-msvc";
async function build(out) {
  await command("cargo", [
    "build",
    "--release",
    "--locked",
    "--config",
    "profile.release.package.wallpaper-native.debug-assertions=true",
    "--target",
    arch,
    "--target-dir",
    target,
    "--manifest-path",
    join(source, "desktop/windows/Cargo.toml"),
  ]);
  await copyFile(join(target, arch, "release", "bing-wallpaper-desktop.exe"), out);
}
await build(original);
for (const path of ["desktop/windows/Cargo.toml", "desktop/ext/wallpaper-native/Cargo.toml"])
  await replace(path, 'version = "0.3.0"', 'version = "0.3.1"');
const lock = join(source, "desktop/windows/Cargo.lock");
await writeFile(
  lock,
  (await readFile(lock, "utf8")).replace(
    /(name = "(?:wallpaper-native|bing-wallpaper-desktop)"\r?\nversion = ")0\.3\.0/g,
    (_match, prefix) => prefix + "0.3.1",
  ),
);
await build(candidate);
async function stopIsolatedWindow() {
  // The path is an environment value; look up only this rehearsal's process.
  await command(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      "Get-Process -Name BingWallpaperLib -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $env:BING_REHEARSAL_EXE } | Stop-Process -Force; exit 0",
    ],
    { env: { ...env, BING_REHEARSAL_EXE: executable } },
  );
}
async function exercise(file, success) {
  await copyFile(original, executable);
  await copyFile(file, join(staging, downloadName));
  const signature = signManifest(
    {
      format: 1,
      version: "0.3.1",
      artifacts: [
        {
          os: "windows",
          arch: "x64",
          minimumSystem: "10.0.17763",
          filename: downloadName,
          ...(await digest(file)),
        },
      ],
    },
    privateKey,
  );
  await writeFile(join(staging, "signed-release.json"), JSON.stringify(signature));
  const proc = spawn(executable, ["--install-wallpaper-update", "0.3.1"], {
    env,
    stdio: ["pipe", "pipe", "inherit"],
    windowsHide: true,
  });
  const end = once(proc, "exit");
  let staged = false,
    output = "";
  const lines = createInterface({ input: proc.stdout });
  lines.on("line", (line) => {
    output += line + "\n";
    if (line === "BING-WALLPAPER-STAGED") {
      staged = true;
      proc.stdin.end("1");
    }
  });
  const timer = setTimeout(() => proc.kill(), 180000);
  try {
    const [code] = await end;
    assert.ok(staged, output);
    assert.equal(code === 0, success, output);
    assert.deepEqual(await digest(executable), await digest(success ? candidate : original));
  } finally {
    clearTimeout(timer);
    lines.close();
    await stopIsolatedWindow();
  }
}
await exercise(candidate, true);
const broken = join(run, "broken");
await writeFile(broken, "unlaunchable test payload");
await exercise(broken, false);

await writeFile(
  join(run, "result.json"),
  JSON.stringify(
    { old: await digest(original), updated: await digest(candidate), success: true, rollback: true },
    null,
    2,
  ),
);
console.log(
  "PASS · signed portable upgrade, real NSH startup acknowledgement and failed-launch rollback",
  run,
);
