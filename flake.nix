{
  description = "wine-app development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  # pnpm moves faster than the NixOS release cycle; track unstable for it alone.
  inputs.nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = { nixpkgs, nixpkgs-unstable, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
      forAll = f: nixpkgs.lib.genAttrs systems
        (s: f nixpkgs.legacyPackages.${s} nixpkgs-unstable.legacyPackages.${s});
    in
    {
      devShells = forAll (pkgs: unstable: {
        default = import ./nix/dev-shell.nix { inherit pkgs unstable; };
      });
    };
}
