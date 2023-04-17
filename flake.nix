{
  inputs = {
    flake-compat = { url = "github:edolstra/flake-compat"; flake = false; };
    rust-overlay.url = "github:oxalica/rust-overlay";
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, rust-overlay, ... }:
    let
      pkgsFor = system: import nixpkgs {
        inherit system;

        overlays = [
          self.overlays.default
          rust-overlay.overlays.default
        ];
      };

      targetSystems = [ "aarch64-linux" "x86_64-linux" ];
      mkRustToolchain = pkgs: pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    in
    {
      overlays.default = final: prev:
        let
          rust = mkRustToolchain final;

          rustPlatform = prev.makeRustPlatform {
            cargo = rust;
            rustc = rust;
          };
        in
        rec {
          # TODO update this to reflect the changes in upstream (nixpkgs), when upstream is updated
          eww = (prev.eww.override { inherit rustPlatform; withWayland = true; }).overrideAttrs (old: {
            version = self.rev or "dirty";
            src = builtins.path { name = "eww"; path = prev.lib.cleanSource ./.; };
            cargoDeps = rustPlatform.importCargoLock { lockFile = ./Cargo.lock; };
            cargoBuildNoDefaultFeatures = false;
            cargoCheckNoDefaultFeatures = false;
            patches = [ ];
          });

          eww-wayland = eww;
        };

      packages = nixpkgs.lib.genAttrs targetSystems (system:
        let
          pkgs = pkgsFor system;
        in
        (self.overlays.default pkgs pkgs) // {
          default = self.packages.${system}.eww;
        }
      );

      devShells = nixpkgs.lib.genAttrs targetSystems (system:
        let
          pkgs = pkgsFor system;
          rust = mkRustToolchain pkgs;
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rust
              rust-analyzer-unwrapped
              gcc
              gtk3
              gtk-layer-shell
              pkg-config
              deno
              mdbook
            ];

            RUST_SRC_PATH = "${rust}/lib/rustlib/src/rust/library";
          };
        }
      );
    };
}
