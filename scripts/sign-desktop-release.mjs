import { createHash, createPrivateKey, sign } from "node:crypto";
import { createReadStream } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const repositories = [
  ["GitHub", "https://api.github.com/repos/pandaligx/bing-wallpaper-lib/releases/tags/"],
  ["GitCode", "https://api.gitcode.com/api/v5/repos/gcw_qWIjmve7/bing-wallpaper-lib/releases/tags/"],
];

export async function fingerprint(stream, limit = Infinity) {
  const hash = createHash("sha256");
  let bytes = 0;
  for await (const chunk of stream) {
    bytes += chunk.length;
    if (bytes > limit) throw new Error("Attachment exceeds the expected size");
    hash.update(chunk);
  }
  if (!bytes) throw new Error("Empty attachment");
  return { bytes, sha256: hash.digest("hex") };
}

export async function makeManifest(directory, version) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error("Expected a stable semantic version");
  const files = [
    ["windows", "x64", "10.0.17763", "windows-x64.exe"],
    ["linux", "x86_64", "GTK 4.14 and WebKitGTK 6.0", "linux-x86_64.AppImage"],
  ];
  const artifacts = [];
  for (const [os, arch, minimumSystem, suffix] of files) {
    const filename = `BingWallpaperLib-v${version}-${suffix}`;
    const digest = await fingerprint(createReadStream(join(directory, filename)));
    artifacts.push({ os, arch, minimumSystem, filename, ...digest });
  }
  return { format: 1, version, artifacts };
}

async function anonymousRequest(provider, address, timeout, request) {
  let url = new URL(address);
  const signal = AbortSignal.timeout(timeout);
  for (let redirect = 0; redirect <= 8; redirect++) {
    const host = url.hostname;
    const permitted = provider === "GitHub"
      ? host === "github.com" || host === "api.github.com" || host.endsWith(".githubusercontent.com")
      : host === "gitcode.com" || host.endsWith(".gitcode.com") || host === "gitcodeusercontent.com" || host.endsWith(".gitcodeusercontent.com");
    if (!permitted || url.protocol !== "https:" || url.username || url.password) {
      throw new Error(`${provider}: unexpected download origin ${host}`);
    }
    const response = await request(url, { signal, redirect: "manual" });
    if (![301,302,303,307,308].includes(response.status)) return response;
    const next = response.headers.get("location");
    await response.body?.cancel();
    if (!next) throw new Error(`${provider}: redirect without a location`);
    url = new URL(next, url);
  }
  throw new Error(`${provider}: too many redirects`);
}

export async function verifyPublished(manifest, request = fetch, tag = "v" + manifest.version) {
  const stable = "v" + manifest.version;
  const candidate = tag !== stable;
  if (candidate && !(tag.startsWith(stable) && /^-rc\.\d+$/.test(tag.slice(stable.length)))) {
    throw new Error("The rehearsal tag must belong to the tested version");
  }
  for (const [provider, api] of repositories) {
    const response = await anonymousRequest(provider, api + encodeURIComponent(tag), 30_000, request);
    if (!response.ok) throw new Error(`${provider} release API: HTTP ${response.status}`);
    const release = await response.json();
    if (release.tag_name !== tag || release.draft || Boolean(release.prerelease) !== candidate)
      throw new Error(`${provider}: wrong release tag or publication channel`);
    for (const asset of manifest.artifacts) {
      const matches = release.assets.filter(row => row.name === asset.filename);
      if (matches.length !== 1) throw new Error(`${provider}: expected exactly one ${asset.filename}`);
      const url = new URL(matches[0].browser_download_url);
      const download = await anonymousRequest(provider, url, 600_000, request);
      if (!download.ok || !download.body) throw new Error(`${provider} ${asset.filename}: HTTP ${download.status}`);
      const actual = await fingerprint(download.body, asset.bytes);
      if (actual.bytes !== asset.bytes || actual.sha256 !== asset.sha256) {
        throw new Error(`${provider}: different content for ${asset.filename}`);
      }
    }
  }
}

export function signManifest(manifest, privateKey) {
  if (privateKey.asymmetricKeyType !== "ed25519") throw new Error("Expected an Ed25519 signing key");
  const payload = Buffer.from(JSON.stringify(manifest), "utf8");
  const signature = sign(null, payload, privateKey);
  return { payload: payload.toString("base64"), signature: signature.toString("base64") };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [directory, version, privateKeyFile, tag] = process.argv.slice(2);
  if (!directory || !version || !privateKeyFile) {
    throw new Error("Usage: node scripts/sign-desktop-release.mjs ARTIFACT_DIR VERSION ED25519_PRIVATE_PEM [RELEASE_TAG]");
  }
  const manifest = await makeManifest(directory, version);
  await verifyPublished(manifest, fetch, tag);
  const key = createPrivateKey(await readFile(privateKeyFile));
  const envelope = signManifest(manifest, key);
  await writeFile(join(directory, "signed-release.json"), JSON.stringify(envelope) + "\n", { flag: "wx" });
}
