import test from "node:test";
import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, verify } from "node:crypto";
import { signManifest, verifyPublished } from "./sign-desktop-release.mjs";

test("both public copies must match the local bytes before signing", async () => {
  const body = Buffer.from("test release binary");
  const row = { filename: "BingWallpaperLib-v1.0.0-windows-x64.exe", bytes: body.length, sha256: createHash("sha256").update(body).digest("hex") };
  const manifest = { format: 1, version: "1.0.0", artifacts: [row] };
  function provider(bad, omit) {
    return async url => {
      const link = new URL(url);
      if (link.pathname.includes("/releases/tags/")) {
        const assets = [{ name: "Source code.zip", browser_download_url: "https://example.com/archive" }];
        if (!omit) assets.push({ name: row.filename, browser_download_url: "https://" + link.hostname + "/attachment" });
        return Response.json({ tag_name: "v1.0.0", assets });
      }
      return new Response(bad && link.hostname === "api.gitcode.com" ? Buffer.from("wrong") : body);
    };
  }
  await verifyPublished(manifest, provider(false, false));
  await assert.rejects(verifyPublished(manifest, provider(true, false)), /different content/);
  await assert.rejects(verifyPublished(manifest, provider(false, true)), /exactly one/);
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const signed = signManifest(manifest, privateKey);
  const payload = Buffer.from(signed.payload, "base64"), signature = Buffer.from(signed.signature, "base64");
  assert(verify(null, payload, publicKey, signature));
  payload[0] ^= 1;
  assert(!verify(null, payload, publicKey, signature));
});
