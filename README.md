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

### Development environment

The Nix flake pins the development tools, following the Cascade setup. It includes
Rust with the WASM target, Clang 19, the wasm-bindgen CLI selected from `Cargo.lock`,
Node 24, pnpm, just, and database/container clients. Docker must be running separately.

New flake files must be tracked by Git before `use flake` / `nix develop` can see them.
With direnv's shell hook and nix-direnv installed:

```sh
direnv allow
just setup
# Edit .env, including DATABASE_URL if you change the database credentials.
just dev
```

Alternatively, enter the environment with `nix develop`. `just dev` builds the
frontend, starts the Docker database, and runs the frontend watcher and Rust server
with process-compose. Ctrl-C stops the watcher and server; `just down` stops the
database. Restart `just dev` after Rust changes to rebuild the WASM and server.

`just build` builds the whole app; `just check` checks native Rust, WASM, frontend
types, Clippy, and Rust tests. `just check-native` checks only native Rust.
Nix builds use a toolchain-specific directory under `target/`; pnpm caches live in
`.cache/wine-app/`. Both and `.direnv/` are ignored.

Keep `rust-toolchain.toml` and `web/package.json` aligned with the versions in
`flake.lock`; the shell checks them. Outside Nix, install the tools above and the
wasm-bindgen CLI version matching `Cargo.lock`; rustup reads `rust-toolchain.toml`.

### Docker Compose

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
just run                       # loads DATABASE_URL from .env; build assets first
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
