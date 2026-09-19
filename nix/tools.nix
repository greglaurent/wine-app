{ pkgs }:
let
  lock = builtins.fromTOML (builtins.readFile ../Cargo.lock);
  bindgen = builtins.head (builtins.filter (p: p.name == "wasm-bindgen") lock.package);
in
{
  # nixpkgs Rust includes the wasm32-unknown-unknown standard library.
  rust = with pkgs; [ cargo rustc rustfmt clippy ];
  wasmBindgen = pkgs.${"wasm-bindgen-cli_" + builtins.replaceStrings [ "." ] [ "_" ] bindgen.version};
  llvm = pkgs.llvmPackages_19;
  node = pkgs.nodejs_24;
  pnpm = pkgs.pnpm;
}
