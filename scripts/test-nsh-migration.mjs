// Test the real Rust module against an isolated copy of a legacy profile.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { once } from "node:events";
import { copyFile, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const fixture = await mkdtemp(join(tmpdir(),"bing-profile-"));
const data = join(fixture,"BingWallpaperLib"), originals = join(fixture,"Original photos");
const favorites = '["2024-01-26"]';
const settings = JSON.stringify({language:"french",theme_preference:"dark",download_dir:originals,
  download_resolution:"two_k",future_field:"keep this value"});
const sha = async file => createHash("sha256").update(await readFile(file)).digest("hex");
async function exercise(steps) {
  const cli=process.env.NSH_BIN || "nsh";
  const proc=spawn(cli,["test","desktop/App.tsx"],{env:{...process.env,BING_WALLPAPER_TEST_PROFILE:fixture},stdio:["pipe","inherit","inherit"]});
  const end=once(proc,"exit");
  proc.stdin.end(steps);
  const [code]=await end;
  assert.equal(code,0,"NSH interaction failed");
}
try {
  await mkdir(data);await mkdir(originals);
  await writeFile(join(data,"settings.json"),settings);
  await writeFile(join(data,"favorites.json"),favorites);
  for(const name of ["original.png","partial.png","remove.png"]) await copyFile(resolve("desktop/icon.png"),join(originals,name));
  await writeFile(join(originals,"partial.png.aria2"),"resume");
  const hash=await sha(join(originals,"original.png"));
  await exercise(`
    await app.setViewport(1120,840);
    await app.waitFor("latest-photo",{timeout:30000});
    await app.press("nav-favorites");
    await app.waitFor("photo-2024-01-26");
    await app.press("nav-local");
    await app.waitFor("select-original.png");
    if(app.texts().includes("partial.png")) throw Error("partial download visible");
    await app.press("select-original.png");
    await app.press("delete-selected");
    await app.press("cancel-delete");
    await app.press("select-original.png");
    await app.press("select-remove.png");
    await app.press("delete-selected");
    await app.press("confirm-delete");
    await app.waitFor("select-original.png");
    await app.press("nav-settings");
    await app.waitFor("folder-path");
    await app.press("default-folder");
  `);
  assert.equal(await readFile(join(data,"settings.pre-nsh.json"),"utf8"),settings);
  assert.equal((JSON.parse(await readFile(join(data,"settings.json"),"utf8"))).future_field,"keep this value");
  assert.equal(await readFile(join(data,"favorites.json"),"utf8"),favorites);
  assert.deepEqual((await readdir(originals)).sort(),["original.png","partial.png","partial.png.aria2"]);
  assert.equal(await sha(join(originals,"original.png")),hash);
  await exercise(`
    await app.setViewport(1120,840);
    await app.waitFor("latest-photo",{timeout:30000});
    await app.press("nav-settings");
    await app.waitFor("default-folder");
    await app.press("default-folder");
  `);
  assert.equal(await readFile(join(data,"settings.pre-nsh.json"),"utf8"),settings);
  console.log("PASS · legacy favorites, custom directory, partial exclusion, confirmed deletion, original digest and idempotent backup");
} finally {await rm(fixture,{recursive:true,force:true});}
