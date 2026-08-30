import { brotliCompressSync } from "node:zlib";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";

const root = new URL("../dist", import.meta.url).pathname;
const limits = { initialJs: 200 * 1024, total: 350 * 1024, chunk: 150 * 1024 };

function files(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    return entry.isDirectory() ? files(path) : [path];
  });
}

const assets = files(root)
  .filter((path) => /\.(js|css)$/.test(path))
  .map((path) => ({
    file: relative(root, path),
    raw: statSync(path).size,
    compressed: brotliCompressSync(readFileSync(path)).byteLength,
  }));
const js = assets.filter((asset) => asset.file.endsWith(".js"));
const entry = js.filter((asset) =>
  !asset.file.includes("OPML") && !asset.file.includes("Settings")
);
const report = {
  limits,
  initialJs: entry.reduce((sum, asset) => sum + asset.compressed, 0),
  total: assets.reduce((sum, asset) => sum + asset.compressed, 0),
  assets,
};
writeFileSync("bundle-report.json", JSON.stringify(report, null, 2));

const violations = [];
if (report.initialJs > limits.initialJs) {
  violations.push(`initial JS ${report.initialJs} > ${limits.initialJs}`);
}
if (report.total > limits.total) {
  violations.push(`total JS/CSS ${report.total} > ${limits.total}`);
}
for (const asset of assets) {
  if (asset.compressed > limits.chunk) {
    violations.push(`${asset.file} ${asset.compressed} > ${limits.chunk}`);
  }
}
console.table(assets);
if (violations.length) {
  console.error(`Bundle budget exceeded:\n${violations.join("\n")}`);
  process.exit(1);
}
