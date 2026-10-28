import { readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../dist/", import.meta.url));
const manifest = JSON.parse(readFileSync(resolve(root, ".vite/manifest.json"), "utf8"));
const entry = Object.keys(manifest).find((key) => manifest[key].isEntry);
if (!entry) throw new Error("No entry in the Vite manifest; run npm run build first.");

function initialFiles(component) {
  const visited = new Set();
  const files = new Set();
  function visit(key) {
    if (visited.has(key)) return;
    visited.add(key);
    const chunk = manifest[key];
    if (!chunk) throw new Error(`Missing chunk: ${key}`);
    files.add(chunk.file);
    for (const css of chunk.css ?? []) files.add(css);
    for (const dependency of chunk.imports ?? []) visit(dependency);
  }
  visit(entry);
  const componentKey = Object.keys(manifest).find((key) => manifest[key].name === component && manifest[key].isDynamicEntry);
  if (!componentKey) throw new Error(`Missing dynamic entry: ${component}`);
  visit(componentKey);
  return [...files];
}

// Count transitive static dependencies, not only the tiny bootstrap entry.
// Budgets allow modest growth while catching accidentally eager dialogs/stores.
for (const [name, component, jsBudget, cssBudget] of [
  ["main", "App", 140 * 1024, 24 * 1024],
  ["floating-ball", "FloatingBall", 110 * 1024, 10 * 1024],
]) {
  const sizes = { js: 0, css: 0 };
  for (const file of initialFiles(component)) {
    const kind = file.endsWith(".css") ? "css" : "js";
    sizes[kind] += statSync(resolve(root, file)).size;
  }
  console.log(`${name}: JS ${sizes.js} / ${jsBudget} bytes; CSS ${sizes.css} / ${cssBudget} bytes`);
  if (sizes.js > jsBudget || sizes.css > cssBudget) {
    throw new Error(`${name} exceeded its initial-load budget. Check newly eager imports before changing the budget.`);
  }
}
