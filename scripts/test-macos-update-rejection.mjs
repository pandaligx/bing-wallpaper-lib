// Verify that a valid release manifest cannot authorize an ad-hoc macOS app for installation.
import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { generateKeyPairSync } from "node:crypto";
import { createReadStream } from "node:fs";
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { fingerprint, signManifest } from "./sign-desktop-release.mjs";

if (process.platform !== "darwin" || !process.argv[2])
  throw Error("Supply a development DMG on macOS");
const root = resolve(".");
const dmg = resolve(process.argv[2]);
const work = await mkdtemp(join(tmpdir(), "bing-mac-update-"));
const installParent =
  process.env.RUNNER_ENVIRONMENT === "github-hosted" ? "/Applications" : work;
const installation = await mkdtemp(
  join(installParent, "BingWallpaperRehearsal-"),
);
const app = join(installation, "BingWallpaperLib.app");
const mount = join(work, "mount");
const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const env = {
  ...process.env,
  BING_WALLPAPER_UPDATE_PUBLIC_KEY: publicKey.export({ format: "jwk" }).x,
  BING_WALLPAPER_TEST_PROFILE: join(work, "profile"),
  CARGO_TARGET_DIR: join(work, "cargo"),
};
let mounted = false;
try {
  await mkdir(mount);
  execFileSync("hdiutil", [
    "attach",
    "-readonly",
    "-nobrowse",
    "-mountpoint",
    mount,
    dmg,
  ]);
  mounted = true;
  const candidate = join(mount, "BingWallpaperLib.app");
  execFileSync("codesign", ["--verify", "--deep", "--strict", candidate]);
  const identity = spawnSync("codesign", ["-dv", "--verbose=4", candidate], {
    encoding: "utf8",
  });
  assert.equal(identity.status, 0);
  assert.match(
    identity.stderr,
    /Signature=adhoc/,
    "This test expects a development DMG",
  );
  execFileSync("ditto", [candidate, app]);
  execFileSync("hdiutil", ["detach", mount]);
  mounted = false;
  const info = join(app, "Contents/Info.plist");
  const plist = (key) =>
    execFileSync("/usr/libexec/PlistBuddy", ["-c", "Print " + key, info], {
      encoding: "utf8",
    }).trim();
  const installed = plist("CFBundleShortVersionString");
  assert.match(installed, /^\d+\.\d+\.\d+$/);
  const [major, minor, patch] = installed.split(".");
  const nextVersion = [major, minor, Number(patch) + 1].join(".");
  const main = join(app, "Contents/MacOS", plist("CFBundleExecutable"));
  // Only the predecessor gets the rehearsal public key. The signed payload is the original public DMG.
  execFileSync(
    "cargo",
    [
      "build",
      "--locked",
      "--manifest-path",
      "desktop/ext/wallpaper-native/Cargo.toml",
      "--bin",
      "bing-wallpaper-task",
    ],
    { cwd: root, env, stdio: "inherit" },
  );
  const helper = join(app, "Contents/Helpers/bing-wallpaper-task");
  await copyFile(
    join(env.CARGO_TARGET_DIR, "debug/bing-wallpaper-task"),
    helper,
  );
  execFileSync("codesign", ["--force", "--sign", "-", helper]);
  execFileSync("codesign", ["--force", "--sign", "-", app]);
  execFileSync("codesign", ["--verify", "--deep", "--strict", app]);
  const before = await fingerprint(createReadStream(main));
  const seal = join(app, "Contents/_CodeSignature/CodeResources");
  const signedResources = await readFile(seal);
  const profile = join(env.BING_WALLPAPER_TEST_PROFILE, "BingWallpaperLib");
  const stage = join(profile, "updates/GitHub", nextVersion);
  await mkdir(stage, { recursive: true });
  await writeFile(join(profile, "update-region"), "international");
  const filename = "BingWallpaperLib-v" + nextVersion + "-macos-universal.dmg";
  await copyFile(dmg, join(stage, filename));
  const artifact = {
    os: "macos",
    arch: "universal",
    minimumSystem: "14.0",
    filename,
    ...(await fingerprint(createReadStream(dmg))),
  };
  const envelope = signManifest(
    { format: 1, version: nextVersion, artifacts: [artifact] },
    privateKey,
  );
  await writeFile(join(stage, "signed-release.json"), JSON.stringify(envelope));
  const result = spawnSync(
    helper,
    ["--install-wallpaper-update", nextVersion],
    {
      env,
      input: "1",
      encoding: "utf8",
      timeout: 150000,
    },
  );
  if (result.error) throw result.error;
  const trace = result.stdout + "\n" + result.stderr;
  assert.notEqual(result.status, 0, trace);
  assert.doesNotMatch(trace, /BING-WALLPAPER-STAGED/, trace);
  assert.match(trace, /codesign|spctl/, trace);
  assert.match(
    trace,
    /failed to satisfy|does not satisfy|rejected|not trusted|not notarized/i,
    trace,
  );
  assert.deepEqual(await readFile(seal), signedResources);
  assert.deepEqual(await fingerprint(createReadStream(main)), before);
  execFileSync("codesign", ["--verify", "--deep", "--strict", app]);
  const child = spawn(main, [], {
    env: { ...env, BING_WALLPAPER_CONFIRM_WINDOW: "1" },
    stdio: ["ignore", "pipe", "inherit"],
  });
  const lines = createInterface({ input: child.stdout });
  const timeout = setTimeout(() => child.kill(), 90000);
  let opened = false;
  try {
    for await (const line of lines)
      if (line === "BING-WALLPAPER-WINDOW-READY " + installed) {
        opened = true;
        break;
      }
    assert(opened, "The previous installation did not reopen after rejection");
  } finally {
    clearTimeout(timeout);
    lines.close();
    child.kill();
  }
  console.log(
    "PASS · genuine ad-hoc DMG rejected before replacement; preserved app signature, executable and native window",
    installed,
  );
} finally {
  if (mounted) spawnSync("hdiutil", ["detach", mount]);
  if (
    dirname(installation) === installParent &&
    basename(installation).startsWith("BingWallpaperRehearsal-")
  )
    await rm(installation, { recursive: true, force: true });
}
