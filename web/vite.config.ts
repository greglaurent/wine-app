import { defineConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";
import wasm from "vite-plugin-wasm";

// Builds the client glue (offline.ts entry + the worker it spawns + the wasm),
// content-hashed, with a manifest the Rust server reads to resolve asset URLs.
// The service worker is generated from src/sw.ts with a revisioned precache
// manifest injected (no manual cache versioning). Targets modern browsers
// (OPFS, wasm, service workers) so top-level await is native.
export default defineConfig({
  build: {
    target: "esnext",
    outDir: "dist",
    emptyOutDir: true,
    manifest: true,
    rollupOptions: {
      input: { offline: "src/offline.ts" },
    },
  },
  worker: {
    format: "es",
    plugins: () => [wasm()],
  },
  plugins: [
    wasm(),
    VitePWA({
      strategies: "injectManifest",
      srcDir: "src",
      filename: "sw.ts",
      injectRegister: false,
      manifest: false,
      injectManifest: {
        globPatterns: ["**/*.{js,css,wasm,svg,webmanifest}"],
        // debug wasm is ~2.6 MB; release + wasm-opt will shrink this.
        maximumFileSizeToCacheInBytes: 8 * 1024 * 1024,
      },
      devOptions: { enabled: false },
    }),
  ],
});
