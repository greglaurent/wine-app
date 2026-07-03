# ---- toolchain: one image with everything the build needs, cached + reused ----
# rust (+ wasm32 target) + clang-19 (sqlite-wasm-rs compiles SQLite to wasm; the
# bookworm default clang-14 is too old for the C23 attributes its shim uses) +
# pinned wasm-bindgen-cli (must match the lockfile lib) + node + pnpm (corepack).
FROM rust:1-bookworm AS toolchain
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl gnupg clang-19 \
 && curl -fsSL https://deb.nodesource.com/setup_24.x | bash - \
 && apt-get install -y --no-install-recommends nodejs \
 && rm -rf /var/lib/apt/lists/* \
 && corepack enable \
 && rustup target add wasm32-unknown-unknown \
 && cargo install wasm-bindgen-cli --version 0.2.126 --locked
ENV CC_wasm32_unknown_unknown=clang-19

# ---- build: wasm glue, server binary, and the Vite bundle (all in one stage) ----
FROM toolchain AS build
WORKDIR /app
# .cargo/config.toml carries the kellnr registry (abmac-io) for causal-id.
COPY .cargo ./.cargo
COPY Cargo.toml Cargo.lock* ./
COPY crates ./crates
COPY migrations ./migrations
RUN cargo build --release -p wine-client --target wasm32-unknown-unknown \
 && wasm-bindgen --target bundler --out-dir web/src/wasm \
        target/wasm32-unknown-unknown/release/wine_client.wasm
RUN cargo build --release -p wine-server
COPY web ./web
RUN cd web && pnpm install --frozen-lockfile && pnpm build

# ---- runtime ----
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/wine-server /usr/local/bin/wine-server
COPY --from=build /app/web/dist ./web/dist
ENV BIND_ADDR=0.0.0.0:8090
EXPOSE 8090
CMD ["wine-server"]
