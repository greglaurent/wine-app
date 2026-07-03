# wine-app task runner -- `just <recipe>` (run `just` for the list)

set dotenv-load := true

# show all recipes
default:
    @just --list

# create .env from the example (won't overwrite an existing one)
setup:
    cp -n .env.example .env || true

# --- build / check ---

# full build: wasm glue -> frontend bundle (Vite) -> server
build: build-client
    cd web && pnpm install && pnpm build
    cargo build -p wine-server

# build the wasm client + generate JS glue into web/src/wasm (for Vite to bundle)
build-client:
    rustup target add wasm32-unknown-unknown
    cargo build -p wine-client --target wasm32-unknown-unknown
    wasm-bindgen --target bundler --out-dir web/src/wasm \
        target/wasm32-unknown-unknown/debug/wine_client.wasm

check:
    cargo check

# cargo check the wasm client (excluded from default-members, so `check` misses it)
check-client:
    cargo check -p wine-client --target wasm32-unknown-unknown

# typecheck the frontend TS without a full build
typecheck:
    cd web && pnpm typecheck

fmt:
    cargo fmt

# lint native crates AND the wasm client
clippy:
    cargo clippy --all-targets -- -D warnings
    cargo clippy -p wine-client --target wasm32-unknown-unknown -- -D warnings

test:
    cargo test

clean:
    cargo clean

# --- run ---

# run the server on the host (needs the db up: `just db`)
run:
    cargo run -p wine-server

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
