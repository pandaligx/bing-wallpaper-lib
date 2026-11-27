// Exercise the Rust updater against an existing public RC on both distribution sites.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  appendFile,
  mkdir,
  mkdtemp,
  readFile,
  writeFile,
} from "node:fs/promises";
import { join, resolve } from "node:path";

const [tag, keyFile] = process.argv.slice(2);
const version = /^v(\d+\.\d+\.\d+)-rc\.\d+$/.exec(tag || "")?.[1];
assert(
  version && keyFile,
  "Usage: node scripts/test-public-update.mjs vX.Y.Z-rc.N TEST_PUBLIC_KEY_FILE",
);
const key = (await readFile(keyFile, "utf8")).trim();
assert.match(key, /^[A-Za-z0-9_-]{43}$/);
const root = resolve(".");
const output = join(root, "target/nsh-validation");
await mkdir(output, { recursive: true });
const run = await mkdtemp(join(output, "public-update-"));
const tar = join(run, "source.tar");
execFileSync("git", [
  "-c",
  "safe.directory=" + root.replaceAll("\\", "/"),
  "archive",
  "--format=tar",
  "--output",
  tar,
  "HEAD",
  "--",
  "src",
  "assets/data",
  "desktop/ext/wallpaper-native",
]);
execFileSync("tar", ["-xf", tar, "-C", run]);
const native = join(run, "desktop/ext/wallpaper-native");
async function substitute(file, pattern, value) {
  const before = await readFile(file, "utf8");
  const after = before.replace(pattern, value);
  assert.notEqual(before, after, "Expected rehearsal input in " + file);
  await writeFile(file, after);
}
// The disposable predecessor accepts only the one named RC. Shipped builds ignore previews.
for (const config of ["Cargo.toml", "Cargo.lock"]) {
  await substitute(
    join(native, config),
    /(name = "wallpaper-native"\r?\nversion = ")[^"]+"/,
    (_match, prefix) => prefix + '0.0.1"',
  );
}
const file = join(native, "src/release.rs");
await substitute(
  file,
  ".filter(|r| !r.draft && !r.prerelease)",
  '.filter(|r| !r.draft && r.prerelease && r.tag_name == "' + tag + '")',
);
await substitute(
  file,
  "version(&r.tag_name).map(|v| (v, r))",
  'version("' + version + '").map(|v| (v, r))',
);
await appendFile(
  file,
  `
#[test]
fn exercise_public_candidate() -> anyhow::Result<()> {
    let version = "${version}";
    for (preference, site) in [("cn", Source::GitCode), ("international", Source::GitHub)] {
        crate::region::select(preference)?;
        let source = crate::region::selected()?.context("Requested update site missing")?;
        assert_eq!(source, site);
        let runtime = crate::runtime()?;
        let available: serde_json::Value = serde_json::from_str(&runtime.block_on(check(source, false))?)?;
        assert_eq!(available["state"], "available");
        assert_eq!(available["provider"], site.name());
        let staged: serde_json::Value = serde_json::from_str(&runtime.block_on(check(source, true))?)?;
        assert_eq!(staged["state"], "staged");
        let destination = crate::paths::app_data_dir()?.join(format!("{}-verified", site.name()));
        copy_verified(version, &destination)?;
        assert_eq!(std::fs::metadata(&destination)?.len(), available["bytes"].as_u64().unwrap());
        println!("PASS {} signed update, {} bytes", site.name(), available["bytes"]);
    }
    Ok(())
}
`,
);
execFileSync(
  "cargo",
  [
    "test",
    "--locked",
    "--manifest-path",
    join(native, "Cargo.toml"),
    "--lib",
    "--",
    "--exact",
    "release::exercise_public_candidate",
    "--nocapture",
  ],
  {
    cwd: root,
    stdio: "inherit",
    env: {
      ...process.env,
      BING_WALLPAPER_UPDATE_PUBLIC_KEY: key,
      BING_WALLPAPER_TEST_PROFILE: join(run, "profile"),
      CARGO_TARGET_DIR: join(root, "target/nsh-cargo"),
    },
  },
);
console.log(
  "Verified both public updates through the native downloader in",
  run,
);
