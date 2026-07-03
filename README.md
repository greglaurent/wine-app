# wine-app

Self-hosted, mobile-friendly, offline-first wine cellar tracker.

## Architecture

- **Server:** Axum + datastar + Askama + **sqlx / Postgres** -- sync target & source of truth.
- **Client:** PWA. A service worker intercepts datastar's `fetch` requests; offline,
  a dedicated worker runs the WASM `core` + **`sqlite-wasm-rs`** (local SQLite, OPFS)
  as the local responder, rendering the **same** Askama templates the server uses.
- **`crates/core`:** wasm-safe shared crate -- templates, datastar event generation,
  handler/render logic, sync logic, Lamport id, and the `Store` trait. Compiled into
  both the server and the client. **One renderer.**
- **Sync:** client-generated Lamport id PKs - last-write-wins on `updated_at`
  (Lamport id breaks ties) - soft deletes (`deleted_at`) - per-row `revision` -
  server-authoritative `server_seq` cursor for delta pulls.

```
crates/core    shared: templates, handlers, sync, Store trait   (wasm-safe)
crates/server  Axum + sqlx/Postgres Store impl                  (native)
crates/client  wasm32: service worker + sqlite-wasm-rs Store     (browser)
migrations/    Postgres schema (sqlx, embedded at compile time)
```

## Run

Compose uses the base + override pattern, so one setup serves both.

```sh
cp .env.example .env   # edit POSTGRES_PASSWORD
```

**Local development** (base + auto override -- Postgres exposed on the host, debug logs):

```sh
docker compose up --build
```

Or run only the database in Docker and the server on the host for fast iteration:

```sh
docker compose up -d db          # Postgres at localhost:5432
cargo run -p wine-server         # uses DATABASE_URL from .env
```

**Production / self-hosted server** (base + prod -- DB internal-only, restart always, log rotation):

```sh
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d --build
```

Migrations apply automatically on startup; a fresh volume self-initializes.
Health check: `GET /health`.

## Status

Scaffolding. The schema (`migrations/0001_init.sql`) is the locked layout.
Next: reference-data seed migration, the `Store` trait impls, the first
datastar vertical slice (bottle list + add), then the client worker + sync.
