// Exercise the actual AppImage installer with a disposable earlier build and XDG profile.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { generateKeyPairSync } from "node:crypto";
import { once } from "node:events";
import { createReadStream } from "node:fs";
import { chmod, copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { fingerprint, signManifest } from "./sign-desktop-release.mjs";

if (process.platform !== "linux" || !process.env.DISPLAY) throw Error("Run on a Linux desktop or Xvfb");
const root = resolve(".");
const { version } = JSON.parse(await readFile("desktop/nsh.json", "utf8"));
const name = "BingWallpaperLib-v" + version + "-linux-x86_64.AppImage";
const candidate = join(root, "target/nsh-release", name);
const run = await mkdtemp(join(tmpdir(), "bing-appimage-upgrade-"));
const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const env = {
  ...process.env,
  XDG_CONFIG_HOME: join(run, "config"),
  XDG_DATA_HOME: join(run, "data"),
  XDG_CACHE_HOME: join(run, "cache"),
  XDG_CURRENT_DESKTOP: "GNOME",
  APPIMAGE_EXTRACT_AND_RUN: "1",
  CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || join(root, "target/nsh-cargo"),
  BING_WALLPAPER_UPDATE_PUBLIC_KEY: publicKey.export({ format: "jwk" }).x,
};
const original = join(run, "target/nsh-release/BingWallpaperLib-v0.0.1-linux-x86_64.AppImage");
const current = join(run, "portable.AppImage");
const profile = join(env.XDG_CONFIG_HOME, "BingWallpaperLib");
const staged = join(env.XDG_DATA_HOME, "BingWallpaperLib/updates/GitHub", version);

async function exercise(input, success) {
  await copyFile(original, current);
  await chmod(current, 0o755);
  await copyFile(input, join(staged, name));
  const entry = {
    os: "linux",
    arch: "x86_64",
    minimumSystem: "GTK 4.14",
    filename: name,
    ...(await fingerprint(createReadStream(input))),
  };
  const envelope = signManifest({ format: 1, version, artifacts: [entry] }, privateKey);
  await writeFile(join(staged, "signed-release.json"), JSON.stringify(envelope));
  const child = spawn(current, ["--install-wallpaper-update", version], {
    env,
    detached: true,
    stdio: ["pipe", "pipe", "inherit"],
  });
  const ended = once(child, "exit");
  const lines = createInterface({ input: child.stdout });
  let ready = false;
  const trace = [];
  lines.on("line", (line) => {
    trace.push(line);
    if (line === "BING-WALLPAPER-STAGED") {
      ready = true;
      child.stdin.end("1");
    }
  });
  function stopGroup() {
    try {
      process.kill(-child.pid, "SIGKILL");
    } catch (error) {
      if (error.code !== "ESRCH") throw error;
    }
  }
  const timer = setTimeout(stopGroup, 180000);
  try {
    const [code] = await ended;
    assert.ok(ready, trace.join("\n"));
    assert.equal(code === 0, success, trace.join("\n"));
    assert.deepEqual(
      await fingerprint(createReadStream(current)),
      await fingerprint(createReadStream(success ? candidate : original)),
    );
  } finally {
    clearTimeout(timer);
    lines.close();
    stopGroup();
  }
}

try {
  const tar = join(run, "source.tar");
  execFileSync("git", [
    "archive",
    "--format=tar",
    "--output",
    tar,
    "HEAD",
    "--",
    "desktop",
    "src",
    "ico",
    "assets/data",
    "scripts/nsh-appimagetool.sh",
    "scripts/package-nsh-unix.sh",
  ]);
  execFileSync("tar", ["-xf", tar, "-C", run]);
  const config = join(run, "desktop/nsh.json");
  const app = JSON.parse(await readFile(config, "utf8"));
  await writeFile(config, JSON.stringify({ ...app, version: "0.0.1" }));
  const lockPath = join(run, "desktop/nsh.lock");
  const lock = JSON.parse(await readFile(lockPath, "utf8"));
  lock.packages.find((p) => p.name === "wallpaper-native").version = "0.0.1";
  await writeFile(lockPath, JSON.stringify(lock));
  for (const file of ["Cargo.toml", "Cargo.lock"]) {
    const path = join(run, "desktop/ext/wallpaper-native", file);
    const source = await readFile(path, "utf8");
    const edited = source.replace(
      /(name = "wallpaper-native"\s+version = ")[^"]+"/,
      (_, prefix) => prefix + '0.0.1"',
    );
    assert.notEqual(edited, source);
    await writeFile(path, edited);
  }
  execFileSync("bash", ["scripts/package-nsh-unix.sh", "linux"], { cwd: run, env, stdio: "inherit" });
  await mkdir(staged, { recursive: true });
  await mkdir(profile, { recursive: true });
  await writeFile(join(profile, "update-region"), "international");
  await exercise(candidate, true);
  const broken = join(run, "broken");
  await writeFile(broken, "unlaunchable update fixture");
  await exercise(broken, false);
  console.log(
    "PASS · signed AppImage upgrade activated the real NSH window and a failed launch restored the previous AppImage",
  );
} finally {
  await rm(run, { recursive: true, force: true });
}
