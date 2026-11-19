#!/usr/bin/env node
/**
 * Web bundle budget. Run after `npm run build` in web/.
 * It reports gzip and raw sizes by chunk role, then fails if a budget is exceeded.
 */
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";

const root = path.resolve(new URL("../..", import.meta.url).pathname);
const dist = path.join(root, "web/dist/assets");
const budgets = JSON.parse(fs.readFileSync(path.join(root, "tools/bench/budgets.json"), "utf8"));

if (!fs.existsSync(dist)) {
  console.error("web-budget: web/dist/assets missing; run `npm run build` first");
  process.exit(1);
}

const files = fs.readdirSync(dist).filter((name) => /\.(js|css)$/.test(name));
const js = files.filter((name) => name.endsWith(".js"));
const css = files.filter((name) => name.endsWith(".css"));
const raw = (file) => fs.statSync(path.join(dist, file)).size / 1024;
const gzip = (file) => zlib.gzipSync(fs.readFileSync(path.join(dist, file))).length / 1024;

const entry = js.filter((name) => name.startsWith("index-"));
const vendor = js.filter((name) => name.startsWith("vendor-"));
const routes = js.filter((name) => !entry.includes(name) && !vendor.includes(name));

const failures = [];
function budget(label, value, limit) {
  const status = value <= limit ? "OK" : "FAIL";
  console.log(`${status.padEnd(4)} ${label.padEnd(24)} ${value.toFixed(2).padStart(7)} KB / ${limit} KB`);
  if (value > limit) failures.push(`${label}: ${value.toFixed(2)} > ${limit}`);
}

if (entry.length !== 1) failures.push(`expected one entry chunk, got ${entry.length}`);
if (vendor.length !== 1) failures.push(`expected one vendor chunk, got ${vendor.length}`);

for (const file of entry) budget(`entry ${file}`, raw(file), budgets.entryRawKb);
for (const file of vendor) budget(`vendor ${file}`, raw(file), budgets.vendorRawKb);
for (const file of routes) budget(`route ${file}`, raw(file), budgets.routeRawKb);

const totalJsGzip = js.reduce((sum, file) => sum + gzip(file), 0);
const totalCssGzip = css.reduce((sum, file) => sum + gzip(file), 0);
const totalGzip = files.reduce((sum, file) => sum + gzip(file), 0);

console.log(`     ${"total js gzip".padEnd(24)} ${totalJsGzip.toFixed(2).padStart(7)} KB / ${budgets.totalJsGzipKb} KB`);
console.log(`     ${"total css gzip".padEnd(24)} ${totalCssGzip.toFixed(2).padStart(7)} KB / ${budgets.totalCssGzipKb} KB`);
console.log(`     ${"total gzip".padEnd(24)} ${totalGzip.toFixed(2).padStart(7)} KB / ${budgets.totalGzipKb} KB`);

budget("js gzip total", totalJsGzip, budgets.totalJsGzipKb);
budget("css gzip total", totalCssGzip, budgets.totalCssGzipKb);
budget("all gzip total", totalGzip, budgets.totalGzipKb);

if (failures.length) {
  console.error(`web-budget: FAILED (${failures.length})`);
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("web-budget: OK");
