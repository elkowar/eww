{
  inputs = {
    # maybe consider __getFlake github:elkowar/eww instead
    flake-compat = { url = "github:edolstra/flake-compat"; flake = false; };
    parts.url = "github:hercules-ci/flake-parts";
    rust.url = "github:oxalica/rust-overlay";
    nix-filter.url = "github:numtide/nix-filter";
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    rust.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = inputs@{ self, parts, nixpkgs, rust, ... }:
    parts.lib.mkFlake { inherit inputs; } {
      systems = [ "aarch64-linux" "x86_64-linux" ];

      flake.overlays.default = throw "The eww overlay was removed, please use packages.\${system}.* instead.";

      perSystem = ctx@{ self', pkgs, lib, system, ... }:
        let
          pkgs = ctx.pkgs.extend rust.overlays.default;
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
        in
        {
          devShells.default = pkgs.mkShell {
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

            RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
          };

          packages = {
            default = self'.packages.eww;

            eww = (pkgs.eww.override { inherit rustPlatform; }).overrideAttrs (old: {
              version = self.rev or "dirty";

              src = inputs.nix-filter.lib.filter {
                root = ./.;

                include = [
                  ./crates
                  ./Cargo.toml
                  ./Cargo.lock
                ];
              };

              patches = [ ];
              cargoDeps = rustPlatform.importCargoLock { lockFile = ./Cargo.lock; };
              buildFeatures = [ "wayland" "x11" ];
              buildInputs = old.buildInputs ++ [ pkgs.gtk-layer-shell ];
            });

            eww-wayland = self'.packages.eww.overrideAttrs (_: { buildFeatures = [ "wayland" ]; });

            eww-x11 = self'.packages.eww.overrideAttrs (old: {
              buildFeatures = [ "x11" ];
              buildInputs = lib.remove pkgs.gtk-layer-shell old.buildInputs;
            });
          };
        };
    };
}
