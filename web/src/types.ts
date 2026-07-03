// Shared types for the client glue.

// Sync wire types: mirror crates/core/src/sync.rs. Timestamps are epoch ms.
export interface SyncBottle {
  id: string;
  lot_id: string;
  status: string;
  updated_at: number;
  deleted_at: number | null;
  revision: number;
}

export interface PushRequest {
  bottles: SyncBottle[];
}

export interface PushAck {
  id: string;
  server_seq: number;
}

export interface PushResponse {
  cursor: number;
  acks: PushAck[];
}

export interface PullResponse {
  cursor: number;
  default_lot: string | null;
  bottles: SyncBottle[];
}

// Worker RPC. Each op maps to its params and its result type, so `call(op, ...)`
// and the worker dispatch are both checked end to end.
export interface WorkerApi {
  request: { params: { method: string; path: string; body: string }; result: string };
  get_dirty: { params: Record<string, never>; result: string };
  apply_acks: { params: { json: string }; result: null };
  apply_pull: { params: { json: string }; result: null };
  apply_reference: { params: { json: string }; result: null };
  check_epoch: { params: { json: string }; result: boolean };
  // i64 on the Rust side maps to BigInt in JS (wasm-bindgen).
  cursor: { params: Record<string, never>; result: bigint };
  ref_cursor: { params: Record<string, never>; result: bigint };
}

export type WorkerOp = keyof WorkerApi;

export type WorkerRequest = {
  [K in WorkerOp]: { rid: number; op: K } & WorkerApi[K]["params"];
}[WorkerOp];

export type WorkerResult = WorkerApi[WorkerOp]["result"];

export type WorkerResponse =
  | { rid: number; ok: true; result: WorkerResult }
  | { rid: number; ok: false; error: string };
