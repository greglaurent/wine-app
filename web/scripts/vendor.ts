// Downloads the pinned datastar bundle into src/vendor/ so Vite can hash,
// bundle, and precache it like any other asset.
//
// datastar's npm package is deprecated and frozen at a beta; the v1 line ships
// only as a prebuilt bundle, so this one file is fetched here. Bump the version
// and re-run (the build does this automatically via the `prebuild` step).
import { mkdir, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const DATASTAR_VERSION = "1.0.2";
const src = `https://cdn.jsdelivr.net/gh/starfederation/datastar@v${DATASTAR_VERSION}/bundles/datastar.js`;
const out = fileURLToPath(new URL("../src/vendor/datastar.js", import.meta.url));

const res = await fetch(src);
if (!res.ok) {
  throw new Error(`fetch datastar ${DATASTAR_VERSION} failed: ${res.status}`);
}
await mkdir(dirname(out), { recursive: true });
await writeFile(out, await res.text());
console.log(`vendored datastar ${DATASTAR_VERSION}`);
