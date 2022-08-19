{
  inputs = {
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };

    rust-overlay.url = "github:oxalica/rust-overlay";
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-compat, ... }:
    let
      pkgsFor = system: import nixpkgs {
        inherit system;

        overlays = [
          self.overlays.default
          rust-overlay.overlays.default
        ];
      };

      targetSystems = [ "aarch64-linux" "x86_64-linux" ];
    in
    {
      overlays.default = final: prev: {
        eww = prev.rustPlatform.buildRustPackage rec {
          pname = "eww";
          version = self.rev or "dirty";

          src = builtins.path {
            name = "eww";
            path = prev.lib.cleanSource ./.;
          };

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [
            prev.pkg-config
            (final.rust-bin.fromRustupToolchainFile ./rust-toolchain)
          ];

          buildInputs = [ prev.gtk3 ];

          cargoBuildFlags = [ "--bin" "eww" ];
          cargoTestFlags = cargoBuildFlags;
        };

        eww-wayland = final.eww.overrideAttrs (old: {
          buildInputs = (old.buildInputs or [ ]) ++ [ prev.gtk-layer-shell ];
          buildNoDefaultFeatures = true;
          buildFeatures = [ "wayland" ];
        });
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

          rust-toolchain = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain).override {
            extensions = [ "rust-src" ];
          };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rust-toolchain
              rust-analyzer-unwrapped
              gcc
              gtk3
              gtk-layer-shell
              pkg-config
              deno
              mdbook
            ];

            RUST_SRC_PATH = "${rust-toolchain}/lib/rustlib/src/rust/library";
          };
        }
      );
    };
}
