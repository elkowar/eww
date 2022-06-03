{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };

    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "nixpkgs/nixpkgs-unstable";
    naersk.url = "github:nmattia/naersk";
  };
  outputs = { self, flake-utils, fenix, nixpkgs, naersk, flake-compat, ... }:
    flake-utils.lib.eachSystem [ "aarch64-linux" "x86_64-linux" ] (system:
      let
        pkgs = import nixpkgs { inherit system; };
        toolchain = fenix.packages.${system}.latest;

        naersk-lib = (naersk.lib."${system}".override {
          inherit (toolchain) cargo rustc;
        });

        mkEww = { wayland ? false }:
          naersk-lib.buildPackage {
            pname = "eww";
            src = builtins.path { name = "eww"; path = ./.; };

            nativeBuildInputs = with pkgs; [ pkg-config gtk3 ];
            buildInputs = pkgs.lib.optional wayland pkgs.gtk-layer-shell;

            cargoBuildOptions = opts: opts ++ pkgs.lib.optionals wayland [
              "--no-default-features"
              "--features=wayland"
            ];
          };
      in
      {
        apps = rec {
          default = eww;
          eww = flake-utils.lib.mkApp { drv = self.packages.${system}.eww; };
          eww-wayland = flake-utils.lib.mkApp { drv = self.packages.${system}.eww-wayland; };
        };

        packages = rec {
          default = eww;
          eww = mkEww { };
          eww-wayland = mkEww { wayland = true; };
        };

        devShells.default = pkgs.mkShell {
          packages = builtins.attrValues {
            inherit (toolchain)
              cargo
              rustc
              rust-src
              clippy-preview
              rustfmt-preview;

            inherit (pkgs)
              rust-analyzer
              gcc
              gtk3
              gtk-layer-shell
              pkg-config
              deno
              mdbook;
          };

          RUST_SRC_PATH = "${toolchain.rust-src}/lib/rustlib/src/rust/library";
        };
      });
}
