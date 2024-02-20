{
  inputs = {
    flake-compat = { url = "github:edolstra/flake-compat"; flake = false; };
    rust-overlay.url = "github:oxalica/rust-overlay";
    nixpkgs.url = "github:nixos/nixpkgs";
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
          # TODO update this to reflect the changes in upstream (nixpkgs), when upstream is updated
          eww = features: (prev.eww.override { inherit rustPlatform; withWayland = builtins.elem "wayland" features; }).overrideAttrs (old: {
            version = self.rev or "dirty";
            src = builtins.path { name = "eww"; path = prev.lib.cleanSource ./.; };
            cargoDeps = rustPlatform.importCargoLock { lockFile = ./Cargo.lock; };
            cargoBuildFeatures = features;
            cargoCheckFeatures = features;
          });
        in
        {
          # this doesn't support wayland currently (although it should, see https://github.com/elkowar/eww/issues/739)
          eww = eww [ "wayland" "x11" ];
          eww-x11 = eww [ "x11" ];
          eww-wayland = eww [ "wayland" ];
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
