// Usage: node scripts/bench-download.mjs <compiled download_bench> <aria2c> [URL]
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { mkdir, mkdtemp, unlink, writeFile } from "node:fs/promises";
import http from "node:http";
import { once } from "node:events";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

const [engine, aria, external] = process.argv.slice(2);
if (!engine || !aria) throw new Error("Provide paths to download_bench and aria2c");
const root = await mkdtemp(join(tmpdir(), "bing-wallpaper-bench-"));
const payload = Buffer.alloc(8 * 1024 * 1024);
for (let i = 0; i < payload.length; i++) payload[i] = i % 251;
const expected = createHash("sha256").update(payload).digest("hex");
let active = 0, peak = 0;

const server = http.createServer(async (req, res) => {
  const range = /^bytes=(\d+)-(\d*)$/.exec(req.headers.range ?? "");
  const first = range ? Number(range[1]) : 0;
  const last = range && range[2] ? Number(range[2]) : payload.length - 1;
  if (first > last || last >= payload.length) { res.writeHead(416).end(); return; }
  res.writeHead(range ? 206 : 200, {
    "Content-Length": last - first + 1, "Accept-Ranges": "bytes", "ETag": '"benchmark-v1"',
    ...(range ? {"Content-Range": `bytes ${first}-${last}/${payload.length}`} : {})
  });
  if (req.method === "HEAD") { res.end(); return; }
  active++;
  peak = Math.max(peak, active);
  try {
    for (let pos = first; pos <= last && !res.destroyed; pos += 32768) {
      res.write(payload.subarray(pos, Math.min(last + 1, pos + 32768)));
      await delay(8);
    }
    res.end();
  } finally { active--; }
});
if (!external) {
  server.listen(0,"127.0.0.1");
  await once(server,"listening");
}
const url = external ?? `http://127.0.0.1:${server.address().port}/fixture`;
const results = [];
let digest = external ? undefined : expected;
try {
  for (let round = 1; round <= 5; round++) {
    for (const mode of ["single","multi","aria2"]) {
      const dir = join(root,`${round}-${mode}`);
      await mkdir(dir);
      const args = mode === "aria2"
        ? ["--no-conf","--console-log-level=warn","--summary-interval=0","--split=16","--max-connection-per-server=16","--min-split-size=1M","--dir",dir,"--out","payload",url]
        : [url,dir,mode === "single" ? "1" : "16"];
      peak = 0;
      const start = performance.now();
      const proc = spawn(resolve(mode === "aria2" ? aria : engine), args, {windowsHide:true,stdio:["ignore","pipe","inherit"]});
      let output = "";
      proc.stdout.on("data", chunk => { output = (output + chunk.toString()).slice(-8192); });
      const [code] = await once(proc,"exit");
      if (code !== 0) throw new Error(`${mode} exited with ${code}\n${output}`);
      const ms = performance.now()-start;
      const hash = createHash("sha256");
      let bytes = 0;
      for await (const chunk of createReadStream(join(dir,"payload"))) {hash.update(chunk); bytes+=chunk.length;}
      const sha256 = hash.digest("hex");
      await unlink(join(dir,"payload"));
      digest ??= sha256;
      if (sha256 !== digest) throw new Error(`Checksum mismatch for ${mode}`);
      results.push({round,mode,ms,bytes,sha256,peak:external ? null : peak});
    }
  }
  const medianMs = Object.fromEntries(["single","multi","aria2"].map(mode => [mode,results.filter(v=>v.mode===mode).map(v=>v.ms).sort((a,b)=>a-b)[2]]));
  const report = {url,results,medianMs,relativeToAria2:medianMs.aria2/medianMs.multi};
  const file=join(root,"report.json");
  await writeFile(file,JSON.stringify(report,null,2));
  console.log(JSON.stringify({file,medianMs,relativeToAria2:report.relativeToAria2,sha256:digest}));
} finally {
  if (!external) server.close();
}
