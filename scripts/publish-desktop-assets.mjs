// Upload one already-built distribution to existing releases, publishing the signed manifest last.
import { execFileSync } from "node:child_process";
import { createPrivateKey } from "node:crypto";
import { createReadStream } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import { makeManifest, signManifest, verifyPublished } from "./sign-desktop-release.mjs";

const [directory, version, keyFile] = process.argv.slice(2);
if (!directory || !version || !keyFile)
  throw Error("Usage: node scripts/publish-desktop-assets.mjs ARTIFACT_DIR VERSION ED25519_PRIVATE_PEM");
const manifest = await makeManifest(directory, version);
const windows = manifest.artifacts.find((file) => file.os === "windows");
const executable = join(directory, windows.filename);
// The publishing job receives the digest verified by its prerequisite Windows job.
// Standalone invocations verify the local executable before making any network request.
if (process.env.GITHUB_ACTIONS === "true") {
  if (windows.sha256 !== process.env.VERIFIED_WINDOWS_SHA256?.toLowerCase())
    throw Error("The Windows executable differs from the Authenticode-verified artifact");
} else if (process.platform === "win32") {
  const verify = [
    "$ErrorActionPreference = 'Stop'",
    "Import-Module (Join-Path $PSHOME 'Modules/Microsoft.PowerShell.Security/Microsoft.PowerShell.Security.psd1')",
    "$s = Get-AuthenticodeSignature -LiteralPath $env:WALLPAPER_SIGNED_EXE",
    "if ($s.Status -ne 'Valid' -or $s.SignatureType -ne 'Authenticode') { throw ('Windows signature: ' + $s.Status) }",
  ].join("; ");
  execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", verify], {
    env: { ...process.env, WALLPAPER_SIGNED_EXE: executable },
    stdio: "inherit",
  });
} else {
  execFileSync("osslsigncode", ["verify", "-in", executable], { stdio: "inherit" });
}
const privateKey = createPrivateKey(await readFile(keyFile));
const tag = "v" + version;
const releases = [
  {
    name: "GitHub",
    base: "https://api.github.com/repos/pandaligx/bing-wallpaper-lib",
    token: process.env.GH_TOKEN,
  },
  {
    name: "GitCode",
    base: "https://api.gitcode.com/api/v5/repos/gcw_qWIjmve7/bing-wallpaper-lib",
    token: process.env.GITCODE_TOKEN,
  },
];
async function checked(response, operation) {
  if (!response.ok) throw Error(operation + " returned HTTP " + response.status);
  return response;
}
for (const site of releases) {
  if (!site.token) throw Error("Missing publishing credential for " + site.name);
  site.headers = { Authorization: "Bearer " + site.token, "User-Agent": "BingWallpaperLib-release" };
  const response = await checked(
    await fetch(site.base + "/releases/tags/" + tag, { headers: site.headers, redirect: "error" }),
    site.name + " release lookup",
  );
  site.release = await response.json();
  if (site.release.tag_name !== tag || site.release.draft)
    throw Error(site.name + " requires an existing public release for " + tag);
}
async function upload(site, name, bytes, body) {
  if (site.release.assets.some((a) => a.name === name)) return;
  let url, headers, method;
  if (site.name === "GitHub") {
    url = new URL(
      "https://uploads.github.com/repos/pandaligx/bing-wallpaper-lib/releases/" +
        Number(site.release.id) +
        "/assets",
    );
    url.searchParams.set("name", name);
    method = "POST";
    headers = { ...site.headers, "content-type": "application/octet-stream" };
  } else {
    const request = new URL(site.base + "/releases/" + tag + "/upload_url");
    request.searchParams.set("file_name", name);
    const response = await checked(
      await fetch(request, { headers: site.headers, redirect: "error" }),
      "GitCode upload URL",
    );
    const signed = await response.json();
    url = new URL(signed.url);
    if (url.protocol !== "https:" || url.username || url.password) throw Error("Invalid signed upload URL");
    method = "PUT";
    headers = signed.headers;
  }
  const response = await fetch(url, {
    method,
    headers: { ...headers, "content-length": String(bytes) },
    body: body(),
    duplex: "half",
    redirect: "error",
  });
  await checked(response, site.name + " " + name);
  await response.body?.cancel();
}
for (const site of releases)
  for (const asset of manifest.artifacts) {
    const file = join(directory, asset.filename);
    await upload(site, asset.filename, (await stat(file)).size, () => createReadStream(file));
  }
await verifyPublished(manifest);
const envelope = Buffer.from(JSON.stringify(signManifest(manifest, privateKey)) + "\n");
for (const site of releases) {
  const previous = site.release.assets.find((a) => a.name === "signed-release.json");
  if (previous) {
    const response = await checked(
      await fetch(previous.browser_download_url),
      "Read existing public manifest",
    );
    if (!envelope.equals(Buffer.from(await response.arrayBuffer())))
      throw Error(site.name + " already has a different signed manifest");
  } else await upload(site, "signed-release.json", envelope.length, () => envelope);
}
console.log("Published and compared both platform files on GitHub and GitCode for " + tag);
