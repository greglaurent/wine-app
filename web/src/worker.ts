// Dedicated worker: the local-first backend. Holds the OPFS SQLite and runs the
// wasm router + sync functions. OPFS SyncAccessHandle requires a dedicated worker.
import {
  init_db,
  handle,
  get_dirty,
  apply_acks,
  apply_pull,
  apply_reference,
  check_epoch,
  get_cursor,
  get_ref_cursor,
} from "./wasm/wine_client";
import type { WorkerRequest, WorkerResponse, WorkerResult } from "./types";

const ready: Promise<void> = init_db();
// Signal readiness AFTER init: messages posted before the (wasm-heavy) worker
// module finishes evaluating can be dropped, so the main thread waits for this.
ready.then(() => self.postMessage({ ready: true }));

function dispatch(req: WorkerRequest): WorkerResult {
  switch (req.op) {
    case "request":
      return handle(req.method, req.path, req.body);
    case "get_dirty":
      return get_dirty();
    case "apply_acks":
      apply_acks(req.json);
      return null;
    case "apply_pull":
      apply_pull(req.json);
      return null;
    case "apply_reference":
      apply_reference(req.json);
      return null;
    case "check_epoch":
      return check_epoch(req.json);
    case "cursor":
      return get_cursor();
    case "ref_cursor":
      return get_ref_cursor();
  }
}

self.addEventListener("message", async (ev: MessageEvent<WorkerRequest>) => {
  const req = ev.data;
  try {
    await ready;
    const result = dispatch(req);
    const reply: WorkerResponse = { rid: req.rid, ok: true, result };
    self.postMessage(reply);
  } catch (e) {
    const error = e instanceof Error ? e.message : String(e);
    self.postMessage({ rid: req.rid, ok: false, error } satisfies WorkerResponse);
  }
});
