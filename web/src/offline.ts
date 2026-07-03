// Local-first router (the app entry). The UI always reads/writes the local
// store via the worker (online and offline) so the count is one coherent
// number; a background sync engine reconciles with the server. window.fetch is
// patched BEFORE datastar loads, then datastar is imported dynamically.
import "@picocss/pico/css/pico.min.css";
import type { PushRequest, WorkerApi, WorkerOp, WorkerResponse } from "./types";

const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });

interface Pending {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
}
const pending = new Map<number, Pending>();
let seq = 0;

// The worker drops messages posted before its (wasm-heavy) module finishes
// evaluating, so it sends a `{ ready: true }` sentinel and we hold posts until then.
let markReady = (): void => {};
const workerReady = new Promise<void>((resolve) => (markReady = resolve));

worker.addEventListener("message", (ev: MessageEvent<WorkerResponse | { ready: true }>) => {
  const data = ev.data;
  if ("ready" in data) {
    markReady();
    return;
  }
  const p = pending.get(data.rid);
  if (!p) return;
  pending.delete(data.rid);
  if (data.ok) p.resolve(data.result);
  else p.reject(new Error(data.error));
});

// Typed RPC: the result type is inferred from the op. Posts are deferred until
// the worker signals ready.
function call<K extends WorkerOp>(
  op: K,
  params: WorkerApi[K]["params"],
): Promise<WorkerApi[K]["result"]> {
  return new Promise<WorkerApi[K]["result"]>((resolve, reject) => {
    const rid = ++seq;
    pending.set(rid, { resolve: resolve as (value: unknown) => void, reject });
    void workerReady.then(() => worker.postMessage({ rid, op, ...params }));
  });
}

const ignore = (): void => {};

// Offline / server-unreachable is a NORMAL state in offline-first, not an error:
// the local store keeps serving. A failed network fetch rejects with a TypeError,
// so treat that (or a known-offline navigator) as a warning; reserve error level
// for genuine failures (a bad response, a local apply error).
function reportSync(label: string, e: unknown): void {
  if (!navigator.onLine || e instanceof TypeError) {
    console.warn(`${label} deferred: offline or server unreachable`);
  } else {
    console.error(`${label} error:`, e);
  }
}

const realFetch = window.fetch.bind(window);
const isLocal = (path: string): boolean => path === "/bottles" || path.startsWith("/fragments/");

// datastar requests to app endpoints always go to the local worker.
window.fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
  const req = new Request(input, init);
  const url = new URL(req.url, location.origin);
  if (url.origin === location.origin && isLocal(url.pathname)) {
    const method = req.method.toUpperCase();
    const body = method === "GET" ? "" : await req.text().catch(() => "");
    const html = await call("request", { method, path: url.pathname, body });
    if (method !== "GET") void sync().catch((e) => reportSync("sync", e));
    return new Response(html, {
      status: 200,
      headers: { "Content-Type": "text/html; charset=utf-8" },
    });
  }
  return realFetch(req);
};

let syncing = false;
async function sync(): Promise<void> {
  if (!navigator.onLine || syncing) return;
  syncing = true;
  try {
    // Epoch check FIRST: if the server was nuked, this wipes the local store and
    // resets cursors so the pulls below re-hydrate from scratch.
    await syncMeta().catch((e) => reportSync("meta sync", e));
    // Bottles (bidirectional) and reference (pull-only) are independent: one
    // failing must not block the other.
    await syncBottles().catch((e) => reportSync("bottle sync", e));
    await syncReference().catch((e) => reportSync("reference sync", e));
  } finally {
    await refreshCount();
    syncing = false;
  }
}

// Detect a server reset (nuke): if the epoch changed, the worker wipes the local
// store and zeroes the cursors, so the following pulls re-hydrate everything.
async function syncMeta(): Promise<void> {
  const res = await realFetch("/sync/meta");
  if (!res.ok) return;
  const reset = await call("check_epoch", { json: await res.text() });
  if (reset) console.info("server reset detected: local store cleared, re-syncing");
}

// Bidirectional, last-write-wins: push dirty bottles, then pull the deltas.
async function syncBottles(): Promise<void> {
  const dirty = await call("get_dirty", {});
  const pushReq = JSON.parse(dirty) as PushRequest;
  if (pushReq.bottles.length > 0) {
    const res = await realFetch("/sync/push", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: dirty,
    });
    if (res.ok) await call("apply_acks", { json: await res.text() });
  }
  const cursor = await call("cursor", {});
  const res = await realFetch(`/sync/pull?since=${cursor}`);
  if (res.ok) await call("apply_pull", { json: await res.text() });
}

// Pull-only, server-authoritative: the reference vocabulary.
async function syncReference(): Promise<void> {
  const refCursor = await call("ref_cursor", {});
  const res = await realFetch(`/sync/reference?since=${refCursor}`);
  if (res.ok) await call("apply_reference", { json: await res.text() });
}

// Update #bottle-count IN PLACE: swap only its children, never the element
// itself. Replacing the node makes datastar re-scan and re-bind the Add button,
// which stacks click listeners into a runaway.
async function refreshCount(): Promise<void> {
  const html = await call("request", { method: "GET", path: "/fragments/bottle-count", body: "" });
  const el = document.getElementById("bottle-count");
  if (!el) return;
  const fresh = new DOMParser().parseFromString(html, "text/html").getElementById("bottle-count");
  if (fresh) el.replaceChildren(...fresh.childNodes);
}

window.addEventListener("online", () => void sync().catch((e) => reportSync("sync", e)));
window.addEventListener("load", () => {
  void refreshCount().catch(ignore);
  void sync().catch((e) => reportSync("sync", e));
});

if ("serviceWorker" in navigator) {
  window.addEventListener("load", () => {
    void navigator.serviceWorker.register("/sw.js", { type: "module" }).catch(ignore);
  });
}

// datastar last, so window.fetch is already patched when it initializes.
await import("./vendor/datastar.js");
