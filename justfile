# wine-app task runner -- `just <recipe>` (run `just` for the list)

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load

# show all recipes
default:
    @just --list

# Create .env if absent and install the locked frontend dependencies.
setup:
    test -e .env || cp .env.example .env
    pnpm -C web install --frozen-lockfile

# --- build / check ---

# Build WASM and the frontend before serving it.
frontend: setup build-client
    pnpm -C web build

# Full build: WASM glue, frontend bundle, server.
build: frontend
    cargo build --locked -p wine-server

# Start the dev database, frontend watcher, and server; Ctrl-C stops the latter two.
dev: frontend db
    process-compose --no-server --disable-dotenv --config process-compose.yaml up

# build the wasm client + generate JS glue into web/src/wasm (for Vite to bundle)
build-client:
    cargo build --locked -p wine-client --target wasm32-unknown-unknown
    wasm-bindgen --target bundler --out-dir web/src/wasm \
        "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/debug/wine_client.wasm"

# Check native Rust, WASM, frontend types, lint, and tests.
check: check-native check-client typecheck clippy test

check-native:
    cargo check --locked

# Check the WASM client (excluded from Cargo's default members).
check-client:
    cargo check --locked -p wine-client --target wasm32-unknown-unknown

# Generate WASM bindings and typecheck the frontend without bundling it.
typecheck: setup build-client
    pnpm -C web typecheck

fmt:
    cargo fmt

# lint native crates AND the wasm client
clippy:
    cargo clippy --locked --all-targets -- -D warnings
    cargo clippy --locked -p wine-client --target wasm32-unknown-unknown -- -D warnings

test:
    cargo test --locked

clean:
    cargo clean

# --- run ---

# run the server on the host (needs the db up: `just db`)
run:
    cargo run --locked -p wine-server

# --- docker compose ---

# dev stack: db (port exposed) + app, debug logging
up:
    docker compose up --build

# dev stack, detached
up-d:
    docker compose up -d --build

# only the database (for host-run server / tooling)
db:
    docker compose up -d db

# build just the app image (toolchain + build + runtime)
image:
    docker compose build app

# production stack (DB internal, restart always, log rotation)
prod:
    docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d --build

# stop the stack
down:
    docker compose down

# stop and DELETE the postgres data volume
nuke:
    docker compose down -v

# follow logs
logs:
    docker compose logs -f

ps:
    docker compose ps

# open a psql shell in the db container
psql:
    docker compose exec db psql -U "${POSTGRES_USER:-wine}" "${POSTGRES_DB:-wine}"

# --- sqlx (needs sqlx-cli: `cargo install sqlx-cli --no-default-features -F postgres,rustls`) ---

# run migrations against DATABASE_URL (the server also does this on startup)
migrate:
    sqlx migrate run

# regenerate the offline query cache (.sqlx) for builds without a live DB
sqlx-prepare:
    cargo sqlx prepare --workspace
