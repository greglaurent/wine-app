{ pkgs }:
let
  tools = import ./tools.nix { inherit pkgs; };
  frontend = builtins.fromJSON (builtins.readFile ../web/package.json);
  toolchain = builtins.fromTOML (builtins.readFile ../rust-toolchain.toml);
in
assert pkgs.lib.assertMsg (frontend.packageManager == "pnpm@${tools.pnpm.version}")
  "Align web/package.json packageManager with pnpm in flake.lock.";
assert pkgs.lib.assertMsg (pkgs.rustc.version == toolchain.toolchain.channel)
  "Align rust-toolchain.toml with Rust in flake.lock.";
pkgs.mkShell {
  packages = tools.rust ++ [
    tools.wasmBindgen tools.llvm.clang tools.llvm.lld tools.node tools.pnpm
    pkgs.just pkgs.process-compose pkgs.pkg-config pkgs.openssl
    pkgs.sqlx-cli pkgs.postgresql_16 pkgs.docker-client pkgs.docker-compose
  ];

  # The unwrapped compiler avoids host-only Nix flags when compiling SQLite to WASM.
  CC_wasm32_unknown_unknown = "${tools.llvm.clang-unwrapped}/bin/clang";
  # Nix puts Clang's builtin headers in a separate output.
  CFLAGS_wasm32_unknown_unknown = "-resource-dir=${tools.llvm.clang-unwrapped.lib}/lib/clang/${pkgs.lib.versions.major tools.llvm.clang-unwrapped.version}";

  shellHook = ''
    wine_root="$PWD"
    while [[ ! -f "$wine_root/flake.nix" || ! -f "$wine_root/crates/server/Cargo.toml" ]]; do
      if [[ "$wine_root" == / ]]; then
        echo "Enter the development shell from the wine-app checkout." >&2
        return 1
      fi
      wine_root="$(dirname "$wine_root")"
    done
    export CARGO_TARGET_DIR="$wine_root/target/nix-${builtins.baseNameOf (toString pkgs.rustc)}"
    export PNPM_CONFIG_STORE_DIR="$wine_root/.cache/wine-app/pnpm-store"
    export PNPM_CONFIG_CACHE_DIR="$wine_root/.cache/wine-app/pnpm-cache"
    export PNPM_CONFIG_PM_ON_FAIL=error
    unset wine_root
  '';
}
