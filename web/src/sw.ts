/// <reference lib="webworker" />
import { precacheAndRoute } from "workbox-precaching";

declare let self: ServiceWorkerGlobalScope & {
  __WB_MANIFEST: Array<{ url: string; revision: string | null }>;
};

// Build assets (offline.[hash].js, css, wasm, worker, icon) -- precached with
// content-hash revisioning injected by vite-plugin-pwa. No manual versioning.
precacheAndRoute(self.__WB_MANIFEST);

self.addEventListener("install", () => {
  void self.skipWaiting();
});
self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim());
});

// Page navigations: network-first, fall back to the last cached shell offline.
const SHELL = "shell-v1";
self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.mode !== "navigate") return;
  event.respondWith(
    fetch(req)
      .then((res) => {
        const copy = res.clone();
        void caches.open(SHELL).then((c) => c.put("/", copy));
        return res;
      })
      .catch(async () => {
        const cached = await (await caches.open(SHELL)).match("/");
        return cached ?? Response.error();
      }),
  );
});
